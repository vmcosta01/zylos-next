//! Error types for browser sandbox operations.

use thiserror::Error;

/// Errors that can occur during browser sandbox operations.
#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("Browser launch failed: {0}")]
    LaunchFailed(String),

    #[error("Fingerprint generation failed: {0}")]
    FingerprintError(String),

    #[error("Navigation failed: {0}")]
    NavigationFailed(String),

    #[error("WebMCP compliance violation: {0}")]
    WebMcpViolation(String),

    #[error("Context isolation breach: {0}")]
    IsolationBreach(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, BrowserError>;