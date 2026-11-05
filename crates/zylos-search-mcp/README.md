# zylos-search-mcp

Parallel search MCP server with fan-out, result merging, and deduplication.

## Architecture

### Modules

- **`error`** — `SearchError` enum and `Result` type alias for search operations
- **`provider`** — `SearchResult`, `SearchMeta`, `SearchProvider` trait, plus `MockProvider` and `FailingProvider` for testing
- **`engine`** — `SearchEngine`, `SearchEngineConfig`, `SearchResponse` for concurrent multi-provider query execution with score-based merging and URL deduplication

### Key Exports

```rust
pub use error::{SearchError, Result};
pub use provider::{SearchResult, SearchMeta, SearchProvider, MockProvider, FailingProvider};
pub use engine::{SearchEngine, SearchEngineConfig, SearchResponse};
```

## Usage

```rust
use zylos_search_mcp::{SearchEngine, SearchEngineConfig, MockProvider};

// Configure and create a search engine
let config = SearchEngineConfig::default();
let engine = SearchEngine::new(config);

// Execute a parallel search across registered providers
let response = engine.search("rust async patterns").await?;
for result in &response.results {
    println!("{}: {}", result.title, result.url);
}
```

## Dependencies

| Crate | Features | Purpose |
|-------|----------|---------|
| `tokio` | full | Async runtime |
| `serde` | derive | Serialization |
| `serde_json` | — | JSON handling |
| `tracing` | — | Structured logging |
| `uuid` | v4, serde | Request/result IDs |
| `chrono` | serde | Timestamps |
| `thiserror` | — | Error types |
| `sha2` | — | Deduplication hashing |
| `hex` | — | Hash encoding |
| `futures` | — | Concurrent provider fan-out |
| `async-trait` | — | Async trait support for `SearchProvider` |

## Testing

```bash
cargo test -p zylos-search-mcp
```

Dev dependencies: `tempfile` for temporary directory fixtures. Use `MockProvider` and `FailingProvider` for unit tests without external API calls.

## Related Crates

- None (standalone crate with no internal workspace dependencies)