//! Outcome Uncertainty Tri-State (Task 7C.1.3)
//!
//! Three-valued outcome classification for write operations where network
//! errors or 5xx responses make success/failure indeterminate. OutcomeUnknown
//! forces human confirmation rather than auto-retry. Includes audit logging
//! and notification deduplication.
//!
//! Source: OpenMuse `google.ts:20` (OutcomeUnknownError)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Three-valued outcome for write operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração Outcome.
pub enum Outcome {
    Succeeded,
    Failed,
    OutcomeUnknown,
}

impl Outcome {
    /// Whether this outcome requires human intervention.
    pub fn requires_human_confirmation(&self) -> bool {
        matches!(self, Outcome::OutcomeUnknown)
    }

    /// Whether this outcome is terminal (no retry needed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Outcome::Succeeded | Outcome::Failed)
    }

    /// Serialize to string for storage/logging.
    pub fn to_db(&self) -> &'static str {
        match self {
            Outcome::Succeeded => "succeeded",
            Outcome::Failed => "failed",
            Outcome::OutcomeUnknown => "outcome_unknown",
        }
    }

    /// Parse from string.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "succeeded" => Some(Outcome::Succeeded),
            "failed" => Some(Outcome::Failed),
            "outcome_unknown" => Some(Outcome::OutcomeUnknown),
            _ => None,
        }
    }
}

/// Classification of HTTP/write errors into Outcome tri-state.
#[derive(Debug, Clone)]
/// Estrutura OutcomeClassifier.
pub struct OutcomeClassifier;

impl OutcomeClassifier {
    /// Classify an HTTP status code into an Outcome.
    /// - 2xx → Succeeded
    /// - 4xx → Failed (client error, deterministic)
    /// - 5xx → OutcomeUnknown (server error, may have succeeded)
    /// - Other → OutcomeUnknown
    pub fn from_status(status: u16) -> Outcome {
        match status {
            200..=299 => Outcome::Succeeded,
            400..=499 => Outcome::Failed,
            500..=599 => Outcome::OutcomeUnknown,
            _ => Outcome::OutcomeUnknown,
        }
    }

    /// Classify a network/IO error as OutcomeUnknown.
    /// Network errors mean the write may or may not have reached the server.
    pub fn from_network_error() -> Outcome {
        Outcome::OutcomeUnknown
    }

    /// Classify a timeout as OutcomeUnknown.
    /// Timeout means the server may have processed the request.
    pub fn from_timeout() -> Outcome {
        Outcome::OutcomeUnknown
    }

    /// Classify from a Result where Err = network/IO error.
    pub fn from_result<T, E>(result: &Result<T, E>) -> Outcome {
        match result {
            // Variante Ok.
            Ok(_) => Outcome::Succeeded,
            // Variante Err.
            Err(_) => Outcome::OutcomeUnknown,
        }
    }
}

/// A single audit entry recording an outcome transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura OutcomeAuditEntry.
pub struct OutcomeAuditEntry {
    /// Campo timestamp.
    pub timestamp: DateTime<Utc>,
    /// Campo operation id.
    pub operation_id: String,
    /// Campo previous outcome.
    pub previous_outcome: Option<String>,
    /// Campo new outcome.
    pub new_outcome: String,
    /// Campo reason.
    pub reason: String,
    /// Campo actor.
    pub actor: String,
}

/// Audit log for outcome transitions. Ring buffer with configurable capacity.
pub struct OutcomeAuditLog {
    entries: Arc<Mutex<VecDeque<OutcomeAuditEntry>>>,
    capacity: usize,
}

impl OutcomeAuditLog {
    /// Função new.
    pub fn new(capacity: usize) -> Self {
        // Variante Self.
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Record an outcome transition.
    pub fn record(
        &self,
        operation_id: &str,
        previous: Option<&Outcome>,
        new: &Outcome,
        reason: &str,
        actor: &str,
    ) {
        let entry = OutcomeAuditEntry {
            timestamp: Utc::now(),
            operation_id: operation_id.to_string(),
            previous_outcome: previous.map(|o| o.to_db().to_string()),
            new_outcome: new.to_db().to_string(),
            reason: reason.to_string(),
            actor: actor.to_string(),
        };

        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(entry);
    }

    /// Get all entries (oldest first).
    pub fn entries(&self) -> Vec<OutcomeAuditEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    /// Get entries for a specific operation.
    pub fn entries_for_operation(&self, operation_id: &str) -> Vec<OutcomeAuditEntry> {
        self.entries
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.operation_id == operation_id)
            .cloned()
            .collect()
    }

    /// Count entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }
}

/// Notification deduplication key generator.
/// Prevents duplicate alerts for the same logical operation across retries.
pub struct NotificationDedup;

impl NotificationDedup {
    /// Generate a dedup key from operation components.
    /// Same logical operation → same key → suppress duplicate notifications.
    pub fn compute_key(operation_type: &str, target: &str, payload_hash: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(operation_type.as_bytes());
        hasher.update(b":");
        hasher.update(target.as_bytes());
        hasher.update(b":");
        hasher.update(payload_hash);
        hex::encode(hasher.finalize())
    }

