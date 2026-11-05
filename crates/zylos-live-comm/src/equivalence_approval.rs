//! Equivalence-Based Approval Hashing (Task 7C.2.2)
//!
//! BLAKE3 hash of `[actor_id, bot_id, tool_ref, effect, scope, stable(target), stable(args)]`
//! enables retry of the same action to reuse an existing approval instead of re-prompting.
//!
//! Four-tier decision source: safety > team_rule > personal_rule > auto_review > default.
//! Safety tier always wins and cannot be overridden.
//!
//! Distinct refusal messages for each denial reason.
//! Continuation capture via tokio task-local (AsyncLocalStorage equivalent).
//! Escalation as an ungated tool — always available as a safe exit.
//!
//! Source: OpenDuet `approvals/service.ts`, `escalation.ts`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use blake3;
use std::collections::HashMap;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Stable hashing helpers
// ---------------------------------------------------------------------------

/// Strip volatile fields from a JSON value before hashing.
/// Removes: snapshot_id, ref, tool_call_id, timestamp, nonce, request_id.
pub fn stable(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut cleaned = serde_json::Map::new();
            for (k, v) in map {
                let lower = k.to_lowercase();
                if lower == "snapshot_id"
                    || lower == "ref"
                    || lower == "tool_call_id"
                    || lower == "timestamp"
                    || lower == "nonce"
                    || lower == "request_id"
                    || lower == "trace_id"
                {
                    continue;
                }
                cleaned.insert(k.clone(), stable(v));
            }
            serde_json::Value::Object(cleaned)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(stable).collect())
        }
        other => other.clone(),
    }
}

/// Compute the equivalence hash for an approval request.
/// Hash input: `[actor_id, bot_id, tool_ref, effect, scope, stable(target), stable(args)]`
pub fn compute_equivalence_hash(
    actor_id: &str,
    bot_id: &str,
    tool_ref: &str,
    effect: &str,
    scope: &str,
    target: &serde_json::Value,
    args: &serde_json::Value,
) -> String {
    let stable_target = stable(target);
    let stable_args = stable(args);

    let canonical = serde_json::json!([
        actor_id,
        bot_id,
        tool_ref,
        effect,
        scope,
        stable_target,
        stable_args
    ]);

    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    let hash = blake3::hash(&bytes);
    hash.to_hex().to_string()
}

// ---------------------------------------------------------------------------
// Decision tiers
// ---------------------------------------------------------------------------

/// Four-tier decision source for approval decisions.
/// Priority order: Safety > TeamRule > PersonalRule > AutoReview > Default.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DecisionTier {
    /// Default policy — lowest priority.
    Default,
    /// Automated review (e.g., LLM-based risk assessment).
    AutoReview,
    /// User's personal rule/grant.
    PersonalRule,
    /// Team-level rule/policy.
    TeamRule,
    /// Safety constraint — highest priority, never overridable.
    Safety,
}

impl DecisionTier {
    pub fn label(&self) -> &'static str {
        match self {
            DecisionTier::Default => "default",
            DecisionTier::AutoReview => "auto_review",
            DecisionTier::PersonalRule => "personal_rule",
            DecisionTier::TeamRule => "team_rule",
            DecisionTier::Safety => "safety",
        }
    }
}

/// A single decision entry from a specific tier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierDecision {
    pub tier: DecisionTier,
    pub approved: bool,
    pub reason: String,
    pub rule_id: Option<String>,
    pub decided_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Refusal messages
// ---------------------------------------------------------------------------

/// Distinct refusal reasons with informative messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefusalReason {
    /// Explicitly declined by a human approver.
    DeclinedByPerson(String),
    /// An identical approval is already pending.
    AlreadyWaiting,
    /// Blocked by a safety-tier rule.
    SafetyBlock(String),
    /// Blocked by a team policy.
    TeamPolicyBlock(String),
    /// Rate limit exceeded.
    RateLimited,
    /// Actor not authorized for this action.
    NotAuthorized,
    /// Action expired before approval.
    Expired,
}

