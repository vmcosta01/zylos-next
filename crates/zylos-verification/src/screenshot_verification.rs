//! Screenshot Verification — Multi-Layer Integrity Check
//!
//! Provides magic bytes validation, completeness checks, perceptual hashing (dHash),
//! pixel-level diff classification, placeholder text detection, and anti-tampering checks.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Verdict produced by screenshot verification pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenshotVerdict {
    Valid,
    Tampered(String),
    Truncated(String),
    FormatMismatch { expected: ImageFormat, actual: Option<ImageFormat> },
    PlaceholderDetected(String),
}

impl fmt::Display for ScreenshotVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "Valid"),
            Self::Tampered(reason) => write!(f, "Tampered: {}", reason),
            Self::Truncated(reason) => write!(f, "Truncated: {}", reason),
            Self::FormatMismatch { expected, actual } => {
                write!(f, "FormatMismatch: expected {:?}, got {:?}", expected, actual)
            }
            Self::PlaceholderDetected(text) => write!(f, "PlaceholderDetected: {}", text),
        }
    }
}

/// Supported image formats detected via magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Gif,
    Bmp,
    Unknown,
}

/// Classification of pixel-level differences between two screenshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiffClass {
    Cosmetic,
    Layout,
    Content,
    Structural,
}

/// Result of a full screenshot verification run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verdict: ScreenshotVerdict,
    pub format: Option<ImageFormat>,
    pub dimensions: Option<(u32, u32)>,
    pub dhash: Option<u64>,
    pub sha256: String,
    pub diff_class: Option<DiffClass>,
    pub details: Vec<String>,
}

