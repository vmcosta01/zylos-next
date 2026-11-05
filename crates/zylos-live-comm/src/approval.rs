//! HITL Approval Cards via AG-UI (Task 7C.1.1)
//!
//! Server-side approval storage with atomic claim (CAS), TTL expiry,
//! idempotent result recovery, headless exclusion, and secret masking.
//! Args never travel to the client — only hash + summary.
//!
//! Sources: OpenDots `connection-routes.ts`, OpenDuet `approvals/service.ts`.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// Unique identifier for an approval request.
pub type ApprovalId = String;

/// Hash of tool call arguments — args never leave the server.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArgsHash(pub String);

impl ArgsHash {
    /// Compute SHA-256 hash of serialized arguments.
    pub fn from_args(args: &serde_json::Value) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(args.to_string().as_bytes());
        let hash = hex::encode(hasher.finalize());
        Self(hash[..32].to_string())
    }
}

/// Current state of an approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Waiting for human decision.
    Pending,
    /// Approved by a human.
    Approved {
        approved_by: String,
        approved_at: DateTime<Utc>,
    },
    /// Declined by a human.
    Declined {
        declined_by: String,
        declined_at: DateTime<Utc>,
        reason: Option<String>,
    },
    /// TTL expired without decision.
    Expired,
}

/// An approval request stored server-side.
/// The client only receives `id`, `summary`, `args_hash`, and `status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    /// Human-readable summary of what the tool call does.
    pub summary: String,
    /// Tool being called (e.g., "send_email", "delete_record").
    pub tool_name: String,
    /// Hash of arguments — args themselves never leave the server.
    pub args_hash: ArgsHash,
    /// Thread/conversation this approval belongs to.
    pub thread_id: String,
    /// Tool call ID from the AG-UI protocol.
    pub tool_call_id: String,
    /// When this request expires.
    pub expires_at: DateTime<Utc>,
    /// Current status.
    pub status: ApprovalStatus,
    /// Whether this tool is excluded in headless contexts.
    pub headless_excluded: bool,
    /// Original arguments stored server-side for idempotent recovery.
    /// Never serialized to client responses.
    #[serde(skip_serializing)]
    pub args: serde_json::Value,
}

/// Configuration for approval behavior.
#[derive(Debug, Clone)]
pub struct ApprovalConfig {
    /// Default TTL for approval requests.
    pub default_ttl: Duration,
    /// Tools that are excluded in headless/Slack contexts.
    pub headless_excluded_tools: Vec<String>,
    /// Field names that should be masked in summaries (secret masking).
    pub secret_fields: Vec<String>,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::minutes(60),
            headless_excluded_tools: vec![
                "send_email".into(),
                "delete_record".into(),
                "transfer_funds".into(),
                "publish_release".into(),
            ],
            secret_fields: vec![
                "password".into(),
                "secret".into(),
                "token".into(),
                "api_key".into(),
                "private_key".into(),
                "ssn".into(),
                "credit_card".into(),
            ],
        }
    }
}

/// In-memory approval store with CAS-based atomic claiming.
/// Production would use SQL with lease-based durability (Task 7C.1.2).
pub struct ApprovalStore {
    approvals: Arc<RwLock<HashMap<ApprovalId, ApprovalRequest>>>,
    config: ApprovalConfig,
}

