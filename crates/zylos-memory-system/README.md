# zylos-memory-system

Sistema de memória estruturada para agentes Zylos, inspirado no Memanto/LICOES. Fornece armazenamento vetorial, grafo de contexto SQLite WAL, detecção de contradições, reranking aware de briefs e migração Hermes→Zylos.

## Status vs Master Plan (Fase 1)

| Task | Descrição | Status |
|------|-----------|--------|
| 1.1.1 | VectorStore trait (Rig-compatible) | ✅ Implementado |
| 1.1.2 | SQLite ContextGraph WAL | ✅ Implementado |
| 1.1.3 | 4-Scope Isolation (Global/Team/Agent/Task) | ✅ Implementado |
| 1.2.1 | MemoryAgent (Memanto-inspired) | ✅ Implementado |
| 1.2.2 | BriefAwareRerank (PaperRadar) | ✅ Implementado |
| 1.2.3 | TypedSemanticMemory (OKF) | ✅ Implementado — wrapper typed com routing automático |
| 1.3.1 | ContradictionDetection | ✅ Implementado |
| 1.3.2 | LICOES ProceduralMemory (MemoryKind/ProvenanceTriple/CompressionPipeline/WriteGate/SessionGoals) | ✅ Implementado |
| 1.3.3 | MigrationAdapter Hermes→Zylos | ✅ Implementado |
| 1.4 | Benchmarks | ❌ Ausente |

## Features Implementadas

- **VectorStore**: trait async com insert/search/delete/count/get; InMemoryVectorStore brute-force cosine
- **Qdrant Adapter**: feature-gated (`qdrant`), backend real usando qdrant-client
- **LanceDB Adapter**: feature-gated (`lancedb`), backend real usando lancedb crate
- **ContextGraph**: petgraph DiGraph + rusqlite WAL, A* shortest_path, checkpoint
- **MemoryScope**: Global, Team, Agent, Task com isolamento por owner_id
- **TypedSemanticMemory**: wrapper OKF-inspired com routing automático por tipo (Working/Episodic/Semantic/Procedural/Meta), provenance tracking, factory pattern
- **MemoryAgent**: consolidation cycle, decay, conflict detection via CancellationToken
- **BriefAwareReranker**: 4-component scoring (semantic/keyword/tag/recency), extract_key_concepts PT+EN
- **ContradictionDetector**: direct/implicit/temporal detection, political_weight resolution
- **MemoryKind**: Working/Episodic/Semantic/Procedural/Meta com route_backend()
- **CompressionPipeline**: extract_facts → link_entities → score_decay → schedule_gc (pure stages)
- **WriteGate**: disposition-based validation (Approved/Rejected/PendingReview)
- **SessionGoal**: criteria tracking with evidence marking
- **MigrationAdapter**: JSON/JSONL Hermes import com integrity validation
- **EvidenceCapsule**: finding/evidence/decision/disposition struct

## API Pública

```rust
// Vector storage
pub trait VectorStore: Send + Sync { ... }
pub struct InMemoryVectorStore;

// Context graph
pub struct ContextGraph { ... }
impl ContextGraph {
    pub fn open(path) -> Result<Self>;
    pub fn in_memory() -> Self;
    pub fn add_node/add_edge/remove_node/get_node/query/get_neighbors/shortest_path/stats/checkpoint;
}

// Typed semantic memory (OKF-inspired)
pub enum SemanticType { Working, Episodic, Semantic, Procedural, Meta }
pub struct TypedSemanticMemory { ... }
impl TypedSemanticMemory {
    pub fn new(semantic_type: SemanticType) -> Self;
    pub fn with_vector_store(self, store: Arc<dyn VectorStore>) -> Self;
    pub fn with_context_graph(self, graph: Arc<ContextGraph>) -> Self;
    pub async fn store(&self, content, scope, owner_id, tags) -> Result<ContextNode, TypedMemoryError>;
    pub async fn query(&self, query, scope, limit) -> Result<Vec<ContextNode>, TypedMemoryError>;
}
pub struct TypedMemoryFactory { ... }
impl TypedMemoryFactory {
    pub fn working/episodic/semantic/procedural/meta(&self) -> TypedSemanticMemory;
}

// Memory agent
pub struct MemoryAgent { ... }
impl MemoryAgent {
    pub fn start/stop/run_consolidation_cycle/detect_conflicts/apply_decay/reconcile_memories;
}

// Reranking
pub struct BriefAwareReranker { ... }
impl BriefAwareReranker { pub fn rerank(&self, ...) -> Vec<RankedMemory>; }

// Contradictions
pub struct ContradictionDetector;
impl ContradictionDetector { pub fn detect_all(&self, ...) -> Vec<Contradiction>; }

// Types
pub enum MemoryScope { Global, Team, Agent, Task }
pub enum MemoryKind { Working, Episodic, Semantic, Procedural, Meta }
pub struct ContextNode { id, content, scope, owner_id, content_hash, embedding, tags, ... }
```

## Gaps Críticos

1. **Benchmarks (Task 1.4)**: nenhum benchmark de performance ou recall implementado
2. **ReasoningBank**: mencionado no master plan como parte do LICOES mas não encontrado no código

## Exemplo de Uso

```rust
use zylos_memory_system::{ContextGraph, MemoryScope, ContextNode};

let mut graph = ContextGraph::in_memory();
let node = ContextNode {
    id: Uuid::new_v4(),
    content: "User prefers dark mode".into(),
    scope: MemoryScope::Agent,
    owner_id: Some("agent-001".into()),
    ..Default::default()
};
graph.add_node(node)?;
let results = graph.query(ContextQuery { scope: Some(MemoryScope::Agent), ..Default::default() })?;
```