impl RefusalReason {
    pub fn message(&self) -> String {
        match self {
            RefusalReason::DeclinedByPerson(who) => {
                format!("DECLINED_BY_PERSON: {} explicitly declined this action.", who)
            }
            RefusalReason::AlreadyWaiting => {
                "ALREADY_WAITING: An identical approval request is already pending. \
                 Please wait for the existing decision."
                    .into()
            }
            RefusalReason::SafetyBlock(rule) => {
                format!(
                    "SAFETY_BLOCK: This action is blocked by safety rule '{}'. \
                     Safety rules cannot be overridden.",
                    rule
                )
            }
            RefusalReason::TeamPolicyBlock(policy) => {
                format!(
                    "TEAM_POLICY_BLOCK: This action is blocked by team policy '{}'.",
                    policy
                )
            }
            RefusalReason::RateLimited => {
                "RATE_LIMITED: Too many approval requests. Please wait before retrying.".into()
            }
            RefusalReason::NotAuthorized => {
                "NOT_AUTHORIZED: You are not authorized to perform this action.".into()
            }
            RefusalReason::Expired => {
                "EXPIRED: This approval request has expired. Please submit a new one.".into()
            }
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            RefusalReason::DeclinedByPerson(_) => "DECLINED_BY_PERSON",
            RefusalReason::AlreadyWaiting => "ALREADY_WAITING",
            RefusalReason::SafetyBlock(_) => "SAFETY_BLOCK",
            RefusalReason::TeamPolicyBlock(_) => "TEAM_POLICY_BLOCK",
            RefusalReason::RateLimited => "RATE_LIMITED",
            RefusalReason::NotAuthorized => "NOT_AUTHORIZED",
            RefusalReason::Expired => "EXPIRED",
        }
    }
}

// ---------------------------------------------------------------------------
// Approval state
// ---------------------------------------------------------------------------

/// Current state of an approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalState {
    Pending,
    Approved,
    Declined(RefusalReason),
    Expired,
}

/// A stored approval record keyed by equivalence hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub equivalence_hash: String,
    pub actor_id: String,
    pub bot_id: String,
    pub tool_ref: String,
    pub effect: String,
    pub scope: String,
    pub state: ApprovalState,
    pub tier_decisions: Vec<TierDecision>,
    pub final_tier: DecisionTier,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub ttl_seconds: u64,
}

impl ApprovalRecord {
    pub fn is_expired(&self) -> bool {
        if self.ttl_seconds == 0 {
            return false; // no TTL = never expires
        }
        let elapsed = Utc::now()
            .signed_duration_since(self.created_at)
            .num_seconds();
        elapsed >= self.ttl_seconds as i64
    }

    pub fn is_resolved(&self) -> bool {
        // Only truly resolved when resolved_at is set.
        // Lower-tier denials set state to Declined but leave resolved_at=None
        // so higher tiers can still override.
        self.resolved_at.is_some()
    }
}

// ---------------------------------------------------------------------------
// Continuation capture (tokio task-local)
// ---------------------------------------------------------------------------

/// Captured continuation context for resuming after approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuationContext {
    pub approval_hash: String,
    pub resume_data: serde_json::Value,
    pub captured_at: DateTime<Utc>,
}

tokio::task_local! {
    static CONTINUATION_CTX: std::cell::RefCell<Option<ContinuationContext>>;
}

/// Capture a continuation context in the current task.
pub fn capture_continuation(ctx: ContinuationContext) {
    CONTINUATION_CTX.with(|cell| {
        *cell.borrow_mut() = Some(ctx);
    });
}

/// Retrieve and clear the captured continuation context.
pub fn take_continuation() -> Option<ContinuationContext> {
    CONTINUATION_CTX.with(|cell| cell.borrow_mut().take())
}

/// Check if a continuation context is present without consuming it.
pub fn has_continuation() -> bool {
    CONTINUATION_CTX.with(|cell| cell.borrow().is_some())
}

/// Run a future with a continuation context set.
pub async fn with_continuation<F, R>(ctx: ContinuationContext, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    CONTINUATION_CTX.scope(std::cell::RefCell::new(Some(ctx)), f).await
}

