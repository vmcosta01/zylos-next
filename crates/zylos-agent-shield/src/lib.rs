//! # zylos-agent-shield
//!
//! Security layer for zylos-gateway: PII detection, credential masking,
//! outbound content filtering, and comprehensive audit logging.
//!
//! Provides a middleware-style API that can be inserted into any agent
//! communication pipeline to detect and redact sensitive data before it
//! leaves the system, and to log all security-relevant events.
//!
//! ## Resource Limiting (T8.3.1 + T8.3.2)
//! O módulo `resource_limiter` fornece controle de CPU/memória/PIDs por agente
//! via cgroups v2 (Linux), Docker (cross-platform) ou noop (testes).

pub mod capability_token;
pub mod host_allocator;
pub mod network_egress;
pub mod output_sanitizer;
pub mod resource_limiter;

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// Classification of detected sensitive content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveType {
    Email,
    PhoneNumber,
    Ssn,
    CreditCard,
    ApiKey,
    BearerToken,
    Password,
    AwsAccessKey,
    PrivateKey,
    IpAddress,
    Custom,
}

impl std::fmt::Display for SensitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Email => write!(f, "email"),
            Self::PhoneNumber => write!(f, "phone_number"),
            Self::Ssn => write!(f, "ssn"),
            Self::CreditCard => write!(f, "credit_card"),
            Self::ApiKey => write!(f, "api_key"),
            Self::BearerToken => write!(f, "bearer_token"),
            Self::Password => write!(f, "password"),
            Self::AwsAccessKey => write!(f, "aws_access_key"),
            Self::PrivateKey => write!(f, "private_key"),
            Self::IpAddress => write!(f, "ip_address"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// A single finding from PII/credential scanning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub finding_id: String,
    pub sensitive_type: SensitiveType,
    /// Byte offset in the original text where the match starts.
    pub start: usize,
    /// Byte offset in the original text where the match ends.
    pub end: usize,
    /// The matched text (may be partially masked in logs).
    pub matched_text: String,
    /// Confidence score 0.0–1.0.
    pub confidence: f64,
}

/// Result of scanning a text payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub scan_id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub findings: Vec<Finding>,
    /// The text after all redactions have been applied.
    pub redacted_text: String,
    /// Whether any findings were detected.
    pub has_findings: bool,
}

/// Configuration for the shield scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldConfig {
    /// Enable email detection.
    pub detect_email: bool,
    /// Enable phone number detection.
    pub detect_phone: bool,
    /// Enable SSN detection.
    pub detect_ssn: bool,
    /// Enable credit card detection.
    pub detect_credit_card: bool,
    /// Enable API key / secret detection.
    pub detect_api_keys: bool,
    /// Enable bearer token detection.
    pub detect_bearer_tokens: bool,
    /// Enable password field detection.
    pub detect_passwords: bool,
    /// Enable AWS access key detection.
    pub detect_aws_keys: bool,
    /// Enable private key detection.
    pub detect_private_keys: bool,
    /// Enable IP address detection.
    pub detect_ip_addresses: bool,
    /// Custom regex patterns to scan for.
    pub custom_patterns: Vec<CustomPattern>,
    /// Replacement strategy for redacted content.
    pub redaction_strategy: RedactionStrategy,
}

impl Default for ShieldConfig {
    fn default() -> Self {
        Self {
            detect_email: true,
            detect_phone: true,
            detect_ssn: true,
            detect_credit_card: true,
            detect_api_keys: true,
            detect_bearer_tokens: true,
            detect_passwords: true,
            detect_aws_keys: true,
            detect_private_keys: true,
            detect_ip_addresses: false, // high false-positive rate
            custom_patterns: Vec::new(),
            redaction_strategy: RedactionStrategy::Mask,
        }
    }
}

/// How to replace detected sensitive content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactionStrategy {
    /// Replace with `[REDACTED:<type>]`.
    Mask,
    /// Replace with a deterministic hash placeholder.
    Hash,
    /// Remove entirely.
    Remove,
    /// Replace with a fixed string.
    Fixed(String),
}