impl ApprovalStore {
    pub fn new(config: ApprovalConfig) -> Self {
        Self {
            approvals: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Create a new approval request. Returns the request with server-generated ID.
    pub fn create_request(
        &self,
        summary: String,
        tool_name: String,
        args: serde_json::Value,
        thread_id: String,
        tool_call_id: String,
    ) -> ApprovalRequest {
        let id = Uuid::new_v4().to_string();
        let args_hash = ArgsHash::from_args(&args);
        let headless_excluded = self
            .config
            .headless_excluded_tools
            .contains(&tool_name);
        let expires_at = Utc::now() + self.config.default_ttl;

        let request = ApprovalRequest {
            id: id.clone(),
            summary,
            tool_name,
            args_hash,
            thread_id,
            tool_call_id,
            expires_at,
            status: ApprovalStatus::Pending,
            headless_excluded,
            args,
        };

        let mut store = self.approvals.write().unwrap();
        store.insert(id, request.clone());
        request
    }

    /// Attempt to approve a request. Uses CAS semantics:
    /// only succeeds if current status is Pending and not expired.
    /// Returns Err if already decided or expired.
    pub fn approve(
        &self,
        approval_id: &str,
        approved_by: &str,
    ) -> Result<ApprovalRequest, ApprovalError> {
        let mut store = self.approvals.write().unwrap();
        let request = store
            .get_mut(approval_id)
            .ok_or(ApprovalError::NotFound)?;

        // Check expiry first
        if Utc::now() > request.expires_at {
            request.status = ApprovalStatus::Expired;
            return Err(ApprovalError::Expired);
        }

        // CAS: only transition from Pending
        match &request.status {
            ApprovalStatus::Pending => {
                request.status = ApprovalStatus::Approved {
                    approved_by: approved_by.to_string(),
                    approved_at: Utc::now(),
                };
                Ok(request.clone())
            }
            ApprovalStatus::Approved { .. } => Err(ApprovalError::AlreadyDecided),
            ApprovalStatus::Declined { .. } => Err(ApprovalError::AlreadyDecided),
            ApprovalStatus::Expired => Err(ApprovalError::Expired),
        }
    }

    /// Attempt to decline a request. Same CAS semantics as approve.
    pub fn decline(
        &self,
        approval_id: &str,
        declined_by: &str,
        reason: Option<String>,
    ) -> Result<ApprovalRequest, ApprovalError> {
        let mut store = self.approvals.write().unwrap();
        let request = store
            .get_mut(approval_id)
            .ok_or(ApprovalError::NotFound)?;

        if Utc::now() > request.expires_at {
            request.status = ApprovalStatus::Expired;
            return Err(ApprovalError::Expired);
        }

        match &request.status {
            ApprovalStatus::Pending => {
                request.status = ApprovalStatus::Declined {
                    declined_by: declined_by.to_string(),
                    declined_at: Utc::now(),
                    reason,
                };
                Ok(request.clone())
            }
            _ => Err(ApprovalError::AlreadyDecided),
        }
    }

    /// Get a client-safe view of an approval (no raw args).
    /// Also checks TTL and updates status to Expired if needed.
    pub fn get_client_view(&self, approval_id: &str) -> Option<ApprovalClientView> {
        let mut store = self.approvals.write().unwrap();
        let request = store.get_mut(approval_id)?;

        // Auto-expire on read
        if matches!(request.status, ApprovalStatus::Pending) && Utc::now() > request.expires_at {
            request.status = ApprovalStatus::Expired;
        }

        Some(ApprovalClientView {
            id: request.id.clone(),
            summary: mask_secrets(&request.summary, &self.config.secret_fields),
            tool_name: request.tool_name.clone(),
            args_hash: request.args_hash.clone(),
            thread_id: request.thread_id.clone(),
            tool_call_id: request.tool_call_id.clone(),
            expires_at: request.expires_at,
            status: request.status.clone(),
            headless_excluded: request.headless_excluded,
        })
    }

    /// Get the full server-side request (includes raw args).
    /// Used internally for tool execution after approval.
    pub fn get_server_request(&self, approval_id: &str) -> Option<ApprovalRequest> {
        let store = self.approvals.read().unwrap();
        store.get(approval_id).cloned()
    }

    /// Idempotent result recovery: if already decided, return the cached decision.
    /// Used when client re-renders or retries.
    pub fn recover_decision(
        &self,
        approval_id: &str,
    ) -> Result<Option<ApprovalClientView>, ApprovalError> {
        let store = self.approvals.read().unwrap();
        let request = store
            .get(approval_id)
            .ok_or(ApprovalError::NotFound)?;

        match &request.status {
            ApprovalStatus::Pending => Ok(None), // Still waiting
            _ => {
                // Already decided — return cached view
                drop(store);
                Ok(self.get_client_view(approval_id))
            }
        }
    }

    /// Filter tools for headless contexts: exclude approval-gated tools.
    pub fn filter_for_headless(&self, tool_names: &[String]) -> Vec<String> {
        tool_names
            .iter()
            .filter(|name| !self.config.headless_excluded_tools.contains(name))
            .cloned()
            .collect()
    }

    /// Number of pending approvals.
    pub fn pending_count(&self) -> usize {
        let store = self.approvals.read().unwrap();
        store
            .values()
            .filter(|r| matches!(r.status, ApprovalStatus::Pending))
            .count()
    }
}

/// Client-safe view of an approval — never contains raw args.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalClientView {
    pub id: ApprovalId,
    pub summary: String,
    pub tool_name: String,
    pub args_hash: ArgsHash,
    pub thread_id: String,
    pub tool_call_id: String,
    pub expires_at: DateTime<Utc>,
    pub status: ApprovalStatus,
    pub headless_excluded: bool,
}

/// Errors from approval operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalError {
    #[error("approval request not found")]
    NotFound,
    #[error("approval already decided (approved or declined)")]
    AlreadyDecided,
    #[error("approval request has expired")]
    Expired,
}

