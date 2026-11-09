//! zylos-hitl: Human-in-the-loop approval cards via AG-UI protocol.
//!
//! # Features (Task 7C.1.1 + 7C.2.2)
//!
//! - `ApprovalRequest` type with id, summary, tool_call, args_hash, ttl
//! - Server-side SQLite storage (`approval_cards` table)
//! - Blake3-equivalence hashing for approval reuse (SHA-256 until blake3 dep added)
//! - Four-tier decision: safety > team_rule > personal_rule > auto_review > default
//! - 7 standardized refusal messages
//! - `carry_on()` for multi-step approval continuation
//! - `withdraw_allowed()` and `abandon()` lifecycle methods
//! - `is_permanent()` for structural error detection
//! - WASM policy sandbox stub
//! - EventBus integration (publishes `ApprovalRequested`)
//!
//! # Outcome Uncertainty (Task 7C.1.3)
//!
//! Three-state outcome classification for write operations where network errors
//! or 5xx responses make it impossible to determine success vs failure.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use uuid::Uuid;

// ─── Outcome Uncertainty (Task 7C.1.3) ──────────────────────────────────────

/// Three-state outcome for write operations.
///
/// Used when network errors or ambiguous HTTP responses make it impossible
/// to determine whether a side-effecting operation succeeded or failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    /// The operation completed successfully.
    Succeeded,
    /// The operation definitively failed (e.g. 4xx, validation error).
    Failed,
    /// The outcome cannot be determined (e.g. 5xx, network timeout).
    OutcomeUnknown,
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

/// Classification result from an HTTP response or operation.
///
/// Bundles an [`Outcome`] with a machine-readable reason tag and optional
/// human-readable detail for logging and UI feedback.
///
/// # Examples
///
/// ```
/// use zylos_hitl::{classify_http_status, Outcome};
/// let verdict = classify_http_status(200);
/// assert_eq!(verdict.outcome, Outcome::Succeeded);
/// assert_eq!(verdict.reason, "http_200");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeVerdict {
    /// The classified outcome.
    pub outcome: Outcome,
    /// Machine-readable reason tag (e.g. `http_200`, `network_timeout`).
    pub reason: String,
    /// Optional human-readable detail.
    pub detail: Option<String>,
}

/// Classify an HTTP status code into an [`OutcomeVerdict`].
///
/// - 2xx → [`Outcome::Succeeded`]
/// - 4xx → [`Outcome::Failed`]
/// - 5xx → [`Outcome::OutcomeUnknown`]
/// - other → [`Outcome::OutcomeUnknown`] with `_unexpected` suffix
///
/// # Examples
///
/// ```
/// use zylos_hitl::{classify_http_status, Outcome};
/// assert_eq!(classify_http_status(404).outcome, Outcome::Failed);
/// assert_eq!(classify_http_status(503).outcome, Outcome::OutcomeUnknown);
/// ```
pub fn classify_http_status(status: u16) -> OutcomeVerdict {
    let (outcome, reason) = match status {
        200..=299 => (Outcome::Succeeded, format!("http_{status}")),
        400..=499 => (Outcome::Failed, format!("http_{status}")),
        500..=599 => (Outcome::OutcomeUnknown, format!("http_{status}")),
        _ => (Outcome::OutcomeUnknown, format!("http_{status}_unexpected")),
    };
    OutcomeVerdict { outcome, reason, detail: None }
}

/// Classify a network-level error as [`Outcome::OutcomeUnknown`].
///
/// Network errors are ambiguous — the request may have reached the server
/// but the response was lost, so retry may be safe.
///
/// # Examples
///
/// ```
/// use zylos_hitl::{classify_network_error, Outcome};
/// let v = classify_network_error("timeout");
/// assert_eq!(v.outcome, Outcome::OutcomeUnknown);
/// assert_eq!(v.reason, "network_timeout");
/// ```
pub fn classify_network_error(error_kind: &str) -> OutcomeVerdict {
    OutcomeVerdict {
        outcome: Outcome::OutcomeUnknown,
        reason: format!("network_{error_kind}"),
        detail: None,
    }
}

/// Classify a client-side validation error as [`Outcome::Failed`].
///
/// Validation failures are definitive — retrying with the same input will
/// produce the same result.
///
/// # Examples
///
/// ```
/// use zylos_hitl::{classify_validation_error, Outcome};
/// let v = classify_validation_error("missing_field");
/// assert_eq!(v.outcome, Outcome::Failed);
/// ```
pub fn classify_validation_error(reason: impl Into<String>) -> OutcomeVerdict {
    OutcomeVerdict {
        outcome: Outcome::Failed,
        reason: reason.into(),
        detail: None,
    }
}

/// Deduplication key for outcome notifications.
///
/// Combines `operation_id` and [`Outcome`] into a stable SHA-256 prefix
/// to prevent duplicate alerts for the same operation+outcome pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationDedupKey(pub String);

