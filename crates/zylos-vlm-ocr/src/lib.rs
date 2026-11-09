//! zylos-vlm-ocr — VLM OCR Sidecar for Zylos-Next
//!
//! Integrates with dots.mocr (or compatible VLM OCR endpoints) to provide
//! structured extraction of text, tables, formulas, and diagrams from images.
//! Designed as a sidecar process that agents can invoke via MCP or direct API.

/// HTTP client for VLM OCR endpoints.
pub mod client;
/// Structured types for OCR requests, responses, and elements.
pub mod types;
/// Error types and result alias for OCR operations.
pub mod error;
/// SVG parsing and rendering for extracted diagrams and formulas.
pub mod svg;

pub use client::{VlmOcrClient, OcrConfig};
pub use types::{OcrRequest, OcrResponse, OcrElement, ElementType};
pub use error::{OcrError, OcrResult};

/// Default benchmark target: dots.mocr 3B Elo 1124.7
pub const BENCHMARK_ELO_TARGET: f64 = 1124.7;

/// Minimum acceptable Elo for production use
pub const MIN_PRODUCTION_ELO: f64 = 1100.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_constants() {
        assert!(BENCHMARK_ELO_TARGET > MIN_PRODUCTION_ELO);
        assert_eq!(BENCHMARK_ELO_TARGET, 1124.7);
        assert_eq!(MIN_PRODUCTION_ELO, 1100.0);
    }

    #[test]
    fn test_element_type_serialization() {
        let el = ElementType::Text;
        let json = serde_json::to_string(&el).unwrap();
        assert_eq!(json, "\"text\"");

        let formula = ElementType::Formula;
        let json = serde_json::to_string(&formula).unwrap();
        assert_eq!(json, "\"formula\"");
    }

    #[tokio::test]
    async fn test_client_creation() {
        let config = OcrConfig {
            endpoint: "http://localhost:8080".to_string(),
            api_key: None,
            timeout_secs: 30,
            max_retries: 3,
        };
        let client = VlmOcrClient::new(config);
        assert_eq!(client.config().endpoint, "http://localhost:8080");
        assert_eq!(client.config().max_retries, 3);
    }
}