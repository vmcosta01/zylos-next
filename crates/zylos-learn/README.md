# zylos-learn

**Fase:** 7B.2.3 — Padrões CORTEX
**Status:** ⚠️ Parcial (~25% do plano)

## Descrição

Loop Aprender→Persistir→Aplicar com classificação de desfechos, recorrência mecânica e witness anti-regressão. Transforma falhas em lições duradouras.

## Tipos Principais

- `OutcomeClass` — Sucesso/Escalacao/FalhaProvada/LacunaCapacidade
- `Outcome` — Desfecho registrado com assinatura normalizada
- `PendingLesson` — Lição pendente em JSONL append-only
- `Witness` — Marcador-em-arquivo + check determinístico anti-regressão

## Gaps vs Master Plan

- ❌ `append_pending()` em JSONL append-only
- ❌ Assinatura normalizada para agrupamento por recorrência
- ❌ Gate de recorrência ≥3× → lição madura
- ❌ Witness anti-regressão marcador-em-arquivo + check determinístico

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/learn.mjs` (41 linhas)