/// Configuration for verification thresholds.
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    /// Maximum hamming distance for dHash to still be considered matching.
    pub dhash_tolerance: u32,
    /// Minimum allowed dimension (width or height).
    pub min_dimension: u32,
    /// Maximum allowed dimension (width or height).
    pub max_dimension: u32,
    /// Placeholder patterns to detect (case-insensitive substrings).
    pub placeholder_patterns: Vec<String>,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            dhash_tolerance: 10,
            min_dimension: 10,
            max_dimension: 7680, // 8K
            placeholder_patterns: vec![
                "lorem ipsum".into(),
                "todo".into(),
                "placeholder".into(),
                "fixme".into(),
                "xxx".into(),
                "[tbd]".into(),
                "sample text".into(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Magic bytes detection
// ---------------------------------------------------------------------------

/// Detect image format from magic bytes in the header.
pub fn detect_format(data: &[u8]) -> ImageFormat {
    if data.len() < 4 {
        return ImageFormat::Unknown;
    }
    // PNG: 89 50 4E 47 0D 0A 1A 0A
    if data.len() >= 8 && data[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return ImageFormat::Png;
    }
    // JPEG: FF D8 FF
    if data[..3] == [0xFF, 0xD8, 0xFF] {
        return ImageFormat::Jpeg;
    }
    // WebP: RIFF .... WEBP
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return ImageFormat::WebP;
    }
    // GIF: GIF87a or GIF89a
    if data.len() >= 6 && &data[..3] == b"GIF" && (data[3] == b'8') && (data[4] == b'7' || data[4] == b'9') && data[5] == b'a' {
        return ImageFormat::Gif;
    }
    // BMP: BM
    if &data[..2] == b"BM" {
        return ImageFormat::Bmp;
    }
    ImageFormat::Unknown
}

// ---------------------------------------------------------------------------
// Completeness verification
// ---------------------------------------------------------------------------

/// Check EOF markers and basic structural integrity.
pub fn check_completeness(data: &[u8], format: ImageFormat) -> Result<(), String> {
    match format {
        ImageFormat::Png => {
            // PNG ends with IEND chunk: 00 00 00 00 49 45 4E 44 AE 42 60 82
            let iend_marker: &[u8] = &[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];
            if data.len() < 12 {
                return Err("PNG file too small to contain IEND".into());
            }
            if !data.windows(8).any(|w| w == iend_marker) {
                return Err("PNG missing IEND chunk — file is truncated".into());
            }
            Ok(())
        }
        ImageFormat::Jpeg => {
            // JPEG ends with FFD9
            if data.len() < 2 {
                return Err("JPEG file too small".into());
            }
            if data[data.len() - 2] != 0xFF || data[data.len() - 1] != 0xD9 {
                return Err("JPEG missing EOI marker (FFD9) — file is truncated".into());
            }
            Ok(())
        }
        ImageFormat::WebP => {
            // RIFF header declares size at bytes 4..8 (little-endian)
            if data.len() < 12 {
                return Err("WebP file too small".into());
            }
            let declared_size = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
            // Total file should be declared_size + 8 (RIFF header)
            if data.len() < declared_size + 8 {
                return Err(format!(
                    "WebP truncated: declared {} bytes but file has {}",
                    declared_size + 8,
                    data.len()
                ));
            }
            Ok(())
        }
        ImageFormat::Gif => {
            // GIF trailer byte 0x3B
            if data.is_empty() || *data.last().unwrap() != 0x3B {
                return Err("GIF missing trailer byte (0x3B) — file is truncated".into());
            }
            Ok(())
        }
        ImageFormat::Bmp => {
            // BMP header bytes 2..6 declare file size (little-endian)
            if data.len() < 6 {
                return Err("BMP file too small".into());
            }
            let declared_size = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
            if data.len() < declared_size {
                return Err(format!(
                    "BMP truncated: declared {} bytes but file has {}",
                    declared_size,
                    data.len()
                ));
            }
            Ok(())
        }
        ImageFormat::Unknown => Err("Cannot verify completeness of unknown format".into()),
    }
}

/// Extract dimensions from raw image bytes (best-effort, no external image lib).
pub fn extract_dimensions(data: &[u8], format: ImageFormat) -> Option<(u32, u32)> {
    match format {
        ImageFormat::Png => {
            // IHDR chunk starts at offset 16: width(4 BE) height(4 BE)
            if data.len() >= 24 {
                let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
                let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
                Some((w, h))
            } else {
                None
            }
        }
        ImageFormat::Jpeg => {
            // Scan for SOF0/SOF2 marker (FFC0 or FFC2)
            let mut i = 2;
            while i + 9 < data.len() {
                if data[i] == 0xFF {
                    let marker = data[i + 1];
                    if marker == 0xC0 || marker == 0xC2 {
                        let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                        let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                        return Some((w, h));
                    }
                    // Skip segment
                    if i + 3 < data.len() {
                        let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                        i += 2 + seg_len;
                    } else {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            None
        }
        ImageFormat::Bmp => {
            // BITMAPINFOHEADER at offset 14: width(i32 LE) height(i32 LE)
            if data.len() >= 26 {
                let w = i32::from_le_bytes([data[18], data[19], data[20], data[21]]).unsigned_abs();
                let h = i32::from_le_bytes([data[22], data[23], data[24], data[25]]).unsigned_abs();
                Some((w, h))
            } else {
                None
            }
        }
        // WebP/GIF dimension extraction without full decode is complex; skip for now
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Perceptual hashing — simplified dHash (8×8 grayscale + horizontal gradient)
// ---------------------------------------------------------------------------

/// Compute a simplified dHash from raw decoded RGBA/RGB pixel data.
///
/// `pixels` must be row-major grayscale (1 byte per pixel) of at least 9×8 = 72 bytes.
/// For production use, callers should resize their image to 9×8 grayscale before calling.
/// This function performs the horizontal gradient threshold step only.
pub fn compute_dhash_from_gray(pixels: &[u8], width: usize, height: usize) -> u64 {
    debug_assert!(width >= 9 && height >= 8);
    let mut hash: u64 = 0;
    for row in 0..8 {
        for col in 0..8 {
            let idx = row * width + col;
            let left = pixels[idx] as i16;
            let right = pixels[idx + 1] as i16;
            if left > right {
                hash |= 1 << (row * 8 + col);
            }
        }
    }
    hash
}

/// Convenience: compute dHash from raw RGBA pixel data (no resize — assumes 9×8 input).
/// Converts RGBA → grayscale inline, then applies horizontal gradient.
pub fn compute_dhash_rgba_9x8(rgba: &[u8]) -> u64 {
    // Expect 9*8*4 = 288 bytes minimum
    let mut gray = Vec::with_capacity(72);
    for chunk in rgba.chunks(4) {
        if chunk.len() >= 3 {
            // ITU-R BT.601 luminance
            let g = ((chunk[0] as u32 * 299 + chunk[1] as u32 * 587 + chunk[2] as u32 * 114) / 1000) as u8;
            gray.push(g);
        }
    }
    if gray.len() >= 72 {
        compute_dhash_from_gray(&gray, 9, 8)
    } else {
        0
    }
}

/// Hamming distance between two 64-bit hashes.
pub fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

// ---------------------------------------------------------------------------
// Pixel-level diff classification
// ---------------------------------------------------------------------------

/// Classify the difference between two grayscale images of equal dimensions.
///
/// Returns a `DiffClass` based on the percentage and spatial distribution of changed pixels.
pub fn classify_diff(gray_a: &[u8], gray_b: &[u8], width: u32, height: u32, threshold: u8) -> DiffClass {
    let total = (width * height) as usize;
    if total == 0 || gray_a.len() < total || gray_b.len() < total {
        return DiffClass::Structural;
    }

    let mut changed = 0usize;
    let mut top_changed = 0usize;
    let mut _bottom_changed = 0usize;
    let half = height as usize / 2;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            let diff = (gray_a[idx] as i16 - gray_b[idx] as i16).unsigned_abs();
            if diff > threshold as u16 {
                changed += 1;
                if y < half {
                    top_changed += 1;
                } else {
                    _bottom_changed += 1;
                }
            }
        }
    }

    let pct = changed as f64 / total as f64;

    if pct < 0.01 {
        DiffClass::Cosmetic
    } else if pct < 0.05 {
        // Check if changes are concentrated in one region → Layout
        let top_ratio = if changed > 0 { top_changed as f64 / changed as f64 } else { 0.5 };
        if top_ratio > 0.8 || top_ratio < 0.2 {
            DiffClass::Layout
        } else {
            DiffClass::Content
        }
    } else if pct < 0.20 {
        DiffClass::Content
    } else {
        DiffClass::Structural
    }
}

// ---------------------------------------------------------------------------
// Placeholder text detection
// ---------------------------------------------------------------------------

/// Check extracted text for common placeholder patterns.
pub fn detect_placeholder(text: &str, config: &VerificationConfig) -> Option<String> {
    let lower = text.to_lowercase();
    for pattern in &config.placeholder_patterns {
        if lower.contains(&pattern.to_lowercase()) {
            return Some(pattern.clone());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Anti-tampering checks
// ---------------------------------------------------------------------------

/// Detect signs of EXIF manipulation or re-save artifacts in JPEG data.
pub fn check_tampering(data: &[u8], format: ImageFormat) -> Vec<String> {
    let mut warnings = Vec::new();

    if format == ImageFormat::Jpeg {
        // Look for multiple APP1 (EXIF) segments — unusual
        let mut app1_count = 0;
        let mut i = 2;
        while i + 3 < data.len() {
            if data[i] == 0xFF {
                let marker = data[i + 1];
                if marker == 0xE1 {
                    app1_count += 1;
                }
                if marker == 0xDA {
                    // Start of scan — stop scanning headers
                    break;
                }
                if i + 3 < data.len() {
                    let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                    i += 2 + seg_len;
                } else {
                    break;
                }
            } else {
                i += 1;
            }
        }
        if app1_count > 1 {
            warnings.push(format!("Multiple EXIF (APP1) segments detected ({})", app1_count));
        }

        // Check for software tag indicating re-save
        let needle = b"Software";
        if data.windows(needle.len()).any(|w| w == needle) {
            warnings.push("Software tag present — possible re-save artifact".into());
        }
    }

    // Generic: check if SHA-256 embedded in comment differs (heuristic)
    // This is a simplified check; full provenance requires signed metadata.

    warnings
}

// ---------------------------------------------------------------------------
// Main verification entry point
// ---------------------------------------------------------------------------

/// Run the full screenshot verification pipeline on raw image bytes.
pub fn verify_screenshot(data: &[u8], config: &VerificationConfig) -> VerificationResult {
    let sha = {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hex::encode(hasher.finalize())
    };

    let format = detect_format(data);
    let mut details = Vec::new();

    // 1. Format check
    if format == ImageFormat::Unknown {
        return VerificationResult {
            verdict: ScreenshotVerdict::FormatMismatch {
                expected: ImageFormat::Unknown,
                actual: None,
            },
            format: None,
            dimensions: None,
            dhash: None,
            sha256: sha,
            diff_class: None,
            details: vec!["Unrecognized image format".into()],
        };
    }

    // 2. Completeness
    if let Err(msg) = check_completeness(data, format) {
        return VerificationResult {
            verdict: ScreenshotVerdict::Truncated(msg.clone()),
            format: Some(format),
            dimensions: None,
            dhash: None,
            sha256: sha,
            diff_class: None,
            details: vec![msg],
        };
    }

    // 3. Dimensions
    let dims = extract_dimensions(data, format);
    if let Some((w, h)) = dims {
        if w < config.min_dimension || h < config.min_dimension {
            details.push(format!("Dimensions {}x{} below minimum {}", w, h, config.min_dimension));
            return VerificationResult {
                verdict: ScreenshotVerdict::Truncated(format!("Image too small: {}x{}", w, h)),
                format: Some(format),
                dimensions: Some((w, h)),
                dhash: None,
                sha256: sha,
                diff_class: None,
                details,
            };
        }
        if w > config.max_dimension || h > config.max_dimension {
            details.push(format!("Dimensions {}x{} exceed maximum {}", w, h, config.max_dimension));
            return VerificationResult {
                verdict: ScreenshotVerdict::Tampered(format!("Image exceeds max resolution: {}x{}", w, h)),
                format: Some(format),
                dimensions: Some((w, h)),
                dhash: None,
                sha256: sha,
                diff_class: None,
                details,
            };
        }
    }

    // 4. Anti-tampering
    let tamper_warnings = check_tampering(data, format);
    if !tamper_warnings.is_empty() {
        details.extend(tamper_warnings.iter().cloned());
        return VerificationResult {
            verdict: ScreenshotVerdict::Tampered(tamper_warnings.join("; ")),
            format: Some(format),
            dimensions: dims,
            dhash: None,
            sha256: sha,
            diff_class: None,
            details,
        };
    }

    // 5. dHash (only meaningful when caller provides pre-decoded pixels;
    //    we store None here since we don't decode in this module)
    let dhash_val: Option<u64> = None;

    VerificationResult {
        verdict: ScreenshotVerdict::Valid,
        format: Some(format),
        dimensions: dims,
        dhash: dhash_val,
        sha256: sha,
        diff_class: None,
        details,
    }
}

/// Compare two dHash values against configured tolerance.
pub fn compare_hashes(a: u64, b: u64, config: &VerificationConfig) -> bool {
    hamming_distance(a, b) <= config.dhash_tolerance
}

// ---------------------------------------------------------------------------
// OCR-assisted verification (optional, behind feature flag)
// ---------------------------------------------------------------------------

#[cfg(feature = "ocr")]
pub mod ocr_assisted {
    //! OCR-assisted screenshot verification using VlmOcrClient.

    use super::*;
    use zylos_vlm_ocr::client::{OcrConfig, VlmOcrClient};

    /// Verify screenshot text content via OCR.
    /// Returns `Some(placeholder)` if placeholder text is detected.
    pub async fn verify_with_ocr(
        client: &VlmOcrClient,
        image_base64: &str,
        expected_text: Option<&str>,
        config: &VerificationConfig,
    ) -> Result<Option<String>, String> {
        use zylos_vlm_ocr::types::OcrRequest;

        let request = OcrRequest {
            image_base64: image_base64.to_string(),
            mime_type: Some("image/png".to_string()),
            language: Some("en".to_string()),
            target_types: vec![],
        };

        let response = client.ocr(request).await.map_err(|e| format!("OCR failed: {}", e))?;

        let extracted = &response.full_text;

        // Check for placeholder
        if let Some(ph) = detect_placeholder(extracted, config) {
            return Ok(Some(ph));
        }

        // Compare against expected text if provided
        if let Some(expected) = expected_text {
            if !extracted.contains(expected) {
                return Err(format!(
                    "Expected text not found in OCR output. Expected substring: '{}'",
                    expected
                ));
            }
        }

        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Magic bytes tests --

    #[test]
    fn test_detect_png() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
        assert_eq!(detect_format(&png_header), ImageFormat::Png);
    }

    #[test]
    fn test_detect_jpeg() {
        let jpeg_header = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
        assert_eq!(detect_format(&jpeg_header), ImageFormat::Jpeg);
    }

    #[test]
    fn test_detect_webp() {
        let mut webp = vec![0u8; 16];
        webp[0..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        assert_eq!(detect_format(&webp), ImageFormat::WebP);
    }

    #[test]
    fn test_detect_gif89a() {
        let gif = b"GIF89a\x00\x00";
        assert_eq!(detect_format(gif), ImageFormat::Gif);
    }

    #[test]
    fn test_detect_gif87a() {
        let gif = b"GIF87a\x00\x00";
        assert_eq!(detect_format(gif), ImageFormat::Gif);
    }

    #[test]
    fn test_detect_bmp() {
        let bmp = b"BM\x00\x00\x00\x00";
        assert_eq!(detect_format(bmp), ImageFormat::Bmp);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_format(&[0, 1, 2, 3]), ImageFormat::Unknown);
    }

    #[test]
    fn test_detect_too_short() {
        assert_eq!(detect_format(&[0xFF, 0xD8]), ImageFormat::Unknown);
    }

    // -- Truncated file rejection --

    #[test]
    fn test_truncated_png_no_iend() {
        let png_header = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0, 0, 0, 0, 0];
        let result = check_completeness(&png_header, ImageFormat::Png);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("IEND"));
    }

    #[test]
    fn test_valid_png_with_iend() {
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(&[0; 20]); // padding
        data.extend_from_slice(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]);
        assert!(check_completeness(&data, ImageFormat::Png).is_ok());
    }

    #[test]
    fn test_truncated_jpeg_no_eoi() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 10, 0, 0, 0, 0];
        let result = check_completeness(&jpeg, ImageFormat::Jpeg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("EOI"));
    }

    #[test]
    fn test_valid_jpeg_with_eoi() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 10, 0, 0, 0, 0];
        jpeg.push(0xFF);
        jpeg.push(0xD9);
        assert!(check_completeness(&jpeg, ImageFormat::Jpeg).is_ok());
    }

    #[test]
    fn test_truncated_gif_no_trailer() {
        let gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff";
        let result = check_completeness(gif, ImageFormat::Gif);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_gif_with_trailer() {
        let mut gif = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff".to_vec();
        gif.push(0x3B);
        assert!(check_completeness(&gif, ImageFormat::Gif).is_ok());
    }

    // -- dHash tests --

    #[test]
    fn test_dhash_identical_images() {
        // All same gray value → all bits 0
        let gray = vec![128u8; 9 * 8];
        let h = compute_dhash_from_gray(&gray, 9, 8);
        assert_eq!(h, 0);
    }

    #[test]
    fn test_dhash_gradient() {
        // Increasing gradient → left < right everywhere → all bits 0
        let mut gray = vec![0u8; 9 * 8];
        for row in 0..8 {
            for col in 0..9 {
                gray[row * 9 + col] = (col * 28) as u8;
            }
        }
        let h = compute_dhash_from_gray(&gray, 9, 8);
        assert_eq!(h, 0); // left never > right
    }

    #[test]
    fn test_dhash_reverse_gradient() {
        // Decreasing gradient → left > right everywhere → all bits 1
        let mut gray = vec![0u8; 9 * 8];
        for row in 0..8 {
            for col in 0..9 {
                gray[row * 9 + col] = (255 - col * 28) as u8;
            }
        }
        let h = compute_dhash_from_gray(&gray, 9, 8);
        assert_eq!(h, u64::MAX >> (64 - 64)); // all 64 bits set
    }

    #[test]
    fn test_hamming_distance_zero() {
        assert_eq!(hamming_distance(0xDEAD, 0xDEAD), 0);
    }

    #[test]
    fn test_hamming_distance_one() {
        assert_eq!(hamming_distance(0b1000, 0b1001), 1);
    }

    #[test]
    fn test_phash_tolerance_within_threshold() {
        let config = VerificationConfig { dhash_tolerance: 5, ..Default::default() };
        let a = 0b1111_0000_1111_0000u64;
        let b = 0b1111_0000_1111_0001u64; // 1 bit diff
        assert!(compare_hashes(a, b, &config));
    }

    #[test]
    fn test_phash_tolerance_exceeds_threshold() {
        let config = VerificationConfig { dhash_tolerance: 2, ..Default::default() };
        let a = 0u64;
        let b = 0b1111u64; // 4 bits diff
        assert!(!compare_hashes(a, b, &config));
    }

    // -- Placeholder detection --

    #[test]
    fn test_detect_lorem_ipsum() {
        let config = VerificationConfig::default();
        assert_eq!(detect_placeholder("This is Lorem Ipsum text", &config), Some("lorem ipsum".into()));
    }

    #[test]
    fn test_detect_todo() {
        let config = VerificationConfig::default();
        assert_eq!(detect_placeholder("TODO: implement this", &config), Some("todo".into()));
    }

    #[test]
    fn test_no_placeholder() {
        let config = VerificationConfig::default();
        assert_eq!(detect_placeholder("Real production content here", &config), None);
    }

    // -- Diff classification --

    #[test]
    fn test_diff_cosmetic() {
        let a = vec![128u8; 100 * 100];
        let mut b = a.clone();
        // Change 5 pixels (< 1%)
        b[0] = 200;
        b[1] = 200;
        b[2] = 200;
        b[3] = 200;
        b[4] = 200;
        assert_eq!(classify_diff(&a, &b, 100, 100, 10), DiffClass::Cosmetic);
    }

    #[test]
    fn test_diff_structural() {
        let a = vec![0u8; 100 * 100];
        let b = vec![255u8; 100 * 100];
        assert_eq!(classify_diff(&a, &b, 100, 100, 10), DiffClass::Structural);
    }

    // -- Full pipeline --

    #[test]
    fn test_verify_unknown_format() {
        let config = VerificationConfig::default();
        let result = verify_screenshot(&[0, 1, 2, 3, 4, 5, 6, 7], &config);
        assert!(matches!(result.verdict, ScreenshotVerdict::FormatMismatch { .. }));
    }

    #[test]
    fn test_verify_truncated_jpeg() {
        let config = VerificationConfig::default();
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 10, 0, 0, 0, 0];
        let result = verify_screenshot(&jpeg, &config);
        assert!(matches!(result.verdict, ScreenshotVerdict::Truncated(_)));
    }

    // -- Dimension extraction --

    #[test]
    fn test_extract_png_dimensions() {
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        // IHDR chunk length (13)
        data.extend_from_slice(&13u32.to_be_bytes());
        // "IHDR"
        data.extend_from_slice(b"IHDR");
        // Width: 1920
        data.extend_from_slice(&1920u32.to_be_bytes());
        // Height: 1080
        data.extend_from_slice(&1080u32.to_be_bytes());
        let dims = extract_dimensions(&data, ImageFormat::Png);
        assert_eq!(dims, Some((1920, 1080)));
    }

    #[test]
    fn test_extract_bmp_dimensions() {
        let mut data = vec![0u8; 30];
        data[0] = b'B';
        data[1] = b'M';
        // Width at offset 18: 800
        let w_bytes = 800i32.to_le_bytes();
        data[18..22].copy_from_slice(&w_bytes);
        // Height at offset 22: 600
        let h_bytes = 600i32.to_le_bytes();
        data[22..26].copy_from_slice(&h_bytes);
        let dims = extract_dimensions(&data, ImageFormat::Bmp);
        assert_eq!(dims, Some((800, 600)));
    }
}