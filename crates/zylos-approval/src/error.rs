//! Erros tipados para o crate zylos-approval

use thiserror::Error;

/// Erros possíveis nas operações de approval
#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Approval request not found: {0}")]
    NotFound(String),

    #[error("Approval request already decided: {0}")]
    AlreadyDecided(String),

    #[error("Approval request expired: {0}")]
    Expired(String),

    #[error("Invalid decision: {0}")]
    InvalidDecision(String),

    #[error("Invalid action category: {0}")]
    InvalidCategory(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Rule already exists for bot={bot_id} tool={tool_name}")]
    DuplicateRule { bot_id: String, tool_name: String },
}

impl ApprovalError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    pub fn is_expired(&self) -> bool {
        matches!(self, Self::Expired(_))
    }
}