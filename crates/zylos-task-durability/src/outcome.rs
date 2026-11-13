//! Outcome Uncertainty Tri-State — Task 7C.1.3
//!
//! Implements three-valued outcome classification for external operations:
//! - `Succeeded`: operation confirmed complete (2xx, write acknowledged)
//! - `Failed`: operation confirmed failed (4xx client error, validation error)
//! - `OutcomeUnknown`: operation may or may not have completed (5xx, timeout,
//!   network error) — requires human confirmation before proceeding
//!
//! Source: OpenMuse `google.ts:20` (OutcomeUnknownError pattern)
//!
//! Key invariant: a write that fails with a network error MUST NEVER be
//! classified as Failed — it might have succeeded server-side. Only
//! explicit rejection (4xx, validation) is safe to classify as Failed.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Three-valued outcome for external operations.
///
/// The critical insight: when a write operation fails due to network issues,
/// we cannot know whether the server processed it. Classifying this as `Failed`
/// risks data loss (retry creates duplicates); classifying as `Succeeded` risks
/// silent failures. `OutcomeUnknown` forces human verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Outcome {
    /// Operation confirmed successful (2xx response, write acknowledged).
    Succeeded = 0,
    /// Operation confirmed failed (4xx client error, validation rejection).
    /// Safe to retry without human intervention.
    Failed = 1,
    /// Operation outcome uncertain (5xx, timeout, network error, DNS failure).
    /// MUST NOT auto-retry — requires human confirmation.
    OutcomeUnknown = 2,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Succeeded => write!(f, "succeeded"),
            Outcome::Failed => write!(f, "failed"),
            Outcome::OutcomeUnknown => write!(f, "outcome_unknown"),
        }
    }
}

impl Outcome {
    /// Whether this outcome allows automatic retry without human confirmation.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Outcome::Failed)
    }

    /// Whether this outcome requires human confirmation before proceeding.
    pub fn requires_human_confirmation(&self) -> bool {
        matches!(self, Outcome::OutcomeUnknown)
    }

    /// Whether the operation is definitively complete (success or failure).
    pub fn is_terminal(&self) -> bool {
        !matches!(self, Outcome::OutcomeUnknown)
    }
}

/// Classification of an HTTP/network error into an Outcome.
///
/// This is the core decision function. It must be conservative:
/// when in doubt, return `OutcomeUnknown`.
pub fn classify_error(error: &OperationError) -> Outcome {
    match error {
        // Explicit server rejection → safe to classify as Failed
        OperationError::HttpStatus(code) if *code >= 400 && *code < 500 => Outcome::Failed,

        // Server error → might have processed the request → Unknown
        OperationError::HttpStatus(code) if *code >= 500 => Outcome::OutcomeUnknown,

        // Success range
        OperationError::HttpStatus(code) if *code >= 200 && *code < 300 => Outcome::Succeeded,

        // Any other HTTP status (1xx, 3xx, or unexpected ranges) → Unknown
        OperationError::HttpStatus(_) => Outcome::OutcomeUnknown,

        // Network-level errors → always Unknown (request may have reached server)
        OperationError::Timeout => Outcome::OutcomeUnknown,
        OperationError::ConnectionReset => Outcome::OutcomeUnknown,
        OperationError::DnsFailure => Outcome::OutcomeUnknown,
        OperationError::TlsError => Outcome::OutcomeUnknown,
        OperationError::IoError(_) => Outcome::OutcomeUnknown,

        // Client-side validation errors → safe to classify as Failed
        // (request never left the client)
        OperationError::Validation(_) => Outcome::Failed,
        OperationError::Serialization(_) => Outcome::Failed,

        // Catch-all: unknown error types → conservative Unknown
        OperationError::Other(_) => Outcome::OutcomeUnknown,
    }
}

/// Errors from external operations, categorized for outcome classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationError {
    /// HTTP response with non-success status code.
    HttpStatus(u16),
    /// Request timed out (server may have processed it).
    Timeout,
    /// Connection reset by peer (server may have processed it).
    ConnectionReset,
    /// DNS resolution failure (request never left client).
    DnsFailure,
    /// TLS handshake failure.
    TlsError,
    /// Generic I/O error.
    IoError(String),
    /// Client-side validation error (request never sent).
    Validation(String),
    /// Serialization/deserialization error.
    Serialization(String),
    /// Uncategorized error.
    Other(String),
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OperationError::HttpStatus(code) => write!(f, "HTTP {code}"),
            OperationError::Timeout => write!(f, "request timeout"),
            OperationError::ConnectionReset => write!(f, "connection reset"),
            OperationError::DnsFailure => write!(f, "DNS resolution failed"),
            OperationError::TlsError => write!(f, "TLS handshake failed"),
            OperationError::IoError(msg) => write!(f, "I/O error: {msg}"),
            OperationError::Validation(msg) => write!(f, "validation error: {msg}"),
            OperationError::Serialization(msg) => write!(f, "serialization error: {msg}"),
            OperationError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for OperationError {}

