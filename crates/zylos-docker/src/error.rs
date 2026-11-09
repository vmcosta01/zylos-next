//! Error types for Docker sandbox operations.

use thiserror::Error;

/// Errors that can occur during Docker sandbox operations.
#[derive(Debug, Error)]
pub enum DockerError {
    /// The Docker daemon could not be reached or is not running.
    #[error("Docker daemon unavailable: {0}")]
    DaemonUnavailable(String),

    /// A container could not be created (invalid config, image pull failure, etc.).
    #[error("Container creation failed: {0}")]
    CreationFailed(String),

    /// The referenced container does not exist.
    #[error("Container not found: {0}")]
    NotFound(String),

    /// An operation was blocked by the configured [`ApprovalWorkflow`](crate::ApprovalWorkflow).
    #[error("Operation denied by approval workflow: {0}")]
    ApprovalDenied(String),

    /// A filesystem access attempt violated sandbox isolation rules.
    #[error("Filesystem isolation violation: {0}")]
    IsolationViolation(String),

    /// A command executed inside the container failed.
    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),

    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An I/O error occurred during sandbox operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// An error from the underlying Bollard Docker client.
    #[error("Bollard error: {0}")]
    Bollard(#[from] bollard::errors::Error),
}

/// Convenience result type for Docker sandbox operations.
pub type Result<T> = std::result::Result<T, DockerError>;