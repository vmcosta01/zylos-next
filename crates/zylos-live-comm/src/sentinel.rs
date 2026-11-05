//! Sentinel Health Scanner (AIOps) — Task 7C.2.3
//!
//! Pure-function detectors over read-only stores, normalized signatures for
//! dedup, privacy-first PII stripping, rate-limited team incident reporting,
//! and JSONL append-only output outside the main data directory.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A health issue detected by a sentinel detector.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthIssue {
    /// Normalized signature hash (tool + error category).
    pub signature: String,
    /// Detector that produced this issue.
    pub detector: String,
    /// Severity level.
    pub severity: Severity,
    /// Human-readable summary (PII-free).
    pub summary: String,
    /// When first observed.
    pub first_seen: DateTime<Utc>,
    /// When last observed.
    pub last_seen: DateTime<Utc>,
    /// Occurrence count since first_seen.
    pub count: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Raw observation fed into detectors.
/// Detectors are pure functions over slices of these — no side effects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub tool: String,
    pub error_message: Option<String>,
    pub status_code: Option<u16>,
    pub actor_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    /// Arbitrary metadata (will be privacy-stripped before output).
    pub metadata: HashMap<String, String>,
}

/// Result from a single detector invocation.
#[derive(Debug, Clone)]
pub struct DetectorFinding {
    pub detector: String,
    pub severity: Severity,
    pub summary: String,
    /// Raw text used to compute the normalized signature.
    pub signature_input: String,
}

/// Team incident report (rate-limited).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentReport {
    pub signature: String,
    pub severity: Severity,
    pub summary: String,
    pub reported_at: DateTime<Utc>,
    pub suppressed: bool,
}

// ---------------------------------------------------------------------------
// Privacy stripping
// ---------------------------------------------------------------------------

/// Strip PII and sensitive fields from a string.
/// Replaces email addresses, UUIDs, hex tokens, and IP addresses with
/// redacted placeholders.
pub fn strip_pii(input: &str) -> String {
    let mut result = input.to_string();
    // Order matters: UUIDs before hex tokens (UUIDs contain hex chars),
    // emails before generic patterns.
    result = replace_uuids(&result, "[UUID_REDACTED]");
    result = replace_emails(&result, "[EMAIL_REDACTED]");
    result = replace_ipv4(&result, "[IP_REDACTED]");
    result = replace_hex_tokens(&result, "[TOKEN_REDACTED]");
    result
}

fn replace_emails(input: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(at_pos) = rest.find('@') {
        // Scan backwards for local part: [a-zA-Z0-9._%+-]+
        let before = &rest[..at_pos];
        let local_start = before
            .rfind(|c: char| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '%' | '+' | '-'))
            .map(|p| p + 1)
            .unwrap_or(0);
        let local_part = &before[local_start..];

        if local_part.is_empty() {
            result.push_str(&rest[..=at_pos]);
            rest = &rest[at_pos + 1..];
            continue;
        }

        // Scan forwards for domain: [a-zA-Z0-9.-]+\.[a-zA-Z]{2,}
        let after_at = &rest[at_pos + 1..];
        let domain_end = after_at
            .find(|c: char| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-'))
            .unwrap_or(after_at.len());
        let domain = &after_at[..domain_end];

        // Domain must contain at least one dot and end with 2+ alpha chars
        if let Some(last_dot) = domain.rfind('.') {
            let tld = &domain[last_dot + 1..];
            if last_dot > 0 && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) {
                result.push_str(&rest[..local_start]);
                result.push_str(replacement);
                rest = &after_at[domain_end..];
                continue;
            }
        }

        result.push_str(&rest[..=at_pos]);
        rest = &rest[at_pos + 1..];
    }
    result.push_str(rest);
    result
}

fn replace_uuids(input: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut rest = input;
    while rest.len() >= 36 {
        // Look for 8-4-4-4-12 pattern
        if let Some(pos) = rest.find('-') {
            if pos >= 8 && rest.len() >= pos + 28 {
                let candidate = &rest[pos - 8..pos - 8 + 36];
                if is_uuid_format(candidate) {
                    result.push_str(&rest[..pos - 8]);
                    result.push_str(replacement);
                    rest = &rest[pos - 8 + 36..];
                    continue;
                }
            }
            // Not a UUID at this dash, skip past it
            result.push_str(&rest[..=pos]);
            rest = &rest[pos + 1..];
        } else {
            break;
        }
    }
    result.push_str(rest);
    result
}

