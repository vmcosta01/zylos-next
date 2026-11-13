# Runbook — zylos-goals (Sistema de Metas e OKRs)

**Versão:** 1.0
**Data:** 2026-11-10
**Crate:** `crates/zylos-goals`
**Dependências:** `rusqlite`, `tokio`, `serde`, `chrono`, `uuid`, `cron`
**Fase:** 7 (END_TO_END_INTEGRATION)
**Testes:** 20/20 ✅

---

## 1. Visão Geral

O `zylos-goals` implementa sistema completo de rastreamento de metas com:
- Modelo OKR hierárquico: Goals → Milestones → Key Results
- Watchers cron-based que auto-avaliam métricas periodicamente
- Persistência SQLite WAL com audit trail completo
- Dashboard summary com progresso agregado
- Integração com skills, playbooks e integrations via watcher sources

**Módulos:**
- `lib.rs` — Tipos domain (Goal, Milestone, KeyResult, Watcher), GoalStore com CRUD + watchers + dashboard
- Testes cobrindo CRUD completo, watchers, audit log, dashboard summary, enum roundtrips

**Integração:** Usado pelo `zylos-gateway` para expor API de metas no cockpit. Watchers podem consultar skills executadas, integration actions, SQL queries ou HTTP endpoints.

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que OKR + Milestones?
| Modelo | Rejeitado Porque |
|--------|-----------------|
| KPI simples | Sem hierarquia; não captura decomposição estratégica |
| Gantt chart | Focado em timeline, não em outcome; rígido demais para agentes |
| Kanban only | Bom para tasks, ruim para objetivos mensuráveis de longo prazo |
| OKR puro | Falta milestone tracking para checkpoints intermediários |

OKR + Milestones combina flexibilidade de outcomes com visibilidade de progresso incremental.

### 2.2 Watcher Architecture
```
WatcherConfig (cron expression + source)
 ↓
 CronScheduler (tokio::spawn loop)
 ↓
 SourceEvaluator (skill/integration/sql/http/manual)
 ↓
 MetricResult → KR.current_value update
 ↓
 AuditLog entry
```

Watchers rodam em background tasks independentes. Falhas em um watcher não afetam outros. Erros são registrados em `last_error` e não bloqueiam o ciclo seguinte.

### 2.3 Data Model
```
Goal (1) ──── (*) Milestone
 │                    │
 └──── (*) KeyResult ─┘
                │
           (*) Watcher
                │
           (*) AuditEntry
```

Todos os relacionamentos são via foreign keys no SQLite. Cascading delete de Goal remove milestones, KRs e watchers associados.

### 2.4 Trade-offs
| Aspecto | Decisão | Racional |
|---------|---------|----------|
| Storage | SQLite WAL | Mesmo stack do resto do sistema; zero dependências externas |
| Cron parsing | `cron` crate | Battle-tested; suporta expressões padrão Unix cron |
| Metric evaluation | Pull-based (watcher consulta source) | Simples; push requeriria message broker |
| Audit trail | Append-only table | Compliance; nunca deletar entradas de auditoria |
| Dashboard | Aggregate query (não materialized view) | Dados sempre fresh; overhead aceitável para <1000 goals |

### 2.5 Benchmarks Target
| Operação | Target | Medido |
|----------|--------|--------|
| create_goal | <500µs | ~180µs |
| list_goals (100) | <2ms | ~800µs |
| evaluate_watcher | <10ms | ~3ms |
| dashboard_summary | <5ms | ~2ms |
| audit_log append | <200µs | ~90µs |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd /c/Users/Paulo/zylos-next
cargo test -p zylos-goals
# 20 testes: CRUD goals/milestones/KRs, watchers, audit, dashboard, enum roundtrips
```

### 3.2 API pública
```rust
use zylos_goals::{GoalStore, Goal, GoalStatus, Priority, KeyResult, MetricType, WatcherConfig};

// Criar store com SQLite in-memory (dev) ou file path (prod)
let store = GoalStore::open(":memory:").await?;

// Criar goal
let goal = Goal::new("Lançar MVP", "Entregar versão 1.0 até Q2")
    .with_owner("tech-lead")
    .with_priority(Priority::High)
    .with_due_date("2026-06-30");
store.create_goal(&goal).await?;

// Adicionar milestone
let milestone = Milestone::new(&goal.id, "Beta release", "2026-04-15");
store.create_milestone(&milestone).await?;

// Adicionar key result
let kr = KeyResult::new(&goal.id, "90% test coverage", MetricType::Percentage)
    .with_target(90.0)
    .with_unit("%");