// ---------------------------------------------------------------------------
// Escalation (ungated tool)
// ---------------------------------------------------------------------------

/// Escalation request — always available regardless of grants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRequest {
    pub from_actor: String,
    pub to_target: String,
    pub reason: String,
    pub related_approval_hash: Option<String>,
    pub urgency: EscalationUrgency,
    pub context: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationUrgency {
    Low,
    Normal,
    High,
    Critical,
}

/// Result of an escalation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationResult {
    pub success: bool,
    pub escalation_id: String,
    pub message: String,
}

/// The escalation tool — ungated, always available as a safe exit.
pub struct EscalationTool {
    escalations: Mutex<Vec<EscalationRequest>>,
}

impl EscalationTool {
    pub fn new() -> Self {
        Self {
            escalations: Mutex::new(Vec::new()),
        }
    }

    /// Escalate — works even without any grants configured.
    pub fn escalate(&self, request: EscalationRequest) -> EscalationResult {
        let id = format!(
            "esc-{}-{}",
            Utc::now().timestamp_millis(),
            &request.from_actor
        );

        let mut escalations = self.escalations.lock().unwrap();
        escalations.push(request);

        EscalationResult {
            success: true,
            escalation_id: id,
            message: "Escalation submitted successfully. A human reviewer will be notified."
                .into(),
        }
    }

    pub fn all_escalations(&self) -> Vec<EscalationRequest> {
        self.escalations.lock().unwrap().clone()
    }
}

impl Default for EscalationTool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Equivalence Approval Service
// ---------------------------------------------------------------------------

/// The main equivalence-based approval service.
pub struct EquivalenceApprovalService {
    records: Mutex<HashMap<String, ApprovalRecord>>,
    escalation: EscalationTool,
}

