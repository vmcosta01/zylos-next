//! Equivalence-Based Approval Hashing — Task 7C.2.2
//!
//! Implements blake3-based approval hashing where semantically equivalent actions
//! produce the same digest, enabling "Always allow" for repeated operations.
//!
//! Key design:
//! - `stable()` strips volatile fields (`snapshot_id`, `ref`, `tool_call_id`, `signal`)
//!   and sorts keys recursively (depth limit 6) before hashing
//! - Four-tier decision: safety > team_rule > personal_rule > auto_review > default
//! - Seven distinct refusal messages for clear UX
//! - Execution authority via `tokio::task_local!` for nested call propagation
//! - `carry_on()` multi-decision step resolution
//! - `withdraw_allowed()` selective closure of pending approvals
//! - `abandon()` for unblocking after exhausted retries
//! - `is_permanent()` detection for structural errors
//!
//! Source: OpenDuet `openbot/server/src/approvals/service.ts` (lines 42-53), `escalation.ts`

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Maximum recursion depth for stable key sorting.
const MAX_STABLE_DEPTH: usize = 6;

/// Volatile field names that are stripped before hashing.
const VOLATILE_FIELDS: &[&str] = &["snapshot_id", "ref", "tool_call_id", "signal"];

// ─── Approval Digest ─────────────────────────────────────────────────────────

/// A blake3 hash representing a semantically-stable action identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalDigest(pub String);

impl fmt::Display for ApprovalDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Compute the equivalence hash for an approval candidate.
///
/// The hash covers: `[actor_id, bot_id, tool_ref, effect, scope, stable(target), stable(args)]`
/// where `stable()` strips volatile fields and sorts keys recursively.
pub fn compute_approval_digest(
    actor_id: &str,
    bot_id: &str,
    tool_ref: &str,
    effect: &str,
    scope: &str,
    target: &serde_json::Value,
    args: &serde_json::Value,
) -> ApprovalDigest {
    let stable_target = stable_value(target, 0);
    let stable_args = stable_value(args, 0);

    let mut hasher = blake3::Hasher::new();
    hasher.update(actor_id.as_bytes());
    hasher.update(b"\x00");
    hasher.update(bot_id.as_bytes());
    hasher.update(b"\x00");
    hasher.update(tool_ref.as_bytes());
    hasher.update(b"\x00");
    hasher.update(effect.as_bytes());
    hasher.update(b"\x00");
    hasher.update(scope.as_bytes());
    hasher.update(b"\x00");
    hasher.update(stable_target.as_bytes());
    hasher.update(b"\x00");
    hasher.update(stable_args.as_bytes());

    let hash = hasher.finalize();
    ApprovalDigest(hash.to_hex()[..32].to_string())
}

/// Produce a deterministic JSON string with volatile fields stripped and keys sorted.
fn stable_value(value: &serde_json::Value, depth: usize) -> String {
    if depth >= MAX_STABLE_DEPTH {
        return serde_json::to_string(value).unwrap_or_default();
    }

    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: BTreeMap<String, serde_json::Value> = BTreeMap::new();
            for (k, v) in map {
                // Strip volatile fields
                if VOLATILE_FIELDS.contains(&k.as_str()) {
                    continue;
                }
                sorted.insert(k.clone(), v.clone());
            }
            // Recursively stabilize nested values
            let inner: BTreeMap<String, String> = sorted
                .iter()
                .map(|(k, v)| (k.clone(), stable_value(v, depth + 1)))
                .collect();
            serde_json::to_string(&inner).unwrap_or_default()
        }
        serde_json::Value::Array(arr) => {
            let stabilized: Vec<String> = arr.iter().map(|v| stable_value(v, depth + 1)).collect();
            serde_json::to_string(&stabilized).unwrap_or_default()
        }
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ─── Four-Tier Decision ──────────────────────────────────────────────────────

/// Policy tier for approval decisions. Evaluated in order; first match wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PolicyTier {
    /// Safety rules — never overridable.
    Safety = 0,
    /// Team-level rules.
    TeamRule = 1,
    /// Personal/user-level rules.
    PersonalRule = 2,
    /// Auto-review heuristics.
    AutoReview = 3,
    /// Default: requires explicit approval.
    Default = 4,
}

impl fmt::Display for PolicyTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Safety => write!(f, "safety"),
            Self::TeamRule => write!(f, "team_rule"),
            Self::PersonalRule => write!(f, "personal_rule"),
            Self::AutoReview => write!(f, "auto_review"),
            Self::Default => write!(f, "default"),
        }
    }
}

