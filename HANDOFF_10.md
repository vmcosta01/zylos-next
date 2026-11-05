# HANDOFF — Fase 10: SOAR Engine (zylos-soar)

**Data:** 2026-11-02
**Status:** ✅ CONCLUÍDA
**Testes:** 48/48 passando (`cargo test -p zylos-soar`)

---

## Resumo Executivo

A Fase 10 implementou o crate `zylos-soar` completo — motor de orquestração de segurança determinístico, stateless e auditável. Todos os módulos foram validados com testes unitários e de integração. O import ausente de `PlaybookState` no módulo de export foi corrigido durante a validação final.

---

## Módulos Entregues

| Módulo | Arquivo | Descrição | Testes |
|--------|---------|-----------|--------|
| **event** | `src/event.rs` | Tipos core: `Severity`, `EventCategory`, `ZylosSecurityEvent` | 2 |
| **triage** | `src/triage.rs` | Auto-triage determinístico (5 ações: IsolateAndNotify, EnrichAndAssign, CreateTicket, LogOnly, Suppress) | 3 |
| **playbook** | `src/playbook.rs` | State machine YAML-driven com validação estrutural, `PlaybookCursor` para avanço de estados | 5 |
| **router** | `src/router.rs` | EventRouter: match de triggers contra eventos, dispatch para playbooks | 5 |
| **orchestrator** | `src/orchestrator.rs` | Orquestrador end-to-end: ingest → triage → route → execute | 2 |
| **executor** | `src/executor.rs` | Executor de playbook: drive cursor até estado terminal | 2 |
| **runnable** | `src/runnable.rs` | Trait `SoarRunnable` (dyn-compatible) + `NoopRunbook` para testes | 4 |
| **registry** | `src/registry.rs` | Registry de runbooks com lookup por nome | 3 |
| **audit** | `src/audit.rs` | Audit log com hash chaining e verificação de integridade | 3 |
| **validation** | `src/validation.rs` | Validador estático: detecta orphan runbooks, approval sem timeout, self-loop, triggerless, unbound refs | 6 |
| **webhook** | `src/webhook.rs` | Axum HTTP endpoints: `/webhook/ingest` + `/health` com normalização de payload | 5 |
| **export** | `src/export.rs` | Multi-adapter export: `.cursor/rules` (.mdc), `.clinerules` (.json), `.claude-plugin` (manifest + playbooks/) | 8 |

**Total: 48 testes passando**

---

## Correções Aplicadas Durante Validação

- `crates/zylos-soar/src/export.rs:511` — Adicionado `PlaybookState` ao import do módulo de testes (`use crate::playbook::{Playbook, PlaybookRoles, PlaybookState, PlaybookTrigger, Transition}`)

---

## Arquitetura

```
Webhook Ingest → normalize_payload() → TriageEngine.classify()
                                            ↓
                                    EventRouter.route()
                                            ↓
                                    PlaybookCursor.advance()
                                            ↓
                                    Executor (SoarRunnable trait)
                                            ↓
                                    AuditLog.append()
```

### Decisões de Design

1. **Determinístico por padrão** — Triage usa pattern matching puro, sem LLM. LLM entra apenas como enrichment opcional em runbooks.
2. **Stateless engine** — Estado vive no `PlaybookCursor` (caller-owned), não no engine. Permite múltiplas execuções concorrentes sem lock global.
3. **Audit trail obrigatório** — Cada entrada de audit inclui hash da anterior. Cadeia verificável offline.
4. **Export multi-formato** — Mesma representação interna (`ExportablePlaybook`) gera 3 formatos de agente externo sem duplicar lógica.
5. **Validação estática separada** — `validation.rs` roda antes da execução, falhando cedo em playbooks malformados.

---

## Dependências do Crate

```toml
axum = "0.7"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
sha2 = "0.10"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["trace"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

---

## Próximos Passos (Fase 11+)

1. **Integração Cockpit ↔ SOAR** — Conectar dashboard ao webhook endpoint para visualização de eventos em tempo real
2. **Condition evaluator** — Implementar avaliação de expressões condicionais nos transitions (atualmente TODO em `playbook.rs:179`)
3. **Background executor** — Hand off de incidents despachados para task pool assíncrona (TODO em `webhook.rs:156`)
4. **Runbooks reais** — Substituir `NoopRunbook` por implementações que chamam APIs de segurança (CrowdStrike, SentinelOne, etc.)
5. **Métricas Prometheus** — Expor contadores de eventos por severidade/categoria e latência de triage

---

## Referências

- Master Plan: `zylos-next-master-plan.md` linhas 11-12 (SOAR architecture)
- SOAR Architecture: `docs/soar/SOAR_ARCHITECTURE.md`
- Runbook Schema: `docs/soar/runbook-schema.yaml`
- Skills Hermes: `runbook-authoring`, `playbook-authoring`, `soar-orchestration`, `aiops-observability`