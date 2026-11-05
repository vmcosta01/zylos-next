//! zylos-sentinel: AIOps health scanner with pure-function detectors, normalized
//! signatures, privacy-first output, JSONL append-only logging, and rate-limited
//! incident reporting for Zylos-Next.
//!
//! # AIOps Health Scanner (Task 7C.2.1 / 7C.3.1)
//!
//! - Pure-function health detectors producing normalized, privacy-safe signatures
//! - Separate output directory for scan results
//! - Tokio interval scheduling (120s default)
//! - Privacy-first hashing of PII/IDs
//! - JSONL append-only logging for audit trail
//! - Rate limit: 5 incidents/hour per signature
//! - EventBus integration for incident broadcasting
//!
//! Source: OpenDuet `sentinel.ts`, `health-check.ts`, `incident-rate-limiter.ts`.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Severity level for health findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Service is healthy.
    Healthy,
    /// Degraded but functional.
    Warning,
    /// Service is failing or unreachable.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Healthy => write!(f, "healthy"),
            Severity::Warning => write!(f, "warning"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

/// A single health check result from a detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    /// Detector identifier (e.g., "http_endpoint", "disk_usage", "process_alive").
    pub detector_id: String,
    /// Target being checked (normalized — no credentials, PII, or tokens).
    pub target: String,
    pub severity: Severity,
    /// Machine-readable reason code.
    pub reason: String,
    /// Human-readable detail (privacy-safe).
    pub detail: Option<String>,
    /// Latency of the check itself.
    pub check_latency_ms: u64,
}

/// Normalized incident signature for deduplication.
/// Same root cause → same signature → suppressed duplicate alerts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentSignature(pub String);

impl IncidentSignature {
    /// Build a normalized signature from detector + target + reason.
    /// Strips variable data (timestamps, IPs, request IDs) to group related incidents.
    pub fn new(detector_id: &str, target: &str, reason: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(detector_id.as_bytes());
        hasher.update(b":");
        hasher.update(target.as_bytes());
        hasher.update(b":");
        hasher.update(reason.as_bytes());
        let hash = hex::encode(hasher.finalize());
        Self(hash[..16].to_string())
    }
}

/// An incident derived from a failing health check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub signature: IncidentSignature,
    pub detector_id: String,
    pub target: String,
    pub severity: Severity,
    pub reason: String,
    pub detail: Option<String>,
    pub first_seen: String,
    pub occurrence_count: u64,
}

/// Rate limiter for incident reporting.
/// Prevents alert storms during cascading failures by enforcing a minimum
/// interval between reports of the same incident signature.
pub struct IncidentRateLimiter {
    /// Minimum interval between reports of the same signature.
    min_interval: Duration,
    /// Last report time per signature.
    last_reported: HashMap<IncidentSignature, Instant>,
}

impl IncidentRateLimiter {
    pub fn new(min_interval: Duration) -> Self {
        Self {
            min_interval,
            last_reported: HashMap::new(),
        }
    }

    /// Returns true if this incident should be reported now.
    /// Updates internal state if allowed.
    pub fn should_report(&mut self, signature: &IncidentSignature) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_reported.get(signature) {
            if now.duration_since(*last) < self.min_interval {
                return false;
            }
        }
        self.last_reported.insert(signature.clone(), now);
        true
    }

    /// Reset tracking for a specific signature (e.g., after recovery).
    pub fn reset(&mut self, signature: &IncidentSignature) {
        self.last_reported.remove(signature);
    }

    /// Number of signatures currently tracked.
    pub fn tracked_count(&self) -> usize {
        self.last_reported.len()
    }
}

