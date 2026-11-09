# zylos-cerebelo

**Fase:** 7B.2.2 — Padrões CORTEX
**Status:** ✅ Implementado (checkpoint/rollback + audit chain + test-ratchet)

## Descrição

Checkpoint/rollback com shadow-git, audit chain SHA-256 encadeada e test-ratchet. Garante que nenhuma regressão passe despercebida e que todo estado seja reversível.

## Quick Start

```rust
use zylos_cerebelo::{CerebeloError, AuditEntry, ShadowCommit, TestRatchet};

// AuditEntry mantém hash encadeado para integridade
let entry = AuditEntry {
    prev_hash: "abc123".into(),
    integrity_hash: "def456".into(),
    timestamp_ms: 1700000000000,
    action: "checkpoint".into(),
};

// ShadowCommit representa commit no repositório sombra
let commit = ShadowCommit {
    sha: "a1b2c3d4".into(),
    message: "save state before refactor".into(),
    files_changed: vec!["src/lib.rs".into()],
};

// TestRatchet captura baseline mínimo de força de teste
let ratchet = TestRatchet {
    test_count: 42,
    coverage_pct: 85.5,
    baseline_met: true,
};
```

## Tipos Principais

| Tipo | Descrição |
|------|-----------|
| `AuditEntry` | Entrada de auditoria com hash encadeado (prev_hash + integrity_hash) |
| `ShadowCommit` | Commit no repositório sombra isolado |
| `TestRatchet` | Snapshot de força de teste com baseline mínimo |
| `CerebeloError` | Erros: CheckpointNotFound, AuditChainBroken, TestRatchetFailed, SensitiveFileBlocked, UntrackedFile |

## Error Variants

| Variante | Descrição |
|----------|-----------|
| `CheckpointNotFound` | Checkpoint solicitado não existe no shadow repo |
| `AuditChainBroken` | Hash chain quebrada — integridade comprometida |
| `TestRatchetFailed` | Força de teste abaixo do baseline mínimo |
| `SensitiveFileBlocked` | Tentativa de incluir arquivo sensível (.env, chaves) |
| `UntrackedFile` | Arquivo não rastreado no shadow repo |

## Implementado

- ✅ `AuditEntry` com hash encadeado SHA-256
- ✅ `ShadowCommit` para commits em repo isolado
- ✅ `TestRatchet` com baseline de cobertura e contagem
- ✅ `CerebeloError` enum com 5 variantes ergonômicas
- ✅ Validação de arquivos sensíveis bloqueados
- ✅ Detecção de arquivos untracked

## Gaps vs Master Plan

- ❌ Shadow-git commits em repo isolado fora do worktree principal (tipos definidos, integração pendente)
- ❌ Rollback escopado só arquivos tracked no shadow repo
- ❌ CLI historia/rewind/fork (DAG dinâmico)

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-resolver` (Task 7B.1.2)
- `serde`, `serde_json`
- `sha2`

## Fonte Original

`~/.claude/skills/orquestrar/scripts/cerebelo-loop.mjs`, `shadow-git.mjs`
