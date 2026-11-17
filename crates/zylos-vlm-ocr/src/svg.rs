//! SVG parsing and rendering for OCR-extracted diagrams and formulas
//!
//! Provides both **rendering** (OcrElement → SVG string) and **parsing**
//! (SVG string → structured SvgDocument) to support the full VLM OCR pipeline.

use crate::types::{ElementType, OcrElement};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Structured SVG types (output of parsing)
// ---------------------------------------------------------------------------

/// A parsed SVG document with extracted semantic elements
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgDocument {
    pub width: f64,
    pub height: f64,
    pub nodes: Vec<SvgNode>,
    pub texts: Vec<SvgText>,
    pub paths: Vec<SvgPath>,
    pub formulas: Vec<SvgFormula>,
}

/// A graphical node (rect, circle, ellipse, etc.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgNode {
    pub tag: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub label: Option<String>,
}

/// A text element extracted from SVG
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgText {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub font_size: Option<f64>,
    pub font_family: Option<String>,
    pub font_style: Option<String>,
}

/// A path element (lines, curves, arrows)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgPath {
    pub d: String,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub fill: Option<String>,
}

/// A formula extracted from SVG (italic serif text or MathML-like groups)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgFormula {
    pub content: String,
    pub x: f64,
    pub y: f64,
    pub raw_svg: String,
}

// ---------------------------------------------------------------------------
// SVG Parser
// ---------------------------------------------------------------------------

/// Parse an SVG string into a structured `SvgDocument`.
///
/// Uses lightweight regex-based extraction suitable for VLM-generated SVGs
/// which follow predictable patterns. For production-grade parsing of
/// arbitrary SVGs, consider integrating `usvg` or `roxmltree`.
pub fn parse_svg(svg: &str) -> Result<SvgDocument, SvgParseError> {
    if svg.trim().is_empty() {
        return Err(SvgParseError::EmptyInput);
    }
    if !svg.contains("<svg") {
        return Err(SvgParseError::MissingRoot);
    }

    let width = extract_viewbox_width(svg).unwrap_or(400.0);
    let height = extract_viewbox_height(svg).unwrap_or(300.0);

    let nodes = extract_nodes(svg);
    let texts = extract_texts(svg);
    let paths = extract_paths(svg);
    let formulas = extract_formulas(svg, &texts);

    Ok(SvgDocument {
        width,
        height,
        nodes,
        texts,
        paths,
        formulas,
    })
}

/// Errors during SVG parsing
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SvgParseError {
    #[error("Empty SVG input")]
    EmptyInput,
    #[error("Missing <svg> root element")]
    MissingRoot,
}

// ---------------------------------------------------------------------------
// Internal extraction helpers
// ---------------------------------------------------------------------------

fn extract_viewbox_width(svg: &str) -> Option<f64> {
    if let Some(vb) = extract_attr(svg, "viewBox") {
        let parts: Vec<&str> = vb.split_whitespace().collect();
        if parts.len() >= 3 {
            return parts[2].parse().ok();
        }
    }
    extract_attr(svg, "width").and_then(|v| v.trim_end_matches("px").parse().ok())
}

fn extract_viewbox_height(svg: &str) -> Option<f64> {
    if let Some(vb) = extract_attr(svg, "viewBox") {
        let parts: Vec<&str> = vb.split_whitespace().collect();
        if parts.len() >= 4 {
            return parts[3].parse().ok();
        }
    }
    extract_attr(svg, "height").and_then(|v| v.trim_end_matches("px").parse().ok())
}

fn extract_attr(tag_str: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    if let Some(start) = tag_str.find(&pattern) {
        let val_start = start + pattern.len();
        if let Some(end) = tag_str[val_start..].find('"') {
            return Some(tag_str[val_start..val_start + end].to_string());
        }
    }
    let pattern_sq = format!("{}='", attr);
    if let Some(start) = tag_str.find(&pattern_sq) {
        let val_start = start + pattern_sq.len();
        if let Some(end) = tag_str[val_start..].find('\'') {
            return Some(tag_str[val_start..val_start + end].to_string());
        }
    }
    None
}

