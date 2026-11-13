# Runbook — zylos-swarm-orchestrator (Orquestração Multi-Agente)

**Versão:** 1.0
**Data:** 2026-11-10
**Crate:** `crates/zylos-swarm-orchestrator`
**Dependências:** `petgraph`, `sqlx`, `tokio`, `serde`, `chrono`, `uuid`
**Fase:** 7 (END_TO_END_INTEGRATION)
**Testes:** 10/10 ✅

---

## 1. Visão Geral

O `zylos-swarm-orchestrator` gerencia orquestração multi-agente com:
- Decomposição de tarefas em unidades atômicas de trabalho
- DAG de dependências com ordenação topológica (`petgraph`)
- Agendamento de agentes com balanceamento de carga e matching de capacidades
- Máquina de estados de execução com retry/backoff exponencial
- Persistência SQLite WAL para recovery após crash

**Módulos:**
- `lib.rs` — Tipos core (Task, TaskState, SwarmAgent, ScheduleResult), SwarmOrchestrator com CRUD + scheduling
- Testes integrados cobrindo lifecycle completo, cycle detection, capability matching, retry exhaustion

**Integração:** Usado pelo `zylos-gateway` para orquestrar workflows multi-agente via DAGs. Conecta-se ao `zylos-bot-system` (DAG executor) e `zylos-cerebelo` (recovery ladder).

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que petgraph + SQLite?
| Alternativa | Rejeitada Porque |
|-------------|-----------------|
| Redis/Tarantool | Overkill para <100 agentes; adiciona dependência externa |
| In-memory only | Sem crash recovery; perda de estado em restart |
| PostgreSQL | Latência desnecessária para scheduling local; SQLite WAL suficiente |
| Custom graph lib | petgraph é battle-tested, toposort O(V+E), cycle detection built-in |

### 2.2 Máquina de Estados de Task
```
Pending → Ready → Running → Done
                  ↓         ↑
                Failed ← Retry (max 3)
                  ↓
              Cancelled
Ready → Blocked (waiting external input)
```

Transições válidas são enforceadas em `update_task_state()`. Transições inválidas retornam `SwarmError::InvalidTransition`.

### 2.3 Scheduling Algorithm
1. Filtrar tasks em estado `Ready`
2. Ordenar por prioridade (Critical > High > Normal > Low)
3. Para cada task ready:
   - Filtrar agentes com `current_load < max_concurrent_tasks`
   - Se task tem `capabilities` requeridas, filtrar agentes com match
   - Assignar ao agente com menor `current_load` (load balancing)
4. Tasks sem agente elegível → `unassignable_tasks` no resultado

### 2.4 Trade-offs
| Aspecto | Decisão | Racional |
|---------|---------|----------|
| Lock granularity | Row-level via SQL UPDATE WHERE state=X | Evita race conditions sem mutex global |
| Retry strategy | Exponential backoff (1s, 2s, 4s) | Balance entre recuperação rápida e não sobrecarregar |
| Cycle detection | toposort antes de cada schedule | Previne deadlocks; O(V+E) aceitável para <1000 nodes |
| Capability matching | Exact match (não fuzzy) | Simplicidade; fuzzy matching adiciona complexidade sem ganho claro |

### 2.5 Benchmarks Target
| Operação | Target | Medido |
|----------|--------|--------|
| add_task | <1ms | ~200µs |
| schedule (100 tasks, 10 agents) | <10ms | ~3ms |
| cycle_detection (1000 nodes) | <50ms | ~12ms |
| state_transition | <500µs | ~150µs |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd /c/Users/Paulo/zylos-next
cargo test -p zylos-swarm-orchestrator
# 10 testes: lifecycle, state transitions, DAG, scheduling, capability matching, retry
```

### 3.2 API pública
```rust
use zylos_swarm_orchestrator::{SwarmOrchestrator, Task, SwarmAgent, Priority};

// Criar orchestrator com SQLite in-memory (dev) ou file path (prod)
let orch = SwarmOrchestrator::open(":memory:").await?;

// Adicionar tarefa com dependências
let task = Task::new("Processar dados", "ETL pipeline")
    .with_priority(Priority::High)
    .with_dependencies(vec!["task-extract".into()])
    .with_capabilities(vec!["python".into(), "data-engineering".into()]);
