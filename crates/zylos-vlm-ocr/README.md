# zylos-vlm-ocr

VLM OCR Sidecar for Zylos-Next — dots.mocr integration with structured output for text, tables, formulas, and diagrams.

## Architecture

### Modules

- **`client`** — `VlmOcrClient`, `OcrConfig` for communicating with dots.mocr or compatible VLM OCR endpoints
- **`types`** — `OcrRequest`, `OcrResponse`, `OcrElement`, `ElementType` for structured OCR input/output
- **`error`** — `OcrError` enum and `OcrResult` type alias
- **`svg`** — SVG rendering utilities for extracted diagram elements

### Constants

```rust
/// Default benchmark target: dots.mocr 3B Elo 1124.7
pub const BENCHMARK_ELO_TARGET: f64 = 1124.7;

/// Minimum acceptable Elo for production use
pub const MIN_PRODUCTION_ELO: f64 = 1100.0;
```

### Key Exports

```rust
pub use client::{VlmOcrClient, OcrConfig};
pub use types::{OcrRequest, OcrResponse, OcrElement, ElementType};
pub use error::{OcrError, OcrResult};
```

## Usage

```rust
use zylos_vlm_ocr::{VlmOcrClient, OcrConfig, ElementType};

// Configure the OCR sidecar client
let config = OcrConfig {
    endpoint: "http://localhost:8080".to_string(),
    api_key: None,
    timeout_secs: 30,
    max_retries: 3,
};
let client = VlmOcrClient::new(config);

// Submit an image for structured extraction
let response = client.extract(image_bytes).await?;
for element in &response.elements {
    match element.element_type {
        ElementType::Text => println!("Text: {}", element.content),
        ElementType::Formula => println!("Formula: {}", element.content),
        ElementType::Table => println!("Table: {} rows", element.rows.len()),
        _ => {}
    }
}
```

## Dependencies

| Crate | Version/Features | Purpose |
|-------|------------------|---------|
| `zylos-core` | workspace | Shared types |
| `tokio` | full | Async runtime |
| `serde` | derive | Serialization |
| `serde_json` | — | JSON handling |
| `thiserror` | — | Error types |
| `tracing` | — | Structured logging |
| `reqwest` | 0.12, json+multipart | HTTP client for OCR API |
| `base64` | 0.22 | Image encoding |
| `image` | 0.25, png+jpeg | Image format support |
| `tempfile` | 3 | Temporary file handling |

## Testing

```bash
cargo test -p zylos-vlm-ocr
```

Dev dependencies: `tokio-test` (async test utilities), `wiremock` (HTTP mocking for OCR endpoint tests).

Tests cover benchmark constant validation, `ElementType` serialization round-trips, and client configuration. Integration tests use `wiremock` to simulate the dots.mocr endpoint without requiring a live service.

## Related Crates

- Depends on: `zylos-core`