fn extract_nodes(svg: &str) -> Vec<SvgNode> {
    let mut nodes = Vec::new();
    for tag in ["rect", "circle", "ellipse"] {
        let search = format!("<{}", tag);
        let mut pos = 0;
        while let Some(idx) = svg[pos..].find(&search) {
            let abs = pos + idx;
            if let Some(end) = svg[abs..].find('>') {
                let elem = &svg[abs..abs + end + 1];
                let x = extract_attr(elem, "x")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let y = extract_attr(elem, "y")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let w = extract_attr(elem, "width")
                    .or_else(|| {
                        extract_attr(elem, "r").map(|r| {
                            let r: f64 = r.parse().unwrap_or(0.0);
                            (r * 2.0).to_string()
                        })
                    })
                    .or_else(|| extract_attr(elem, "rx"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let h = extract_attr(elem, "height")
                    .or_else(|| {
                        extract_attr(elem, "r").map(|r| {
                            let r: f64 = r.parse().unwrap_or(0.0);
                            (r * 2.0).to_string()
                        })
                    })
                    .or_else(|| extract_attr(elem, "ry"))
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                nodes.push(SvgNode {
                    tag: tag.to_string(),
                    x,
                    y,
                    width: w,
                    height: h,
                    fill: extract_attr(elem, "fill"),
                    stroke: extract_attr(elem, "stroke"),
                    label: None,
                });
                pos = abs + end + 1;
            } else {
                break;
            }
        }
    }
    nodes
}

fn extract_texts(svg: &str) -> Vec<SvgText> {
    let mut texts = Vec::new();
    let mut pos = 0;
    while let Some(idx) = svg[pos..].find("<text") {
        let abs = pos + idx;
        if let Some(close) = svg[abs..].find("</text>") {
            let elem = &svg[abs..abs + close + 7];
            if let Some(gt) = elem.find('>') {
                let content = elem[gt + 1..elem.len() - 7].trim().to_string();
                let x = extract_attr(elem, "x")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let y = extract_attr(elem, "y")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let font_size = extract_attr(elem, "font-size").and_then(|v| v.parse().ok());
                let font_family = extract_attr(elem, "font-family");
                let font_style = extract_attr(elem, "font-style");
                texts.push(SvgText {
                    content,
                    x,
                    y,
                    font_size,
                    font_family,
                    font_style,
                });
            }
            pos = abs + close + 7;
        } else {
            break;
        }
    }
    texts
}

fn extract_paths(svg: &str) -> Vec<SvgPath> {
    let mut paths = Vec::new();
    let mut pos = 0;
    while let Some(idx) = svg[pos..].find("<path") {
        let abs = pos + idx;
        if let Some(end) = svg[abs..].find('>') {
            let elem = &svg[abs..abs + end + 1];
            if let Some(d) = extract_attr(elem, "d") {
                paths.push(SvgPath {
                    d,
                    stroke: extract_attr(elem, "stroke"),
                    stroke_width: extract_attr(elem, "stroke-width").and_then(|v| v.parse().ok()),
                    fill: extract_attr(elem, "fill"),
                });
            }
            pos = abs + end + 1;
        } else {
            break;
        }
    }
    paths
}

fn extract_formulas(_svg: &str, texts: &[SvgText]) -> Vec<SvgFormula> {
    texts
        .iter()
        .filter(|t| {
            let is_serif = t
                .font_family
                .as_ref()
                .map(|f| f.contains("serif"))
                .unwrap_or(false);
            let is_italic = t
                .font_style
                .as_ref()
                .map(|s| s.contains("italic"))
                .unwrap_or(false);
            is_serif && is_italic
        })
        .map(|t| {
            let raw = format!(
                r##"<text x="{}" y="{}" font-family="serif" font-style="italic">{}</text>"##,
                t.x,
                t.y,
                html_escape(&t.content)
            );
            SvgFormula {
                content: t.content.clone(),
                x: t.x,
                y: t.y,
                raw_svg: raw,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rendering (existing functionality, preserved)
// ---------------------------------------------------------------------------

/// Render an extracted diagram element as inline SVG
pub fn render_diagram_svg(element: &OcrElement) -> Option<String> {
    if element.element_type != ElementType::Diagram && element.element_type != ElementType::Chart {
        return None;
    }
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">
  <rect width="400" height="300" fill="#f8f9fa" stroke="#dee2e6" rx="4"/>
  <text x="200" y="150" text-anchor="middle" font-family="sans-serif" font-size="14" fill="#495057">{}</text>
</svg>"##,
        html_escape(&element.content)
    ))
}

/// Render a formula element as MathML-compatible SVG placeholder
pub fn render_formula_svg(element: &OcrElement) -> Option<String> {
    if element.element_type != ElementType::Formula {
        return None;
    }
    Some(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 60">
  <text x="150" y="35" text-anchor="middle" font-family="serif" font-style="italic" font-size="18" fill="#212529">{}</text>
</svg>"##,
        html_escape(&element.content)
    ))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BBox;
    use std::collections::HashMap;

    fn make_element(etype: ElementType, content: &str) -> OcrElement {
        OcrElement {
            element_type: etype,
            content: content.to_string(),
            bbox: Some(BBox {
                x: 0,
                y: 0,
                width: 100,
                height: 50,
            }),
            confidence: 0.9,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_render_diagram_returns_svg() {
        let el = make_element(ElementType::Diagram, "System Architecture");
        let svg = render_diagram_svg(&el).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("System Architecture"));
    }

    #[test]
    fn test_render_diagram_rejects_text() {
        let el = make_element(ElementType::Text, "Hello");
        assert!(render_diagram_svg(&el).is_none());
    }

    #[test]
    fn test_render_formula_returns_svg() {
        let el = make_element(ElementType::Formula, "E = mc^2");
        let svg = render_formula_svg(&el).unwrap();
        assert!(svg.contains("E = mc^2"));
        assert!(svg.contains("serif"));
    }

    #[test]
    fn test_html_escape_special_chars() {
        assert_eq!(html_escape("<b>&\""), "&lt;b&gt;&amp;&quot;");
    }

    #[test]
    fn test_parse_simple_svg() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">
  <rect x="10" y="20" width="100" height="50" fill="#f8f9fa" stroke="#dee2e6"/>
  <text x="200" y="150" font-family="sans-serif" font-size="14">Hello World</text>
</svg>"##;
        let doc = parse_svg(svg).unwrap();
        assert_eq!(doc.width, 400.0);
        assert_eq!(doc.height, 300.0);
        assert_eq!(doc.nodes.len(), 1);
        assert_eq!(doc.nodes[0].tag, "rect");
        assert_eq!(doc.texts.len(), 1);
        assert_eq!(doc.texts[0].content, "Hello World");
    }

    #[test]
    fn test_parse_svg_with_path() {
        let svg = r##"<svg viewBox="0 0 200 200">
  <path d="M10 10 L100 100" stroke="#333" stroke-width="2"/>
</svg>"##;
        let doc = parse_svg(svg).unwrap();
        assert_eq!(doc.paths.len(), 1);
        assert_eq!(doc.paths[0].d, "M10 10 L100 100");
        assert_eq!(doc.paths[0].stroke.as_deref(), Some("#333"));
    }

    #[test]
    fn test_parse_svg_extracts_formula() {
        let svg = r##"<svg viewBox="0 0 300 60">
  <text x="150" y="35" font-family="serif" font-style="italic" font-size="18">E = mc^2</text>
</svg>"##;
        let doc = parse_svg(svg).unwrap();
        assert_eq!(doc.formulas.len(), 1);
        assert_eq!(doc.formulas[0].content, "E = mc^2");
    }

    #[test]
    fn test_parse_svg_no_formula_for_sans_serif() {
        let svg = r##"<svg viewBox="0 0 300 60">
  <text x="150" y="35" font-family="sans-serif" font-size="14">Regular text</text>
</svg>"##;
        let doc = parse_svg(svg).unwrap();
        assert_eq!(doc.formulas.len(), 0);
        assert_eq!(doc.texts.len(), 1);
    }

    #[test]
    fn test_parse_svg_roundtrip_render_then_parse() {
        let el = make_element(ElementType::Formula, "x^2 + y^2 = r^2");
        let rendered = render_formula_svg(&el).unwrap();
        let doc = parse_svg(&rendered).unwrap();
        assert_eq!(doc.formulas.len(), 1);
        assert_eq!(doc.formulas[0].content, "x^2 + y^2 = r^2");
    }

    #[test]
    fn test_parse_empty_svg_returns_error() {
        assert_eq!(parse_svg(""), Err(SvgParseError::EmptyInput));
    }

    #[test]
    fn test_extract_attr_double_quotes() {
        let tag = r##"<rect x="10" y="20" fill="#fff"/>"##;
        assert_eq!(extract_attr(tag, "x"), Some("10".to_string()));
        assert_eq!(extract_attr(tag, "fill"), Some("#fff".to_string()));
        assert_eq!(extract_attr(tag, "missing"), None);
    }
}