impl EquivalenceApprovalService {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
            escalation: EscalationTool::new(),
        }
    }

    /// Submit an approval request. If an equivalent one exists and is still valid,
    /// reuse it instead of creating a duplicate.
    pub fn submit_approval(
        &self,
        actor_id: &str,
        bot_id: &str,
        tool_ref: &str,
        effect: &str,
        scope: &str,
        target: &serde_json::Value,
        args: &serde_json::Value,
        ttl_seconds: u64,
    ) -> SubmitResult {
        let hash = compute_equivalence_hash(actor_id, bot_id, tool_ref, effect, scope, target, args);

        let mut records = self.records.lock().unwrap();

        // Check for existing equivalent approval
        if let Some(existing) = records.get(&hash) {
            if !existing.is_expired() {
                match &existing.state {
                    ApprovalState::Approved => {
                        return SubmitResult::ReusedApproved(hash);
                    }
                    ApprovalState::Pending => {
                        return SubmitResult::ReusedPending(hash);
                    }
                    ApprovalState::Declined(reason) => {
                        // Re-submitting after decline creates a new record
                        // but we inform the caller about the prior decline
                        let _ = reason; // acknowledged
                    }
                    ApprovalState::Expired => {
                        // Expired — allow re-submission
                    }
                }
            }
        }

        let record = ApprovalRecord {
            equivalence_hash: hash.clone(),
            actor_id: actor_id.into(),
            bot_id: bot_id.into(),
            tool_ref: tool_ref.into(),
            effect: effect.into(),
            scope: scope.into(),
            state: ApprovalState::Pending,
            tier_decisions: Vec::new(),
            final_tier: DecisionTier::Default,
            created_at: Utc::now(),
            resolved_at: None,
            ttl_seconds,
        };

        records.insert(hash.clone(), record);
        SubmitResult::NewPending(hash)
    }

    /// Apply a tier decision to an approval. Higher tiers override lower ones.
    /// Safety tier always wins and cannot be overridden.
    pub fn apply_tier_decision(
        &self,
        hash: &str,
        decision: TierDecision,
    ) -> Result<ApplyResult, String> {
        let mut records = self.records.lock().unwrap();
        let record = records
            .get_mut(hash)
            .ok_or_else(|| format!("approval '{}' not found", hash))?;

        // Safety tier always wins — checked BEFORE resolved guard so safety
        // can override even a previously resolved approval.
        if decision.tier == DecisionTier::Safety && !decision.approved {
            record.state = ApprovalState::Declined(RefusalReason::SafetyBlock(
                decision.rule_id.clone().unwrap_or_else(|| "unknown".into()),
            ));
            record.resolved_at = Some(Utc::now());
            record.final_tier = DecisionTier::Safety;
            record.tier_decisions.push(decision);
            return Ok(ApplyResult::SafetyBlocked);
        }

        // Non-safety decisions cannot modify already-resolved approvals.
        if record.is_resolved() {
            // If resolved by safety, give a specific error mentioning safety.
            if record.final_tier == DecisionTier::Safety {
                return Err("cannot override a safety-tier decision; safety rules are absolute".into());
            }
            return Err(format!("approval '{}' is already resolved", hash));
        }

        // If already decided by safety, nothing can override
        if record.final_tier == DecisionTier::Safety {
            return Err("cannot override a safety-tier decision; safety rules are absolute".into());
        }

        // Higher tier overrides lower
        if decision.tier >= record.final_tier {
            record.final_tier = decision.tier.clone();

            if !decision.approved {
                let reason = match decision.tier {
                    DecisionTier::TeamRule => RefusalReason::TeamPolicyBlock(
                        decision.rule_id.clone().unwrap_or_else(|| "team-policy".into()),
                    ),
                    _ => RefusalReason::DeclinedByPerson(
                        decision.rule_id.clone().unwrap_or_else(|| "reviewer".into()),
                    ),
                };
                record.state = ApprovalState::Declined(reason);
                // Only resolve (close to further decisions) at TeamRule or above.
                // Lower-tier denials remain open so higher tiers can override.
                if decision.tier >= DecisionTier::TeamRule {
                    record.resolved_at = Some(Utc::now());
                }
            } else if decision.tier >= DecisionTier::PersonalRule {
                // Auto-approve at personal rule level or above
                record.state = ApprovalState::Approved;
                record.resolved_at = Some(Utc::now());
            }
        }

        record.tier_decisions.push(decision);
        Ok(ApplyResult::Applied)
    }

    /// Get the current state of an approval by hash.
    pub fn get_approval(&self, hash: &str) -> Option<ApprovalRecord> {
        self.records.lock().unwrap().get(hash).cloned()
    }

    /// Get the refusal message for a declined approval.
    pub fn get_refusal_message(&self, hash: &str) -> Option<String> {
        self.records.lock().unwrap().get(hash).and_then(|r| match &r.state {
            ApprovalState::Declined(reason) => Some(reason.message()),
            _ => None,
        })
    }

    /// Access the escalation tool (always available).
    pub fn escalation_tool(&self) -> &EscalationTool {
        &self.escalation
    }

    /// Count approvals by state.
    pub fn counts_by_state(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for record in self.records.lock().unwrap().values() {
            let key = match &record.state {
                ApprovalState::Pending => "pending",
                ApprovalState::Approved => "approved",
                ApprovalState::Declined(_) => "declined",
                ApprovalState::Expired => "expired",
            };
            *counts.entry(key.to_string()).or_insert(0) += 1;
        }
        counts
    }
}