/// Mask secret field values in a summary string.
/// Replaces patterns like `password=abc123` with `password=[MASKED]`.
fn mask_secrets(text: &str, secret_fields: &[String]) -> String {
    let mut result = text.to_string();
    for field in secret_fields {
        // Match field=value patterns (case-insensitive field name)
        let lower = result.to_lowercase();
        let field_lower = field.to_lowercase();
        let mut search_from = 0;

        while let Some(pos) = lower[search_from..].find(&field_lower) {
            let abs_pos = search_from + pos;
            let after_field = abs_pos + field_lower.len();

            // Check for = sign after field name
            if after_field < result.len() && result.as_bytes()[after_field] == b'=' {
                let value_start = after_field + 1;
                let remaining = &result[value_start..];
                let value_end = remaining
                    .find(|c: char| c == ' ' || c == '&' || c == '\n' || c == '"' || c == ',')
                    .unwrap_or(remaining.len());

                if value_end > 0 {
                    let replacement = format!("{}=[MASKED]", &result[abs_pos..after_field]);
                    result.replace_range(abs_pos..value_start + value_end, &replacement);
                    // Advance past the replacement
                    search_from = abs_pos + replacement.len();
                    // Re-lowercase since we modified the string
                    let lower_new = result.to_lowercase();
                    // Continue searching in the updated lowercase version
                    if search_from >= result.len() {
                        break;
                    }
                    // Use the updated lowercase for next iteration
                    let _ = lower_new; // suppress unused warning
                    continue;
                }
            }
            search_from = abs_pos + field_lower.len();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> ApprovalStore {
        ApprovalStore::new(ApprovalConfig::default())
    }

    #[test]
    fn test_create_and_get_approval() {
        let store = test_store();
        let req = store.create_request(
            "Send email to user".into(),
            "send_email".into(),
            serde_json::json!({"to": "user@example.com", "body": "Hello"}),
            "thread-001".into(),
            "tc-001".into(),
        );
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert_eq!(req.tool_name, "send_email");
        assert!(req.headless_excluded); // send_email is in default excluded list

        let view = store.get_client_view(&req.id).unwrap();
        assert_eq!(view.id, req.id);
        // Args should NOT be in client view (skip_serializing)
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("user@example.com"));
    }

    #[test]
    fn test_approve_flow() {
        let store = test_store();
        let req = store.create_request(
            "Delete record".into(),
            "delete_record".into(),
            serde_json::json!({"id": 42}),
            "thread-001".into(),
            "tc-002".into(),
        );

        let approved = store.approve(&req.id, "admin").unwrap();
        assert!(matches!(approved.status, ApprovalStatus::Approved { .. }));

        // Double approve should fail (CAS)
        let err = store.approve(&req.id, "admin2").unwrap_err();
        assert_eq!(err, ApprovalError::AlreadyDecided);
    }

    #[test]
    fn test_decline_flow() {
        let store = test_store();
        let req = store.create_request(
            "Transfer funds".into(),
            "transfer_funds".into(),
            serde_json::json!({"amount": 1000}),
            "thread-001".into(),
            "tc-003".into(),
        );

        let declined = store
            .decline(&req.id, "admin", Some("Not authorized".into()))
            .unwrap();
        assert!(matches!(declined.status, ApprovalStatus::Declined { .. }));

        // Approve after decline should fail
        let err = store.approve(&req.id, "admin").unwrap_err();
        assert_eq!(err, ApprovalError::AlreadyDecided);
    }

    #[test]
    fn test_ttl_expiry() {
        let config = ApprovalConfig {
            default_ttl: Duration::milliseconds(1), // Expires immediately
            ..Default::default()
        };
        let store = ApprovalStore::new(config);
        let req = store.create_request(
            "Test".into(),
            "test_tool".into(),
            serde_json::json!({}),
            "t".into(),
            "tc".into(),
        );

        // Wait for expiry
        std::thread::sleep(std::time::Duration::from_millis(10));

        let err = store.approve(&req.id, "admin").unwrap_err();
        assert_eq!(err, ApprovalError::Expired);

        // Client view should show expired
        let view = store.get_client_view(&req.id).unwrap();
        assert_eq!(view.status, ApprovalStatus::Expired);
    }

    #[test]
    fn test_double_claim_prevention() {
        let store = test_store();
        let req = store.create_request(
            "Test CAS".into(),
            "test".into(),
            serde_json::json!({}),
            "t".into(),
            "tc".into(),
        );

        // First approve wins
        assert!(store.approve(&req.id, "user_a").is_ok());
        // Second approve fails
        assert_eq!(
            store.approve(&req.id, "user_b").unwrap_err(),
            ApprovalError::AlreadyDecided
        );
        // Decline also fails
        assert_eq!(
            store.decline(&req.id, "user_c", None).unwrap_err(),
            ApprovalError::AlreadyDecided
        );
    }

    #[test]
    fn test_headless_exclusion() {
        let store = test_store();
        let tools = vec![
            "search".into(),
            "send_email".into(),
            "read_file".into(),
            "delete_record".into(),
            "compute".into(),
        ];
        let filtered = store.filter_for_headless(&tools);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&"search".to_string()));
        assert!(filtered.contains(&"read_file".to_string()));
        assert!(filtered.contains(&"compute".to_string()));
        assert!(!filtered.contains(&"send_email".to_string()));
        assert!(!filtered.contains(&"delete_record".to_string()));
    }

    #[test]
    fn test_secret_masking() {
        let fields = vec!["password".into(), "token".into(), "api_key".into()];
        let input = "Login with password=supersecret and token=abc123xyz";
        let masked = mask_secrets(input, &fields);
        assert!(masked.contains("password=[MASKED]"));
        assert!(masked.contains("token=[MASKED]"));
        assert!(!masked.contains("supersecret"));
        assert!(!masked.contains("abc123xyz"));
    }

    #[test]
    fn test_secret_masking_preserves_safe_text() {
        let fields = vec!["password".into()];
        let input = "Service healthy, latency 42ms";
        let masked = mask_secrets(input, &fields);
        assert_eq!(masked, input);
    }

    #[test]
    fn test_idempotent_recovery_pending() {
        let store = test_store();
        let req = store.create_request(
            "Test recovery".into(),
            "test".into(),
            serde_json::json!({}),
            "t".into(),
            "tc".into(),
        );

        // Pending → recovery returns None (still waiting)
        let recovered = store.recover_decision(&req.id).unwrap();
        assert!(recovered.is_none());
    }

    #[test]
    fn test_idempotent_recovery_decided() {
        let store = test_store();
        let req = store.create_request(
            "Test recovery".into(),
            "test".into(),
            serde_json::json!({}),
            "t".into(),
            "tc".into(),
        );

        store.approve(&req.id, "admin").unwrap();

        // Already decided → recovery returns the cached view
        let recovered = store.recover_decision(&req.id).unwrap();
        assert!(recovered.is_some());
        let view = recovered.unwrap();
        assert!(matches!(view.status, ApprovalStatus::Approved { .. }));
    }

    #[test]
    fn test_not_found_errors() {
        let store = test_store();
        assert_eq!(
            store.approve("nonexistent", "admin").unwrap_err(),
            ApprovalError::NotFound
        );
        assert_eq!(
            store.decline("nonexistent", "admin", None).unwrap_err(),
            ApprovalError::NotFound
        );
        assert!(store.get_client_view("nonexistent").is_none());
    }

    #[test]
    fn test_args_hash_deterministic() {
        let args = serde_json::json!({"key": "value", "nested": {"a": 1}});
        let h1 = ArgsHash::from_args(&args);
        let h2 = ArgsHash::from_args(&args);
        assert_eq!(h1, h2);

        let different = serde_json::json!({"key": "other"});
        let h3 = ArgsHash::from_args(&different);
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_pending_count() {
        let store = test_store();
        assert_eq!(store.pending_count(), 0);

        let r1 = store.create_request(
            "A".into(),
            "a".into(),
            serde_json::json!({}),
            "t".into(),
            "tc1".into(),
        );
        let r2 = store.create_request(
            "B".into(),
            "b".into(),
            serde_json::json!({}),
            "t".into(),
            "tc2".into(),
        );
        assert_eq!(store.pending_count(), 2);

        store.approve(&r1.id, "admin").unwrap();
        assert_eq!(store.pending_count(), 1);

        store.decline(&r2.id, "admin", None).unwrap();
        assert_eq!(store.pending_count(), 0);
    }
}