/// A deduplication key for notifications about outcome transitions.
///
/// Prevents spamming the user with duplicate alerts when the same
/// operation is retried multiple times with the same unknown outcome.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationDedupKey {
    /// The operation being performed (e.g., "write_email", "update_calendar").
    pub operation_id: String,
    /// The target resource (e.g., email ID, calendar event ID).
    pub target_id: String,
    /// The outcome that triggered the notification.
    pub outcome: Outcome,
}

impl NotificationDedupKey {
    pub fn new(operation_id: impl Into<String>, target_id: impl Into<String>, outcome: Outcome) -> Self {
        Self {
            operation_id: operation_id.into(),
            target_id: target_id.into(),
            outcome,
        }
    }
}

/// An audit log entry recording an outcome transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAuditEntry {
    /// Unique ID for this audit entry.
    pub id: String,
    /// The operation that produced this outcome.
    pub operation_id: String,
    /// The target resource.
    pub target_id: String,
    /// The previous outcome (None for initial classification).
    pub previous_outcome: Option<Outcome>,
    /// The new outcome after classification.
    pub new_outcome: Outcome,
    /// The error that caused the transition (if any).
    pub error: Option<OperationError>,
    /// When this transition occurred.
    pub timestamp: DateTime<Utc>,
    /// Who or what triggered the transition.
    pub actor: String,
}

/// Tracks outcome state and enforces the tri-state protocol.
pub struct OutcomeTracker {
    /// Current outcome for each tracked operation.
    states: std::collections::HashMap<String, TrackedOutcome>,
    /// Audit log of all transitions.
    audit_log: Vec<OutcomeAuditEntry>,
    /// Recently sent notification keys (for dedup).
    recent_notifications: std::collections::HashSet<NotificationDedupKey>,
    /// Maximum age for notification dedup entries (seconds).
    #[allow(dead_code)]
    dedup_window_secs: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TrackedOutcome {
    outcome: Outcome,
    last_updated: DateTime<Utc>,
    retry_count: u32,
}

impl OutcomeTracker {
    pub fn new(dedup_window_secs: u64) -> Self {
        Self {
            states: std::collections::HashMap::new(),
            audit_log: Vec::new(),
            recent_notifications: std::collections::HashSet::new(),
            dedup_window_secs,
        }
    }

    /// Record the outcome of an operation. Returns whether a notification
    /// should be sent (false if deduplicated).
    pub fn record_outcome(
        &mut self,
        operation_id: &str,
        target_id: &str,
        error: Option<&OperationError>,
        actor: &str,
    ) -> (Outcome, bool) {
        let outcome = match error {
            Some(err) => classify_error(err),
            None => Outcome::Succeeded,
        };

        let key = operation_id.to_string();
        let now = Utc::now();

        let previous = self.states.get(&key).map(|s| s.outcome);
        let retry_count = self.states.get(&key).map_or(0, |s| s.retry_count + 1);

        self.states.insert(key.clone(), TrackedOutcome {
            outcome,
            last_updated: now,
            retry_count,
        });

        // Audit log
        self.audit_log.push(OutcomeAuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            operation_id: operation_id.to_string(),
            target_id: target_id.to_string(),
            previous_outcome: previous,
            new_outcome: outcome,
            error: error.cloned(),
            timestamp: now,
            actor: actor.to_string(),
        });

        // Notification dedup
        let dedup_key = NotificationDedupKey::new(operation_id, target_id, outcome);
        let should_notify = self.recent_notifications.insert(dedup_key);

        (outcome, should_notify)
    }

    /// Get the current outcome for an operation.
    pub fn get_outcome(&self, operation_id: &str) -> Option<Outcome> {
        self.states.get(operation_id).map(|s| s.outcome)
    }

    /// Check if an operation requires human confirmation.
    pub fn needs_human_confirmation(&self, operation_id: &str) -> bool {
        self.states.get(operation_id)
            .is_some_and(|s| s.outcome.requires_human_confirmation())
    }

    /// Get the audit log (most recent first).
    pub fn audit_log(&self) -> &[OutcomeAuditEntry] {
        &self.audit_log
    }

    /// Clear stale notification dedup entries older than the window.
    pub fn prune_dedup_cache(&mut self) {
        // In production, track timestamps per entry. For now, clear all
        // when called (typically on a timer).
        self.recent_notifications.clear();
    }

    /// Number of tracked operations.
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_2xx_as_succeeded() {
        assert_eq!(classify_error(&OperationError::HttpStatus(200)), Outcome::Succeeded);
        assert_eq!(classify_error(&OperationError::HttpStatus(201)), Outcome::Succeeded);
        assert_eq!(classify_error(&OperationError::HttpStatus(204)), Outcome::Succeeded);
    }

