use crate::mcp_server::McpErrorBody;
use thiserror::Error;

/// Unified error type for all mobile MCP server operations.
///
/// Each variant maps to a specific JSON-RPC error code via the
/// [`From<MobileError> for McpErrorBody`](#impl-From<MobileError>-for-McpErrorBody)
/// implementation, ensuring consistent error responses across the protocol.
#[derive(Error, Debug)]
pub enum MobileError {
    /// Internal MCP protocol violation or unexpected server state (code -32603).
    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    /// Network or transport-level connection failure (code -32603).
    #[error("Connection error: {0}")]
    Connection(String),

    /// JSON serialization or deserialization failure (code -32603).
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Error propagated from the underlying LiveComm event bus (code -32603).
    #[error("LiveComm error: {0}")]
    LiveComm(#[from] zylos_live_comm::LiveCommError),

    /// Operation timed out before completion (code -32005).
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Authentication or authorization failure (code -32001).
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Client exceeded the configured rate limit (code -32002).
    ///
    /// The `retry_after_secs` field indicates how many seconds the client
    /// should wait before retrying.
    #[error("Rate limit exceeded for client {client_id}: retry after {retry_after_secs}s")]
    RateLimit {
        /// Identifier of the rate-limited client.
        client_id: String,
        /// Seconds until the client's token bucket refills.
        retry_after_secs: u64,
    },

    /// Attachment storage or retrieval failure (code -32003).
    #[error("Attachment error: {0}")]
    Attachment(String),

    /// Filesystem I/O error during attachment or config operations (code -32603).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Requested resource URI does not exist (code -32004).
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    /// Malformed or semantically invalid JSON-RPC request (code -32600).
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

/// Maps each [`MobileError`] variant to a JSON-RPC error code and message.
///
/// | Variant              | Code   |
/// |----------------------|--------|
/// | `InvalidRequest`     | -32600 |
/// | `Auth`               | -32001 |
/// | `RateLimit`          | -32002 |
/// | `Attachment`         | -32003 |
/// | `ResourceNotFound`   | -32004 |
/// | `Timeout`            | -32005 |
/// | All others           | -32603 |
impl From<MobileError> for crate::mcp_server::McpErrorBody {
    fn from(err: MobileError) -> Self {
        let (code, message) = match &err {
            MobileError::McpProtocol(_) => (-32603, err.to_string()),
            MobileError::Auth(_) => (-32001, err.to_string()),
            MobileError::RateLimit { .. } => (-32002, err.to_string()),
            MobileError::Attachment(_) => (-32003, err.to_string()),
            MobileError::ResourceNotFound(_) => (-32004, err.to_string()),
            MobileError::InvalidRequest(_) => (-32600, err.to_string()),
            MobileError::Timeout(_) => (-32005, err.to_string()),
            _ => (-32603, err.to_string()),
        };
        McpErrorBody {
            code,
            message,
            data: None,
        }
    }
}
