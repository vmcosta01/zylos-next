//! Error types for zylos-bot-system

use thiserror::Error;

/// Errors that can occur in the BOT_SYSTEM
#[derive(Debug, Error)]
pub enum BotError {
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Agent already exists: {0}")]
    AgentAlreadyExists(String),

    #[error("Invalid agent state transition: {from:?} -> {to:?}")]
    InvalidStateTransition { from: String, to: String },

    #[error("Spawn failed: {0}")]
    SpawnFailed(String),

    #[error("Shutdown timeout after {0}ms")]
    ShutdownTimeout(u64),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, BotError>;
