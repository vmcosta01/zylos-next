//! HTTP client for VLM OCR endpoints (dots.mocr compatible)

use crate::error::{OcrError, OcrResult};
use crate::types::{OcrRequest, OcrResponse};
use reqwest::Client;
use std::time::Duration;

/// Configuration for the OCR client
#[derive(Debug, Clone)]
pub struct OcrConfig {
    pub endpoint: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:8080".to_string(),
            api_key: None,
            timeout_secs: 30,
            max_retries: 3,
        }
    }
}

/// Client for interacting with VLM OCR services
#[derive(Debug, Clone)]
pub struct VlmOcrClient {
    config: OcrConfig,
    http: Client,
}

impl VlmOcrClient {
    /// Create a new VLM OCR client with the given configuration.
    pub fn new(config: OcrConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to build HTTP client");
        Self { config, http }
    }

    /// Returns a reference to the current client configuration.
    pub fn config(&self) -> &OcrConfig {
        &self.config
    }

    /// Send an OCR request with retry logic
    pub async fn ocr(&self, request: OcrRequest) -> OcrResult<OcrResponse> {
        let mut _last_err = None;
        for attempt in 0..=self.config.max_retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
            }
            match self.do_request(&request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    tracing::warn!("OCR attempt {} failed: {}", attempt + 1, e);
                    _last_err = Some(e);
                }
            }
        }
        Err(OcrError::RetriesExhausted(self.config.max_retries))
    }

    async fn do_request(&self, request: &OcrRequest) -> OcrResult<OcrResponse> {
        let mut req = self.http.post(format!("{}/ocr", self.config.endpoint));
        if let Some(ref key) = self.config.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }
        let resp = req.json(request).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(OcrError::ServiceError {
                status: status.as_u16(),
                message: body,
            });
        }
        let ocr_resp: OcrResponse = resp.json().await.map_err(|e| {
            OcrError::ParseError(format!("Failed to parse OCR response: {}", e))
        })?;
        Ok(ocr_resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OcrConfig::default();
        assert_eq!(config.timeout_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert!(config.api_key.is_none());
    }
}