    #[test]
    fn test_classify_4xx_as_failed() {
        assert_eq!(classify_error(&OperationError::HttpStatus(400)), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::HttpStatus(401)), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::HttpStatus(403)), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::HttpStatus(404)), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::HttpStatus(422)), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::HttpStatus(429)), Outcome::Failed);
    }

    #[test]
    fn test_classify_5xx_as_unknown() {
        assert_eq!(classify_error(&OperationError::HttpStatus(500)), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::HttpStatus(502)), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::HttpStatus(503)), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::HttpStatus(504)), Outcome::OutcomeUnknown);
    }

    #[test]
    fn test_classify_network_errors_as_unknown() {
        assert_eq!(classify_error(&OperationError::Timeout), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::ConnectionReset), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::DnsFailure), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::TlsError), Outcome::OutcomeUnknown);
        assert_eq!(classify_error(&OperationError::IoError("broken pipe".into())), Outcome::OutcomeUnknown);
    }

    #[test]
    fn test_classify_client_errors_as_failed() {
        assert_eq!(classify_error(&OperationError::Validation("missing field".into())), Outcome::Failed);
        assert_eq!(classify_error(&OperationError::Serialization("invalid JSON".into())), Outcome::Failed);
    }

    #[test]
    fn test_classify_other_as_unknown() {
        assert_eq!(classify_error(&OperationError::Other("something weird".into())), Outcome::OutcomeUnknown);
    }

    #[test]
    fn test_no_error_is_succeeded() {
        let mut tracker = OutcomeTracker::new(300);
        let (outcome, _) = tracker.record_outcome("op1", "target1", None, "agent-1");
        assert_eq!(outcome, Outcome::Succeeded);
    }

    #[test]
    fn test_outcome_properties() {
        assert!(Outcome::Succeeded.is_terminal());
        assert!(!Outcome::Succeeded.requires_human_confirmation());
        assert!(!Outcome::Succeeded.is_retryable());

        assert!(Outcome::Failed.is_terminal());
        assert!(!Outcome::Failed.requires_human_confirmation());
        assert!(Outcome::Failed.is_retryable());

        assert!(!Outcome::OutcomeUnknown.is_terminal());
        assert!(Outcome::OutcomeUnknown.requires_human_confirmation());
        assert!(!Outcome::OutcomeUnknown.is_retryable());
    }

    #[test]
    fn test_tracker_records_audit_log() {
        let mut tracker = OutcomeTracker::new(300);

        tracker.record_outcome("write_email", "msg-123", Some(&OperationError::HttpStatus(500)), "agent-1");
        tracker.record_outcome("write_email", "msg-123", Some(&OperationError::HttpStatus(200)), "agent-1");

        assert_eq!(tracker.audit_log().len(), 2);
        assert_eq!(tracker.audit_log()[0].new_outcome, Outcome::OutcomeUnknown);
        assert_eq!(tracker.audit_log()[1].new_outcome, Outcome::Succeeded);
        assert_eq!(tracker.audit_log()[1].previous_outcome, Some(Outcome::OutcomeUnknown));
    }

    #[test]
    fn test_notification_dedup() {
        let mut tracker = OutcomeTracker::new(300);

        let (_, notify1) = tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");
        let (_, notify2) = tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");

        assert!(notify1, "First notification should be sent");
        assert!(!notify2, "Duplicate notification should be suppressed");
    }

    #[test]
    fn test_different_outcomes_not_deduped() {
        let mut tracker = OutcomeTracker::new(300);

        let (_, n1) = tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");
        let (_, n2) = tracker.record_outcome("op1", "t1", Some(&OperationError::HttpStatus(200)), "a1");

        assert!(n1);
        assert!(n2, "Different outcome should trigger new notification");
    }

    #[test]
    fn test_needs_human_confirmation() {
        let mut tracker = OutcomeTracker::new(300);

        tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");
        assert!(tracker.needs_human_confirmation("op1"));

        tracker.record_outcome("op2", "t2", Some(&OperationError::HttpStatus(400)), "a1");
        assert!(!tracker.needs_human_confirmation("op2"));

        assert!(!tracker.needs_human_confirmation("nonexistent"));
    }

    #[test]
    fn test_display_formatting() {
        assert_eq!(format!("{}", Outcome::Succeeded), "succeeded");
        assert_eq!(format!("{}", Outcome::Failed), "failed");
        assert_eq!(format!("{}", Outcome::OutcomeUnknown), "outcome_unknown");

        assert_eq!(format!("{}", OperationError::HttpStatus(503)), "HTTP 503");
        assert_eq!(format!("{}", OperationError::Timeout), "request timeout");
        assert_eq!(format!("{}", OperationError::Validation("bad input".into())), "validation error: bad input");
    }

    #[test]
    fn test_prune_dedup_cache() {
        let mut tracker = OutcomeTracker::new(300);
        tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");
        tracker.prune_dedup_cache();

        // After pruning, same notification should be sent again
        let (_, notify) = tracker.record_outcome("op1", "t1", Some(&OperationError::Timeout), "a1");
        assert!(notify, "After cache prune, notification should be sent again");
    }
}