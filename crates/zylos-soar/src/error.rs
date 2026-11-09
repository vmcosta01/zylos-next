//! Error types for the SOAR engine.

use thiserror::Error;

/// Top-level error type for zylos-soar operations.
#[derive(Debug, Error)]
/// Enumeração SoarError.
pub enum SoarError {
    #[error("event validation failed: {0}")]
    // Variante EventValidation.
    EventValidation(String),

    #[error("triage classification failed: {0}")]
    // Variante TriageFailed.
    TriageFailed(String),

    #[error("playbook parse error: {0}")]
    // Variante PlaybookParse.
    PlaybookParse(String),

    #[error("runbook execution failed: {0}")]
        // Variante RunbookExecution.
        RunbookExecution(String),

        #[error("configuration error: {0}")]
        // Variante Config.
        Config(String),

        #[error("execution error: {0}")]
        // Variante Execution.
        Execution(String),

    #[error("audit trail error: {0}")]
    // Variante Audit.
    Audit(#[from] crate::audit::AuditError),

    #[error("serialization error: {0}")]
    // Variante Serialization.
    Serialization(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    // Variante YamlParse.
    YamlParse(#[from] serde_yaml::Error),

    #[error("I/O error: {0}")]
    // Variante Io.
    Io(#[from] std::io::Error),

    #[error("{0}")]
    // Variante Other.
    Other(String),
}

/// Convenience result alias.
pub type Result<T> = std::result::Result<T, SoarError>;