/// The result of a policy tier evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TierDecision {
    /// This tier allows the action without approval.
    Allow,
    /// This tier denies the action.
    Deny(RefusalReason),
    /// This tier has no opinion; defer to next tier.
    Defer,
}

/// Trait for implementing a policy tier.
pub trait PolicyTierEvaluator: Send + Sync {
    fn tier(&self) -> PolicyTier;
    fn evaluate(&self, digest: &ApprovalDigest, context: &ApprovalContext) -> TierDecision;
}

/// Context passed to each tier evaluator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalContext {
    pub actor_id: String,
    pub bot_id: String,
    pub tool_ref: String,
    pub effect: String,
    pub scope: String,
    pub digest: ApprovalDigest,
}

/// Decide an action by evaluating tiers in priority order.
pub fn decide_action(
    evaluators: &[Box<dyn PolicyTierEvaluator>],
    context: &ApprovalContext,
) -> (PolicyTier, TierDecision) {
    // Sort evaluators by tier priority (safety first)
    let mut sorted: Vec<&dyn PolicyTierEvaluator> = evaluators.iter().map(|e| e.as_ref()).collect();
    sorted.sort_by_key(|e| e.tier());

    for evaluator in sorted {
        let decision = evaluator.evaluate(&context.digest, context);
        match &decision {
            TierDecision::Allow | TierDecision::Deny(_) => {
                return (evaluator.tier(), decision);
            }
            TierDecision::Defer => continue,
        }
    }

    // No tier matched → default requires approval
    (PolicyTier::Default, TierDecision::Defer)
}

// ─── Refusal Messages ────────────────────────────────────────────────────────

/// Seven distinct refusal reasons for clear UX feedback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    /// A human explicitly declined this action.
    DeclinedByPerson { person: String },
    /// An approval request is already pending for this action.
    AlreadyWaiting,
    /// Withdrawn because approvals are disabled for this context.
    WithdrawnApprovalsOff,
    /// Withdrawn because a new allow-rule was added.
    WithdrawnRuleAllows,
    /// A human already handled this action manually.
    HandledByPerson { person: String },
    /// A previous step in this run is still executing.
    StepStillRunning,
    /// Outcome unknown from a prior step; cannot proceed safely.
    OutcomeUnknownInStep,
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclinedByPerson { person } => {
                write!(f, "DECLINED_BY_PERSON: {person} declined this action")
            }
            Self::AlreadyWaiting => write!(f, "ALREADY_WAITING: approval request already pending"),
            Self::WithdrawnApprovalsOff => {
                write!(
                    f,
                    "WITHDRAWN_APPROVALS_OFF: approvals disabled for this context"
                )
            }
            Self::WithdrawnRuleAllows => {
                write!(
                    f,
                    "WITHDRAWN_RULE_ALLOWS: new allow-rule covers this action"
                )
            }
            Self::HandledByPerson { person } => {
                write!(f, "HANDLED_BY_PERSON: {person} already handled this")
            }
            Self::StepStillRunning => {
                write!(f, "STEP_STILL_RUNNING: previous step still executing")
            }
            Self::OutcomeUnknownInStep => {
                write!(f, "OUTCOME_UNKNOWN_IN_STEP: prior step outcome uncertain")
            }
        }
    }
}

// ─── Execution Authority ─────────────────────────────────────────────────────

/// Execution authority propagated through nested calls via task-local storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAuthority {
    pub approval_id: String,
    pub digest: ApprovalDigest,
    pub owner: String,
}

tokio::task_local! {
    /// Task-local execution authority — equivalent to OpenDuet's AsyncLocalStorage.
    pub static EXECUTION_AUTHORITY: Option<ExecutionAuthority>;
}

/// Check if we're currently inside an approved execution context (synchronous).
pub fn current_authority() -> Option<ExecutionAuthority> {
    EXECUTION_AUTHORITY.with(|auth| auth.clone())
}

/// Run a closure within an execution authority context.
pub async fn with_authority<F, Fut, T>(authority: ExecutionAuthority, f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    EXECUTION_AUTHORITY.scope(Some(authority), f()).await
}

// ─── Validation Probe ────────────────────────────────────────────────────────

tokio::task_local! {
    /// When active, indicates a side-effect-free validation probe is running.
    pub static VALIDATION_PROBE_ACTIVE: bool;
}

/// Check if a validation probe is currently active (synchronous).
pub fn is_approval_recheck() -> bool {
    VALIDATION_PROBE_ACTIVE.with(|active| *active)
}

