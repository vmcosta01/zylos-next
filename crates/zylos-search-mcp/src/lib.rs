//! Parallel search MCP server with fan-out, result merging, and deduplication.
//!
//! Provides a pluggable search engine that queries multiple providers concurrently,
//! merges results by score, and deduplicates by URL.

pub mod error;
pub mod provider;
pub mod engine;

pub use error::{SearchError, Result};
pub use provider::{SearchResult, SearchMeta, SearchProvider, MockProvider, FailingProvider};
pub use engine::{SearchEngine, SearchEngineConfig, SearchResponse};