fn is_uuid_format(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    for (i, c) in s.chars().enumerate() {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            if c != '-' {
                return false;
            }
        } else if !c.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

fn replace_hex_tokens(input: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut run_start: Option<usize> = None;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i <= len {
        let is_hex = i < len && chars[i].is_ascii_hexdigit();
        if is_hex {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(start) = run_start {
            let run_len = i - start;
            if run_len >= 16 {
                result.push_str(replacement);
            } else {
                for c in &chars[start..i] {
                    result.push(*c);
                }
            }
            run_start = None;
            if i < len {
                result.push(chars[i]);
            }
        } else if i < len {
            result.push(chars[i]);
        }
        i += 1;
    }
    result
}

fn replace_ipv4(input: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        // Try to match an IPv4 address starting at position i
        if chars[i].is_ascii_digit() {
            if let Some((ip_len,)) = try_match_ipv4_at(&chars, i) {
                // Check boundary: must not be preceded or followed by digit/dot
                let preceded_ok = i == 0 || (!chars[i - 1].is_ascii_digit() && chars[i - 1] != '.');
                let followed_ok = i + ip_len >= len || (!chars[i + ip_len].is_ascii_digit() && chars[i + ip_len] != '.');
                if preceded_ok && followed_ok {
                    result.push_str(replacement);
                    i += ip_len;
                    continue;
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Try to match `\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}` starting at `start`.
/// Returns total character length if matched.
fn try_match_ipv4_at(chars: &[char], start: usize) -> Option<(usize,)> {
    let len = chars.len();
    let mut pos = start;
    for octet_idx in 0..4 {
        // Parse 1-3 digits
        let digit_start = pos;
        while pos < len && pos - digit_start < 3 && chars[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == digit_start {
            return None; // no digits
        }
        // Between octets, expect a dot (except after last)
        if octet_idx < 3 {
            if pos >= len || chars[pos] != '.' {
                return None;
            }
            pos += 1; // skip dot
        }
    }
    Some((pos - start,))
}

// ---------------------------------------------------------------------------
// Normalized signature
// ---------------------------------------------------------------------------

/// Compute a normalized signature from tool name and error message.
/// Strips PII, lowercases, trims whitespace, and hashes with SHA-256.
pub fn normalize_signature(tool: &str, error: Option<&str>) -> String {
    let clean_tool = tool.trim().to_lowercase();
    let clean_error = error
        .map(|e| strip_pii(e).trim().to_lowercase())
        .unwrap_or_default();

    let input = format!("{}|{}", clean_tool, clean_error);
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(&hash[..16]) // 128-bit truncated hash
}

// ---------------------------------------------------------------------------
// Detectors (pure functions)
// ---------------------------------------------------------------------------

/// Detect HTTP 5xx errors from observations.
pub fn detect_http_5xx(observations: &[Observation]) -> Vec<DetectorFinding> {
    observations
        .iter()
        .filter(|o| o.status_code.map_or(false, |c| c >= 500))
        .map(|o| DetectorFinding {
            detector: "http_5xx".into(),
            severity: Severity::Error,
            summary: format!(
                "HTTP {} on {}",
                o.status_code.unwrap_or(0),
                strip_pii(&o.tool)
            ),
            signature_input: format!(
                "{}|{}",
                o.tool,
                o.error_message.as_deref().unwrap_or("http_5xx")
            ),
        })
        .collect()
}

/// Detect timeout errors by keyword matching.
pub fn detect_timeouts(observations: &[Observation]) -> Vec<DetectorFinding> {
    let timeout_keywords = ["timeout", "timed out", "deadline exceeded", "request timed out"];

    observations
        .iter()
        .filter(|o| {
            o.error_message
                .as_ref()
                .map(|msg| {
                    let lower = msg.to_lowercase();
                    timeout_keywords.iter().any(|kw| lower.contains(kw))
                })
                .unwrap_or(false)
        })
        .map(|o| DetectorFinding {
            detector: "timeout".into(),
            severity: Severity::Warning,
            summary: format!("Timeout on {}", strip_pii(&o.tool)),
            signature_input: format!("{}|timeout", o.tool),
        })
        .collect()
}

/// Detect repeated failures (>= threshold within window).
pub fn detect_repeated_failures(
    observations: &[Observation],
    threshold: usize,
) -> Vec<DetectorFinding> {
    let mut error_counts: HashMap<String, usize> = HashMap::new();

    for o in observations {
        if o.error_message.is_some() || o.status_code.map_or(false, |c| c >= 400) {
            let key = normalize_signature(&o.tool, o.error_message.as_deref());
            *error_counts.entry(key).or_insert(0) += 1;
        }
    }

    error_counts
        .into_iter()
        .filter(|(_, count)| *count >= threshold)
        .map(|(sig, count)| DetectorFinding {
            detector: "repeated_failure".into(),
            severity: if count >= threshold * 3 {
                Severity::Critical
            } else {
                Severity::Warning
            },
            summary: format!("Repeated failure pattern: {} occurrences", count),
            signature_input: sig,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Issue store (JSONL append-only, upsert by signature)
// ---------------------------------------------------------------------------

/// In-memory issue store with upsert-by-signature semantics.
/// Output is designed to be written as JSONL append-only outside the
/// main data directory.
pub struct IssueStore {
    issues: Mutex<HashMap<String, HealthIssue>>,
}

impl IssueStore {
    pub fn new() -> Self {
        Self {
            issues: Mutex::new(HashMap::new()),
        }
    }

    /// Upsert an issue by signature. Returns the updated issue.
    pub fn upsert(&self, finding: DetectorFinding, now: DateTime<Utc>) -> HealthIssue {
        let signature = normalize_signature_from_input(&finding.signature_input);
        let mut issues = self.issues.lock().unwrap();

        let issue = issues.entry(signature.clone()).or_insert_with(|| HealthIssue {
            signature: signature.clone(),
            detector: finding.detector.clone(),
            severity: finding.severity,
            summary: strip_pii(&finding.summary),
            first_seen: now,
            last_seen: now,
            count: 0,
        });

        issue.last_seen = now;
        issue.count += 1;
        // Escalate severity if higher
        if finding.severity > issue.severity {
            issue.severity = finding.severity;
        }
        // Update summary with latest (always PII-stripped)
        issue.summary = strip_pii(&finding.summary);

        issue.clone()
    }

    /// Get all current issues.
    pub fn list(&self) -> Vec<HealthIssue> {
        self.issues.lock().unwrap().values().cloned().collect()
    }

    /// Get issue count by severity.
    pub fn counts_by_severity(&self) -> HashMap<Severity, u64> {
        let mut counts = HashMap::new();
        for issue in self.issues.lock().unwrap().values() {
            *counts.entry(issue.severity).or_insert(0) += issue.count;
        }
        counts
    }

    /// Serialize all issues as JSONL (one JSON object per line).
    pub fn to_jsonl(&self) -> String {
        let issues = self.issues.lock().unwrap();
        let mut lines = Vec::with_capacity(issues.len());
        for issue in issues.values() {
            if let Ok(json) = serde_json::to_string(issue) {
                lines.push(json);
            }
        }
        lines.join("\n")
    }
}

fn normalize_signature_from_input(input: &str) -> String {
    let hash = Sha256::digest(input.as_bytes());
    hex::encode(&hash[..16])
}

// ---------------------------------------------------------------------------
// Rate limiter for incident reporting
// ---------------------------------------------------------------------------

/// Token-bucket rate limiter for team incident reports.
/// Default: max_reports per window_seconds.
pub struct IncidentRateLimiter {
    max_reports: u32,
    window_seconds: i64,
    reports: Mutex<Vec<DateTime<Utc>>>,
}

impl IncidentRateLimiter {
    pub fn new(max_reports: u32, window_seconds: i64) -> Self {
        Self {
            max_reports,
            window_seconds,
            reports: Mutex::new(Vec::new()),
        }
    }

    /// Default: 5 reports per hour.
    pub fn default_hourly() -> Self {
        Self::new(5, 3600)
    }

    /// Try to record a report. Returns true if allowed, false if rate-limited.
    pub fn try_report(&self, now: DateTime<Utc>) -> bool {
        let mut reports = self.reports.lock().unwrap();
        let cutoff = now - chrono::Duration::seconds(self.window_seconds);
        reports.retain(|t| *t > cutoff);

        if reports.len() < self.max_reports as usize {
            reports.push(now);
            true
        } else {
            false
        }
    }

    /// Seconds until next report slot opens.
    pub fn retry_after_seconds(&self, now: DateTime<Utc>) -> Option<i64> {
        let reports = self.reports.lock().unwrap();
        let cutoff = now - chrono::Duration::seconds(self.window_seconds);
        let active: Vec<_> = reports.iter().filter(|t| **t > cutoff).collect();

        if active.len() < self.max_reports as usize {
            None
        } else {
            // Oldest active report expires first
            active.iter().min().map(|oldest| {
                (**oldest + chrono::Duration::seconds(self.window_seconds) - now).num_seconds().max(0)
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Sentinel scanner (orchestrates detectors + store + rate limiter)
// ---------------------------------------------------------------------------

pub struct SentinelScanner {
    store: IssueStore,
    rate_limiter: IncidentRateLimiter,
}

impl SentinelScanner {
    pub fn new() -> Self {
        Self {
            store: IssueStore::new(),
            rate_limiter: IncidentRateLimiter::default_hourly(),
        }
    }

    pub fn with_rate_limiter(mut self, limiter: IncidentRateLimiter) -> Self {
        self.rate_limiter = limiter;
        self
    }

    /// Run all detectors over observations and upsert findings.
    pub fn scan(&self, observations: &[Observation], now: DateTime<Utc>) -> Vec<HealthIssue> {
        let mut all_findings = Vec::new();
        all_findings.extend(detect_http_5xx(observations));
        all_findings.extend(detect_timeouts(observations));
        all_findings.extend(detect_repeated_failures(observations, 3));

        all_findings
            .into_iter()
            .map(|f| self.store.upsert(f, now))
            .collect()
    }

    /// Report an incident to the team (rate-limited).
    pub fn report_incident(&self, signature: &str, severity: Severity, summary: &str, now: DateTime<Utc>) -> IncidentReport {
        let allowed = self.rate_limiter.try_report(now);
        IncidentReport {
            signature: signature.to_string(),
            severity,
            summary: strip_pii(summary),
            reported_at: now,
            suppressed: !allowed,
        }
    }

    /// Get current issues.
    pub fn issues(&self) -> Vec<HealthIssue> {
        self.store.list()
    }

    /// Export issues as JSONL.
    pub fn export_jsonl(&self) -> String {
        self.store.to_jsonl()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(tool: &str, error: Option<&str>, status: Option<u16>) -> Observation {
        Observation {
            tool: tool.into(),
            error_message: error.map(Into::into),
            status_code: status,
            actor_id: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_strip_pii_email() {
        let input = "Failed for user@example.com during sync";
        let stripped = strip_pii(input);
        assert!(stripped.contains("[EMAIL_REDACTED]"));
        assert!(!stripped.contains("user@example.com"));
    }

    #[test]
    fn test_strip_pii_uuid() {
        let input = "Task 550e8400-e29b-41d4-a716-446655440000 failed";
        let stripped = strip_pii(input);
        assert!(stripped.contains("[UUID_REDACTED]"));
        assert!(!stripped.contains("550e8400"));
    }

    #[test]
    fn test_strip_pii_ipv4() {
        let input = "Connection to 192.168.1.100 refused";
        let stripped = strip_pii(input);
        assert!(stripped.contains("[IP_REDACTED]"));
        assert!(!stripped.contains("192.168.1.100"));
    }

    #[test]
    fn test_strip_pii_hex_token() {
        let input = "Token abcdef0123456789abcdef0123456789 expired";
        let stripped = strip_pii(input);
        assert!(stripped.contains("[TOKEN_REDACTED]"));
        assert!(!stripped.contains("abcdef0123456789"));
    }

    #[test]
    fn test_normalize_signature_deterministic() {
        let sig1 = normalize_signature("fs.write", Some("permission denied"));
        let sig2 = normalize_signature("fs.write", Some("permission denied"));
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_normalize_signature_strips_pii() {
        let sig1 = normalize_signature("email.send", Some("failed for user@test.com"));
        let sig2 = normalize_signature("email.send", Some("failed for other@domain.org"));
        // Both should produce the same signature since emails are stripped
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_normalize_signature_differs_on_tool() {
        let sig1 = normalize_signature("fs.write", Some("error"));
        let sig2 = normalize_signature("fs.read", Some("error"));
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn test_detect_http_5xx_pure() {
        let observations = vec![
            obs("api.call", None, Some(200)),
            obs("api.call", Some("internal error"), Some(500)),
            obs("api.call", Some("bad gateway"), Some(502)),
            obs("api.call", None, Some(404)),
        ];

        let findings = detect_http_5xx(&observations);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|f| f.detector == "http_5xx"));
        assert!(findings.iter().all(|f| f.severity == Severity::Error));
    }

    #[test]
    fn test_detect_timeouts_pure() {
        let observations = vec![
            obs("db.query", Some("query timeout after 30s"), None),
            obs("db.query", Some("connection refused"), None),
            obs("http.fetch", Some("request timed out"), None),
        ];

        let findings = detect_timeouts(&observations);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|f| f.detector == "timeout"));
    }

    #[test]
    fn test_detect_repeated_failures_pure() {
        let observations: Vec<_> = (0..5)
            .map(|_| obs("api.call", Some("rate limited"), Some(429)))
            .collect();

        let findings = detect_repeated_failures(&observations, 3);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, "repeated_failure");
    }

    #[test]
    fn test_issue_store_upsert_dedup() {
        let store = IssueStore::new();
        let now = Utc::now();

        let finding = DetectorFinding {
            detector: "test".into(),
            severity: Severity::Warning,
            summary: "test issue".into(),
            signature_input: "test|issue".into(),
        };

        let issue1 = store.upsert(finding.clone(), now);
        assert_eq!(issue1.count, 1);

        let issue2 = store.upsert(finding, now);
        assert_eq!(issue2.count, 2);
        assert_eq!(store.list().len(), 1); // Deduped by signature
    }

    #[test]
    fn test_issue_store_privacy_stripping() {
        let store = IssueStore::new();
        let now = Utc::now();

        let finding = DetectorFinding {
            detector: "test".into(),
            severity: Severity::Error,
            summary: "Failed for user@secret.com on 10.0.0.1".into(),
            signature_input: "test|pii".into(),
        };

        let issue = store.upsert(finding, now);
        assert!(!issue.summary.contains("user@secret.com"));
        assert!(!issue.summary.contains("10.0.0.1"));
        assert!(issue.summary.contains("[EMAIL_REDACTED]"));
        assert!(issue.summary.contains("[IP_REDACTED]"));
    }

    #[test]
    fn test_issue_store_jsonl_output() {
        let store = IssueStore::new();
        let now = Utc::now();

        store.upsert(
            DetectorFinding {
                detector: "a".into(),
                severity: Severity::Info,
                summary: "issue a".into(),
                signature_input: "a|1".into(),
            },
            now,
        );
        store.upsert(
            DetectorFinding {
                detector: "b".into(),
                severity: Severity::Warning,
                summary: "issue b".into(),
                signature_input: "b|2".into(),
            },
            now,
        );

        let jsonl = store.to_jsonl();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line must be valid JSON
        for line in &lines {
            serde_json::from_str::<HealthIssue>(line).expect("valid JSON line");
        }
    }

    #[test]
    fn test_rate_limiter_allows_within_budget() {
        let limiter = IncidentRateLimiter::new(3, 3600);
        let now = Utc::now();

        assert!(limiter.try_report(now));
        assert!(limiter.try_report(now));
        assert!(limiter.try_report(now));
    }

    #[test]
    fn test_rate_limiter_blocks_over_budget() {
        let limiter = IncidentRateLimiter::new(2, 3600);
        let now = Utc::now();

        assert!(limiter.try_report(now));
        assert!(limiter.try_report(now));
        assert!(!limiter.try_report(now)); // Blocked
    }

    #[test]
    fn test_rate_limiter_retry_after() {
        let limiter = IncidentRateLimiter::new(1, 60);
        let now = Utc::now();

        assert!(limiter.try_report(now));
        let retry = limiter.retry_after_seconds(now);
        assert!(retry.is_some());
        assert!(retry.unwrap() > 0);
        assert!(retry.unwrap() <= 60);
    }

    #[test]
    fn test_sentinel_scan_integration() {
        let scanner = SentinelScanner::new();
        let observations = vec![
            obs("api.users", Some("internal error"), Some(500)),
            obs("api.users", Some("timeout"), None),
            obs("api.health", None, Some(200)),
        ];

        let issues = scanner.scan(&observations, Utc::now());
        assert!(!issues.is_empty());

        // All summaries must be PII-free
        for issue in &issues {
            assert!(!issue.summary.contains("@"));
        }
    }

    #[test]
    fn test_sentinel_incident_report_rate_limited() {
        let limiter = IncidentRateLimiter::new(1, 3600);
        let scanner = SentinelScanner::new().with_rate_limiter(limiter);
        let now = Utc::now();

        let r1 = scanner.report_incident("sig-1", Severity::Critical, "critical issue", now);
        assert!(!r1.suppressed);

        let r2 = scanner.report_incident("sig-2", Severity::Error, "another issue", now);
        assert!(r2.suppressed);
    }

    #[test]
    fn test_detectors_are_pure_functions() {
        // Verify detectors produce identical output for identical input
        let observations = vec![
            obs("api.test", Some("error msg"), Some(500)),
            obs("api.test", Some("timeout occurred"), None),
        ];

        let run1_5xx = detect_http_5xx(&observations);
        let run2_5xx = detect_http_5xx(&observations);
        assert_eq!(run1_5xx.len(), run2_5xx.len());
        assert_eq!(run1_5xx[0].summary, run2_5xx[0].summary);

        let run1_timeout = detect_timeouts(&observations);
        let run2_timeout = detect_timeouts(&observations);
        assert_eq!(run1_timeout.len(), run2_timeout.len());
    }
}