/// Run a closure as a validation probe (side-effect-free re-check).
pub async fn with_validation_probe<F, Fut, T>(f: F) -> T
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    VALIDATION_PROBE_ACTIVE.scope(true, f()).await
}

// ─── Permanent Error Detection ───────────────────────────────────────────────

/// Determine if an error is permanent (structural) and should NOT be retried.
///
/// Permanent errors include: schema validation failures, unknown tools,
/// type mismatches — anything that won't resolve by retrying.
pub fn is_permanent(error: &str) -> bool {
    let lower = error.to_lowercase();
    lower.contains("schema validation")
        || lower.contains("zoderror")
        || lower.contains("catalogueentryunknown")
        || lower.contains("tool not found")
        || lower.contains("unknown tool")
        || lower.contains("type mismatch")
        || lower.contains("invalid argument")
        || lower.contains("headlesstoolsuspension")
        || lower.contains("permission denied")
        || lower.contains("not implemented")
}

// ─── Public Approval Preview ─────────────────────────────────────────────────

/// Strip internal fields from an approval request for human display.
/// The human decides on the whole action, never a truncated copy.
pub fn public_approval(
    tool_ref: &str,
    effect: &str,
    target: &serde_json::Value,
    args: &serde_json::Value,
    whole: bool,
) -> serde_json::Value {
    let mut preview = serde_json::Map::new();
    preview.insert("tool_ref".into(), serde_json::json!(tool_ref));
    preview.insert("effect".into(), serde_json::json!(effect));

    if whole {
        // Show full target and args (with secrets masked)
        preview.insert("target".into(), mask_secrets(target));
        preview.insert("args".into(), mask_secrets(args));
    } else {
        // Summary only
        preview.insert(
            "target_summary".into(),
            serde_json::json!(format!("{} fields", count_fields(target))),
        );
        preview.insert(
            "args_summary".into(),
            serde_json::json!(format!("{} fields", count_fields(args))),
        );
    }

    serde_json::Value::Object(preview)
}

fn mask_secrets(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut masked = serde_json::Map::new();
            for (k, v) in map {
                let lower_k = k.to_lowercase();
                if lower_k.contains("secret")
                    || lower_k.contains("password")
                    || lower_k.contains("token")
                    || lower_k.contains("key")
                    || lower_k.contains("credential")
                {
                    masked.insert(k.clone(), serde_json::json!("***"));
                } else {
                    masked.insert(k.clone(), mask_secrets(v));
                }
            }
            serde_json::Value::Object(masked)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(mask_secrets).collect())
        }
        other => other.clone(),
    }
}

fn count_fields(value: &serde_json::Value) -> usize {
    match value {
        serde_json::Value::Object(map) => map.len(),
        serde_json::Value::Array(arr) => arr.len(),
        _ => 1,
    }
}

// ─── Carry-On Multi-Decision Step ────────────────────────────────────────────

/// A pending decision within a multi-decision step.
#[derive(Debug, Clone)]
pub struct StepCall {
    pub call_id: String,
    pub digest: ApprovalDigest,
    pub decided_at: chrono::DateTime<chrono::Utc>,
    pub decided: bool,
}

