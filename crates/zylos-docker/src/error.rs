//! Error types for Docker sandbox operations.

use thiserror::Error;

/// Errors that can occur during Docker sandbox operations.
#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker daemon unavailable: {0}")]
    DaemonUnavailable(String),

    #[error("Container creation failed: {0}")]
    CreationFailed(String),

    #[error("Container not found: {0}")]
    NotFound(String),

    #[error("Operation denied by approval workflow: {0}")]
    ApprovalDenied(String),

    #[error("Filesystem isolation violation: {0}")]
    IsolationViolation(String),

    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Bollard error: {0}")]
    Bollard(#[from] bollard::errors::Error),
}

pub type Result<T> = std::result::Result<T, DockerError>;