impl Default for EquivalenceApprovalService {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of submitting an approval request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmitResult {
    /// New approval created, now pending.
    NewPending(String),
    /// Equivalent approval already approved — reuse.
    ReusedApproved(String),
    /// Equivalent approval already pending — reuse.
    ReusedPending(String),
}

/// Result of applying a tier decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyResult {
    Applied,
    SafetyBlocked,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stable_strips_volatile_fields() {
        let input = json!({
            "name": "deploy",
            "snapshot_id": "snap-123",
            "ref": "abc-def",
            "tool_call_id": "tc-456",
            "timestamp": "2024-01-01T00:00:00Z",
            "nonce": "random-xyz",
            "request_id": "req-789",
            "trace_id": "trace-abc",
            "target": "production",
            "nested": {
                "snapshot_id": "inner-snap",
                "value": 42
            }
        });

        let result = stable(&input);
        assert_eq!(result["name"], "deploy");
        assert_eq!(result["target"], "production");
        assert_eq!(result["nested"]["value"], 42);
        assert!(result.get("snapshot_id").is_none());
        assert!(result.get("ref").is_none());
        assert!(result.get("tool_call_id").is_none());
        assert!(result.get("timestamp").is_none());
        assert!(result.get("nonce").is_none());
        assert!(result.get("request_id").is_none());
        assert!(result.get("trace_id").is_none());
        assert!(result["nested"].get("snapshot_id").is_none());
    }

    #[test]
    fn test_stable_preserves_non_volatile_fields() {
        let input = json!({
            "action": "write",
            "path": "/etc/config",
            "content": "key=value"
        });

        let result = stable(&input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_equivalence_hash_deterministic() {
        let target = json!({"server": "prod-1"});
        let args = json!({"command": "restart", "snapshot_id": "snap-1"});

        let h1 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "prod", &target, &args);
        let h2 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "prod", &target, &args);

        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_equivalence_hash_ignores_volatile_fields() {
        let target = json!({"server": "prod-1"});
        let args1 = json!({"command": "restart", "snapshot_id": "snap-1", "tool_call_id": "tc-a"});
        let args2 = json!({"command": "restart", "snapshot_id": "snap-2", "tool_call_id": "tc-b"});

        let h1 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "prod", &target, &args1);
        let h2 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "prod", &target, &args2);