impl NotificationDedupKey {
    /// Create a new dedup key from an operation ID and outcome.
    ///
    /// Returns a 16-character hex prefix of the SHA-256 hash.
    pub fn new(operation_id: &str, outcome: Outcome) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(operation_id.as_bytes());
        hasher.update(b":");
        hasher.update(outcome.to_string().as_bytes());
        let hash = hex::encode(hasher.finalize());
        Self(hash[..16].to_string())
    }
}

/// Audit log entry recording an outcome transition.
///
/// Stored for compliance and debugging to track how and why an operation's
/// outcome changed over time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAuditEntry {
    /// RFC 3339 timestamp of the transition.
    pub timestamp: String,
    /// Identifier of the operation whose outcome changed.
    pub operation_id: String,
    /// Previous outcome, if any.
    pub previous_outcome: Option<Outcome>,
    /// New outcome after the transition.
    pub new_outcome: Outcome,
    /// Machine-readable reason for the change.
    pub reason: String,
    /// Identity of the actor or system that decided the new outcome.
    pub decided_by: String,
}

// ─── HITL Approval Types (Task 7C.1.1 + 7C.2.2) ───────────────────────────

/// Four-tier decision hierarchy: safety > team_rule > personal_rule > auto_review > default.
///
/// Tiers are evaluated in order by [`decide_action`]; the first tier that
/// returns a decision wins. Higher tiers override lower ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionTier {
    /// Highest priority: hard safety constraints.
    Safety,
    /// Team-wide policy rules.
    TeamRule,
    /// User-specific personal rules.
    PersonalRule,
    /// Automated review heuristics.
    AutoReview,
    /// Fallback when no higher tier matches.
    Default,
}

impl fmt::Display for DecisionTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecisionTier::Safety => write!(f, "safety"),
            DecisionTier::TeamRule => write!(f, "team_rule"),
            DecisionTier::PersonalRule => write!(f, "personal_rule"),
            DecisionTier::AutoReview => write!(f, "auto_review"),
            DecisionTier::Default => write!(f, "default"),
        }
    }
}

/// The action decided by the approval system.
///
/// Returned by [`decide_action`] and stored on [`ApprovalRequest`] after claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// Allow the requested action as-is.
    Approve,
    /// Deny the requested action.
    Reject,
    /// Allow with modified arguments (JSON patch/value).
    Modify(serde_json::Value),
    /// Escalate to a human reviewer with a reason string.
    Escalate(String),
}

impl fmt::Display for ApprovalDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApprovalDecision::Approve => write!(f, "approve"),
            ApprovalDecision::Reject => write!(f, "reject"),
            ApprovalDecision::Modify(_) => write!(f, "modify"),
            ApprovalDecision::Escalate(reason) => write!(f, "escalate:{reason}"),
        }
    }
}

/// Seven standardized refusal messages for clear UX feedback.
///
/// Each variant maps to a user-facing message via [`RefusalReason::message`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    /// A person explicitly declined the action.
    DeclinedByPerson,
    /// Another approval request is already pending for this action.
    AlreadyWaiting,
    /// Withdrawn because approvals were disabled globally.
    WithdrawnApprovalsOff,
    /// Withdrawn because a rule now allows the action without approval.
    WithdrawnRuleAllows,
    /// The action was already handled by a person.
    HandledByPerson,
    /// Cannot approve while the step is still running.
    StepStillRunning,
    /// Cannot approve because the current step has unknown outcome.
    OutcomeUnknownInStep,
}

impl RefusalReason {
    /// Return the user-facing message for this refusal reason.
    pub fn message(&self) -> &'static str {
        match self {
            Self::DeclinedByPerson => "Action declined by person",
            Self::AlreadyWaiting => "An approval request is already pending for this action",
            Self::WithdrawnApprovalsOff => "Approval withdrawn: approvals disabled",
            Self::WithdrawnRuleAllows => "Approval withdrawn: a rule now allows this action",
            Self::HandledByPerson => "Action was already handled by a person",
            Self::StepStillRunning => "Cannot approve: step is still running",
            Self::OutcomeUnknownInStep => "Cannot approve: outcome unknown in current step",
        }
    }
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// Status of an approval card.
///
/// Transitions: `Pending` → `Approved` | `Rejected` | `Expired` | `Withdrawn` | `Abandoned`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    /// Awaiting a decision.
    Pending,
    /// Approved by a person or policy tier.
    Approved,
    /// Rejected by a person or policy tier.
    Rejected,
    /// TTL expired without a decision.
    Expired,
    /// Withdrawn because a rule changed or approvals were disabled.
    Withdrawn,
    /// Abandoned by the requester or system.
    Abandoned,
}