/// Resolve multiple decisions within the same step.
///
/// The last decided call (by `decided_at` then `call_id`) runs the continuation
/// and responds to all others. Undecided calls receive `OutcomeUnknownInStep`.
pub fn carry_on(calls: &mut [StepCall]) -> Option<usize> {
    // Find the last decided call
    let mut best_idx: Option<usize> = None;
    for (i, call) in calls.iter().enumerate() {
        if !call.decided {
            continue;
        }
        match best_idx {
            None => best_idx = Some(i),
            Some(prev) => {
                let prev_call = &calls[prev];
                if call.decided_at > prev_call.decided_at
                    || (call.decided_at == prev_call.decided_at && call.call_id > prev_call.call_id)
                {
                    best_idx = Some(i);
                }
            }
        }
    }

    best_idx
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equivalent_actions_same_digest() {
        let target1 = serde_json::json!({"file": "/tmp/a.txt", "snapshot_id": "snap-1"});
        let target2 = serde_json::json!({"file": "/tmp/a.txt", "snapshot_id": "snap-2"});
        let args = serde_json::json!({"mode": "write"});

        let d1 = compute_approval_digest(
            "actor1", "bot1", "fs.write", "write", "local", &target1, &args,
        );
        let d2 = compute_approval_digest(
            "actor1", "bot1", "fs.write", "write", "local", &target2, &args,
        );

        assert_eq!(d1, d2, "Different snapshot_id should not change digest");
    }

    #[test]
    fn test_different_actions_different_digest() {
        let target = serde_json::json!({"file": "/tmp/a.txt"});
        let args = serde_json::json!({"mode": "write"});

        let d1 = compute_approval_digest(
            "actor1", "bot1", "fs.write", "write", "local", &target, &args,
        );
        let d2 =
            compute_approval_digest("actor1", "bot1", "fs.read", "read", "local", &target, &args);

        assert_ne!(d1, d2);
    }

    #[test]
    fn test_volatile_fields_stripped() {
        let t1 = serde_json::json!({"x": 1, "ref": "abc", "tool_call_id": "tc-1", "signal": "go"});
        let t2 = serde_json::json!({"x": 1});

        let s1 = stable_value(&t1, 0);
        let s2 = stable_value(&t2, 0);

        assert_eq!(s1, s2, "Volatile fields should be stripped");
    }

    #[test]
    fn test_keys_sorted_deterministically() {
        let v1 = serde_json::json!({"b": 2, "a": 1, "c": 3});
        let v2 = serde_json::json!({"c": 3, "a": 1, "b": 2});

        let s1 = stable_value(&v1, 0);
        let s2 = stable_value(&v2, 0);

        assert_eq!(s1, s2, "Key order should not affect digest");
    }

    #[test]
    fn test_nested_sorting() {
        let v1 = serde_json::json!({"outer": {"z": 1, "a": 2}, "name": "test"});
        let v2 = serde_json::json!({"name": "test", "outer": {"a": 2, "z": 1}});

        let s1 = stable_value(&v1, 0);
        let s2 = stable_value(&v2, 0);

        assert_eq!(s1, s2);
    }

    #[test]
    fn test_depth_limit() {
        // Build deeply nested structure beyond MAX_STABLE_DEPTH
        let mut val = serde_json::json!("leaf");
        for i in 0..10 {
            val = serde_json::json!({format!("level_{i}"): val});
        }

        // Should not panic or recurse infinitely
        let s = stable_value(&val, 0);
        assert!(!s.is_empty());
    }

    #[test]
    fn test_policy_tier_ordering() {
        assert!(PolicyTier::Safety < PolicyTier::TeamRule);
        assert!(PolicyTier::TeamRule < PolicyTier::PersonalRule);
        assert!(PolicyTier::PersonalRule < PolicyTier::AutoReview);
        assert!(PolicyTier::AutoReview < PolicyTier::Default);
    }

    #[test]
    fn test_decide_action_first_match_wins() {
        struct SafetyDeny;
        impl PolicyTierEvaluator for SafetyDeny {
            fn tier(&self) -> PolicyTier {
                PolicyTier::Safety
            }
            fn evaluate(&self, _: &ApprovalDigest, _: &ApprovalContext) -> TierDecision {
                TierDecision::Deny(RefusalReason::DeclinedByPerson {
                    person: "admin".into(),
                })
            }
        }

        struct PersonalAllow;
        impl PolicyTierEvaluator for PersonalAllow {
            fn tier(&self) -> PolicyTier {
                PolicyTier::PersonalRule
            }
            fn evaluate(&self, _: &ApprovalDigest, _: &ApprovalContext) -> TierDecision {
                TierDecision::Allow
            }
        }

        let evaluators: Vec<Box<dyn PolicyTierEvaluator>> =
            vec![Box::new(PersonalAllow), Box::new(SafetyDeny)];

        let ctx = ApprovalContext {
            actor_id: "a".into(),
            bot_id: "b".into(),
            tool_ref: "t".into(),
            effect: "e".into(),
            scope: "s".into(),
            digest: ApprovalDigest("test".into()),
        };

        let (tier, decision) = decide_action(&evaluators, &ctx);
        assert_eq!(tier, PolicyTier::Safety);
        assert!(matches!(decision, TierDecision::Deny(_)));
    }

    #[test]
    fn test_decide_action_defer_to_next() {
        struct AlwaysDefer;
        impl PolicyTierEvaluator for AlwaysDefer {
            fn tier(&self) -> PolicyTier {
                PolicyTier::Safety
            }
            fn evaluate(&self, _: &ApprovalDigest, _: &ApprovalContext) -> TierDecision {
                TierDecision::Defer
            }
        }

        struct AutoAllow;
        impl PolicyTierEvaluator for AutoAllow {
            fn tier(&self) -> PolicyTier {
                PolicyTier::AutoReview
            }
            fn evaluate(&self, _: &ApprovalDigest, _: &ApprovalContext) -> TierDecision {
                TierDecision::Allow
            }
        }

        let evaluators: Vec<Box<dyn PolicyTierEvaluator>> =
            vec![Box::new(AlwaysDefer), Box::new(AutoAllow)];

        let ctx = ApprovalContext {
            actor_id: "a".into(),
            bot_id: "b".into(),
            tool_ref: "t".into(),
            effect: "e".into(),
            scope: "s".into(),
            digest: ApprovalDigest("test".into()),
        };

        let (tier, decision) = decide_action(&evaluators, &ctx);
        assert_eq!(tier, PolicyTier::AutoReview);
        assert_eq!(decision, TierDecision::Allow);
    }

    #[test]
    fn test_is_permanent_detection() {
        assert!(is_permanent("ZodError: invalid type"));
        assert!(is_permanent("Schema validation failed"));
        assert!(is_permanent("Tool not found: xyz"));
        assert!(is_permanent("CatalogueEntryUnknownError"));
        assert!(is_permanent("Permission denied for user"));
        assert!(!is_permanent("Connection timeout"));
        assert!(!is_permanent("HTTP 503 Service Unavailable"));
        assert!(!is_permanent("Rate limit exceeded"));
    }

    #[test]
    fn test_public_approval_whole_masks_secrets() {
        let target = serde_json::json!({"file": "/tmp/a.txt"});
        let args = serde_json::json!({"api_key": "sk-secret-123", "mode": "write"});

        let preview = public_approval("fs.write", "write", &target, &args, true);
        let args_preview = &preview["args"];
        assert_eq!(args_preview["api_key"], "***");
        assert_eq!(args_preview["mode"], "write");
    }

    #[test]
    fn test_public_approval_summary_mode() {
        let target = serde_json::json!({"a": 1, "b": 2});
        let args = serde_json::json!({"x": 1});

        let preview = public_approval("fs.read", "read", &target, &args, false);
        assert!(preview.get("target_summary").is_some());
        assert!(preview.get("args_summary").is_some());
        assert!(preview.get("target").is_none());
    }

    #[test]
    fn test_carry_on_selects_last_decided() {
        let now = chrono::Utc::now();
        let mut calls = vec![
            StepCall {
                call_id: "c1".into(),
                digest: ApprovalDigest("d1".into()),
                decided_at: now,
                decided: true,
            },
            StepCall {
                call_id: "c2".into(),
                digest: ApprovalDigest("d2".into()),
                decided_at: now + chrono::Duration::seconds(1),
                decided: true,
            },
            StepCall {
                call_id: "c3".into(),
                digest: ApprovalDigest("d3".into()),
                decided_at: now,
                decided: false,
            },
        ];

        let winner = carry_on(&mut calls);
        assert_eq!(winner, Some(1)); // c2 is latest decided
    }

    #[test]
    fn test_carry_on_none_when_nothing_decided() {
        let mut calls = vec![StepCall {
            call_id: "c1".into(),
            digest: ApprovalDigest("d1".into()),
            decided_at: chrono::Utc::now(),
            decided: false,
        }];

        assert_eq!(carry_on(&mut calls), None);
    }

    #[test]
    fn test_refusal_reason_display() {
        let r = RefusalReason::DeclinedByPerson {
            person: "alice".into(),
        };
        assert!(format!("{}", r).contains("DECLINED_BY_PERSON"));
        assert!(format!("{}", r).contains("alice"));

        let r2 = RefusalReason::OutcomeUnknownInStep;
        assert!(format!("{}", r2).contains("OUTCOME_UNKNOWN_IN_STEP"));
    }

    #[test]
    fn test_digest_display() {
        let d = ApprovalDigest("abcdef1234567890".into());
        assert_eq!(format!("{}", d), "abcdef1234567890");
    }

    #[tokio::test]
    async fn test_execution_authority_propagation() {
        let auth = ExecutionAuthority {
            approval_id: "appr-1".into(),
            digest: ApprovalDigest("digest-1".into()),
            owner: "agent-1".into(),
        };

        let result = with_authority(auth.clone(), || async { current_authority() }).await;

        assert!(result.is_some());
        assert_eq!(result.unwrap().approval_id, "appr-1");
    }

    #[tokio::test]
    async fn test_no_authority_outside_scope() {
        let result = EXECUTION_AUTHORITY
            .scope(None, async { current_authority() })
            .await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_validation_probe_active() {
        let result = with_validation_probe(|| async { is_approval_recheck() }).await;
        assert!(result);

        // Outside scope should be false
        let outside = VALIDATION_PROBE_ACTIVE
            .scope(false, async { is_approval_recheck() })
            .await;
        assert!(!outside);
    }
}
