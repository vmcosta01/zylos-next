# zylos-soar

**Fase:** 7C — SOAR Orchestration + Two-Stage Triage
**Status:** ✅ Maduro (75%+ do plano)

## Descrição

Security Orchestration, Automation and Response (SOAR) engine com two-stage message triage, event routing e playbook execution. Integra-se com zylos-task-durability para approval hashing e outcome tracking.

## Módulos

### Two-Stage Message Triage (`triage_message`)
- **Stage 1 (Rules):** Safe negatives apenas (CanWait, AutoHandle). NEVER produz NeedsYou.
- **Stage 2 (Model):** Fallback quando nenhuma rule match. Retorna Err se falhar — nunca default silencioso.
- `classify_message()` — Classifica mensagem inbound em TriageVerdict
- `route_message()` — Roteia mensagem baseada no verdict + initiator tracking
- `decided_by` audit: `"rule:<id>"` ou `"model:<name>"` em toda decisão
- Framing patterns: Wrappers para scheduled routines e handoffs previnem misinterpretação
- Initiator tracking: Person, Routine, Responsibility, Memory, Handoff, System
- 20 testes passando

### Event Routing
- `EventRouter` — Routes events to appropriate handlers based on type + priority
- `PlaybookExecutor` — Executes multi-step playbooks with rollback support
- Integration com zylos-task-durability para lease-based task execution

## Design Decisions

- **Two-stage over single-model:** Rules catch safe cases cheaply; model only invoked when needed
- **Safe-negative rules:** Stage 1 can only say "no action needed" or "auto-handle", never "needs human"
- **Audit-first:** Every decision carries `decided_by` for post-incident analysis
- **Framing-aware:** Prevents misclassification of routine messages as urgent

## Runbook

[`fase7c-external-patterns.md`](../../runbooks/fase7c-external-patterns.md)

## Fonte

Master plan linhas 1762-1830, OpenMausBot `server/triage/`