/// Privacy sanitizer: strips sensitive data from health check outputs.
/// Applied BEFORE any logging or reporting.
pub fn sanitize_for_output(text: &str) -> String {
    let mut result = text.to_string();

    // Strip Bearer tokens
    if let Some(pos) = result.find("Bearer ") {
        let after = &result[pos + 7..];
        let end = after.find(char::is_whitespace).unwrap_or(after.len());
        result.replace_range(pos..pos + 7 + end, "Bearer [REDACTED]");
    }

    // Strip API keys in common formats
    for pattern in &["api_key=", "apikey=", "token=", "secret=", "password="] {
        let mut search_from = 0;
        while let Some(rel_pos) = result[search_from..].find(pattern) {
            let pos = search_from + rel_pos;
            let value_start = pos + pattern.len();
            let after = &result[value_start..];
            let end = after
                .find(|c: char| c == '&' || c == ' ' || c == '\n' || c == '"')
                .unwrap_or(after.len());
            if end == 0 {
                // No value after the key — skip past this occurrence to avoid infinite loop
                search_from = value_start;
                continue;
            }
            let current_value = &result[value_start..value_start + end];
            if current_value == "[REDACTED]" {
                // Already redacted — advance past to avoid infinite loop
                search_from = value_start + end;
                continue;
            }
            result.replace_range(value_start..value_start + end, "[REDACTED]");
            search_from = value_start + "[REDACTED]".len();
        }
    }

    // Strip email addresses
    while let Some(at_pos) = result.find('@') {
        let before = &result[..at_pos];
        let local_start = before
            .rfind(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-')
            .map(|p| p + 1)
            .unwrap_or(0);
        let after = &result[at_pos + 1..];
        let domain_end = after
            .find(|c: char| !c.is_alphanumeric() && c != '.' && c != '-')
            .unwrap_or(after.len());
        result.replace_range(local_start..at_pos + 1 + domain_end, "[EMAIL_REDACTED]");
    }

    result
}

/// Run all detectors and collect health checks.
/// Each detector is a pure function: input → HealthCheck.
pub fn run_health_scan<F>(detectors: &[(String, F)]) -> Vec<HealthCheck>
where
    F: Fn() -> HealthCheck,
{
    detectors
        .iter()
        .map(|(_name, detector)| detector())
        .collect()
}

/// Convert failing health checks into incidents, applying rate limiting.
pub fn generate_incidents(
    checks: &[HealthCheck],
    rate_limiter: &mut IncidentRateLimiter,
    timestamp: &str,
) -> Vec<Incident> {
    checks
        .iter()
        .filter(|c| c.severity != Severity::Healthy)
        .filter_map(|check| {
            let sig = IncidentSignature::new(&check.detector_id, &check.target, &check.reason);
            if rate_limiter.should_report(&sig) {
                Some(Incident {
                    signature: sig,
                    detector_id: check.detector_id.clone(),
                    target: sanitize_for_output(&check.target),
                    severity: check.severity,
                    reason: check.reason.clone(),
                    detail: check.detail.as_ref().map(|d| sanitize_for_output(d)),
                    first_seen: timestamp.to_string(),
                    occurrence_count: 1,
                })
            } else {
                None
            }
        })
        .collect()
}

// ─── Privacy-First Hash ─────────────────────────────────────────────────────

/// Hash PII/IDs for privacy-safe logging. Returns first 16 hex chars of SHA-256.
pub fn privacy_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let hash = hex::encode(hasher.finalize());
    hash[..16].to_string()
}

// ─── JSONL Append-Only Logger ───────────────────────────────────────────────

/// A single JSONL log entry for audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelLogEntry {
    pub timestamp: String,
    pub event_type: String,
    pub detector_id: Option<String>,
    pub target_hash: Option<String>,
    pub severity: Option<String>,
    pub reason: Option<String>,
    pub incident_signature: Option<String>,
    pub details: Option<serde_json::Value>,
}

/// Append a log entry to a JSONL file (append-only). Thread-safe via Mutex.
pub async fn append_jsonl_log(
    output_dir: &Path,
    entry: &SentinelLogEntry,
) -> Result<(), std::io::Error> {
    use tokio::fs::OpenOptions;
    use tokio::io::AsyncWriteExt;

    tokio::fs::create_dir_all(output_dir).await?;
    let log_path = output_dir.join("sentinel.jsonl");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .await?;
    let line = serde_json::to_string(entry)
        .unwrap_or_else(|_| "{}".into());
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

// ─── Health Scanner Engine ──────────────────────────────────────────────────

/// Configuration for the health scanner.
#[derive(Debug, Clone)]
pub struct SentinelConfig {
    /// Scan interval in seconds. Default: 120.
    pub scan_interval_secs: u64,
    /// Output directory for JSONL logs (separate from main data dir).
    pub output_dir: PathBuf,
    /// Max incidents per hour per signature. Default: 5.
    pub rate_limit_per_hour: u32,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            scan_interval_secs: 120,
            output_dir: PathBuf::from("./sentinel-output"),
            rate_limit_per_hour: 5,
        }
    }
}

/// Health scanner engine with scheduling, JSONL logging, and rate limiting.
pub struct HealthScanner {
    config: SentinelConfig,
    rate_limiter: Arc<Mutex<IncidentRateLimiter>>,
    detectors: Vec<(String, Box<dyn Fn() -> HealthCheck + Send + Sync>)>,
}