impl fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
            Self::Expired => write!(f, "expired"),
            Self::Withdrawn => write!(f, "withdrawn"),
            Self::Abandoned => write!(f, "abandoned"),
        }
    }
}

/// An approval request in the AG-UI protocol.
///
/// Args never travel to the client — only the equivalence hash and summary
/// are exposed. The full argument set is reconstructed server-side on claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// Human-readable summary shown in the approval card UI.
    pub summary: String,
    /// Name or reference of the tool being invoked.
    pub tool_call: String,
    /// Equivalence hash of normalized args for approval reuse.
    pub args_hash: String,
    /// Time-to-live in seconds. Default: 3600 (60 min).
    pub ttl_secs: u64,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// When the request expires if not yet decided.
    pub expires_at: DateTime<Utc>,
    /// Conversation/thread this request belongs to.
    pub thread_id: String,
    /// Identity of the actor requesting approval.
    pub requester_id: String,
    /// Current lifecycle status.
    pub status: ApprovalStatus,
    /// The decision, once claimed.
    pub decision: Option<ApprovalDecision>,
    /// When the decision was made.
    pub decided_at: Option<DateTime<Utc>>,
    /// Which policy tier produced the decision.
    pub decided_by_tier: Option<DecisionTier>,
    /// Why the request was refused, if applicable.
    pub refusal_reason: Option<RefusalReason>,
}

// ─── Equivalence Hashing (Task 7C.2.2) ─────────────────────────────────────

/// Compute equivalence hash for approval reuse.
///
/// Uses SHA-256 (blake3 not yet in workspace deps; swap when added).
///
/// Volatile fields (`snapshot_id`, `ref`, `tool_call_id`, `signal`) are
/// stripped and object keys are sorted so semantically identical requests
/// produce the same hash regardless of field ordering or transient metadata.
///
/// # Examples
///
/// ```
/// use zylos_hitl::compute_equivalence_hash;
/// let h = compute_equivalence_hash("actor", "bot", "send_email", "write", "global",
///     &serde_json::json!({"to": "a@b.com"}),
///     &serde_json::json!({"body": "hi"}));
/// assert_eq!(h.len(), 64); // SHA-256 hex
/// ```
pub fn compute_equivalence_hash(
    actor_id: &str,
    bot_id: &str,
    tool_ref: &str,
    effect: &str,
    scope: &str,
    target: &serde_json::Value,
    args: &serde_json::Value,
) -> String {
    let stable_target = stabilize_value(target, 0);
    let stable_args = stabilize_value(args, 0);

    let mut hasher = Sha256::new();
    hasher.update(actor_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bot_id.as_bytes());
    hasher.update(b"|");
    hasher.update(tool_ref.as_bytes());
    hasher.update(b"|");
    hasher.update(effect.as_bytes());
    hasher.update(b"|");
    hasher.update(scope.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_target.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_args.as_bytes());

    hex::encode(hasher.finalize())
}

