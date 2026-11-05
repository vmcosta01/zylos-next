# zylos-hitl

**Fase:** 7C.1.1 + 7C.1.3 — Padrões Externos
**Status:** ✅ Maduro (75%+ do plano)

## Descrição

Human-in-the-loop approval cards via AG-UI protocol com storage server-side, equivalence-based hashing, four-tier decision e outcome uncertainty tri-state.

## Tipos Principais

- `Outcome` — Succeeded/Failed/OutcomeUnknown (tri-state)
- `OutcomeVerdict` — Classificação de HTTP/network/validation errors
- `ApprovalRequest` — Pedido de aprovação com thread_id, tool_call_id, TTL
- `DecisionTier` — Safety > TeamRule > PersonalRule > AutoReview > Default
- `RefusalReason` — 7 mensagens de recusa distintas

## Implementado

- ✅ Outcome Uncertainty Tri-State (Task 7C.1.3)
- ✅ Equivalence-based hashing (blake3 em `zylos-task-durability::approval_hash`)
- ✅ Four-tier decision hierarchy
- ✅ 7 refusal messages
- ✅ Secret masking
- ✅ Idempotent result recovery
- ✅ EventBus integration
- ✅ Validation probe task-local (`zylos-task-durability::approval_hash::with_validation_probe`)

## Módulos Relacionados

- **Approval Hashing (blake3):** [`zylos-task-durability`](../zylos-task-durability/README.md) — `approval_hash` module
- **Outcome Uncertainty:** [`zylos-task-durability`](../zylos-task-durability/README.md) — `outcome` module

## Gaps vs Master Plan

- ❌ carry_on() multi-decision step
- ❌ withdraw_allowed() selective close
- ❌ abandon() synthetic result unblock
- ❌ is_permanent() structural error detection
- ❌ public_approval() preview whole
- ❌ AG-UI protocol bindings para HITL Approval Cards (Task 7C.1.1)

## Runbook SOAR

[`hitl-approval.runbook.yaml`](../../docs/soar/runbooks/hitl-approval.runbook.yaml)

## Fontes

- OpenDuet `openbot/server/src/approvals/service.ts` (linhas 42-53)
- OpenDots `connection-routes.ts`
