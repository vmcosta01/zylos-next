//! Security Middleware — Task 6.3.2 (OWASP ASVS Level 2)
//!
//! Implements security headers, input sanitization, and audit logging
//! per OWASP ASVS 4.0 Level 2 requirements for zylos-live-comm.

use axum::{
    extract::Request,
    http::{header, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Audit log entry for security-relevant events
#[derive(Debug, Clone, Serialize)]
/// Estrutura AuditEntry.
pub struct AuditEntry {
    /// Campo timestamp.
    pub timestamp: String,
    /// Campo event type.
    pub event_type: String,
    /// Campo source ip.
    pub source_ip: String,
    /// Campo user agent.
    pub user_agent: String,
    /// Campo path.
    pub path: String,
    /// Campo method.
    pub method: String,
    /// Campo status.
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Campo detail.
    pub detail: Option<String>,
}

/// Thread-safe audit log buffer
#[derive(Clone)]
/// Estrutura AuditLog.
pub struct AuditLog {
    entries: Arc<Mutex<Vec<AuditEntry>>>,
    max_entries: usize,
}

impl AuditLog {
    /// Função new.
    pub fn new(max_entries: usize) -> Self {
        // Variante Self.
        Self {
            entries: Arc::new(Mutex::new(Vec::with_capacity(max_entries.min(10000)))),
            max_entries,
        }
    }

    /// Função record.
    pub async fn record(&self, entry: AuditEntry) {
        let mut log = self.entries.lock().await;
        if log.len() >= self.max_entries {
            // Ring buffer: remove oldest 10% when full
            let drain_count = self.max_entries / 10;
            log.drain(..drain_count);
        }
        log.push(entry);
    }

    /// Função recent.
    pub async fn recent(&self, limit: usize) -> Vec<AuditEntry> {
        let log = self.entries.lock().await;
        log.iter().rev().take(limit).cloned().collect()
    }

    /// Função count.
    pub async fn count(&self) -> usize {
        self.entries.lock().await.len()
    }
}

/// Security headers middleware — adds CSP, HSTS, X-Frame-Options, etc.
pub async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();

    // Content-Security-Policy: restrict script sources, prevent inline execution
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self' ws: wss:; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
        ),
    );

    // Strict-Transport-Security: enforce HTTPS for 1 year with subdomains
    headers.insert(
        header::STRICT_TRANSPORT_SECURITY,
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );

    // X-Content-Type-Options: prevent MIME sniffing
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );

    // X-Frame-Options: prevent clickjacking
    headers.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));

    // Referrer-Policy: limit referrer leakage
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // Permissions-Policy: disable unnecessary browser features
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
    );

    // Cache-Control for API responses: no-store prevents sensitive data caching
    if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store, no-cache, must-revalidate"),
        );
    }

    response
}