    /// Generate a dedup key from a simple operation descriptor.
    pub fn compute_key_simple(operation_type: &str, target: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(operation_type.as_bytes());
        hasher.update(b":");
        hasher.update(target.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Tracks seen dedup keys to suppress duplicate notifications.
pub struct NotificationTracker {
    seen: Arc<Mutex<std::collections::HashSet<String>>>,
    max_entries: usize,
}

impl NotificationTracker {
    /// Função new.
    pub fn new(max_entries: usize) -> Self {
        // Variante Self.
        Self {
            seen: Arc::new(Mutex::new(std::collections::HashSet::new())),
            max_entries,
        }
    }

    /// Check if a notification should be sent. Returns true if new (not seen).
    /// Marks the key as seen if returning true.
    pub fn should_notify(&self, dedup_key: &str) -> bool {
        let mut seen = self.seen.lock().unwrap();
        if seen.contains(dedup_key) {
            false
        } else {
            // Evict oldest entries if at capacity (simple strategy: clear half)
            if seen.len() >= self.max_entries {
                let to_remove: Vec<_> = seen.iter().take(self.max_entries / 2).cloned().collect();
                for key in to_remove {
                    seen.remove(&key);
                }
            }
            seen.insert(dedup_key.to_string());
            true
        }
    }

    /// Reset tracking (e.g., after manual acknowledgment).
    pub fn reset(&self) {
        self.seen.lock().unwrap().clear();
    }

    /// Number of tracked keys.
    pub fn tracked_count(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

/// Retry policy for outcomes. OutcomeUnknown never auto-retries.
pub struct OutcomeRetryPolicy;

impl OutcomeRetryPolicy {
    /// Determine if an outcome should be auto-retried.
    /// Only Failed outcomes are eligible for auto-retry.
    /// OutcomeUnknown ALWAYS requires human confirmation.
    pub fn should_auto_retry(outcome: &Outcome) -> bool {
        match outcome {
            Outcome::Succeeded => false,      // No retry needed
            Outcome::Failed => true,          // Deterministic failure, safe to retry
            Outcome::OutcomeUnknown => false, // NEVER auto-retry unknown outcomes
        }
    }

    /// Get the recommended action for an outcome.
    pub fn recommended_action(outcome: &Outcome) -> &'static str {
        match outcome {
            Outcome::Succeeded => "none",
            Outcome::Failed => "auto_retry",
            Outcome::OutcomeUnknown => "human_confirmation_required",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2xx_classifies_as_succeeded() {
        assert_eq!(OutcomeClassifier::from_status(200), Outcome::Succeeded);
        assert_eq!(OutcomeClassifier::from_status(201), Outcome::Succeeded);
        assert_eq!(OutcomeClassifier::from_status(204), Outcome::Succeeded);
        assert_eq!(OutcomeClassifier::from_status(299), Outcome::Succeeded);
    }

    #[test]
    fn test_4xx_classifies_as_failed() {
        assert_eq!(OutcomeClassifier::from_status(400), Outcome::Failed);
        assert_eq!(OutcomeClassifier::from_status(401), Outcome::Failed);
        assert_eq!(OutcomeClassifier::from_status(403), Outcome::Failed);
        assert_eq!(OutcomeClassifier::from_status(404), Outcome::Failed);
        assert_eq!(OutcomeClassifier::from_status(422), Outcome::Failed);
        assert_eq!(OutcomeClassifier::from_status(499), Outcome::Failed);
    }

    #[test]
    fn test_5xx_classifies_as_outcome_unknown() {
        assert_eq!(OutcomeClassifier::from_status(500), Outcome::OutcomeUnknown);
        assert_eq!(OutcomeClassifier::from_status(502), Outcome::OutcomeUnknown);
        assert_eq!(OutcomeClassifier::from_status(503), Outcome::OutcomeUnknown);
        assert_eq!(OutcomeClassifier::from_status(504), Outcome::OutcomeUnknown);
        assert_eq!(OutcomeClassifier::from_status(599), Outcome::OutcomeUnknown);
    }

    #[test]
    fn test_timeout_classifies_as_outcome_unknown() {
        assert_eq!(OutcomeClassifier::from_timeout(), Outcome::OutcomeUnknown);
    }

    #[test]
    fn test_network_error_classifies_as_outcome_unknown() {
        assert_eq!(
            OutcomeClassifier::from_network_error(),
            Outcome::OutcomeUnknown
        );
    }

    #[test]
    fn test_result_classification() {
        let ok: Result<i32, String> = Ok(42);
        let err: Result<i32, String> = Err("connection reset".into());

        assert_eq!(OutcomeClassifier::from_result(&ok), Outcome::Succeeded);
        assert_eq!(
            OutcomeClassifier::from_result(&err),
            Outcome::OutcomeUnknown
        );
    }

    #[test]
    fn test_outcome_unknown_requires_human_confirmation() {
        assert!(Outcome::OutcomeUnknown.requires_human_confirmation());
        assert!(!Outcome::Succeeded.requires_human_confirmation());
        assert!(!Outcome::Failed.requires_human_confirmation());
    }

    #[test]
    fn test_outcome_unknown_never_auto_retries() {
        assert!(!OutcomeRetryPolicy::should_auto_retry(
            &Outcome::OutcomeUnknown
        ));
        assert!(OutcomeRetryPolicy::should_auto_retry(&Outcome::Failed));
        assert!(!OutcomeRetryPolicy::should_auto_retry(&Outcome::Succeeded));
    }

    #[test]
    fn test_recommended_action() {
        assert_eq!(
            OutcomeRetryPolicy::recommended_action(&Outcome::OutcomeUnknown),
            "human_confirmation_required"
        );
        assert_eq!(
            OutcomeRetryPolicy::recommended_action(&Outcome::Failed),
            "auto_retry"
        );
        assert_eq!(
            OutcomeRetryPolicy::recommended_action(&Outcome::Succeeded),
            "none"
        );
    }

    #[test]
    fn test_audit_log_records_transitions() {
        let log = OutcomeAuditLog::new(100);

        log.record(
            "op-1",
            None,
            &Outcome::OutcomeUnknown,
            "5xx from API",
            "worker-1",
        );
        log.record(
            "op-1",
            Some(&Outcome::OutcomeUnknown),
            &Outcome::Succeeded,
            "human confirmed success",
            "admin",
        );

        assert_eq!(log.len(), 2);

        let entries = log.entries_for_operation("op-1");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].new_outcome, "outcome_unknown");
        assert_eq!(entries[0].previous_outcome, None);
        assert_eq!(entries[1].new_outcome, "succeeded");
        assert_eq!(
            entries[1].previous_outcome,
            Some("outcome_unknown".to_string())
        );
        assert_eq!(entries[1].actor, "admin");
    }

    #[test]
    fn test_audit_log_ring_buffer_eviction() {
        let log = OutcomeAuditLog::new(3);

        log.record("op-1", None, &Outcome::Succeeded, "ok", "w1");
        log.record("op-2", None, &Outcome::Failed, "bad", "w1");
        log.record("op-3", None, &Outcome::OutcomeUnknown, "5xx", "w1");
        log.record("op-4", None, &Outcome::Succeeded, "ok", "w1");

        // Capacity 3, so op-1 should be evicted
        assert_eq!(log.len(), 3);
        let entries = log.entries();
        assert_eq!(entries[0].operation_id, "op-2");
        assert_eq!(entries[2].operation_id, "op-4");
    }

    #[test]
    fn test_notification_dedup_same_key_suppressed() {
        let tracker = NotificationTracker::new(100);

        let key = NotificationDedup::compute_key_simple("write_email", "user@example.com");

        // First notification: should send
        assert!(tracker.should_notify(&key));

        // Second notification with same key: suppressed
        assert!(!tracker.should_notify(&key));

        // Different key: should send
        let key2 = NotificationDedup::compute_key_simple("write_email", "other@example.com");
        assert!(tracker.should_notify(&key2));
    }

    #[test]
    fn test_notification_dedup_with_payload_hash() {
        let hash1 = Sha256::digest(b"payload-v1");
        let hash2 = Sha256::digest(b"payload-v2");

        let key1 = NotificationDedup::compute_key("send", "target", &hash1);
        let key2 = NotificationDedup::compute_key("send", "target", &hash2);

        // Different payloads produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_notification_tracker_reset() {
        let tracker = NotificationTracker::new(100);
        let key = NotificationDedup::compute_key_simple("op", "target");

        assert!(tracker.should_notify(&key));
        assert!(!tracker.should_notify(&key));

        tracker.reset();

        // After reset, same key should notify again
        assert!(tracker.should_notify(&key));
    }

    #[test]
    fn test_notification_tracker_eviction() {
        let tracker = NotificationTracker::new(4);

        // Fill beyond capacity
        for i in 0..6 {
            let key = format!("key-{}", i);
            assert!(tracker.should_notify(&key));
        }

        // Should have evicted some entries, count <= max_entries
        assert!(tracker.tracked_count() <= 4);
    }

    #[test]
    fn test_outcome_serialization_roundtrip() {
        let outcomes = vec![Outcome::Succeeded, Outcome::Failed, Outcome::OutcomeUnknown];

        for outcome in outcomes {
            let db_str = outcome.to_db();
            let parsed = Outcome::from_db(db_str).unwrap();
            assert_eq!(parsed, outcome);
        }
    }

    #[test]
    fn test_outcome_from_db_invalid_returns_none() {
        assert!(Outcome::from_db("invalid").is_none());
        assert!(Outcome::from_db("").is_none());
    }

    #[test]
    fn test_outcome_is_terminal() {
        assert!(Outcome::Succeeded.is_terminal());
        assert!(Outcome::Failed.is_terminal());
        assert!(!Outcome::OutcomeUnknown.is_terminal());
    }
}
