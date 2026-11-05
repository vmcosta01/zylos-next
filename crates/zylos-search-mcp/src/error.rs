//! Error types for the parallel search MCP server.

use thiserror::Error;

/// Errors that can occur during parallel search operations.
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("Provider '{0}' failed: {1}")]
    ProviderFailed(String, String),

    #[error("All providers failed for query: {0}")]
    AllProvidersFailed(String),

    #[error("Timeout after {0}s waiting for providers")]
    Timeout(u64),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, SearchError>;