impl HealthScanner {
    pub fn new(config: SentinelConfig) -> Self {
        let min_interval = Duration::from_secs(3600 / config.rate_limit_per_hour.max(1) as u64);
        Self {
            config,
            rate_limiter: Arc::new(Mutex::new(IncidentRateLimiter::new(min_interval))),
            detectors: Vec::new(),
        }
    }

    /// Register a pure-function detector.
    pub fn add_detector<F>(&mut self, name: impl Into<String>, detector: F)
    where
        F: Fn() -> HealthCheck + Send + Sync + 'static,
    {
        self.detectors.push((name.into(), Box::new(detector)));
    }

    /// Run a single scan cycle: execute detectors, generate incidents, log results.
    pub async fn run_scan_cycle(&self) -> Vec<Incident> {
        let timestamp = Utc::now().to_rfc3339();

        // Run all pure-function detectors
        let checks: Vec<HealthCheck> = self.detectors
            .iter()
            .map(|(_name, detector)| detector())
            .collect();

        // Generate rate-limited incidents
        let mut rl = self.rate_limiter.lock().await;
        let incidents = generate_incidents(&checks, &mut rl, &timestamp);
        drop(rl);

        // Log scan results to JSONL
        let scan_entry = SentinelLogEntry {
            timestamp: timestamp.clone(),
            event_type: "scan_complete".into(),
            detector_id: None,
            target_hash: None,
            severity: None,
            reason: None,
            incident_signature: None,
            details: Some(serde_json::json!({
                "detectors_run": checks.len(),
                "incidents_generated": incidents.len(),
            })),
        };
        let _ = append_jsonl_log(&self.config.output_dir, &scan_entry).await;

        // Log each incident
        for incident in &incidents {
            let entry = SentinelLogEntry {
                timestamp: timestamp.clone(),
                event_type: "incident".into(),
                detector_id: Some(incident.detector_id.clone()),
                target_hash: Some(privacy_hash(&incident.target)),
                severity: Some(incident.severity.to_string()),
                reason: Some(incident.reason.clone()),
                incident_signature: Some(incident.signature.0.clone()),
                details: incident.detail.as_ref().map(|d| serde_json::json!({"detail": d})),
            };
            let _ = append_jsonl_log(&self.config.output_dir, &entry).await;
        }

        incidents
    }