/// Audit logging middleware — records all requests to the audit log
pub async fn audit_middleware(
    axum::extract::State(audit): axum::extract::State<AuditLog>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let user_agent = request
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    // Extract source IP from X-Forwarded-For or X-Real-Ip, fallback to "local"
    let source_ip = request
        .headers()
        .get("X-Forwarded-For")
        .or_else(|| request.headers().get("X-Real-Ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .unwrap_or_else(|| "local".to_string());

    let response = next.run(request).await;
    let status = response.status().as_u16();

    let entry = AuditEntry {
        timestamp: Utc::now().to_rfc3339(),
        event_type: if status >= 400 {
            "security_alert".to_string()
        } else {
            "request".to_string()
        },
        source_ip,
        user_agent,
        path,
        method,
        status,
        detail: if status >= 400 {
            // Variante Some.
            Some(format!("HTTP {} response", status))
        } else {
            // Variante None.
            None
        },
    };

    if status >= 400 {
        warn!(
            target = "audit",
            method = %entry.method,
            path = %entry.path,
            status = entry.status,
            ip = %entry.source_ip,
            "Security-relevant request detected"
        );
    } else {
        info!(
            target = "audit",
            method = %entry.method,
            path = %entry.path,
            status = entry.status,
            "Request logged"
        );
    }

    audit.record(entry).await;

    response
}

/// Input sanitization: strip null bytes and control characters
pub fn sanitize_input(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .filter(|c| *c != '\0')
        .collect()
}

/// Validate that a string does not contain SQL injection patterns (basic heuristic)
pub fn detect_sql_injection(input: &str) -> bool {
    let lower = input.to_lowercase();
    let patterns = [
        "' or ",
        "\" or ",
        "'; drop",
        "\"; drop",
        "union select",
        "1=1",
        "' --",
        "\" --",
        "exec(",
        "execute(",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

pub fn detect_xss(input: &str) -> bool {
    let lower = input.to_lowercase();
    let patterns = [
        "<script",
        "javascript:",
        "onerror=",
        "onload=",
        "onclick=",
        "onmouseover=",
        "eval(",
        "expression(",
        "vbscript:",
    ];
    patterns.iter().any(|p| lower.contains(p))
}

pub fn detect_path_traversal(input: &str) -> bool {
    input.contains("..") || input.contains("%2e%2e") || input.contains("%252e%252e")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_input_strips_null_bytes() {
        assert_eq!(sanitize_input("hello\0world"), "helloworld");
    }

    #[test]
    fn test_sanitize_input_preserves_newlines() {
        assert_eq!(
            sanitize_input("line1\nline2\r\nline3"),
            "line1\nline2\r\nline3"
        );
    }

    #[test]
    fn test_sanitize_input_strips_control_chars() {
        assert_eq!(sanitize_input("hello\x01\x02world"), "helloworld");
    }

    #[test]
    fn test_detect_sql_injection_positive() {
        assert!(detect_sql_injection("' OR 1=1 --"));
        assert!(detect_sql_injection("'; DROP TABLE users;"));
        assert!(detect_sql_injection("UNION SELECT * FROM passwords"));
    }

    #[test]
    fn test_detect_sql_injection_negative() {
        assert!(!detect_sql_injection("normal search query"));
        assert!(!detect_sql_injection("agent-name-v2"));
    }

    #[test]
    fn test_detect_xss_positive() {
        assert!(detect_xss("<script>alert('xss')</script>"));
        assert!(detect_xss("javascript:void(0)"));
        assert!(detect_xss("<img onerror=alert(1)>"));
    }

    #[test]
    fn test_detect_xss_negative() {
        assert!(!detect_xss("normal text content"));
        assert!(!detect_xss("agent description with <bold> tags"));
    }

    #[test]
    fn test_detect_path_traversal() {
        assert!(detect_path_traversal("../../etc/passwd"));
        assert!(detect_path_traversal("%2e%2e/config"));
        assert!(!detect_path_traversal("normal/path/file.txt"));
    }

    #[tokio::test]
    async fn test_audit_log_ring_buffer() {
        let log = AuditLog::new(10);
        for i in 0..15 {
            log.record(AuditEntry {
                timestamp: format!("2026-01-01T00:00:{:02}Z", i),
                event_type: "request".to_string(),
                source_ip: "127.0.0.1".to_string(),
                user_agent: "test".to_string(),
                path: format!("/test/{}", i),
                method: "GET".to_string(),
                status: 200,
                detail: None,
            })
            .await;
        }
        // Should have evicted oldest entries
        assert!(log.count().await <= 10);
    }

    #[tokio::test]
    async fn test_audit_log_recent() {
        let log = AuditLog::new(100);
        for i in 0..5 {
            log.record(AuditEntry {
                timestamp: format!("2026-01-01T00:00:{:02}Z", i),
                event_type: "request".to_string(),
                source_ip: "127.0.0.1".to_string(),
                user_agent: "test".to_string(),
                path: format!("/test/{}", i),
                method: "GET".to_string(),
                status: 200,
                detail: None,
            })
            .await;
        }
        let recent = log.recent(3).await;
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].path, "/test/4");
    }
}
