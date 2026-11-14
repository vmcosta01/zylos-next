//! Audit trail primitives: append-only, HMAC-signed event log.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// A single immutable audit entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura AuditEntry.
pub struct AuditEntry {
    /// Campo id.
    pub id: Uuid,
    /// Campo incident id.
    pub incident_id: Uuid,
    /// Campo timestamp.
    pub timestamp: DateTime<Utc>,
    /// Campo actor.
    pub actor: String,
    /// Campo action.
    pub action: String,
    /// Campo parameters.
    pub parameters: serde_json::Value,
    /// Campo result.
    pub result: AuditResult,
    /// Campo previous state hash.
    pub previous_state_hash: Option<String>,
    /// Campo current state hash.
    pub current_state_hash: String,
    /// Campo signature.
    pub signature: String,
}

/// Outcome of an audited action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Enumeração AuditResult.
pub enum AuditResult {
    /// Variante Success.
    Success,
    // Variante Failure.
    Failure(String),
    /// Variante Timeout.
    Timeout,
    // Variante Skipped.
    Skipped(String),
}

/// Append-only audit trail with HMAC-SHA256 signing.
pub struct AuditTrail {
    signing_key: Vec<u8>,
    entries: Vec<AuditEntry>,
}

impl AuditTrail {
    /// Create a new audit trail with the given HMAC signing key.
    pub fn new(signing_key: impl Into<Vec<u8>>) -> Self {
        // Variante Self.
        Self {
            signing_key: signing_key.into(),
            entries: Vec::new(),
        }
    }

    /// Append a signed entry. Returns the entry's ID.
    pub fn append(
        &mut self,
        incident_id: Uuid,
        actor: impl Into<String>,
        action: impl Into<String>,
        parameters: serde_json::Value,
        result: AuditResult,
        current_state_hash: impl Into<String>,
    ) -> Result<Uuid, AuditError> {
        let previous_hash = self.entries.last().map(|e| e.current_state_hash.clone());

        let mut entry = AuditEntry {
            id: Uuid::new_v4(),
            incident_id,
            timestamp: Utc::now(),
            actor: actor.into(),
            action: action.into(),
            parameters,
            result,
            previous_state_hash: previous_hash,
            current_state_hash: current_state_hash.into(),
            signature: String::new(), // placeholder, filled below
        };

        entry.signature = self.sign(&entry)?;
        self.entries.push(entry);

        // Variante Ok.
        Ok(self.entries.last().unwrap().id)
    }

    /// Verify the signature of an entry against this trail's key.
    pub fn verify(&self, entry: &AuditEntry) -> bool {
        match self.sign(entry) {
            // Variante Ok.
            Ok(expected) => expected == entry.signature,
            // Variante Err.
            Err(_) => false,
        }
    }

    /// Return all entries (read-only).
    pub fn entries(&self) -> &[AuditEntry] {
        &self.entries
    }

    /// Compute HMAC-SHA256 over the canonical representation of an entry.
    fn sign(&self, entry: &AuditEntry) -> Result<String, AuditError> {
        let canonical = format!(
            "{}|{}|{}|{}|{}|{:?}|{:?}|{}",
            entry.id,
            entry.incident_id,
            entry.timestamp.to_rfc3339(),
            entry.actor,
            entry.action,
            entry.result,
            entry.previous_state_hash,
            entry.current_state_hash,
        );

        let mut mac =
            HmacSha256::new_from_slice(&self.signing_key).map_err(|_| AuditError::InvalidKey)?;
        mac.update(canonical.as_bytes());
        // Variante Ok.
        Ok(hex::encode(mac.finalize().into_bytes()))
    }
}

/// Errors from audit operations.
#[derive(Debug, thiserror::Error)]
/// Enumeração AuditError.
pub enum AuditError {
    #[error("invalid HMAC signing key")]
    /// Variante InvalidKey.
    InvalidKey,
    #[error("serialization error: {0}")]
    // Variante Serialization.
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_verify() {
        let mut trail = AuditTrail::new(b"test-secret-key");
        let incident = Uuid::new_v4();

        let id = trail
            .append(
                incident,
                "soar-engine",
                "isolate_host",
                serde_json::json!({"host": "web-01"}),
                AuditResult::Success,
                "abc123hash",
            )
            .unwrap();

        let entry = trail.entries().iter().find(|e| e.id == id).unwrap();
        assert!(trail.verify(entry));
        assert_eq!(entry.previous_state_hash, None);
    }

    #[test]
    fn chained_entries_link_hashes() {
        let mut trail = AuditTrail::new(b"key");
        let incident = Uuid::new_v4();

        trail
            .append(
                incident,
                "engine",
                "action_1",
                serde_json::json!({}),
                AuditResult::Success,
                "hash_1",
            )
            .unwrap();

        trail
            .append(
                incident,
                "engine",
                "action_2",
                serde_json::json!({}),
                AuditResult::Success,
                "hash_2",
            )
            .unwrap();

        let entries = trail.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[1].previous_state_hash.as_deref(),
            // Variante Some.
            Some("hash_1")
        );
    }

    #[test]
    fn tampered_entry_fails_verification() {
        let mut trail = AuditTrail::new(b"key");
        let incident = Uuid::new_v4();

        trail
            .append(
                incident,
                "engine",
                "action",
                serde_json::json!({}),
                AuditResult::Success,
                "hash",
            )
            .unwrap();

        let mut tampered = trail.entries()[0].clone();
        tampered.action = "evil_action".to_string();
        assert!(!trail.verify(&tampered));
    }
}