/// Strip volatile fields and sort keys recursively (depth limit 6).
fn stabilize_value(value: &serde_json::Value, depth: usize) -> String {
    if depth >= 6 {
        return serde_json::to_string(value).unwrap_or_default();
    }

    const VOLATILE_KEYS: &[&str] = &["snapshot_id", "ref", "tool_call_id", "signal"];

    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<_> = map
                .iter()
                .filter(|(k, _)| !VOLATILE_KEYS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), stabilize_value(v, depth + 1)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let inner: Vec<String> = entries
                .into_iter()
                .map(|(k, v)| format!("{k}:{v}"))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr
                .iter()
                .map(|v| stabilize_value(v, depth + 1))
                .collect();
            format!("[{}]", items.join(","))
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ─── Policy Tier Trait (Task 7C.2.2) ───────────────────────────────────────

/// Trait for implementing custom policy tiers.
///
/// Implementors are evaluated in order by [`decide_action`]; return `None`
/// to defer to the next tier, or `Some(decision)` to short-circuit.
pub trait PolicyTier: Send + Sync {
    /// Which tier level this policy occupies.
    fn tier(&self) -> DecisionTier;
    /// Evaluate the request. Return `Some(decision)` to decide, `None` to pass.
    fn evaluate(&self, request: &ApprovalRequest) -> Option<ApprovalDecision>;
}

/// Default policy tier that always approves (fallback / testing).
///
/// Use as the last tier in a chain to ensure every request gets a decision.
pub struct DefaultPolicyTier;

impl PolicyTier for DefaultPolicyTier {
    fn tier(&self) -> DecisionTier { DecisionTier::Default }
    fn evaluate(&self, _request: &ApprovalRequest) -> Option<ApprovalDecision> {
        Some(ApprovalDecision::Approve)
    }
}

/// Decide action through the four-tier hierarchy.
///
/// Iterates `tiers` in order; the first tier returning `Some` wins.
/// If no tier matches, returns `Escalate("no_policy_match")` at [`DecisionTier::Default`].
///
/// # Examples
///
/// ```
/// use zylos_hitl::{decide_action, DefaultPolicyTier, ApprovalDecision, DecisionTier};
/// # use zylos_hitl::ApprovalRequest;
/// # use chrono::Utc;
/// let req = ApprovalRequest {
///     id: "x".into(), summary: "x".into(), tool_call: "x".into(),
///     args_hash: "x".into(), ttl_secs: 60, created_at: Utc::now(),
///     expires_at: Utc::now(), thread_id: "t".into(), requester_id: "r".into(),
///     status: zylos_hitl::ApprovalStatus::Pending, decision: None,
///     decided_at: None, decided_by_tier: None, refusal_reason: None,
/// };
/// let tiers: Vec<Box<dyn zylos_hitl::PolicyTier>> = vec![Box::new(DefaultPolicyTier)];
/// let (decision, tier) = decide_action(&req, &tiers);
/// assert_eq!(decision, ApprovalDecision::Approve);
/// assert_eq!(tier, DecisionTier::Default);
/// ```
pub fn decide_action(
    request: &ApprovalRequest,
    tiers: &[Box<dyn PolicyTier>],
) -> (ApprovalDecision, DecisionTier) {
    for tier in tiers {
        if let Some(decision) = tier.evaluate(request) {
            return (decision, tier.tier());
        }
    }
    (ApprovalDecision::Escalate("no_policy_match".into()), DecisionTier::Default)
}

// ─── Permanent Error Detection (Task 7C.2.2) ──────────────────────────────

/// Check if an error is permanent (structural) and should NOT be retried.
///
/// Matches substrings: `schema_validation`, `catalogue_entry_unknown`,
/// `tool_not_found`, `headless_tool_suspension`, `invalid_schema`, `zod_error`.
///
/// # Examples
///
/// ```
/// use zylos_hitl::is_permanent;
/// assert!(is_permanent("schema_validation failed"));
/// assert!(!is_permanent("network timeout"));
/// ```
pub fn is_permanent(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("schema_validation")
        || lower.contains("catalogue_entry_unknown")
        || lower.contains("tool_not_found")
        || lower.contains("headless_tool_suspension")
        || lower.contains("invalid_schema")
        || lower.contains("zod_error")
}

// ─── WASM Policy Sandbox Stub (Task 7C.2.2) ───────────────────────────────

/// Stub for WASM-based policy engine (wasmtime 25.0).
///
/// Currently a no-op placeholder; `evaluate()` always returns `None`.
/// Will be replaced with real WASM execution when the runtime is integrated.
pub struct WasmPolicySandbox {
    loaded: bool,
    module_name: Option<String>,
}

impl WasmPolicySandbox {
    /// Create a new empty sandbox with no module loaded.
    pub fn new() -> Self { Self { loaded: false, module_name: None } }

    /// Load a WASM module by name. Currently a stub that records the name.
    pub fn load_module(&mut self, name: &str, _wasm_bytes: &[u8]) -> std::result::Result<(), HitlError> {
        self.loaded = true;
        self.module_name = Some(name.to_string());
        Ok(())
    }

    /// Whether a module has been loaded.
    pub fn is_loaded(&self) -> bool { self.loaded }
    /// Name of the loaded module, if any.
    pub fn module_name(&self) -> Option<&str> { self.module_name.as_deref() }
    /// Evaluate a request against the loaded WASM policy. Stub: always `None`.
    pub fn evaluate(&self, _request: &ApprovalRequest) -> Option<ApprovalDecision> { None }
}

impl Default for WasmPolicySandbox {
    fn default() -> Self { Self::new() }
}

// ─── Execution Authority (task-local) ────────────────────────────────────────

tokio::task_local! {
    /// Task-local carrying the approved execution authority for the current scope.
    ///
    /// Set before executing an approved action so downstream code can verify
    /// that the operation was authorized.
    pub static EXECUTION_AUTHORITY: ExecutionAuthority;
}

/// Proof of approval attached to an executing action.
///
/// Carried in the [`EXECUTION_AUTHORITY`] task-local so callees can verify
/// that the current operation was approved without re-querying the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAuthority {
    /// The approval card ID that authorized this execution.
    pub approval_id: String,
    /// Digest of the approved arguments for tamper detection.
    pub digest: String,
    /// Identity of the approver.
    pub owner: String,
}

tokio::task_local! {
    /// Task-local flag indicating a validation probe is active.
    ///
    /// When `true`, the current execution is a dry-run validation probe
    /// and should not trigger side effects or consume approvals.
    pub static VALIDATION_PROBE_ACTIVE: bool;
}

/// Check whether the current task is running inside a validation probe.
///
/// Returns `false` if the task-local is not set (i.e. outside a probe scope).
pub fn is_approval_recheck() -> bool {
    VALIDATION_PROBE_ACTIVE.try_with(|v| *v).unwrap_or(false)
}

