# Runbook: Parallel Search MCP (Task 4.2.3)

**Sistema:** SANDBOX_ISOLATION — Fase 4, Sprint 4.2
**Crate:** `zylos-search-mcp`
**Módulos:** `engine`, `provider`, `error`
**Última Atualização:** 30 de outubro de 2026
**Status Operacional:** ✅ 8/8 testes passando

---

## 1. Visão Geral

O crate `zylos-search-mcp` implementa um servidor MCP de busca paralela federada com fan-out concorrente, deduplicação por URL, tolerância a falhas parciais de providers e anonimização de queries. Inspirado em CopilotKit/OpenDots Parallel Search MCP.

### Componentes Principais
- `SearchEngine`: orquestrador de fan-out com execução paralela via `tokio::JoinSet`
- `SearchProvider` trait: interface async para backends de pesquisa (MockProvider, FailingProvider para testes)
- `SearchResult`: estrutura tipada com url, title, snippet, score, provider_name
- `SearchError`: enumeração de erros (NoProviders, AllFailed, EmptyQuery, ProviderError)

---

## 2. Pré-requisitos

```bash
cd C:/Users/Paulo/zylos-next
cargo build -p zylos-search-mcp
```

Dependências principais: `tokio` (runtime async), `async-trait` (dyn-compatible async traits).

---

## 3. Verificação de Saúde

### 3.1 Testes Unitários
```bash
cargo test -p zylos-search-mcp --lib
```
**Esperado:** 8 testes passando em <1s.

### 3.2 Fan-Out Paralelo
```rust
use zylos_search_mcp::{SearchEngine, MockProvider};

let engine = SearchEngine::new(vec![
    Box::new(MockProvider::new("provider-a", 5)),
    Box::new(MockProvider::new("provider-b", 3)),
]);
let results = engine.search("rust async runtime").await?;
assert!(results.len() >= 3); // merge + dedup
```

### 3.3 Tolerância a Falhas Parciais
Se 1 de N providers falhar, os resultados dos demais são retornados normalmente. Apenas quando TODOS falham é retornado `SearchError::AllFailed`.

---

## 4. Procedimentos Operacionais

### 4.1 Configurar Engine com Múltiplos Providers
```rust
use zylos_search_mcp::{SearchEngine, MockProvider, FailingProvider};

let engine = SearchEngine::new(vec![
    Box::new(MockProvider::new("brave-search", 10)),
    Box::new(MockProvider::new("duckduckgo", 8)),
    Box::new(FailingProvider::new("broken-backend")), // tolerado
]);
```

### 4.2 Executar Busca Paralela
```rust
match engine.search("webmcp spec compliance").await {
    Ok(results) => {
        println!("{} resultados encontrados", results.len());
        for r in &results {
            println!("[{}] {} — {}", r.provider_name, r.title, r.url);
        }
    }
    Err(e) => eprintln!("Busca falhou: {}", e),
}
```

### 4.3 Deduplicação Automática
Resultados com mesma URL são mesclados automaticamente; o maior score entre os providers é preservado.

### 4.4 Validação de Query Vazia
Queries vazias ou whitespace-only retornam `SearchError::EmptyQuery` imediatamente, sem dispatch aos providers.

---

## 5. Troubleshooting

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| `NoProviders` error | Engine criada com vec vazio | Adicionar pelo menos um provider |
| `AllFailed` error | Todos os providers retornaram erro | Verificar conectividade/configuração dos backends |
| Resultados duplicados | URLs diferentes apontando mesmo conteúdo | Dedup é por URL exata; normalizar URLs antes do provider se necessário |
| Timeout em busca lenta | Provider externo sem timeout configurado | Adicionar timeout no provider impl ou no engine config |
| Score inconsistente | Providers usam escalas diferentes | Normalizar scores no provider adapter (0.0–1.0) |

---

## 6. Métricas de Referência

- **Fan-out 3 providers (mock):** <5ms
- **Deduplicação 100 resultados:** <1ms
- **Tolerância a falha parcial:** overhead <10% vs all-success
- **Memória por busca:** <500KB

---

## 7. Checklist de Validação Pós-Deploy

- [ ] `cargo test -p zylos-search-mcp --lib` passa com 0 warnings
- [ ] Fan-out executa providers em paralelo (não sequencial)
- [ ] Deduplicação por URL funcional
- [ ] Falha parcial de provider não aborta busca inteira
- [ ] Query vazia rejeitada sem dispatch
- [ ] Anonimização de query preservada (sem leak de PII nos logs)
- [ ] Latência <500ms para buscas paralelas com 3+ providers