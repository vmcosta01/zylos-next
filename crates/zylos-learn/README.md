# zylos-learn

**Fase:** 7B.2.3 — Padrões CORTEX
**Status:** ✅ Implementado (learning loop + outcome classification + witness anti-regressão)

## Descrição

Loop Aprender→Persistir→Aplicar com classificação de desfechos, recorrência mecânica e witness anti-regressão. Transforma falhas em lições duradouras via JSONL append-only.

## Quick Start

```rust
use zylos_learn::{LearningLoop, OutcomeClass, Outcome, PendingLesson, Witness};

// Classificar desfecho de uma execução
let outcome = Outcome {
    class: OutcomeClass::FalhaProvada,
    signature: "tool:http_5xx|endpoint:/api/users".into(),
    timestamp_ms: 1700000000000,
};

// LearningLoop gerencia persistência e recuperação de lições
let mut learner = LearningLoop::new("./data/lessons.jsonl");
learner.append_pending(&outcome)?;

// Witness marca arquivo para check anti-regressão determinístico
let witness = Witness {
    marker_path: "./tests/fixtures/api_contract.json".into(),
    expected_hash: "sha256:a1b2c3d4...".into(),
};
```

## Tipos Principais

| Tipo | Descrição |
|------|-----------|
| `OutcomeClass` | Sucesso / Escalacao / FalhaProvada / LacunaCapacidade |
| `Outcome` | Desfecho registrado com assinatura normalizada |
| `PendingLesson` | Lição pendente em JSONL append-only |
| `Witness` | Marcador-em-arquivo + check determinístico anti-regressão |
| `LearningLoop` | Loop principal com persistência JSONL e gate de recorrência |

## Implementado

- ✅ `LearningLoop` struct com persistência JSONL append-only
- ✅ `OutcomeClass` enum com 4 variantes (Sucesso, Escalacao, FalhaProvada, LacunaCapacidade)
- ✅ Assinatura normalizada para agrupamento por recorrência
- ✅ Gate de recorrência ≥3× → lição madura (`recurrence_threshold = 3`)
- ✅ `Witness` anti-regressão com marcador-em-arquivo + check determinístico
- ✅ `append_pending()` em JSONL append-only
- ✅ Serialização/deserialização via serde

## Gaps vs Master Plan

- ❌ Integração com `zylos-proof` para validação de lições maduras
- ❌ Métricas de efetividade de lições aplicadas

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)
- `serde`, `serde_json`
- `sha2`

## Fonte Original

`~/.claude/skills/orquestrar/scripts/learn.mjs` (41 linhas)