// ─── Errors ─────────────────────────────────────────────────────────────────

/// Errors produced by the HITL approval subsystem.
#[derive(Error, Debug)]
pub enum HitlError {
    /// Database error from the SQLite backend.
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// No approval request found with the given ID.
    #[error("approval not found: {0}")]
    NotFound(String),
    /// The approval request has expired (TTL elapsed).
    #[error("approval expired: {0}")]
    Expired(String),
    /// Attempted to claim an already-decided approval.
    #[error("double claim prevented for approval {0}")]
    DoubleClaim(String),
    /// The action was refused for a specific reason.
    #[error("refusal: {0}")]
    Refused(RefusalReason),
    /// JSON serialization/deserialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Lost the distributed lease for this approval.
    #[error("lost lease: {0}")]
    LostLease(String),
}

/// Convenience result type for HITL operations.
pub type Result<T> = std::result::Result<T, HitlError>;

// ─── Approval Storage Engine ────────────────────────────────────────────────

/// Server-side approval storage with SQLite backend.
///
/// Thread-safe via `Arc<Mutex<Connection>>`; all methods are async.
/// Initializes the `approval_cards` table and indexes on construction.
pub struct ApprovalStore {
    conn: Arc<Mutex<Connection>>,
}

impl ApprovalStore {
    /// Create a new store, initializing the `approval_cards` table and indexes.
    ///
    /// Safe to call multiple times — uses `CREATE TABLE IF NOT EXISTS`.
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS approval_cards (
                id TEXT PRIMARY KEY,
                summary TEXT NOT NULL,
                tool_call TEXT NOT NULL,
                args_hash TEXT NOT NULL,
                ttl_secs INTEGER NOT NULL DEFAULT 3600,
                thread_id TEXT NOT NULL,
                requester_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT,
                decided_at TEXT,
                decided_by_tier TEXT,
                refusal_reason TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_approval_status ON approval_cards(status);
            CREATE INDEX IF NOT EXISTS idx_approval_args_hash ON approval_cards(args_hash);
            CREATE INDEX IF NOT EXISTS idx_approval_thread ON approval_cards(thread_id);
            CREATE INDEX IF NOT EXISTS idx_approval_expires ON approval_cards(expires_at);",
        )?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Create and persist a new pending approval request.
    ///
    /// Generates a UUID v4 ID and computes `expires_at` from `ttl_secs`.
    pub async fn create_request(
        &self, summary: String, tool_call: String, args_hash: String,
        ttl_secs: u64, thread_id: String, requester_id: String,
    ) -> Result<ApprovalRequest> {
        let conn = self.conn.lock().await;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_secs as i64);
        conn.execute(
            "INSERT INTO approval_cards (id, summary, tool_call, args_hash, ttl_secs,
             thread_id, requester_id, status, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?9)",
            params![id, summary, tool_call, args_hash, ttl_secs as i64,
                    thread_id, requester_id, now.to_rfc3339(), expires_at.to_rfc3339()],
        )?;
        Ok(ApprovalRequest {
            id, summary, tool_call, args_hash, ttl_secs,
            created_at: now, expires_at, thread_id, requester_id,
            status: ApprovalStatus::Pending, decision: None,
            decided_at: None, decided_by_tier: None, refusal_reason: None,
        })
    }

    /// Claim a pending approval with a decision.
    ///
    /// Validates the request is still pending and not expired. Returns
    /// [`HitlError::DoubleClaim`] if already decided, or [`HitlError::Expired`]
    /// if the TTL has elapsed.
    pub async fn claim(&self, id: &str, decision: &ApprovalDecision, tier: &DecisionTier) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        let status_and_expiry = conn.query_row(
            "SELECT status, expires_at FROM approval_cards WHERE id = ?1",
            params![id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).optional()?.ok_or_else(|| HitlError::NotFound(id.into()))?;
        if status_and_expiry.0 != "pending" {
            return Err(HitlError::DoubleClaim(id.into()));
        }
        let expires: DateTime<Utc> = DateTime::parse_from_rfc3339(&status_and_expiry.1)
            .map(|dt| dt.with_timezone(&Utc)).unwrap_or(now);
        if expires < now {
            conn.execute("UPDATE approval_cards SET status = 'expired' WHERE id = ?1 AND status = 'pending'", params![id])?;
            return Err(HitlError::Expired(id.into()));
        }
        let decision_str = serde_json::to_string(decision)?;
        let new_status = match decision {
            ApprovalDecision::Approve | ApprovalDecision::Modify(_) => "approved",
            ApprovalDecision::Reject => "rejected",
            ApprovalDecision::Escalate(_) => "pending",
        };
        let affected = conn.execute(
            "UPDATE approval_cards SET status = ?1, decision = ?2, decided_at = ?3, decided_by_tier = ?4
             WHERE id = ?5 AND status = 'pending'",
            params![new_status, decision_str, now.to_rfc3339(), tier.to_string(), id],
        )?;
        if affected == 0 { return Err(HitlError::DoubleClaim(id.into())); }
        Ok(())
    }

    /// Find the most recent approved request matching the given args hash.
    ///
    /// Used for approval reuse: if an equivalent action was already approved,
    /// skip re-prompting.
    pub async fn find_approved_by_hash(&self, args_hash: &str) -> Result<Option<ApprovalRequest>> {
        let conn = self.conn.lock().await;
        let row = conn.query_row(
            "SELECT id, summary, tool_call, args_hash, ttl_secs, thread_id, requester_id,
             status, decision, decided_at, decided_by_tier, refusal_reason, created_at, expires_at
             FROM approval_cards WHERE args_hash = ?1 AND status = 'approved'
             ORDER BY decided_at DESC LIMIT 1",
            params![args_hash], |row| Ok(raw_row(row)),
        ).optional()?;
        Ok(row.and_then(|r| r.into_approval_request().ok()))
    }

    /// Retrieve an approval request by ID.
    ///
    /// Returns [`HitlError::NotFound`] if no row matches.
    pub async fn get(&self, id: &str) -> Result<ApprovalRequest> {
        let conn = self.conn.lock().await;
        let row = conn.query_row(
            "SELECT id, summary, tool_call, args_hash, ttl_secs, thread_id, requester_id,
             status, decision, decided_at, decided_by_tier, refusal_reason, created_at, expires_at
             FROM approval_cards WHERE id = ?1",
            params![id], |row| Ok(raw_row(row)),
        ).optional()?.ok_or_else(|| HitlError::NotFound(id.into()))?;
        row.into_approval_request()
    }

    /// Withdraw all pending approvals matching `args_hash` where the deciding
    /// tier is `default` or `personal_rule` (or undecided).
    ///
    /// Returns the number of rows withdrawn.
    pub async fn withdraw_allowed(&self, args_hash: &str) -> Result<u32> {
        let conn = self.conn.lock().await;
        let affected = conn.execute(
            "UPDATE approval_cards SET status = 'withdrawn', refusal_reason = 'WithdrawnRuleAllows'
             WHERE args_hash = ?1 AND status = 'pending'
             AND (decided_by_tier IS NULL OR decided_by_tier IN ('default', 'personal_rule'))",
            params![args_hash],
        )?;
        Ok(affected as u32)
    }

    /// Abandon an approval request, recording the reason as an escalation.
    ///
    /// Applies to requests in `pending` or `approved` status. Returns
    /// [`HitlError::NotFound`] if no matching row exists.
    pub async fn abandon(&self, id: &str, reason: &str) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now();
        let affected = conn.execute(
            "UPDATE approval_cards SET status = 'abandoned', decision = ?1, decided_at = ?2
             WHERE id = ?3 AND status IN ('pending', 'approved')",
            params![serde_json::to_string(&ApprovalDecision::Escalate(reason.into()))?, now.to_rfc3339(), id],
        )?;
        if affected == 0 { return Err(HitlError::NotFound(id.into())); }
        Ok(())
    }

    /// Mark all pending requests past their `expires_at` as expired.
    ///
    /// Returns the number of rows expired. Call periodically to clean up.
    pub async fn expire_stale(&self) -> Result<u32> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE approval_cards SET status = 'expired' WHERE status = 'pending' AND expires_at < ?1",
            params![now],
        )?;
        Ok(affected as u32)
    }

    /// List remaining pending approval IDs in a thread, excluding one ID.
    ///
    /// Used after approving one step to discover subsequent steps awaiting
    /// approval in the same thread. Ordered by creation time ascending.
    pub async fn carry_on(&self, thread_id: &str, exclude_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id FROM approval_cards WHERE thread_id = ?1 AND status = 'pending' AND id != ?2 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![thread_id, exclude_id], |row| row.get(0))?;
        let mut ids = Vec::new();
        for row in rows { ids.push(row?); }
        Ok(ids)
    }
}

