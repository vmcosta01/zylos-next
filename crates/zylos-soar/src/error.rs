//! Error types for the SOAR engine.

use thiserror::Error;

/// Top-level error type for zylos-soar operations.
#[derive(Debug, Error)]
pub enum SoarError {
    #[error("event validation failed: {0}")]
    EventValidation(String),

    #[error("triage classification failed: {0}")]
    TriageFailed(String),

    #[error("playbook parse error: {0}")]
    PlaybookParse(String),

    #[error("runbook execution failed: {0}")]
    RunbookExecution(String),

    #[error("audit trail error: {0}")]
    Audit(#[from] crate::audit::AuditError),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, SoarError>;