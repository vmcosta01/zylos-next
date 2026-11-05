# zylos-cerebelo

**Fase:** 7B.2.2 — Padrões CORTEX
**Status:** ❌ Stub (~10% do plano)

## Descrição

Checkpoint/rollback com shadow-git, audit chain SHA-256 encadeada e test-ratchet. Garante que nenhuma regressão passe despercebida e que todo estado seja reversível.

## Tipos Principais

- `AuditEntry` — Entrada de auditoria com hash encadeado (prev_hash + integrity_hash)
- `ShadowCommit` — Commit no repositório sombra isolado
- `TestRatchet` — Snapshot de força de teste com baseline mínimo

## Gaps vs Master Plan

- ❌ Shadow-git commits em repo isolado fora do worktree principal
- ❌ Audit chain SHA256 encadeada completa
- ❌ Test-ratchet snapshot força de teste
- ❌ Rollback escopado só arquivos tracked no shadow repo
- ❌ Exclusão de padrões sensíveis (.env, chaves, credenciais)
- ❌ CLI historia/rewind/fork (DAG dinâmico)

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-resolver` (Task 7B.1.2)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/cerebelo-loop.mjs`, `shadow-git.mjs`
