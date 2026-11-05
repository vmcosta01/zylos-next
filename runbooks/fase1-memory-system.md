# Fase 1: Memory System — Runbook Zylos-Next

## Visão Geral

O MEMORY_SYSTEM (`zylos-memory-system`) implementa armazenamento persistente de contexto com SQLite WAL + petgraph, isolamento em 4 escopos (Global, Team, Agent, Task), busca vetorial via `VectorStore` trait, rerank brief-aware, detecção de contradições, Memory Agent autônomo para consolidação, WriteGate preventivo com evidence capsules, e pipeline de compressão de memória.

**Módulos principais** (de `crates/zylos-memory-system/src/lib.rs`):
- `context_graph` — ContextGraph com SQLite WAL backend
- `vector_store` — VectorStore trait + InMemoryVectorStore
- `memory_kind` — MemoryKind enum (Working, Episodic, Semantic, Procedural, Meta)
- `rerank` — BriefAwareReranker com ResearchBrief
- `contradiction` — ContradictionDetector
- `memory_agent` — MemoryAgent com loop de consolidação
- `write_gate` — WriteGate preventivo com EvidenceCapsule
- `session_goal` — SessionGoal orientado a critérios
- `migration` — MigrationAdapter para integridade
- `document_spaces` — Espaços de documentos isolados

**Critério de Saída:** Benchmark Memanto + LongMemEval + MARBLE passa; grafos 100k nós <1GB RAM; traversal 5 hops <50ms.

## Pré-requisitos

- Fase 0 completa (workspace funcional, `zylos-core` disponível)
- Dependências: `rusqlite` (com feature `bundled`), `petgraph 0.6`, `serde`, `chrono`, `uuid`
- Feature flags opcionais: `qdrant` (para Qdrant adapter), `lancedb` (para LanceDB adapter)
- SQLite com WAL mode habilitado
- Embeddings model configurado (para VectorStore)

### Setup

```bash
# Build com features
cargo build -p zylos-memory-system --features qdrant,lancedb

# Testes
cargo test -p zylos-memory-system

# Benchmarks
cargo bench -p zylos-memory-system
```

## Para Dev Senior

### Decisões Arquiteturais

1. **MemoryKind Enum (Koog-inspired)**: `Working | Episodic | Semantic | Procedural | Meta` — cada tipo tem estratégia de roteamento diferenciada. Working vai para context window, Episodic para SQLite, Semantic para vector store, Procedural para SKILL.md files, Meta para reasoning bank. O enum está em `memory_kind.rs` junto com `BackendTarget`, `route_backend()`, `ProvenanceTriple`, `CompressionPipeline`, `ExtractedFact`, `EntityLink`, `DecayScore`, `GcAction`, `GcDisposition`, `EvidenceCapsule`, `EvidenceDisposition`.

2. **Provenance Triple (Meldwork-inspired)**: Toda memória carrega `{scope: Session|User|Agent|Global, origin: tool_call_id|human_input|inference, completeness: partial|verified|contradicted}`. Metadata obrigatória em todo insert. Definida como `ProvenanceTriple` com `ProvenanceScope` e `ProvenanceOrigin` enums.

3. **WriteGate Preventivo**: Gate pré-escrita que valida `EvidenceCapsule {Finding→Evidence→Decision→Disposition}` ANTES de qualquer write externo. Previne writes baseados em premissas não verificadas. Exportado como `WriteGate`, `GateResult`, `Disposition`.

4. **4-Scope Isolation**: `MemoryScope` enum com Global, Team, Agent, Task. Dados de um agente não vazam para outro. Sharing apenas quando explicitamente permitido. GC remove dados expirados sem afetar ativos.

5. **BriefAwareReranker**: Filtra memórias relevantes ao objetivo atual usando `ResearchBrief`. Reduz ruído em >40% vs Jaccard puro. Latência target <100ms para rerank de 1k memórias.

6. **Session Goals Orientados a Critérios**: Goals definidos como `{criteria: [...], evidence_required: [...]}`. Execução autônoma continua até todos critérios satisfeitos com evidência verificável.

### Extension Points

- **Novos VectorStore backends**: Implemente `trait VectorStore` (object-safe) para qualquer backend. Adicione feature flag. Exemplos existentes: `QdrantVectorStore`, `LanceDBVectorStore`.
- **Novos MemoryKind variants**: Adicione ao enum em `memory_kind.rs`, implemente `route_backend()` para o novo tipo.
- **Custom ContradictionDetector**: Estenda `ContradictionType` enum para novos tipos de contradição.
- **Migration adapters**: Implemente `MigrationAdapter` trait para migração entre formatos.

### Performance Tuning

- Grafos 100k nós devem caber em <1GB RAM — monitore com benchmarks
- Traversal 5 hops <50ms — profile com criterion
- Compactação deve reduzir tamanho >50%
- Search em 10k embeddings <10ms
- Rerank 1k memórias <100ms
- Use WAL mode para concorrência de leitura/escrita
- FTS5 para busca full-text em nodes

## Para Dev Pleno

### Usando ContextGraph