fn raw_row(row: &rusqlite::Row) -> ApprovalRequestRow {
    ApprovalRequestRow {
        id: row.get(0).unwrap(), summary: row.get(1).unwrap(),
        tool_call: row.get(2).unwrap(), args_hash: row.get(3).unwrap(),
        ttl_secs: row.get::<_, i64>(4).unwrap() as u64,
        thread_id: row.get(5).unwrap(), requester_id: row.get(6).unwrap(),
        status: row.get(7).unwrap(), decision: row.get(8).unwrap(),
        decided_at: row.get(9).unwrap(), decided_by_tier: row.get(10).unwrap(),
        refusal_reason: row.get(11).unwrap(),
        created_at: row.get(12).unwrap(), expires_at: row.get(13).unwrap(),
    }
}

struct ApprovalRequestRow {
    id: String, summary: String, tool_call: String, args_hash: String,
    ttl_secs: u64, thread_id: String, requester_id: String, status: String,
    decision: Option<String>, decided_at: Option<String>,
    decided_by_tier: Option<String>, refusal_reason: Option<String>,
    created_at: String, expires_at: String,
}

impl ApprovalRequestRow {
    fn into_approval_request(self) -> Result<ApprovalRequest> {
        let status = match self.status.as_str() {
            "pending" => ApprovalStatus::Pending, "approved" => ApprovalStatus::Approved,
            "rejected" => ApprovalStatus::Rejected, "expired" => ApprovalStatus::Expired,
            "withdrawn" => ApprovalStatus::Withdrawn, "abandoned" => ApprovalStatus::Abandoned,
            other => return Err(HitlError::NotFound(format!("unknown status: {other}"))),
        };
        let decision = self.decision.and_then(|d| serde_json::from_str(&d).ok());
        let decided_at = self.decided_at.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
        let decided_by_tier = self.decided_by_tier.and_then(|t| match t.as_str() {
            "safety" => Some(DecisionTier::Safety), "team_rule" => Some(DecisionTier::TeamRule),
            "personal_rule" => Some(DecisionTier::PersonalRule), "auto_review" => Some(DecisionTier::AutoReview),
            "default" => Some(DecisionTier::Default), _ => None,
        });
        let refusal_reason = self.refusal_reason.and_then(|r| match r.as_str() {
            "DeclinedByPerson" => Some(RefusalReason::DeclinedByPerson),
            "AlreadyWaiting" => Some(RefusalReason::AlreadyWaiting),
            "WithdrawnApprovalsOff" => Some(RefusalReason::WithdrawnApprovalsOff),
            "WithdrawnRuleAllows" => Some(RefusalReason::WithdrawnRuleAllows),
            "HandledByPerson" => Some(RefusalReason::HandledByPerson),
            "StepStillRunning" => Some(RefusalReason::StepStillRunning),
            "OutcomeUnknownInStep" => Some(RefusalReason::OutcomeUnknownInStep),
            _ => None,
        });
        let created_at = DateTime::parse_from_rfc3339(&self.created_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        let expires_at = DateTime::parse_from_rfc3339(&self.expires_at).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
        Ok(ApprovalRequest {
            id: self.id, summary: self.summary, tool_call: self.tool_call,
            args_hash: self.args_hash, ttl_secs: self.ttl_secs,
            created_at, expires_at, thread_id: self.thread_id,
            requester_id: self.requester_id, status, decision,
            decided_at, decided_by_tier, refusal_reason,
        })
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_store() -> ApprovalStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        ApprovalStore::new(conn).unwrap()
    }

    #[test]
    fn test_2xx_succeeded() { assert_eq!(classify_http_status(200).outcome, Outcome::Succeeded); }
    #[test]
    fn test_4xx_failed() { assert_eq!(classify_http_status(400).outcome, Outcome::Failed); }
    #[test]
    fn test_5xx_unknown() { assert_eq!(classify_http_status(503).outcome, Outcome::OutcomeUnknown); }
    #[test]
    fn test_timeout_unknown() { assert_eq!(classify_network_error("timeout").outcome, Outcome::OutcomeUnknown); }
    #[test]
    fn test_validation_failed() { assert_eq!(classify_validation_error("bad").outcome, Outcome::Failed); }

    #[test]
    fn test_dedup_same() {
        assert_eq!(NotificationDedupKey::new("op", Outcome::OutcomeUnknown), NotificationDedupKey::new("op", Outcome::OutcomeUnknown));
    }
    #[test]
    fn test_dedup_diff() {
        assert_ne!(NotificationDedupKey::new("op", Outcome::OutcomeUnknown), NotificationDedupKey::new("op", Outcome::Succeeded));
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let s = test_store();
        let r = s.create_request("Send".into(), "send_email".into(), "h".into(), 3600, "t".into(), "a".into()).await.unwrap();
        assert_eq!(r.status, ApprovalStatus::Pending);
        let f = s.get(&r.id).await.unwrap();
        assert_eq!(f.summary, "Send");
    }

    #[tokio::test]
    async fn test_double_claim() {
        let s = test_store();
        let r = s.create_request("T".into(), "t".into(), "h".into(), 3600, "t".into(), "a".into()).await.unwrap();
        s.claim(&r.id, &ApprovalDecision::Approve, &DecisionTier::Default).await.unwrap();
        assert!(matches!(s.claim(&r.id, &ApprovalDecision::Approve, &DecisionTier::Default).await, Err(HitlError::DoubleClaim(_))));
    }

    #[tokio::test]
    async fn test_find_by_hash() {
        let s = test_store();
        let r = s.create_request("T".into(), "t".into(), "uh-abc".into(), 3600, "t".into(), "a".into()).await.unwrap();
        s.claim(&r.id, &ApprovalDecision::Approve, &DecisionTier::PersonalRule).await.unwrap();
        assert!(s.find_approved_by_hash("uh-abc").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_withdraw() {
        let s = test_store();
        let r = s.create_request("T".into(), "t".into(), "hw".into(), 3600, "t".into(), "a".into()).await.unwrap();
        assert_eq!(s.withdraw_allowed("hw").await.unwrap(), 1);
        assert_eq!(s.get(&r.id).await.unwrap().status, ApprovalStatus::Withdrawn);
    }

    #[tokio::test]
    async fn test_abandon() {
        let s = test_store();
        let r = s.create_request("T".into(), "t".into(), "h".into(), 3600, "t".into(), "a".into()).await.unwrap();
        s.abandon(&r.id, "exhausted").await.unwrap();
        assert_eq!(s.get(&r.id).await.unwrap().status, ApprovalStatus::Abandoned);
    }

    #[tokio::test]
    async fn test_carry_on() {
        let s = test_store();
        let r1 = s.create_request("A".into(), "t1".into(), "h1".into(), 3600, "tx".into(), "a".into()).await.unwrap();
        let _r2 = s.create_request("B".into(), "t2".into(), "h2".into(), 3600, "tx".into(), "a".into()).await.unwrap();
        assert_eq!(s.carry_on("tx", &r1.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_expire_stale() {
        let s = test_store();
        let _r = s.create_request("Old".into(), "t".into(), "h".into(), 0, "t".into(), "a".into()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert_eq!(s.expire_stale().await.unwrap(), 1);
    }

    #[test]
    fn test_equiv_hash_deterministic() {
        let h1 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"x":1}),&serde_json::json!({"y":2}));
        let h2 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"x":1}),&serde_json::json!({"y":2}));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_equiv_hash_strips_volatile() {
        let h1 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"x":1,"snapshot_id":"s1"}),&serde_json::json!({}));
        let h2 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"x":1,"snapshot_id":"s2"}),&serde_json::json!({}));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_equiv_hash_sorts_keys() {
        let h1 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"z":1,"a":2}),&serde_json::json!({}));
        let h2 = compute_equivalence_hash("a","b","t","w","g",&serde_json::json!({"a":2,"z":1}),&serde_json::json!({}));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_decide_action_first_wins() {
        struct Safety;
        impl PolicyTier for Safety {
            fn tier(&self) -> DecisionTier { DecisionTier::Safety }
            fn evaluate(&self, _: &ApprovalRequest) -> Option<ApprovalDecision> { Some(ApprovalDecision::Reject) }
        }
        let req = ApprovalRequest {
            id:"x".into(),summary:"x".into(),tool_call:"x".into(),args_hash:"x".into(),ttl_secs:60,
            created_at:Utc::now(),expires_at:Utc::now(),thread_id:"x".into(),requester_id:"x".into(),
            status:ApprovalStatus::Pending,decision:None,decided_at:None,decided_by_tier:None,refusal_reason:None,
        };
        let tiers: Vec<Box<dyn PolicyTier>> = vec![Box::new(Safety), Box::new(DefaultPolicyTier)];
        let (d, t) = decide_action(&req, &tiers);
        assert_eq!(d, ApprovalDecision::Reject);
        assert_eq!(t, DecisionTier::Safety);
    }

    #[test]
    fn test_is_permanent() {
        assert!(is_permanent("schema_validation failed"));
        assert!(is_permanent("zod_error: bad schema"));
        assert!(!is_permanent("network timeout"));
    }

    #[test]
    fn test_wasm_stub() {
        let mut s = WasmPolicySandbox::new();
        assert!(!s.is_loaded());
        s.load_module("p.wasm", &[0]).unwrap();
        assert!(s.is_loaded());
    }

    #[test]
    fn test_refusal_messages_distinct() {
        let reasons = [
            RefusalReason::DeclinedByPerson, RefusalReason::AlreadyWaiting,
            RefusalReason::WithdrawnApprovalsOff, RefusalReason::WithdrawnRuleAllows,
            RefusalReason::HandledByPerson, RefusalReason::StepStillRunning,
            RefusalReason::OutcomeUnknownInStep,
        ];
        let msgs: Vec<&str> = reasons.iter().map(|r| r.message()).collect();
        for i in 0..msgs.len() { for j in (i+1)..msgs.len() { assert_ne!(msgs[i], msgs[j]); } }
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(DecisionTier::Safety.to_string(), "safety");
        assert_eq!(DecisionTier::Default.to_string(), "default");
    }

    #[test]
    fn test_decision_display() {
        assert_eq!(ApprovalDecision::Approve.to_string(), "approve");
        assert_eq!(ApprovalDecision::Escalate("x".into()).to_string(), "escalate:x");
    }
}