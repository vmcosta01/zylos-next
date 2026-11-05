use thiserror::Error;
use crate::mcp_server::McpErrorBody;

#[derive(Error, Debug)]
pub enum MobileError {
    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("LiveComm error: {0}")]
    LiveComm(#[from] zylos_live_comm::LiveCommError),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limit exceeded for client {client_id}: retry after {retry_after_secs}s")]
    RateLimit {
        client_id: String,
        retry_after_secs: u64,
    },

    #[error("Attachment error: {0}")]
    Attachment(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Resource not found: {0}")]
    ResourceNotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

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
        McpErrorBody { code, message, data: None }
    }
}
