//! Error types for zylos-integrations.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, IntegrationError>;

#[derive(Error, Debug)]
pub enum IntegrationError {
    #[error("Unknown provider: {0}")]
    UnknownProvider(String),

    #[error("No tokens stored for user {user_id} on provider {provider}")]
    NoTokensStored { user_id: String, provider: String },

    #[error("OAuth flow error: {0}")]
    OAuth(String),

    #[error("Token refresh failed: {0}")]
    TokenRefresh(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Provider execution failed: {action} on {provider}: {detail}")]
    ProviderExecution {
        provider: String,
        action: String,
        detail: String,
    },

    #[error("HITL approval required for action {action} on {provider}")]
    ApprovalRequired { provider: String, action: String },

    #[error("Rate limit exceeded for {provider}, retry after {retry_after_secs}s")]
    RateLimited {
        provider: String,
        retry_after_secs: u64,
    },
}