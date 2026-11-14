//! Error types for VLM OCR

use thiserror::Error;

/// Convenience result type for OCR operations.
pub type OcrResult<T> = Result<T, OcrError>;

/// Errors that can occur during VLM OCR processing.
#[derive(Debug, Error)]
pub enum OcrError {
    /// HTTP transport error from the underlying client.
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The supplied image could not be decoded or is unsupported.
    #[error("Invalid image format: {0}")]
    InvalidImage(String),

    /// The remote OCR endpoint returned a non-success status.
    #[error("OCR service returned error: {status} — {message}")]
    ServiceError { status: u16, message: String },

    /// Failed to deserialize the OCR response payload.
    #[error("Response parsing failed: {0}")]
    ParseError(String),

    /// Request exceeded the configured timeout.
    #[error("Timeout after {0}s")]
    Timeout(u64),

    /// All retry attempts were exhausted without success.
    #[error("Max retries ({0}) exhausted")]
    RetriesExhausted(u32),

    /// Model Elo score is below the minimum production threshold.
    #[error("Elo score {actual:.1} below minimum {minimum:.1}")]
    BelowBenchmark { actual: f64, minimum: f64 },

    /// Filesystem or other I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
