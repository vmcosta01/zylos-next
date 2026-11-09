# zylos-resolver

**Fase:** 7B.1.2 — Padrões CORTEX
**Status:** ✅ Implementado (DAG resolver + BFS frontier + checkpoint/resume)

## Descrição

Resolver DAG com fronteira BFS, checkpoint/resume e paralelismo determinístico. Garante que tarefas complexas sejam executadas em ordem topológica com capacidade de pausa e retomada segura.

## Quick Start

```rust
use zylos_resolver::{DagNode, BfsFrontier, DagGraph, ResolverCheckpoint};

// Construir grafo DAG com validação de ciclos
let mut graph = DagGraph::new();
graph.add_node(DagNode {
    id: "task-a".into(),
    deps: vec![],
    depth: 0,
});
graph.add_node(DagNode {
    id: "task-b".into(),
    deps: vec!["task-a".into()],
    depth: 1,
});

// Fronteira BFS com snapshot para checkpoint
let mut frontier = BfsFrontier::from_graph(&graph);
let snapshot = frontier.snapshot(); // serializável para resume

// Checkpoint atômico (tmp + rename)
let checkpoint = ResolverCheckpoint {
    frontier: snapshot,
    completed: vec!["task-a".into()],
    wave: 1,
};
```

## Tipos Principais

| Tipo | Descrição |
|------|-----------|
| `DagNode` | Nó do DAG com id, dependências, status e profundidade |
| `BfsFrontier` | Fila BFS com snapshot/restore para checkpoint |
| `DagGraph` | Grafo direcionado acíclico com validação de ciclos |
| `ResolverCheckpoint` | Estado serializável para persistência e resume |

## Implementado

- ✅ `DagGraph` com validação de ciclos e ordem topológica
- ✅ `BfsFrontier` com snapshot/restore para checkpoint seguro
- ✅ `DagNode` com id, dependências, status e profundidade
- ✅ `ResolverCheckpoint` serializável para persistência
- ✅ Validação anti-forja (arquivo ausente/corrompido → recomeça)
- ✅ Limites `max_depth` e `max_nodes` configuráveis

## Gaps vs Master Plan

- ❌ `resolve()` completo com execução real de nós
- ❌ Paralelismo opt-in `max_parallel` com ordem preservada
- ❌ `glue()` prova da cola entre claims de sub-objetivos
- ❌ Struct `corrida` para validação temporal de screenshots

## Runbook SOAR

[`dag-execution.runbook.yaml`](../../docs/soar/runbooks/dag-execution.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)
- `serde`, `serde_json`

## Fonte Original

`~/.claude/skills/orquestrar/scripts/resolver.mjs` (93 linhas)
