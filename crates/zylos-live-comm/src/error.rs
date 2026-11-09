use thiserror::Error;

#[derive(Error, Debug)]
/// Enumeração LiveCommError.
pub enum LiveCommError {
    #[error("EventBus send failed: {0}")]
    // Variante BroadcastSend.
    BroadcastSend(String),
    #[error("SSE stream error: {0}")]
    // Variante SseStream.
    SseStream(String),
    #[error("Serialization error: {0}")]
    // Variante Serialization.
    Serialization(#[from] serde_json::Error),
    #[error("Invalid event: {0}")]
    // Variante InvalidEvent.
    InvalidEvent(String),
    #[error("Persistence error: {0}")]
    // Variante Persistence.
    Persistence(String),
}