/// A user-defined pattern to scan for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPattern {
    /// Human-readable name for this custom pattern.
    pub name: String,
    /// Regular expression pattern to match against.
    pub pattern: String,
    /// Label applied to findings matched by this pattern.
    pub label: String,
}

/// An audit log entry recording a security event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub actor: String,
    pub target: String,
    pub details: String,
    pub severity: Severity,
    pub fingerprint: String,
}

/// Categories of security events recorded in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// Personally identifiable information was detected in a payload.
    PiiDetected,
    /// A credential or secret was blocked from leaving the system.
    CredentialBlocked,
    /// Outbound content was filtered due to policy violations.
    OutboundFiltered,
    /// A configured security policy was violated.
    PolicyViolation,
    /// A scan operation completed successfully.
    ScanCompleted,
    /// The shield configuration was modified at runtime.
    ConfigChanged,
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PiiDetected => write!(f, "pii_detected"),
            Self::CredentialBlocked => write!(f, "credential_blocked"),
            Self::OutboundFiltered => write!(f, "outbound_filtered"),
            Self::PolicyViolation => write!(f, "policy_violation"),
            Self::ScanCompleted => write!(f, "scan_completed"),
            Self::ConfigChanged => write!(f, "config_changed"),
        }
    }
}

/// Severity level for audit log entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational event; no action required.
    Info,
    /// Potential concern; warrants review.
    Warning,
    /// Significant security event; should be investigated.
    High,
    /// Critical security breach or data leak; immediate action required.
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors that can occur during shield operations.
#[derive(Debug, thiserror::Error)]
pub enum ShieldError {
    /// An error occurred while interacting with the SQLite audit database.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    /// A regex pattern failed to compile.
    #[error("Regex compilation error: {0}")]
    Regex(#[from] regex::Error),
    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A filesystem I/O error occurred.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// An internal logic error with a descriptive message.
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Compiled Pattern Set
// ---------------------------------------------------------------------------

struct CompiledPattern {
    sensitive_type: SensitiveType,
    regex: Regex,
    confidence: f64,
}

// ---------------------------------------------------------------------------
// AgentShield
// ---------------------------------------------------------------------------

/// Main security scanner and audit logger.
#[derive(Clone)]
pub struct AgentShield {
    config: Arc<ShieldConfig>,
    patterns: Arc<Vec<CompiledPattern>>,
    pool: Option<SqlitePool>,
}

impl AgentShield {
    /// Creates a new AgentShield with the given configuration.
    /// If `db_path` is provided, audit logs are persisted to SQLite WAL.
    pub async fn new(config: ShieldConfig, db_path: Option<&str>) -> Result<Self, ShieldError> {
        let patterns = compile_patterns(&config)?;
        let pool = if let Some(path) = db_path {
            let options = SqliteConnectOptions::from_str(path)?
                .create_if_missing(true)
                .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
            let pool = SqlitePool::connect_with(options).await?;
            run_audit_migrations(&pool).await?;
            Some(pool)
        } else {
            None
        };

        Ok(Self {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool,
        })
    }

    /// Scans text for PII and credentials, returning findings and redacted text.
    pub fn scan(&self, text: &str, source: &str) -> ScanResult {
        let scan_id = Uuid::new_v4().to_string();
        let mut findings = Vec::new();

        for cp in self.patterns.iter() {
            for mat in cp.regex.find_iter(text) {
                findings.push(Finding {
                    finding_id: Uuid::new_v4().to_string(),
                    sensitive_type: cp.sensitive_type,
                    start: mat.start(),
                    end: mat.end(),
                    matched_text: mask_for_log(mat.as_str()),
                    confidence: cp.confidence,
                });
            }
        }

        // Sort by start position descending so we can replace without offset drift
        findings.sort_by_key(|a| std::cmp::Reverse(a.start));

        let redacted_text = apply_redaction(text, &findings, &self.config.redaction_strategy);

        ScanResult {
            scan_id,
            timestamp: Utc::now(),
            source: source.to_string(),
            has_findings: !findings.is_empty(),
            findings,
            redacted_text,
        }
    }

    /// Checks outbound content and blocks if policy violations are found.
    /// Returns `Ok(Some(scan_result))` if blocked, `Ok(None)` if allowed.
    pub fn check_outbound(
        &self,
        text: &str,
        destination: &str,
    ) -> Result<Option<ScanResult>, ShieldError> {
        let result = self.scan(text, destination);
        if result.has_findings {
            // Log the block event
            if let Some(ref pool) = self.pool {
                let entry = AuditEntry {
                    id: Uuid::new_v4().to_string(),
                    timestamp: Utc::now(),
                    event_type: AuditEventType::OutboundFiltered,
                    actor: "agent-shield".to_string(),
                    target: destination.to_string(),
                    details: format!(
                        "Blocked {} findings: {}",
                        result.findings.len(),
                        result
                            .findings
                            .iter()
                            .map(|f| f.sensitive_type.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    severity: Severity::High,
                    fingerprint: compute_fingerprint(text),
                };
                // Fire-and-forget; audit failure should not block the pipeline
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    let _ = persist_audit(&pool_clone, &entry).await;
                });
            }
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Logs an audit event. Persists to SQLite if configured.
    pub async fn log_audit(&self, entry: &AuditEntry) -> Result<(), ShieldError> {
        if let Some(ref pool) = self.pool {
            persist_audit(pool, entry).await?;
        }
        tracing::info!(
            event_type = %format!("{:?}", entry.event_type),
            severity = %entry.severity,
            actor = %entry.actor,
            target = %entry.target,
            "audit event"
        );
        Ok(())
    }

    /// Returns audit entries matching the given filters.
    pub async fn query_audit(
        &self,
        since: Option<DateTime<Utc>>,
        severity: Option<Severity>,
        limit: usize,
    ) -> Result<Vec<AuditEntry>, ShieldError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| ShieldError::Internal("No database configured".into()))?;

        let mut sql = String::from("SELECT * FROM shield_audit WHERE 1=1");
        if since.is_some() {
            sql.push_str(" AND timestamp >= ?");
        }
        if severity.is_some() {
            sql.push_str(" AND severity = ?");
        }
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");

        let mut query = sqlx::query(&sql);
        if let Some(s) = since {
            query = query.bind(s.to_rfc3339());
        }
        if let Some(sev) = severity {
            query = query.bind(sev.to_string());
        }
        query = query.bind(limit as i64);

        let rows = query.fetch_all(pool).await?;
        Ok(rows.iter().map(row_to_audit_entry).collect())
    }

    /// Returns aggregate statistics about recent scans and findings.
    pub async fn stats(&self) -> Result<ShieldStats, ShieldError> {
        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| ShieldError::Internal("No database configured".into()))?;

        let total_events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM shield_audit")
            .fetch_one(pool)
            .await?;
        let critical_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM shield_audit WHERE severity = 'critical'")
                .fetch_one(pool)
                .await?;
        let high_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM shield_audit WHERE severity = 'high'")
                .fetch_one(pool)
                .await?;
        let outbound_blocked: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM shield_audit WHERE event_type = 'outbound_filtered'",
        )
        .fetch_one(pool)
        .await?;

        Ok(ShieldStats {
            total_audit_events: total_events as usize,
            critical_events: critical_count as usize,
            high_events: high_count as usize,
            outbound_blocked: outbound_blocked as usize,
        })
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &ShieldConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Aggregate statistics about shield audit events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShieldStats {
    /// Total number of audit events recorded.
    pub total_audit_events: usize,
    /// Number of events with `Severity::Critical`.
    pub critical_events: usize,
    /// Number of events with `Severity::High`.
    pub high_events: usize,
    /// Number of outbound messages blocked by the filter.
    pub outbound_blocked: usize,
}

// ---------------------------------------------------------------------------
// Pattern Compilation
// ---------------------------------------------------------------------------

fn compile_patterns(config: &ShieldConfig) -> Result<Vec<CompiledPattern>, ShieldError> {
    let mut patterns = Vec::new();

    if config.detect_email {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::Email,
            regex: Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}")?,
            confidence: 0.95,
        });
    }

    if config.detect_phone {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::PhoneNumber,
            regex: Regex::new(r"(?:\+?\d{1,3}[-.\s]?)?\(?\d{2,4}\)?[-.\s]?\d{3,4}[-.\s]?\d{3,4}")?,
            confidence: 0.7,
        });
    }

    if config.detect_ssn {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::Ssn,
            regex: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b")?,
            confidence: 0.9,
        });
    }

    if config.detect_credit_card {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::CreditCard,
            regex: Regex::new(r"\b(?:\d[ -]*?){13,16}\b")?,
            confidence: 0.8,
        });
    }

    if config.detect_api_keys {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::ApiKey,
            regex: Regex::new(
                r#"(?i)(?:api[_\-]?key|secret[_\-]?key|access[_\-]?key)[\s]*[=:]\s*['"]?[A-Za-z0-9_\-]{16,}['"]?"#,
            )?,
            confidence: 0.9,
        });
    }

    if config.detect_bearer_tokens {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::BearerToken,
            regex: Regex::new(r"(?i)bearer\s+[A-Za-z0-9_\-\.]{20,}")?,
            confidence: 0.95,
        });
    }

    if config.detect_passwords {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::Password,
            regex: Regex::new(
                r#"(?i)(?:password|passwd|pwd|pass)[\s]*[=:]\s*['"]?[^\s'"]{6,}['"]?"#,
            )?,
            confidence: 0.85,
        });
    }

    if config.detect_aws_keys {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::AwsAccessKey,
            regex: Regex::new(r"(?:AKIA|ABIA|ACCA|ASIA)[0-9A-Z]{16}")?,
            confidence: 0.99,
        });
    }

    if config.detect_private_keys {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::PrivateKey,
            regex: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")?,
            confidence: 0.99,
        });
    }

    if config.detect_ip_addresses {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::IpAddress,
            regex: Regex::new(
                r"\b(?:(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\.){3}(?:25[0-5]|2[0-4]\d|[01]?\d\d?)\b",
            )?,
            confidence: 0.6,
        });
    }

    for cp in &config.custom_patterns {
        patterns.push(CompiledPattern {
            sensitive_type: SensitiveType::Custom,
            regex: Regex::new(&cp.pattern)?,
            confidence: 0.8,
        });
    }

    Ok(patterns)
}

