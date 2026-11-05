# zylos-task-durability

**Fase:** 7C — Padrões Externos + Task Durability
**Status:** ✅ Maduro (75%+ do plano)

## Descrição

Task durability via SQL leases com heartbeat, LostLeaseError, maintenance loop e circuit breaker. Inclui módulos de Outcome Uncertainty Tri-State e Equivalence-Based Approval Hashing implementados na Fase 7C. Inspirado em OpenMuse e OpenDuet.

## Módulos

### Core (SQL Leases)
- `TaskStatus` — State machine completa (9 estados, #[repr(u8)])
- `LostLeaseError` — Erro quando CAS falha ou abort signal fired
- `Lease` — Lease SQL com UUID e timestamp
- `TaskRecord` — Registro completo de tarefa com histórico Q&A
- `ControlAction` — pause/resume/cancel/retry com CAS precondition

### Outcome Uncertainty (`outcome`)
- `Outcome` — Tri-state: Succeeded / Failed / Unknown
- `classify_error()` — Classifica erros HTTP/network/client em Outcome apropriado
- `needs_human_confirmation()` — Retorna true para Unknown outcomes que requerem confirmação humana
- `OutcomeTracker` — Audit log com dedup de notificações por `(task_id, outcome, error_signature)`
- 15 testes passando

### Approval Hashing (`approval_hash`)
- `ApprovalDigest` — Blake3 hash wrapper para dedup de aprovações equivalentes
- `PolicyTier` — Safety > TeamRule > PersonalRule > AutoReview > Default
- `PolicyTierEvaluator` — Pure-function evaluator retornando `TierDecision`
- `ExecutionAuthority` — Task-local context via `with_authority(auth, || async { ... })`
- `current_authority()` / `is_approval_recheck()` — Leitura síncrona de task-locals
- Volatile fields stripped antes do hash: snapshot_id, ref, tool_call_id, timestamp, nonce, request_id, trace_id
- 18 testes passando

## Design Decisions

- **Database-as-clock**: All lease timing uses SQL `now() + interval`, NEVER `Utc::now() + Duration`
- **SELECT FOR UPDATE SKIP LOCKED**: Zero contention em multi-replica
- **LostLeaseError**: Reverte task para `queued` atomicamente
- **pg_notify inside transaction**: Fires on commit never before

## Runbook SOAR

[`task-durability.runbook.yaml`](../../docs/soar/runbooks/task-durability.runbook.yaml)

## Fontes

- OpenMuse `apps/server/src/engine/worker.ts` (linhas 64-76)
- OpenDuet `openbot/server/src/work/queue.ts` (linhas 77-81)
