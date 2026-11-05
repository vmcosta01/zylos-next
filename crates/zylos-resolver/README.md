# zylos-resolver

**Fase:** 7B.1.2 — Padrões CORTEX
**Status:** ⚠️ Parcial (~35% do plano)

## Descrição

Resolver DAG com fronteira BFS, checkpoint/resume e paralelismo determinístico. Garante que tarefas complexas sejam executadas em ordem topológica com capacidade de pausa e retomada segura.

## Tipos Principais

- `DagNode` — Nó do DAG com id, dependências, status e profundidade
- `BfsFrontier` — Fila BFS com snapshot/restore para checkpoint
- `DagGraph` — Grafo direcionado acíclico com validação de ciclos
- `ResolverCheckpoint` — Estado serializável para persistência

## Gaps vs Master Plan

- ❌ `resolve()` completo com limites max_depth/max_nodes
- ❌ Checkpoint por onda via persist_path (JSON atômico tmp+rename)
- ❌ Paralelismo opt-in max_parallel com ordem preservada
- ❌ `glue()` prova da cola entre claims de sub-objetivos
- ❌ Resume com anti-forja (arquivo ausente/corrompido → recomeça)
- ❌ Struct `corrida` para validação temporal de screenshots

## Runbook SOAR

[`dag-execution.runbook.yaml`](../../docs/soar/runbooks/dag-execution.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/resolver.mjs` (93 linhas)