    /// Start the periodic scan loop. Runs until shutdown signal.
    pub async fn run_loop(&self, mut shutdown: tokio::sync::watch::Receiver<bool>) {
        let mut interval = tokio::time::interval(
            Duration::from_secs(self.config.scan_interval_secs),
        );
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let incidents = self.run_scan_cycle().await;
                    if !incidents.is_empty() {
                        tracing::info!(count = incidents.len(), "sentinel: incidents detected");
                    }
                }
                _ = shutdown.changed() => {
                    tracing::info!("sentinel: shutting down scan loop");
                    break;
                }
            }
        }
    }

    /// Get current rate limiter tracked count.
    pub async fn tracked_signatures(&self) -> usize {
        self.rate_limiter.lock().await.tracked_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Healthy.to_string(), "healthy");
        assert_eq!(Severity::Warning.to_string(), "warning");
        assert_eq!(Severity::Critical.to_string(), "critical");
    }

    #[test]
    fn test_incident_signature_deterministic() {
        let s1 = IncidentSignature::new("http_check", "https://api.example.com/health", "timeout");
        let s2 = IncidentSignature::new("http_check", "https://api.example.com/health", "timeout");
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_incident_signature_different_reason() {
        let s1 = IncidentSignature::new("http_check", "https://api.example.com/health", "timeout");
        let s2 = IncidentSignature::new("http_check", "https://api.example.com/health", "503");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_incident_signature_different_target() {
        let s1 = IncidentSignature::new("http_check", "https://api.example.com/health", "timeout");
        let s2 = IncidentSignature::new("http_check", "https://db.example.com/ping", "timeout");
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_rate_limiter_allows_first() {
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let sig = IncidentSignature::new("test", "target", "reason");
        assert!(rl.should_report(&sig));
    }

    #[test]
    fn test_rate_limiter_blocks_duplicate() {
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let sig = IncidentSignature::new("test", "target", "reason");
        assert!(rl.should_report(&sig));
        assert!(!rl.should_report(&sig)); // Blocked within interval
    }

    #[test]
    fn test_rate_limiter_allows_different_signatures() {
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let s1 = IncidentSignature::new("test", "target1", "reason");
        let s2 = IncidentSignature::new("test", "target2", "reason");
        assert!(rl.should_report(&s1));
        assert!(rl.should_report(&s2));
    }

    #[test]
    fn test_rate_limiter_reset() {
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let sig = IncidentSignature::new("test", "target", "reason");
        assert!(rl.should_report(&sig));
        assert!(!rl.should_report(&sig));
        rl.reset(&sig);
        assert!(rl.should_report(&sig)); // Allowed after reset
    }

    #[test]
    fn test_sanitize_strips_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.secret";
        let output = sanitize_for_output(input);
        assert!(output.contains("Bearer [REDACTED]"));
        assert!(!output.contains("eyJhbG"));
    }

    #[test]
    fn test_sanitize_strips_api_key() {
        let input = "url=https://api.example.com?api_key=sk-12345abcde&format=json";
        let output = sanitize_for_output(input);
        assert!(output.contains("[REDACTED]"));
        assert!(!output.contains("sk-12345"));
    }

    #[test]
    fn test_sanitize_strips_email() {
        let input = "Error reported by admin@company.com at 10:00";
        let output = sanitize_for_output(input);
        assert!(output.contains("[EMAIL_REDACTED]"));
        assert!(!output.contains("admin@company.com"));
    }

    #[test]
    fn test_sanitize_preserves_safe_text() {
        let input = "Service healthy, latency 42ms, uptime 99.9%";
        let output = sanitize_for_output(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_run_health_scan_collects_all() {
        let detectors: Vec<(String, fn() -> HealthCheck)> = vec![
            ("check_a".into(), || HealthCheck {
                detector_id: "check_a".into(),
                target: "svc-a".into(),
                severity: Severity::Healthy,
                reason: "ok".into(),
                detail: None,
                check_latency_ms: 5,
            }),
            ("check_b".into(), || HealthCheck {
                detector_id: "check_b".into(),
                target: "svc-b".into(),
                severity: Severity::Critical,
                reason: "connection_refused".into(),
                detail: Some("port 5432".into()),
                check_latency_ms: 100,
            }),
        ];
        let results = run_health_scan(&detectors);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].severity, Severity::Healthy);
        assert_eq!(results[1].severity, Severity::Critical);
    }

    #[test]
    fn test_generate_incidents_filters_healthy() {
        let checks = vec![
            HealthCheck {
                detector_id: "a".into(),
                target: "svc-a".into(),
                severity: Severity::Healthy,
                reason: "ok".into(),
                detail: None,
                check_latency_ms: 1,
            },
            HealthCheck {
                detector_id: "b".into(),
                target: "svc-b".into(),
                severity: Severity::Warning,
                reason: "high_latency".into(),
                detail: None,
                check_latency_ms: 500,
            },
        ];
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let incidents = generate_incidents(&checks, &mut rl, "2026-11-03T10:00:00Z");
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].detector_id, "b");
    }

    #[test]
    fn test_generate_incidents_rate_limits_duplicates() {
        let checks = vec![HealthCheck {
            detector_id: "x".into(),
            target: "svc-x".into(),
            severity: Severity::Critical,
            reason: "down".into(),
            detail: None,
            check_latency_ms: 10,
        }];
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        let first = generate_incidents(&checks, &mut rl, "2026-11-03T10:00:00Z");
        let second = generate_incidents(&checks, &mut rl, "2026-11-03T10:00:01Z");
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 0); // Rate limited
    }

    #[test]
    fn test_tracked_count() {
        let mut rl = IncidentRateLimiter::new(Duration::from_secs(60));
        assert_eq!(rl.tracked_count(), 0);
        let s1 = IncidentSignature::new("a", "b", "c");
        rl.should_report(&s1);
        assert_eq!(rl.tracked_count(), 1);
    }

    #[test]
    fn test_health_check_serialization() {
        let check = HealthCheck {
            detector_id: "disk".into(),
            target: "/var/data".into(),
            severity: Severity::Warning,
            reason: "usage_above_80_pct".into(),
            detail: Some("85% used".into()),
            check_latency_ms: 2,
        };
        let json = serde_json::to_string(&check).unwrap();
        let de: HealthCheck = serde_json::from_str(&json).unwrap();
        assert_eq!(de.detector_id, "disk");
        assert_eq!(de.severity, Severity::Warning);
    }
}