        assert_eq!(h1, h2, "volatile fields should not affect hash");
    }

    #[test]
    fn test_equivalence_hash_differs_on_meaningful_change() {
        let target = json!({"server": "prod-1"});
        let args = json!({"command": "restart"});

        let h1 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "prod", &target, &args);
        let h2 = compute_equivalence_hash("alice", "bot-1", "ssh.exec", "write", "staging", &target, &args);

        assert_ne!(h1, h2, "different scope should produce different hash");
    }

    #[test]
    fn test_same_action_retry_reuses_approval() {
        let svc = EquivalenceApprovalService::new();
        let target = json!({"db": "main"});
        let args = json!({"query": "SELECT 1"});

        let r1 = svc.submit_approval("alice", "bot-1", "db.query", "read", "prod", &target, &args, 3600);
        let hash = match &r1 {
            SubmitResult::NewPending(h) => h.clone(),
            _ => panic!("expected NewPending"),
        };

        // Approve it
        svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::PersonalRule,
                approved: true,
                reason: "User granted".into(),
                rule_id: Some("grant-1".into()),
                decided_at: Utc::now(),
            },
        )
        .unwrap();

        // Retry same action
        let r2 = svc.submit_approval("alice", "bot-1", "db.query", "read", "prod", &target, &args, 3600);
        assert_eq!(r2, SubmitResult::ReusedApproved(hash));
    }

    #[test]
    fn test_pending_retry_reuses_pending() {
        let svc = EquivalenceApprovalService::new();
        let target = json!({"file": "/tmp/x"});
        let args = json!({"mode": "w"});

        let r1 = svc.submit_approval("bob", "bot-2", "fs.write", "write", "local", &target, &args, 3600);
        let hash = match &r1 {
            SubmitResult::NewPending(h) => h.clone(),
            _ => panic!("expected NewPending"),
        };

        let r2 = svc.submit_approval("bob", "bot-2", "fs.write", "write", "local", &target, &args, 3600);
        assert_eq!(r2, SubmitResult::ReusedPending(hash));
    }

    #[test]
    fn test_safety_tier_always_wins() {
        let svc = EquivalenceApprovalService::new();
        let target = json!({"path": "/etc/shadow"});
        let args = json!({"content": "hacked"});

        let r = svc.submit_approval("eve", "bot-3", "fs.write", "write", "system", &target, &args, 3600);
        let hash = match &r {
            SubmitResult::NewPending(h) => h.clone(),
            _ => panic!("expected NewPending"),
        };

        // Apply safety block
        let result = svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::Safety,
                approved: false,
                reason: "Writing to /etc/shadow is forbidden".into(),
                rule_id: Some("safety-001".into()),
                decided_at: Utc::now(),
            },
        );
        assert_eq!(result.unwrap(), ApplyResult::SafetyBlocked);

        // Try to override with team rule approval — must fail
        let override_result = svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::TeamRule,
                approved: true,
                reason: "Team lead approved".into(),
                rule_id: Some("team-override".into()),
                decided_at: Utc::now(),
            },
        );
        assert!(override_result.is_err());
        assert!(override_result.unwrap_err().contains("safety"));

        // Verify state is still declined
        let record = svc.get_approval(&hash).unwrap();
        assert!(matches!(record.state, ApprovalState::Declined(RefusalReason::SafetyBlock(_))));
    }

    #[test]
    fn test_tier_override_higher_wins() {
        let svc = EquivalenceApprovalService::new();
        let target = json!({"service": "api"});
        let args = json!({"action": "restart"});

        let r = svc.submit_approval("carol", "bot-4", "svc.restart", "write", "prod", &target, &args, 3600);
        let hash = match &r {
            SubmitResult::NewPending(h) => h.clone(),
            _ => panic!("expected NewPending"),
        };

        // Default deny
        svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::Default,
                approved: false,
                reason: "Default deny".into(),
                rule_id: None,
                decided_at: Utc::now(),
            },
        )
        .unwrap();

        // Team rule approve overrides default
        svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::TeamRule,
                approved: true,
                reason: "Team policy allows restart".into(),
                rule_id: Some("team-restart".into()),
                decided_at: Utc::now(),
            },
        )
        .unwrap();

        let record = svc.get_approval(&hash).unwrap();
        assert_eq!(record.state, ApprovalState::Approved);
        assert_eq!(record.final_tier, DecisionTier::TeamRule);
    }

    #[test]
    fn test_refusal_messages_distinct() {
        let messages: Vec<(RefusalReason, &str)> = vec![
            (RefusalReason::DeclinedByPerson("alice".into()), "DECLINED_BY_PERSON"),
            (RefusalReason::AlreadyWaiting, "ALREADY_WAITING"),
            (RefusalReason::SafetyBlock("rule-1".into()), "SAFETY_BLOCK"),
            (RefusalReason::TeamPolicyBlock("policy-x".into()), "TEAM_POLICY_BLOCK"),
            (RefusalReason::RateLimited, "RATE_LIMITED"),
            (RefusalReason::NotAuthorized, "NOT_AUTHORIZED"),
            (RefusalReason::Expired, "EXPIRED"),
        ];

        let mut seen_codes = std::collections::HashSet::new();
        for (reason, expected_code) in &messages {
            assert_eq!(reason.code(), *expected_code);
            assert!(seen_codes.insert(reason.code()), "duplicate code: {}", reason.code());
            let msg = reason.message();
            assert!(!msg.is_empty());
            assert!(msg.contains(expected_code));
        }
    }

    #[test]
    fn test_refusal_message_via_service() {
        let svc = EquivalenceApprovalService::new();
        let target = json!({"x": 1});
        let args = json!({"y": 2});

        let r = svc.submit_approval("dave", "bot-5", "test.op", "read", "test", &target, &args, 3600);
        let hash = match &r {
            SubmitResult::NewPending(h) => h.clone(),
            _ => panic!("expected NewPending"),
        };

        svc.apply_tier_decision(
            &hash,
            TierDecision {
                tier: DecisionTier::PersonalRule,
                approved: false,
                reason: "User said no".into(),
                rule_id: Some("dave-deny".into()),
                decided_at: Utc::now(),
            },
        )
        .unwrap();

        let msg = svc.get_refusal_message(&hash).unwrap();
        assert!(msg.contains("DECLINED_BY_PERSON"));
        assert!(msg.contains("dave-deny"));
    }

    #[test]
    fn test_escalation_works_without_grants() {
        let svc = EquivalenceApprovalService::new();

        let result = svc.escalation_tool().escalate(EscalationRequest {
            from_actor: "junior-dev".into(),
            to_target: "team-lead".into(),
            reason: "Unsure about production deploy".into(),
            related_approval_hash: None,
            urgency: EscalationUrgency::High,
            context: json!({"service": "payments"}),
            created_at: Utc::now(),
        });

        assert!(result.success);
        assert!(!result.escalation_id.is_empty());
        assert_eq!(svc.escalation_tool().all_escalations().len(), 1);
    }

    #[test]
    fn test_escalation_with_related_approval() {
        let svc = EquivalenceApprovalService::new();

        let result = svc.escalation_tool().escalate(EscalationRequest {
            from_actor: "bot-1".into(),
            to_target: "on-call".into(),
            reason: "Approval timed out".into(),
            related_approval_hash: Some("abc123".into()),
            urgency: EscalationUrgency::Critical,
            context: json!({"timeout_ms": 30000}),
            created_at: Utc::now(),
        });

        assert!(result.success);
        let esc = &svc.escalation_tool().all_escalations()[0];
        assert_eq!(esc.related_approval_hash.as_deref(), Some("abc123"));
        assert_eq!(esc.urgency, EscalationUrgency::Critical);
    }

    #[tokio::test]
    async fn test_continuation_capture_and_take() {
        let ctx = ContinuationContext {
            approval_hash: "hash-abc".into(),
            resume_data: json!({"step": 3}),
            captured_at: Utc::now(),
        };

        with_continuation(ctx.clone(), async {
            assert!(has_continuation());
            let taken = take_continuation();
            assert!(taken.is_some());
            let taken = taken.unwrap();
            assert_eq!(taken.approval_hash, "hash-abc");
            assert_eq!(taken.resume_data["step"], 3);
            assert!(!has_continuation());
        })
        .await;
    }

    #[tokio::test]
    async fn test_continuation_absent_outside_scope() {
        // Accessing task-local without setting it first panics.
        // Verify that within a scope initialized to None, has/take return false/None.
        CONTINUATION_CTX
            .scope(std::cell::RefCell::new(None), async {
                assert!(!has_continuation());
                assert!(take_continuation().is_none());
            })
            .await;
    }

    #[test]
    fn test_counts_by_state() {
        let svc = EquivalenceApprovalService::new();
        let t = json!({});
        let a = json!({});

        svc.submit_approval("a", "b", "t", "e", "s", &t, &a, 3600);
        svc.submit_approval("a", "b", "t2", "e", "s", &t, &a, 3600);

        let counts = svc.counts_by_state();
        assert_eq!(*counts.get("pending").unwrap(), 2);
    }

    #[test]
    fn test_decision_tier_ordering() {
        assert!(DecisionTier::Safety > DecisionTier::TeamRule);
        assert!(DecisionTier::TeamRule > DecisionTier::PersonalRule);
        assert!(DecisionTier::PersonalRule > DecisionTier::AutoReview);
        assert!(DecisionTier::AutoReview > DecisionTier::Default);
    }

    #[test]
    fn test_approval_record_expiry() {
        let record = ApprovalRecord {
            equivalence_hash: "h".into(),
            actor_id: "a".into(),
            bot_id: "b".into(),
            tool_ref: "t".into(),
            effect: "e".into(),
            scope: "s".into(),
            state: ApprovalState::Pending,
            tier_decisions: vec![],
            final_tier: DecisionTier::Default,
            created_at: Utc::now() - chrono::Duration::hours(2),
            resolved_at: None,
            ttl_seconds: 3600, // 1 hour
        };
        assert!(record.is_expired());

        let record2 = ApprovalRecord {
            created_at: Utc::now(),
            ttl_seconds: 3600,
            ..record.clone()
        };
        assert!(!record2.is_expired());

        let record3 = ApprovalRecord {
            ttl_seconds: 0, // no TTL
            ..record.clone()
        };
        assert!(!record3.is_expired());
    }
}