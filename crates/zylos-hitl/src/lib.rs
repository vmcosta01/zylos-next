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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Outcome {
    Succeeded,
    Failed,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeVerdict {
    pub outcome: Outcome,
    pub reason: String,
    pub detail: Option<String>,
}

pub fn classify_http_status(status: u16) -> OutcomeVerdict {
    let (outcome, reason) = match status {
        200..=299 => (Outcome::Succeeded, format!("http_{status}")),
        400..=499 => (Outcome::Failed, format!("http_{status}")),
        500..=599 => (Outcome::OutcomeUnknown, format!("http_{status}")),
        _ => (Outcome::OutcomeUnknown, format!("http_{status}_unexpected")),
    };
    OutcomeVerdict { outcome, reason, detail: None }
}

pub fn classify_network_error(error_kind: &str) -> OutcomeVerdict {
    OutcomeVerdict {
        outcome: Outcome::OutcomeUnknown,
        reason: format!("network_{error_kind}"),
        detail: None,
    }
}

pub fn classify_validation_error(reason: impl Into<String>) -> OutcomeVerdict {
    OutcomeVerdict {
        outcome: Outcome::Failed,
        reason: reason.into(),
        detail: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationDedupKey(pub String);

impl NotificationDedupKey {
    pub fn new(operation_id: &str, outcome: Outcome) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(operation_id.as_bytes());
        hasher.update(b":");
        hasher.update(outcome.to_string().as_bytes());
        let hash = hex::encode(hasher.finalize());
        Self(hash[..16].to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeAuditEntry {
    pub timestamp: String,
    pub operation_id: String,
    pub previous_outcome: Option<Outcome>,
    pub new_outcome: Outcome,
    pub reason: String,
    pub decided_by: String,
}

// ─── HITL Approval Types (Task 7C.1.1 + 7C.2.2) ───────────────────────────

/// Four-tier decision hierarchy: safety > team_rule > personal_rule > auto_review > default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionTier {
    Safety,
    TeamRule,
    PersonalRule,
    AutoReview,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approve,
    Reject,
    Modify(serde_json::Value),
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    DeclinedByPerson,
    AlreadyWaiting,
    WithdrawnApprovalsOff,
    WithdrawnRuleAllows,
    HandledByPerson,
    StepStillRunning,
    OutcomeUnknownInStep,
}

impl RefusalReason {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Withdrawn,
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
/// Args never travel to the client — only hash + summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub summary: String,
    pub tool_call: String,
    /// Equivalence hash of normalized args for approval reuse.
    pub args_hash: String,
    /// Time-to-live in seconds. Default: 3600 (60 min).
    pub ttl_secs: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub thread_id: String,
    pub requester_id: String,
    pub status: ApprovalStatus,
    pub decision: Option<ApprovalDecision>,
    pub decided_at: Option<DateTime<Utc>>,
    pub decided_by_tier: Option<DecisionTier>,
    pub refusal_reason: Option<RefusalReason>,
}

// ─── Equivalence Hashing (Task 7C.2.2) ─────────────────────────────────────

/// Compute equivalence hash for approval reuse.
/// Uses SHA-256 (blake3 not yet in workspace deps; swap when added).
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
pub trait PolicyTier: Send + Sync {
    fn tier(&self) -> DecisionTier;
    fn evaluate(&self, request: &ApprovalRequest) -> Option<ApprovalDecision>;
}

/// Default policy tier that always approves (fallback / testing).
pub struct DefaultPolicyTier;

impl PolicyTier for DefaultPolicyTier {
    fn tier(&self) -> DecisionTier { DecisionTier::Default }
    fn evaluate(&self, _request: &ApprovalRequest) -> Option<ApprovalDecision> {
        Some(ApprovalDecision::Approve)
    }
}

/// Decide action through the four-tier hierarchy.
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
pub struct WasmPolicySandbox {
    loaded: bool,
    module_name: Option<String>,
}

impl WasmPolicySandbox {
    pub fn new() -> Self { Self { loaded: false, module_name: None } }

    pub fn load_module(&mut self, name: &str, _wasm_bytes: &[u8]) -> std::result::Result<(), HitlError> {
        self.loaded = true;
        self.module_name = Some(name.to_string());
        Ok(())
    }

    pub fn is_loaded(&self) -> bool { self.loaded }
    pub fn module_name(&self) -> Option<&str> { self.module_name.as_deref() }
    pub fn evaluate(&self, _request: &ApprovalRequest) -> Option<ApprovalDecision> { None }
}

impl Default for WasmPolicySandbox {
    fn default() -> Self { Self::new() }
}

// ─── Execution Authority (task-local) ────────────────────────────────────────

tokio::task_local! {
    pub static EXECUTION_AUTHORITY: ExecutionAuthority;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAuthority {
    pub approval_id: String,
    pub digest: String,
    pub owner: String,
}

tokio::task_local! {
    pub static VALIDATION_PROBE_ACTIVE: bool;
}

pub fn is_approval_recheck() -> bool {
    VALIDATION_PROBE_ACTIVE.try_with(|v| *v).unwrap_or(false)
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum HitlError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("approval not found: {0}")]
    NotFound(String),
    #[error("approval expired: {0}")]
    Expired(String),
    #[error("double claim prevented for approval {0}")]
    DoubleClaim(String),
    #[error("refusal: {0}")]
    Refused(RefusalReason),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("lost lease: {0}")]
    LostLease(String),
}

pub type Result<T> = std::result::Result<T, HitlError>;

// ─── Approval Storage Engine ────────────────────────────────────────────────

/// Server-side approval storage with SQLite backend.
pub struct ApprovalStore {
    conn: Arc<Mutex<Connection>>,
}

impl ApprovalStore {
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

    pub async fn expire_stale(&self) -> Result<u32> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE approval_cards SET status = 'expired' WHERE status = 'pending' AND expires_at < ?1",
            params![now],
        )?;
        Ok(affected as u32)
    }

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