# zylos-triage

**Fase:** 7C.2.1 — Padrões Externos
**Status:** ⚠️ Migrated to `zylos-soar`

## Nota de Migração

A implementação two-stage triage foi movida para `crates/zylos-soar/src/triage_message.rs` durante a Fase 7C. Este crate permanece como referência histórica e re-export.

**Para a implementação atual, veja:** [`zylos-soar`](../zylos-soar/README.md)

## O que foi implementado (agora em zylos-soar)

- ✅ Two-stage triage: Stage 1 rules (safe negatives only) + Stage 2 model fallback
- ✅ `classify_message()` + `route_message()` com 20 testes passando
- ✅ Campo `decided_by` auditável: `"rule:<id>"` | `"model:<name>"`
- ✅ Framing pattern detection para scheduled routines e handoffs
- ✅ Initiator tracking: Person, Routine, Responsibility, Memory, Handoff, System
- ✅ Safe-negative constraint: Stage 1 NEVER produz NeedsYou

## Gaps vs Master Plan

- ❌ Eval measurement contra corpus rotulado

## Runbook

[`fase7c-external-patterns.md`](../../runbooks/fase7c-external-patterns.md)

## Fontes

OpenDuet `triage.ts`, `routine-firing.ts`, `initiator.ts`
