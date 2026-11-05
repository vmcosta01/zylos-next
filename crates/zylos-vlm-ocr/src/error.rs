//! Error types for VLM OCR

use thiserror::Error;

pub type OcrResult<T> = Result<T, OcrError>;

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid image format: {0}")]
    InvalidImage(String),

    #[error("OCR service returned error: {status} — {message}")]
    ServiceError { status: u16, message: String },

    #[error("Response parsing failed: {0}")]
    ParseError(String),

    #[error("Timeout after {0}s")]
    Timeout(u64),

    #[error("Max retries ({0}) exhausted")]
    RetriesExhausted(u32),

    #[error("Elo score {actual:.1} below minimum {minimum:.1}")]
    BelowBenchmark { actual: f64, minimum: f64 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}