orch.add_task(&task).await?;

// Registrar agente
let agent = SwarmAgent::new("agent-data-01", "Data Engineer")
    .with_capabilities(vec!["python".into(), "data-engineering".into()])
    .with_max_concurrent(3);
orch.register_agent(&agent).await?;

// Inicializar tasks ready (promove Pending → Ready quando deps completas)
orch.initialize_ready_tasks().await?;

// Executar scheduling
let result = orch.schedule().await?;
// result.assignments: Vec<(TaskId, AgentId)>
// result.unassignable_tasks: Vec<TaskId>

// Reportar conclusão
orch.complete_task(&task.id, Some("output.json".into())).await?;

// Ou reportar falha (trigger retry se dentro do limite)
orch.fail_task(&task.id, "Timeout exceeded".into()).await?;
```

### 3.3 Environment Variables
| Variável | Default | Descrição |
|----------|---------|-----------|
| `SWARM_DB_PATH` | `:memory:` | Path do SQLite DB |
| `SWARM_MAX_RETRIES` | `3` | Máximo de retries por task |
| `SWARM_RETRY_BASE_MS` | `1000` | Base do backoff exponencial |

### 3.4 Checklist operacional
- [ ] DB acessível e íntegro (`PRAGMA integrity_check`)
- [ ] Agentes registrados com capabilities corretas
- [ ] Tasks sem ciclos (`validate_dag()` retorna Ok)
- [ ] Load balancing ativo (nenhum agente com load > 80%)
- [ ] Retry policy configurada conforme SLA do workflow

### 3.5 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| `CycleDetected` | Dependências circulares | `orch.validate_dag()` para identificar ciclo; remover aresta |
| Task stuck em `Pending` | Dependência nunca completa | Verificar task pai; completar manualmente ou cancelar |
| `UnassignableTasks` crescente | Nenhum agente com capability match | Registrar novo agente ou relaxar capabilities da task |
| Retry exhaustion | Falha persistente na execução | Investigar causa raiz; considerar decomposição da task |
| Schedule lento (>100ms) | Muitos nodes no DAG | Arquivar tasks antigas; particionar por workflow |
| State transition rejected | Transição inválida | Verificar máquina de estados; usar `get_task()` para estado atual |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd /c/Users/Paulo/zylos-next
cargo check -p zylos-swarm-orchestrator
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-swarm-orchestrator --lib
```
**Esperado:** `test result: ok. 10 passed; 0 failed`

### 4.3 Verificar status do orchestrator
```bash
# Via gateway API
curl -s http://localhost:9876/api/v1/swarm/stats | jq .
```
**Esperado:** JSON com `total_tasks`, `total_agents`, `active_agents`, `pending_tasks`, `running_tasks`.

### 4.4 Quando escalar
- Erro de compilação → verificar dependências no Cargo.toml (`petgraph`, `sqlx`)
- Teste falhando → verificar se DB está acessível; rodar com `--nocapture` para detalhes
- Cycle detected em produção → parar scheduling, notificar Tech Lead, seguir `playbooks/incident-response.md`
- Unassignable tasks > 10 por minuto → verificar registro de agentes; escalar SEV-3

### 4.5 Glossário
| Termo | Definição |
|-------|-----------|
| **DAG** | Directed Acyclic Graph — grafo direcionado sem ciclos que modela dependências entre tasks |
| **Toposort** | Ordenação topológica — sequência linear onde cada task vem após suas dependências |
| **Capability** | Habilidade declarada de um agente (ex: "python", "rust", "data-engineering") |
| **Load balancing** | Distribuição de tasks entre agentes baseada em carga atual |
| **Backoff exponencial** | Estratégia de retry onde espera dobra a cada tentativa (1s → 2s → 4s) |
| **Crash recovery** | Capacidade de restaurar estado após falha via persistência SQLite WAL |

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-swarm-orchestrator/src/lib.rs`
- **Playbook:** `playbooks/workflow-orchestration.md`
- **Runbook relacionado:** `cerebelo-orchestration.md` (recovery ladder integration)
- **Runbook relacionado:** `zylos-bot-system.md` (DAG executor)
- **SOAR workflow:** `docs/soar/runbooks/multi-agent-orchestration.runbook.yaml`
- **HANDOFF_7:** `HANDOFF_7.md`