//! Types for VLM OCR structured output

use serde::{Deserialize, Serialize};

/// Type of element extracted by OCR
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ElementType {
    Text,
    Table,
    Formula,
    Diagram,
    Chart,
    Image,
    Header,
    ListItem,
}

/// Bounding box in pixel coordinates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// A single extracted element from OCR
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrElement {
    pub element_type: ElementType,
    pub content: String,
    pub bbox: Option<BBox>,
    pub confidence: f64,
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Request to OCR an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    /// Base64-encoded image data
    pub image_base64: String,
    /// Optional MIME type hint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Element types to extract (empty = all)
    #[serde(default)]
    pub target_types: Vec<ElementType>,
    /// Language hint for OCR
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Response from OCR processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponse {
    pub elements: Vec<OcrElement>,
    pub full_text: String,
    pub processing_time_ms: u64,
    pub model_version: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_element_roundtrip() {
        let el = OcrElement {
            element_type: ElementType::Formula,
            content: "E = mc^2".to_string(),
            bbox: Some(BBox { x: 10, y: 20, width: 200, height: 50 }),
            confidence: 0.95,
            metadata: Default::default(),
        };
        let json = serde_json::to_string(&el).unwrap();
        let parsed: OcrElement = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.element_type, ElementType::Formula);
        assert_eq!(parsed.confidence, 0.95);
    }

    #[test]
    fn test_ocr_response_empty() {
        let resp = OcrResponse {
            elements: vec![],
            full_text: String::new(),
            processing_time_ms: 0,
            model_version: "dots.mocr-3b".to_string(),
            warnings: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("dots.mocr-3b"));
    }
}