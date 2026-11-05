# Runbook & Playbook: Fase 2 — MEMORY_SYSTEM (ContextGraph + VectorStore + Reranker)

**Sistema:** MEMORY_SYSTEM — Fase 2
**Crate:** `zylos-memory-system`
**Módulos:** `context_graph`, `vector_store`, `brief_aware_reranker`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ 28/28 testes passando

---

## 🎯 Visão Geral (Todos os Níveis)

A Fase 2 implementa o sistema de memória persistente do Zylos-Next: grafo de contexto com SQLite WAL e petgraph para relações semânticas, vector store com similaridade cosseno e filtros por metadata, e reranker consciente de recência/contexto/semântica/palavras-chave. Este crate permite que agentes recordem fatos, relacionamentos e embeddings com recuperação precisa e auditável.

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Validar que o ContextGraph escala para 100k+ nós sem degradação de query
- Garantir que o VectorStore suporta múltiplos backends (InMemory agora, Qdrant/Chroma futuro) via trait
- Auditar que o BriefAwareReranker não introduz viés de recência excessivo
- Definir contratos de persistência (WAL checkpoint, vacuum, backup)

### Procedimentos

#### S2.1: Benchmark de Escala do ContextGraph
```bash
cd C:/Users/Paulo/zylos-next
cargo bench -p zylos-memory-system -- context_graph_query_10k_nodes
```
**Critério Senior:** Query de vizinhança em grafo de 10k nós <5ms. Inserção batch de 1k nós <100ms. Se exceder, investigar índices SQLite ou estrutura petgraph.

#### S2.2: Auditoria de Isolamento de Backends no VectorStore
Verificar que `VectorStore` é trait object-safe e nenhuma implementação vaza detalhes internos:
```rust
// Em vector_store.rs — trait define apenas interface async
// InMemoryStore implementa sem expor HashMap interno
// Futuros backends (Qdrant, Chroma) devem implementar mesma trait
```
**Por que importa:** Troca de backend não deve exigir refatoração em consumidores (conceito #9 do manifesto).

#### S2.3: Validação de Balanceamento no Reranker
```rust
use zylos_memory_system::BriefAwareReranker;

let reranker = BriefAwareReranker::new(RerankConfig {
    recency_weight: 0.3,
    semantic_weight: 0.4,
    keyword_weight: 0.2,
    contextual_weight: 0.1,
});
// Testar com documentos variados: recentes mas irrelevantes vs antigos mas relevantes
// Score final deve refletir pesos configurados, não dominância de um fator
```
**Critério Senior:** Nenhum fator domina >60% do score final em casos de borda. Pesos são ajustáveis via config.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| SQLite WAL mode | Concorrência leitor/escritor sem lock; compatível com petgraph serialização | PostgreSQL (dependência externa pesada para embedded) |
| petgraph DiGraph | Grafo direcionado nativo Rust; algoritmos BFS/DFS/topo sort inclusos | Custom graph struct (reinventar roda, sem otimizações) |
| Cosine similarity | Padrão ouro para embeddings; normalizado por magnitude | Euclidean (sensível a magnitude, pior para texto) |
| BriefAwareReranker multi-fator | Recência pura perde contexto antigo relevante; semântica pura ignora tempo | BM25 only (sem embeddings), pure vector (sem recência) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Executar testes unitários e validar cobertura de módulos
- Configurar cenários de integração grafo + vector + reranker
- Documentar padrões de query e inserção
- Implementar novos backends VectorStore quando necessário

### Procedimentos

#### P2.1: Rodar Suite Completa de Testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-memory-system --lib
```
**Esperado:** 28 testes passando em <3s.

#### P2.2: Cenário de Integração — Memória de Longo Prazo
```rust
use zylos_memory_system::{ContextGraph, VectorStore, InMemoryStore, BriefAwareReranker};

// 1. Criar grafo e adicionar fatos
let mut graph = ContextGraph::new(":memory:")?;
graph.add_node("user_prefers_rust", json!({"type": "preference"}))?;
graph.add_node("project_zylos", json!({"type": "project"}))?;
graph.add_edge("user_prefers_rust", "project_zylos", "applies_to")?;

// 2. Indexar embeddings
let store = InMemoryStore::new();
store.upsert("doc-1", vec![0.1, 0.2, 0.3], json!({"created_at": "2026-10-30"})).await?;

// 3. Buscar e rerankear
let candidates = store.search(&[0.1, 0.2, 0.3], 10, None).await?;
let reranker = BriefAwareReranker::default();
let ranked = reranker.rerank(candidates, &graph, "user_prefers_rust").await?;

println!("Top resultado: {} (score: {:.3})", ranked[0].id, ranked[0].score);
```

#### P2.3: Verificar Persistência WAL
```rust
let graph = ContextGraph::new("memory.db")?;
graph.add_node("test", json!({}))?;
drop(graph); // fecha conexão

// Reabrir e verificar que nó persistiu
let graph2 = ContextGraph::open("memory.db")?;
assert!(graph2.get_node("test").is_some());
```

### Checklist de Validação
- [ ] `cargo test -p zylos-memory-system --lib` passa 28/28
- [ ] ContextGraph query <5ms para 10k nós
- [ ] VectorStore search retorna resultados ordenados por similaridade
- [ ] Reranker respeita pesos configurados
- [ ] Persistência SQLite sobrevive a restart
- [ ] Clippy zero warnings

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Rodar testes e reportar resultados
- Seguir exemplos de uso sem modificar
- Reportar erros com output completo

### Procedimentos Passo-a-Passo

#### J2.1: Compilar o Crate
```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-memory-system
```
✅ `Finished` → prossiga
❌ Erro → copie output completo e reporte ao Pleno

#### J2.2: Rodar Testes
```bash
cargo test -p zylos-memory-system --lib 2>&1 | tee test-fase2.log
```
✅ `test result: ok. 28 passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro

#### J2.3: Verificar Exemplo Básico
Leia `crates/zylos-memory-system/examples/memory_demo.rs` (se existir) e execute:
```bash
cargo run -p zylos-memory-system --example memory_demo
```
Se não existir exemplo, reporte ao Pleno como gap de documentação.

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **ContextGraph** | Estrutura de dados que armazena fatos como nós e relações como arestas em grafo direcionado |
| **VectorStore** | Banco de vetores que permite busca por similaridade semântica (embeddings) |
| **Embedding** | Vetor numérico que representa significado de texto; textos similares têm vetores próximos |
| **Cosine Similarity** | Medida de similaridade entre vetores baseada no ângulo; 1.0 = idêntico, 0.0 = ortogonal |
| **WAL** | Write-Ahead Logging — modo SQLite que escreve mudanças em log antes do banco principal |
| **Reranker** | Componente que reordena resultados de busca combinando múltiplos sinais (recência, semântica, etc.) |
| **petgraph** | Crate Rust para grafos com algoritmos prontos (BFS, DFS, shortest path, topological sort) |

### Quando Escalar
- Teste falha intermitentemente → **Pleno** (possível race condition em WAL)
- Query demora >10ms para 1k nós → **Senior** (performance regression)
- Embeddings retornam NaN ou Inf → **Senior imediatamente** (corrupção de dados)

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` linhas 350-500 (Fase 2)
- **Pré-requisito:** `runbooks/fase1-bot-system.md`
- **Próxima Fase:** `runbooks/fase3-verification-proof.md`
- **Auditorias:** `evals/audits/fase2-audit.md`, `evals/audits/fase2-audit-v2.md`
- **Manifesto:** Conceitos #9 (Trait-First Abstraction), #14 (Provenance-Gated Recall)