// ---------------------------------------------------------------------------
// Redaction
// ---------------------------------------------------------------------------

fn apply_redaction(text: &str, findings: &[Finding], strategy: &RedactionStrategy) -> String {
    if findings.is_empty() {
        return text.to_string();
    }

    // findings are sorted by start desc, so we can replace in-place
    let mut result = text.to_string();
    for f in findings {
        let replacement = match strategy {
            RedactionStrategy::Mask => format!("[REDACTED:{}]", f.sensitive_type),
            RedactionStrategy::Hash => {
                let hash = compute_fingerprint(&result[f.start..f.end]);
                format!("[HASH:{}]", &hash[..8])
            }
            RedactionStrategy::Remove => String::new(),
            RedactionStrategy::Fixed(s) => s.clone(),
        };
        result.replace_range(f.start..f.end, &replacement);
    }
    result
}

fn mask_for_log(text: &str) -> String {
    if text.len() <= 4 {
        "*".repeat(text.len())
    } else {
        let visible = &text[..2];
        let rest = "*".repeat(text.len() - 2);
        format!("{}{}", visible, rest)
    }
}

fn compute_fingerprint(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

// ---------------------------------------------------------------------------
// Audit Persistence
// ---------------------------------------------------------------------------

async fn run_audit_migrations(pool: &SqlitePool) -> Result<(), ShieldError> {
    sqlx::query("CREATE TABLE IF NOT EXISTS shield_audit (id TEXT PRIMARY KEY, timestamp TEXT NOT NULL, event_type TEXT NOT NULL, actor TEXT NOT NULL, target TEXT NOT NULL, details TEXT NOT NULL, severity TEXT NOT NULL, fingerprint TEXT NOT NULL)")
        .execute(pool).await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_shield_audit_ts ON shield_audit(timestamp)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_shield_audit_severity ON shield_audit(severity)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_shield_audit_event ON shield_audit(event_type)")
        .execute(pool)
        .await?;
    Ok(())
}

async fn persist_audit(pool: &SqlitePool, entry: &AuditEntry) -> Result<(), ShieldError> {
    sqlx::query(
        r#"INSERT INTO shield_audit (id, timestamp, event_type, actor, target, details, severity, fingerprint)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&entry.id)
    .bind(entry.timestamp.to_rfc3339())
    .bind(entry.event_type.to_string())
    .bind(&entry.actor)
    .bind(&entry.target)
    .bind(&entry.details)
    .bind(entry.severity.to_string())
    .bind(&entry.fingerprint)
    .execute(pool)
    .await?;
    Ok(())
}

fn row_to_audit_entry(row: &sqlx::sqlite::SqliteRow) -> AuditEntry {
    let event_type_str: String = row.get("event_type");
    let event_type = match event_type_str.as_str() {
        "PiiDetected" => AuditEventType::PiiDetected,
        "CredentialBlocked" => AuditEventType::CredentialBlocked,
        "OutboundFiltered" => AuditEventType::OutboundFiltered,
        "PolicyViolation" => AuditEventType::PolicyViolation,
        "ScanCompleted" => AuditEventType::ScanCompleted,
        "ConfigChanged" => AuditEventType::ConfigChanged,
        _ => AuditEventType::ScanCompleted,
    };
    let severity_str: String = row.get("severity");
    let severity = match severity_str.as_str() {
        "info" => Severity::Info,
        "warning" => Severity::Warning,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::Info,
    };
    AuditEntry {
        id: row.get("id"),
        timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        event_type,
        actor: row.get("actor"),
        target: row.get("target"),
        details: row.get("details"),
        severity,
        fingerprint: row.get("fingerprint"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shield() -> AgentShield {
        let config = ShieldConfig::default();
        let patterns = compile_patterns(&config).unwrap();
        AgentShield {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool: None,
        }
    }

    #[test]
    fn test_detects_email() {
        let shield = test_shield();
        let result = shield.scan("Contact us at user@example.com for help", "test");
        assert!(result.has_findings);
        assert_eq!(result.findings[0].sensitive_type, SensitiveType::Email);
        assert!(result.redacted_text.contains("[REDACTED:email]"));
    }

    #[test]
    fn test_detects_aws_key() {
        let shield = test_shield();
        let result = shield.scan("AWS key: AKIAIOSFODNN7EXAMPLE", "test");
        assert!(result.has_findings);
        assert_eq!(
            result.findings[0].sensitive_type,
            SensitiveType::AwsAccessKey
        );
    }

    #[test]
    fn test_detects_bearer_token() {
        let shield = test_shield();
        let result = shield.scan(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U",
            "test",
        );
        assert!(result.has_findings);
        assert_eq!(
            result.findings[0].sensitive_type,
            SensitiveType::BearerToken
        );
    }

    #[test]
    fn test_no_findings_on_clean_text() {
        let shield = test_shield();
        let result = shield.scan("This is a perfectly normal sentence.", "test");
        assert!(!result.has_findings);
        assert_eq!(result.redacted_text, "This is a perfectly normal sentence.");
    }

    #[test]
    fn test_hash_redaction_strategy() {
        let config = ShieldConfig {
            redaction_strategy: RedactionStrategy::Hash,
            ..Default::default()
        };
        let patterns = compile_patterns(&config).unwrap();
        let shield = AgentShield {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool: None,
        };
        let result = shield.scan("Email: user@example.com", "test");
        assert!(result.redacted_text.contains("[HASH:"));
    }

    #[test]
    fn test_detects_ssn() {
        let shield = test_shield();
        let result = shield.scan("SSN: 123-45-6789", "test");
        assert!(result.has_findings);
        assert_eq!(result.findings[0].sensitive_type, SensitiveType::Ssn);
    }

    #[test]
    fn test_detects_credit_card() {
        let shield = test_shield();
        // Use a format without spaces/dashes to avoid phone regex matching first
        let result = shield.scan("Card: 4111111111111111", "test");
        assert!(result.has_findings);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.sensitive_type == SensitiveType::CreditCard),
            "Expected CreditCard finding, got: {:?}",
            result
                .findings
                .iter()
                .map(|f| f.sensitive_type)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_detects_private_key() {
        let shield = test_shield();
        let result = shield.scan("-----BEGIN RSA PRIVATE KEY-----\nMIIE...", "test");
        assert!(result.has_findings);
        assert_eq!(result.findings[0].sensitive_type, SensitiveType::PrivateKey);
    }

    #[test]
    fn test_custom_pattern_detection() {
        let mut config = ShieldConfig::default();
        config.custom_patterns.push(CustomPattern {
            name: "ticket_id".into(),
            pattern: r"TICKET-\d{4}".into(),
            label: "Support Ticket ID".into(),
        });
        let patterns = compile_patterns(&config).unwrap();
        let shield = AgentShield {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool: None,
        };
        let result = shield.scan("Ref: TICKET-9921", "test");
        assert!(result.has_findings);
        assert_eq!(result.findings[0].sensitive_type, SensitiveType::Custom);
    }

    #[test]
    fn test_remove_redaction_strategy() {
        let config = ShieldConfig {
            redaction_strategy: RedactionStrategy::Remove,
            ..Default::default()
        };
        let patterns = compile_patterns(&config).unwrap();
        let shield = AgentShield {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool: None,
        };
        let result = shield.scan("Email me at user@example.com please", "test");
        assert!(!result.redacted_text.contains("user@example.com"));
        assert!(!result.redacted_text.contains("[REDACTED"));
    }

    #[test]
    fn test_fixed_redaction_strategy() {
        let config = ShieldConfig {
            redaction_strategy: RedactionStrategy::Fixed("***".into()),
            ..Default::default()
        };
        let patterns = compile_patterns(&config).unwrap();
        let shield = AgentShield {
            config: Arc::new(config),
            patterns: Arc::new(patterns),
            pool: None,
        };
        let result = shield.scan("Contact: user@example.com", "test");
        assert!(result.redacted_text.contains("***"));
        assert!(!result.redacted_text.contains("user@example.com"));
    }

    #[test]
    fn test_check_outbound_allows_clean_text() {
        let shield = test_shield();
        let result = shield.check_outbound("Hello world", "dest").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_check_outbound_blocks_sensitive_text() {
        let shield = test_shield();
        let result = shield
            .check_outbound("secret: password=abc123xyz", "dest")
            .unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().has_findings);
    }

    #[test]
    fn test_config_accessor() {
        let shield = test_shield();
        assert!(shield.config().detect_email);
        assert!(!shield.config().detect_ip_addresses);
    }

    #[test]
    fn test_mask_for_log_short() {
        // Short strings (<=4 chars) are fully masked for security
        assert_eq!(mask_for_log("ab"), "**");
        assert_eq!(mask_for_log("abcd"), "****");
        // Longer strings show first 2 chars + rest masked
        assert_eq!(mask_for_log("abcdef"), "ab****");
    }

    #[tokio::test]
    async fn test_audit_persistence() {
        let config = ShieldConfig::default();
        let shield = AgentShield::new(config, Some(":memory:")).await.unwrap();
        let entry = AuditEntry {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            event_type: AuditEventType::PiiDetected,
            actor: "test-agent".to_string(),
            target: "outbound-email".to_string(),
            details: "Found 1 email address".to_string(),
            severity: Severity::Warning,
            fingerprint: "abc123".to_string(),
        };
        shield.log_audit(&entry).await.unwrap();
        let entries = shield.query_audit(None, None, 10).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].actor, "test-agent");
    }

    #[tokio::test]
    async fn test_stats() {
        let config = ShieldConfig::default();
        let shield = AgentShield::new(config, Some(":memory:")).await.unwrap();
        for i in 0..3 {
            let entry = AuditEntry {
                id: Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                event_type: AuditEventType::OutboundFiltered,
                actor: "shield".to_string(),
                target: format!("dest-{}", i),
                details: "blocked".to_string(),
                severity: Severity::High,
                fingerprint: "fp".to_string(),
            };
            shield.log_audit(&entry).await.unwrap();
        }
        let stats = shield.stats().await.unwrap();
        assert_eq!(stats.total_audit_events, 3);
        assert_eq!(stats.high_events, 3);
        assert_eq!(stats.outbound_blocked, 3);
    }
}
