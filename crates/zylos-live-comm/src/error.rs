use thiserror::Error;

#[derive(Error, Debug)]
pub enum LiveCommError {
    #[error("EventBus send failed: {0}")]
    BroadcastSend(String),
    #[error("SSE stream error: {0}")]
    SseStream(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid event: {0}")]
    InvalidEvent(String),
    #[error("Persistence error: {0}")]
    Persistence(String),
}
