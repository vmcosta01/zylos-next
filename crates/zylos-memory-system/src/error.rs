//! Error types for zylos-memory-system

use thiserror::Error;

/// Errors that can occur in the MEMORY_SYSTEM
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Node not found: {0}")]
    NodeNotFound(String),

    #[error("Edge not found: {0} -> {1}")]
    EdgeNotFound(String, String),

    #[error("Duplicate node: {0}")]
    DuplicateNode(String),

    #[error("Invalid scope: {0}")]
    InvalidScope(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("Lock poisoned")]
    LockPoisoned,

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;