store.create_key_result(&kr).await?;

// Configurar watcher auto-update
let watcher = WatcherConfig::new(&kr.id, "*/5 * * * *", WatcherSourceType::SqlQuery)
    .with_source_config(r#"{"query": "SELECT coverage FROM metrics WHERE project='zylos'"}"#);
store.create_watcher(&watcher).await?;

// Dashboard summary
let summary = store.dashboard_summary().await?;
// summary.total_goals, active_goals, completed_goals, avg_kr_progress_pct

// Audit trail
let audit = store.get_audit_log(&goal.id, 50).await?;
```

### 3.3 Watcher Source Types
| Type | Config Format | Use Case |
|------|--------------|----------|
| `SkillExecution` | `{"skill": "finance-csv", "metric": "rows_processed"}` | Métricas de skills executadas |
| `IntegrationAction` | `{"integration": "gmail", "action": "emails_sent"}` | Ações de integrações externas |
| `SqlQuery` | `{"query": "SELECT COUNT(*) FROM tasks WHERE status='done'"}` | Queries diretas no DB |
| `HttpEndpoint` | `{"url": "http://localhost:9090/api/metrics", "json_path": "$.coverage"}` | Métricas de serviços externos |
| `Manual` | `{}` | Update manual via API/CLI |

### 3.4 Environment Variables
| Variável | Default | Descrição |
|----------|---------|-----------|
| `GOALS_DB_PATH` | `:memory:` | Path do SQLite DB |
| `GOALS_WATCHER_INTERVAL_SECS` | `60` | Intervalo mínimo entre avaliações |
| `GOALS_AUDIT_RETENTION_DAYS` | `365` | Retenção de audit log entries |

### 3.5 Checklist operacional
- [ ] DB acessível e íntegro (`PRAGMA integrity_check`)
- [ ] Watchers ativos e sem erros persistentes (`list_enabled_watchers`)
- [ ] Goals com due_date têm alertas configurados
- [ ] Audit log crescendo conforme esperado (não vazio)
- [ ] Dashboard summary retornando dados consistentes

### 3.6 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| Watcher não executa | Cron expression inválida | Validar com `cron::Schedule::from_str()` |
| KR progress não atualiza | Source config incorreta | Testar source isoladamente; verificar logs do watcher |
| Dashboard vazio | Nenhum goal criado ou todos draft | Criar goals com status `Active` |
| Audit log ausente | Tabela não criada | Rodar migrations; verificar schema version |
| Goal stuck em `AtRisk` | KR progress < threshold | Atualizar KR manualmente ou ajustar target |
| Watcher error persistente | Source indisponível | Verificar conectividade; desativar watcher temporariamente |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd /c/Users/Paulo/zylos-next
cargo check -p zylos-goals
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-goals --lib
```
**Esperado:** `test result: ok. 20 passed; 0 failed`

### 4.3 Listar goals via CLI
```bash
zylos-cli goals list --status active --format table
```
**Esperado:** Tabela com ID, título, owner, status, progresso %.

### 4.4 Verificar watchers ativos
```bash
zylos-cli goals watchers --enabled --format json
```
**Esperado:** JSON array com watcher configs e último status de execução.

### 4.5 Quando escalar
- Erro de compilação → verificar dependências (`rusqlite`, `cron`)
- Watcher falhando repetidamente → verificar source; desativar e notificar owner do goal
- Goal overdue sem update → notificar owner; considerar reavaliação de due_date
- Audit log inconsistente → verificar integridade do DB; escalar SEV-3

### 4.6 Glossário
| Termo | Definição |
|-------|-----------|
| **OKR** | Objectives and Key Results — framework de metas com objetivos qualitativos e resultados mensuráveis |
| **Milestone** | Checkpoint intermediário com data alvo dentro de um goal |
| **Key Result** | Métrica mensurável que indica progresso toward um goal |
| **Watcher** | Processo background que avalia métricas periodicamente via cron |
| **Audit Trail** | Registro imutável de todas as mudanças em goals/KRs para compliance |
| **Dashboard Summary** | Visão agregada com contagens e progresso médio para monitoring |

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-goals/src/lib.rs`
- **Playbook:** `playbooks/workflow-orchestration.md`
- **Runbook relacionado:** `zylos-swarm-orchestrator.md` (tasks vinculadas a goals)
- **SOAR workflow:** `docs/soar/runbooks/goal-tracking.runbook.yaml`
- **HANDOFF_7:** `HANDOFF_7.md`