```rust
use zylos_memory_system::{ContextGraph, ContextNode, ContextEdge, MemoryScope};

// Criar grafo com SQLite WAL
let graph = ContextGraph::open("memory.db")?;

// Adicionar node com provenance
let node = ContextNode {
    id: NodeId::new("fact_001"),
    content: "API rate limit is 100 req/min".into(),
    scope: MemoryScope::Agent,
    // ... outros campos
};
graph.add_node(node)?;

// Adicionar edge
let edge = ContextEdge {
    from: NodeId::new("fact_001"),
    to: NodeId::new("fact_002"),
    relation: "supports".into(),
};
graph.add_edge(edge)?;

// Traverse neighborhood
let neighbors = graph.traverse(NodeId::new("fact_001"), 3)?;

// Stats
let stats = graph.stats()?;
println!("Nodes: {}, Edges: {}", stats.node_count, stats.edge_count);
```

### Usando VectorStore Trait

```rust
use zylos_memory_system::{VectorStore, InMemoryVectorStore, VectorDocument, SearchParams};

// InMemory para testes
let store = InMemoryVectorStore::new();

// Insert
let doc = VectorDocument {
    id: "doc_001".into(),
    embedding: vec![0.1, 0.2, 0.3], // 384-dim ou 768-dim
    content: "Rate limiting policy".into(),
    metadata: serde_json::json!({"scope": "agent"}),
};
store.insert(doc).await?;

// Search
let results = store.search(SearchParams {
    query_embedding: vec![0.1, 0.2, 0.3],
    top_k: 5,
    filter: None,
}).await?;
```

### Usando WriteGate

```rust
use zylos_memory_system::{WriteGate, EvidenceCapsule as WgEvidenceCapsule, Disposition};

let gate = WriteGate::new();

// Validar antes de escrever
let capsule = WgEvidenceCapsule {
    finding: "User prefers dark mode".into(),
    evidence: vec!["settings.json contains theme=dark".into()],
    decision: "Apply dark mode preference".into(),
    disposition: Disposition::Proceed,
};

match gate.validate(&capsule) {
    GateResult::Approved => { /* proceed with write */ },
    GateResult::Rejected(reason) => { /* log and skip */ },
}
```

### Usando BriefAwareReranker

```rust
use zylos_memory_system::{BriefAwareReranker, RerankConfig, ResearchBrief, RankedMemory};

let config = RerankConfig::default();
let reranker = BriefAwareReranker::new(config);

let brief = ResearchBrief {
    objective: "Debug authentication failure".into(),
    context: vec!["OAuth2 flow", "token refresh".into()],
    // ...
};

let ranked = reranker.rerank(&memories, &brief)?;
// ranked é Vec<RankedMemory> ordenado por relevância
```

## Para Dev Junior

### Passo a Passo: Adicionando Memória

1. **Entenda os tipos**: Leia `memory_kind.rs` para entender MemoryKind e ProvenanceTriple
2. **Escolha o escopo correto**: Session (temporário), User (cross-session), Agent (específico do agente), Task (vinculado a task)
3. **Sempre forneça provenance**: Todo insert precisa de scope, origin, completeness
4. **Use WriteGate antes de writes externos**: Nunca escreva baseado em premissa não verificada
5. **Teste com InMemoryVectorStore**: Antes de integrar com Qdrant/LanceDB, valide lógica com store in-memory

### Pitfalls Comuns

- **Não misture escopos**: Dados de Agent scope nunca devem aparecer em queries de outro agente sem sharing explícito
- **Não ignore decay scores**: Memórias antigas perdem relevância; GC as remove periodicamente
- **Não faça rerank sem brief**: BriefAwareReranker precisa de ResearchBrief contextualizado
- **Não insira sem provenance triple**: O sistema rejeita inserts sem metadata obrigatória
- **Não confie em embeddings sozinhos**: Sempre combine vector search com graph traversal para contexto completo

### Debugging Tips

- Use `graph.stats()` para verificar integridade do grafo
- Logs de contradiction detection estão em nível `debug` — ative com `RUST_LOG=zylos_memory_system=debug`
- `MigrationAdapter` tem `integrity_report()` para validar consistência após migrações
- WriteGate logs rejections em nível `warn` — monitore para detectar premissas falsas recorrentes

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `MemoryError::Database` | SQLite corrompido ou WAL travado | Backup DB, delete `-wal` e `-shm` files, reabra |
| `MemoryError::NotFound` | Node/edge ID inexistente | Verifique ID, use `graph.stats()` para listar IDs válidos |
| Contradições falsas positivas | Embeddings similares mas conteúdo diferente | Ajuste threshold de similaridade no ContradictionDetector |
| WriteGate rejeita writes válidos | Evidence capsule incompleta | Adicione mais evidências ao capsule, verifique disposition |
| Rerank lento (>100ms) | Muitas memórias ou brief muito amplo | Reduza top_k, refine brief, adicione filtros por scope |
| Vector search retorna irrelevantes | Embeddings desatualizados | Re-indexe documentos, verifique modelo de embedding |
| GC remove dados ativos | Decay score calculado incorretamente | Verifique política de decay, ajuste half-life |

## Referências

- **Source**: `crates/zylos-memory-system/src/` (14 módulos + benches)
- **Key files**: `lib.rs:1-42`, `memory_kind.rs`, `context_graph.rs`, `vector_store.rs`, `write_gate.rs`, `rerank.rs`, `contradiction.rs`, `memory_agent.rs`, `session_goal.rs`
- **Benchmarks**: `crates/zylos-memory-system/benches/memory_system.rs`
- **Master Plan**: Linhas 360-500 (Fase 1 detalhada, Sprints 1.1-1.3)
- **Inspirações**: Memanto, Koog, Meldwork, PaperRadar, AgentMemory, MegaMemory