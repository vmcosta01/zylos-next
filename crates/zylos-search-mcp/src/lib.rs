//! Parallel search MCP server with fan-out, result merging, and deduplication.
//!
//! Provides a pluggable search engine that queries multiple providers concurrently,
//! merges results by score, and deduplicates by URL.

pub mod engine;
pub mod error;
pub mod provider;

pub use engine::{SearchEngine, SearchEngineConfig, SearchResponse};
pub use error::{Result, SearchError};
pub use provider::{FailingProvider, MockProvider, SearchMeta, SearchProvider, SearchResult};
