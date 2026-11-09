//! Two-Stage Triage Engine (Task 7C.2.1)
//!
//! Stage 1: deterministic rules for safe negatives (bulk, auto-reply, promotions).
//! Stage 2: model fallback with structured prompt + parsing when no rule matches.
//! Every verdict carries `decided_by: "rule:<id>" | "model:<name>"` for audit.
//! Framing patterns wrap scheduled instructions to prevent misinterpretation.
//! Initiator tracking records the cause of every turn.
//!
//! Source: OpenDuet `triage.ts`, `routine-firing.ts`, `initiator.ts`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// What caused this triage turn to fire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração Initiator.
pub enum Initiator {
    // Direct human message.
    Person(String),
    // Scheduled routine fired.
    Routine(String),
    // Responsibility/check-in timer.
    Responsibility(String),
    // Memory recall triggered.
    Memory(String),
    // Handoff from another agent or task.
    Handoff(String),
    // System event (webhook, cron, etc.).
    System(String),
}

impl Initiator {
    /// Função label.
    pub fn label(&self) -> &str {
        match self {
            Initiator::Person(_) => "person",
            Initiator::Routine(_) => "routine",
            Initiator::Responsibility(_) => "responsibility",
            Initiator::Memory(_) => "memory",
            Initiator::Handoff(_) => "handoff",
            Initiator::System(_) => "system",
        }
    }

    /// Função detail.
    pub fn detail(&self) -> &str {
        match self {
            Initiator::Person(s)
            | Initiator::Routine(s)
            | Initiator::Responsibility(s)
            | Initiator::Memory(s)
            | Initiator::Handoff(s)
            | Initiator::System(s) => s.as_str(),
        }
    }
}

/// Triage verdict — what action to take on an inbound item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração TriageVerdict.
pub enum TriageVerdict {
    CanWait,
    NeedsYou,
    // Auto-handle (auto-reply, archive, label, etc.).
    AutoHandle(String),
    // Escalate to a specific person or team.
    Escalate(String),
}

impl TriageVerdict {
    /// Função to db.
    pub fn to_db(&self) -> String {
        match self {
            TriageVerdict::CanWait => "can_wait".into(),
            TriageVerdict::NeedsYou => "needs_you".into(),
            TriageVerdict::AutoHandle(action) => format!("auto_handle:{}", action),
            TriageVerdict::Escalate(target) => format!("escalate:{}", target),
        }
    }

    /// Função from db.
    pub fn from_db(s: &str) -> Option<Self> {
        if s == "can_wait" {
            // Variante Some.
            Some(TriageVerdict::CanWait)
        } else if s == "needs_you" {
            // Variante Some.
            Some(TriageVerdict::NeedsYou)
        } else if let Some(action) = s.strip_prefix("auto_handle:") {
            // Variante Some.
            Some(TriageVerdict::AutoHandle(action.to_string()))
        } else if let Some(target) = s.strip_prefix("escalate:") {
            // Variante Some.
            Some(TriageVerdict::Escalate(target.to_string()))
        } else {
            // Variante None.
            None
        }
    }
}

/// Audit record for every triage decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageDecision.
pub struct TriageDecision {
    /// Campo timestamp.
    pub timestamp: DateTime<Utc>,
    /// Campo item id.
    pub item_id: String,
    /// Campo verdict.
    pub verdict: TriageVerdict,
    /// How the decision was reached: `"rule:<id>"` or `"model:<name>"`.
    pub decided_by: String,
    /// Campo initiator.
    pub initiator: Initiator,
    /// Campo confidence.
    pub confidence: f64,
    /// Campo reasoning.
    pub reasoning: String,
}

/// A single deterministic triage rule (Stage 1).
/// Rules only produce safe-negative verdicts (CanWait, AutoHandle).
/// They NEVER produce NeedsYou — that's reserved for Stage 2 model.
pub struct TriageRule {
    /// Campo id.
    pub id: String,
    /// Campo description.
    pub description: String,
    /// Predicate evaluated against the inbound item.
    pub matcher: Box<dyn Fn(&TriageInput) -> bool + Send + Sync>,
    /// Verdict produced when this rule matches. Must NOT be NeedsYou.
    pub verdict: TriageVerdict,
    /// Priority: lower number = higher priority. First match wins.
    pub priority: u32,
}

impl std::fmt::Debug for TriageRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TriageRule")
            .field("id", &self.id)
            .field("description", &self.description)
            .field("verdict", &self.verdict)
            .field("priority", &self.priority)
            .finish_non_exhaustive()
    }
}

/// Inbound item to triage.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageInput.
pub struct TriageInput {
    /// Campo id.
    pub id: String,
    /// Campo subject.
    pub subject: String,
    /// Campo body.
    pub body: String,
    /// Campo sender.
    pub sender: String,
    /// Campo headers.
    pub headers: HashMap<String, String>,
    /// Campo labels.
    pub labels: Vec<String>,
    /// Campo received at.
    pub received_at: DateTime<Utc>,
    /// Campo initiator.
    pub initiator: Initiator,
}

/// Structured prompt for Stage 2 model classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageModelPrompt.
pub struct TriageModelPrompt {
    /// Campo system.
    pub system: String,
    /// Campo user.
    pub user: String,
    /// Campo expected schema.
    pub expected_schema: String,
}

/// Error type for triage operations.
#[derive(Debug, Clone)]
/// Enumeração TriageError.
pub enum TriageError {
    // Stage 2 model failed — never silently default to a verdict.
    ModelFailure(String),
    // Invalid input.
    InvalidInput(String),
    // Rule configuration error.
    RuleConfig(String),
}

impl std::fmt::Display for TriageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriageError::ModelFailure(msg) => write!(f, "model failure: {}", msg),
            TriageError::InvalidInput(msg) => write!(f, "invalid input: {}", msg),
            TriageError::RuleConfig(msg) => write!(f, "rule config: {}", msg),
        }
    }
}

impl std::error::Error for TriageError {}

/// Framing pattern: wraps scheduled/routine instructions so the model
/// doesn't misinterpret them as direct user commands.
pub struct FramingPattern;

impl FramingPattern {
    /// Wrap a scheduled instruction with framing that clarifies its origin.
    pub fn wrap_scheduled(routine_name: &str, instruction: &str) -> String {
        format!(
            "[SCHEDULED ROUTINE: {}]\n\
             The following is an automated routine trigger, not a direct user message.\n\
             Evaluate it using normal triage rules.\n\
             ---\n\
             {}\n\
             ---\n\
             [END SCHEDULED ROUTINE]",
            routine_name, instruction
        )
    }

    /// Wrap a handoff with framing that preserves context.
    pub fn wrap_handoff(from_agent: &str, context: &str) -> String {
        format!(
            "[HANDOFF FROM: {}]\n\
             This item was escalated by another agent. Treat with appropriate urgency.\n\
             ---\n\
             {}\n\
             ---\n\
             [END HANDOFF]",
            from_agent, context
        )
    }

    /// Check if text is already framed (to avoid double-wrapping).
    pub fn is_framed(text: &str) -> bool {
        text.starts_with("[SCHEDULED ROUTINE:")
            || text.starts_with("[HANDOFF FROM:")
            || text.starts_with("[SYSTEM EVENT:")
    }
}

/// Stage 2 model interface — abstracts over the actual LLM call.
pub trait TriageModel: Send + Sync {
    /// Classify an input that didn't match any Stage 1 rule.
    /// Returns a verdict + confidence + reasoning.
    /// MUST return Err on failure — never silently default.
    fn classify(&self, input: &TriageInput) -> Result<(TriageVerdict, f64, String), TriageError>;

    /// Model name for `decided_by` audit field.
    fn name(&self) -> &str;
}

/// The two-stage triage engine.
pub struct TriageEngine {
    rules: Vec<TriageRule>,
    model: Option<Box<dyn TriageModel>>,
    decisions: std::sync::Mutex<Vec<TriageDecision>>,
}

impl TriageEngine {
    /// Função new.
    pub fn new() -> Self {
        // Variante Self.
        Self {
            rules: Vec::new(),
            model: None,
            decisions: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Add a Stage 1 rule. Validates that rules never produce NeedsYou.
    pub fn add_rule(&mut self, rule: TriageRule) -> Result<(), TriageError> {
        if matches!(rule.verdict, TriageVerdict::NeedsYou) {
            return Err(TriageError::RuleConfig(format!(
                "rule '{}' must not produce NeedsYou verdict (reserved for model stage)",
                rule.id
            )));
        }
        self.rules.push(rule);
        // Keep sorted by priority (ascending = higher priority first)
        self.rules.sort_by_key(|r| r.priority);
        // Variante Ok.
        Ok(())
    }

    /// Set the Stage 2 model.
    pub fn set_model(&mut self, model: Box<dyn TriageModel>) {
        self.model = Some(model);
    }

    /// Run triage on an input. Stage 1 rules first, then Stage 2 model fallback.
    pub fn triage(&self, input: &TriageInput) -> Result<TriageDecision, TriageError> {
        // Stage 1: try rules in priority order
        for rule in &self.rules {
            if (rule.matcher)(input) {
                let decision = TriageDecision {
                    timestamp: Utc::now(),
                    item_id: input.id.clone(),
                    verdict: rule.verdict.clone(),
                    decided_by: format!("rule:{}", rule.id),
                    initiator: input.initiator.clone(),
                    confidence: 1.0,
                    reasoning: format!("Matched rule: {}", rule.description),
                };
                self.record_decision(&decision);
                return Ok(decision);
            }
        }

        // Stage 2: model fallback
        let model = self.model.as_ref().ok_or_else(|| {
            TriageError::ModelFailure(
                "no model configured and no rule matched — cannot produce verdict".into(),
            )
        })?;

        let (verdict, confidence, reasoning) = model.classify(input)?;

        let decision = TriageDecision {
            timestamp: Utc::now(),
            item_id: input.id.clone(),
            verdict,
            decided_by: format!("model:{}", model.name()),
            initiator: input.initiator.clone(),
            confidence,
            reasoning,
        };
        self.record_decision(&decision);
        // Variante Ok.
        Ok(decision)
    }

    fn record_decision(&self, decision: &TriageDecision) {
        let mut decisions = self.decisions.lock().unwrap();
        decisions.push(decision.clone());
    }

    /// Get all recorded decisions.
    pub fn decisions(&self) -> Vec<TriageDecision> {
        self.decisions.lock().unwrap().clone()
    }

    /// Get decisions for a specific item.
    pub fn decisions_for_item(&self, item_id: &str) -> Vec<TriageDecision> {
        self.decisions
            .lock()
            .unwrap()
            .iter()
            .filter(|d| d.item_id == item_id)
            .cloned()
            .collect()
    }

    /// Count decisions by decided_by source.
    pub fn decision_counts(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for d in self.decisions.lock().unwrap().iter() {
            *counts.entry(d.decided_by.clone()).or_insert(0) += 1;
        }
        counts
    }

    /// Build the Stage 2 prompt for external use (e.g., sending to an LLM API).
    pub fn build_model_prompt(input: &TriageInput) -> TriageModelPrompt {
        // Variante TriageModelPrompt.
        TriageModelPrompt {
            system: "You are a triage classifier. Given an inbound item, classify it into exactly one verdict:\n\
                     - can_wait: safe to defer, no immediate attention needed\n\
                     - needs_you: requires human decision or attention\n\
                     - auto_handle:<action>: can be handled automatically (specify action)\n\
                     - escalate:<target>: should be escalated to a specific person/team\n\n\
                     Respond in JSON: {\"verdict\": \"...\", \"confidence\": 0.0-1.0, \"reasoning\": \"...\"}\n\
                     Never guess. If uncertain, prefer needs_you."
                .into(),
            user: format!(
                "Subject: {}\nFrom: {}\nLabels: {:?}\nInitiator: {} ({})\n\nBody:\n{}",
                input.subject,
                input.sender,
                input.labels,
                input.initiator.label(),
                input.initiator.detail(),
                input.body
            ),
            expected_schema: "{\"verdict\": string, \"confidence\": number, \"reasoning\": string}".into(),
        }
    }
}

impl Default for TriageEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in rules for common safe-negative patterns.
pub mod builtin_rules {
    use super::*;

    /// Bulk/promotional email detection.
    pub fn bulk_promotions() -> TriageRule {
        // Variante TriageRule.
        TriageRule {
            id: "bulk-promotions".into(),
            description: "Bulk/promotional emails based on headers and labels".into(),
            matcher: Box::new(|input: &TriageInput| {
                input.labels.iter().any(|l| {
                    let lower = l.to_lowercase();
                    lower.contains("promotion") || lower.contains("bulk") || lower.contains("newsletter")
                }) || input
                    .headers
                    .get("List-Unsubscribe")
                    .map(|v| !v.is_empty())
                    .unwrap_or(false)
            }),
            verdict: TriageVerdict::CanWait,
            priority: 10,
        }
    }

    /// Auto-reply / out-of-office detection.
    pub fn auto_reply() -> TriageRule {
        // Variante TriageRule.
        TriageRule {
            id: "auto-reply".into(),
            description: "Auto-reply and out-of-office messages".into(),
            matcher: Box::new(|input: &TriageInput| {
                let subject_lower = input.subject.to_lowercase();
                subject_lower.contains("out of office")
                    || subject_lower.contains("automatic reply")
                    || subject_lower.contains("auto-reply")
                    || subject_lower.contains("auto response")
                    || input
                        .headers
                        .get("X-Auto-Reply")
                        .map(|v| v == "true" || v == "yes")
                        .unwrap_or(false)
            }),
            verdict: TriageVerdict::AutoHandle("archive".into()),
            priority: 5,
        }
    }

    /// Notification / alert emails (CI, monitoring, etc.).
    pub fn notifications() -> TriageRule {
        // Variante TriageRule.
        TriageRule {
            id: "notifications".into(),
            description: "Automated notifications and alerts".into(),
            matcher: Box::new(|input: &TriageInput| {
                let sender_lower = input.sender.to_lowercase();
                sender_lower.contains("noreply")
                    || sender_lower.contains("no-reply")
                    || sender_lower.contains("notifications@")
                    || sender_lower.contains("alert@")
                    || sender_lower.contains("notify@")
                    || input.labels.iter().any(|l| {
                        let lower = l.to_lowercase();
                        lower.contains("notification") || lower.contains("alert")
                    })
            }),
            verdict: TriageVerdict::CanWait,
            priority: 20,
        }
    }

    /// Spam detection via labels.
    pub fn spam_label() -> TriageRule {
        // Variante TriageRule.
        TriageRule {
            id: "spam-label".into(),
            description: "Items labeled as spam".into(),
            matcher: Box::new(|input: &TriageInput| {
                input.labels.iter().any(|l| {
                    let lower = l.to_lowercase();
                    lower == "spam" || lower == "junk"
                })
            }),
            verdict: TriageVerdict::AutoHandle("trash".into()),
            priority: 1,
        }
    }
}

/// Eval harness for measuring triage accuracy against a labeled corpus.
pub struct TriageEval {
    corpus: Vec<EvalCase>,
}

#[derive(Debug, Clone)]
/// Estrutura EvalCase.
pub struct EvalCase {
    /// Campo input.
    pub input: TriageInput,
    /// Campo expected verdict.
    pub expected_verdict: TriageVerdict,
    /// Campo expected decided by prefix.
    pub expected_decided_by_prefix: String, // "rule:" or "model:"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalResult.
pub struct EvalResult {
    /// Campo total.
    pub total: usize,
    /// Campo correct.
    pub correct: usize,
    /// Campo accuracy.
    pub accuracy: f64,
    /// Campo by source.
    pub by_source: HashMap<String, EvalSourceStats>,
    /// Campo failures.
    pub failures: Vec<EvalFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalSourceStats.
pub struct EvalSourceStats {
    /// Campo total.
    pub total: usize,
    /// Campo correct.
    pub correct: usize,
    /// Campo accuracy.
    pub accuracy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalFailure.
pub struct EvalFailure {
    /// Campo item id.
    pub item_id: String,
    /// Campo expected.
    pub expected: String,
    /// Campo got.
    pub got: String,
    /// Campo decided by.
    pub decided_by: String,
}

impl TriageEval {
    /// Função new.
    pub fn new(corpus: Vec<EvalCase>) -> Self {
        // Variante Self.
        Self { corpus }
    }

    /// Função run.
    pub fn run(&self, engine: &TriageEngine) -> Result<EvalResult, TriageError> {
        let mut correct = 0;
        let mut by_source: HashMap<String, EvalSourceStats> = HashMap::new();
        let mut failures = Vec::new();

        for case in &self.corpus {
            let decision = engine.triage(&case.input)?;
            let verdict_match = decision.verdict == case.expected_verdict;
            let source_match = decision
                .decided_by
                .starts_with(&case.expected_decided_by_prefix);

            let is_correct = verdict_match && source_match;
            if is_correct {
                correct += 1;
            } else {
                failures.push(EvalFailure {
                    item_id: case.input.id.clone(),
                    expected: format!(
                        "{} (via {})",
                        case.expected_verdict.to_db(),
                        case.expected_decided_by_prefix
                    ),
                    got: format!("{} (via {})", decision.verdict.to_db(), decision.decided_by),
                    decided_by: decision.decided_by.clone(),
                });
            }

            // Track per-source stats
            let source_key = if decision.decided_by.starts_with("rule:") {
                "rule".to_string()
            } else {
                "model".to_string()
            };
            let stats = by_source
                .entry(source_key)
                .or_insert(EvalSourceStats {
                    total: 0,
                    correct: 0,
                    accuracy: 0.0,
                });
            stats.total += 1;
            if is_correct {
                stats.correct += 1;
            }
        }

        // Compute accuracies
        for stats in by_source.values_mut() {
            stats.accuracy = if stats.total > 0 {
                stats.correct as f64 / stats.total as f64
            } else {
                0.0
            };
        }

        let total = self.corpus.len();
        // Variante Ok.
        Ok(EvalResult {
            total,
            correct,
            accuracy: if total > 0 {
                correct as f64 / total as f64
            } else {
                0.0
            },
            by_source,
            failures,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(id: &str, subject: &str, sender: &str, labels: &[&str]) -> TriageInput {
        /// Variante TriageInput.
        TriageInput {
            id: id.into(),
            subject: subject.into(),
            body: "Test body content.".into(),
            sender: sender.into(),
            headers: HashMap::new(),
            labels: labels.iter().map(|s| s.to_string()).collect(),
            received_at: Utc::now(),
            initiator: Initiator::Person("user@test.com".into()),
        }
    }

    #[test]
    fn test_rule_match_skips_model() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::auto_reply()).unwrap();

        let input = make_input("1", "Out of Office: Re: Meeting", "boss@co.com", &[]);
        let decision = engine.triage(&input).unwrap();

        assert_eq!(decision.verdict, TriageVerdict::AutoHandle("archive".into()));
        assert_eq!(decision.decided_by, "rule:auto-reply");
        assert_eq!(decision.confidence, 1.0);
    }

    #[test]
    fn test_spam_rule_highest_priority() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::bulk_promotions()).unwrap();
        engine.add_rule(builtin_rules::spam_label()).unwrap();

        // Has both spam label AND promotion label — spam should win (priority 1 < 10)
        let input = make_input("2", "Buy now!", "spammer@evil.com", &["Spam", "Promotions"]);
        let decision = engine.triage(&input).unwrap();

        assert_eq!(decision.verdict, TriageVerdict::AutoHandle("trash".into()));
        assert_eq!(decision.decided_by, "rule:spam-label");
    }

    #[test]
    fn test_no_rule_match_no_model_returns_error() {
        let engine = TriageEngine::new();
        // No rules, no model

        let input = make_input("3", "Important question", "colleague@co.com", &[]);
        let err = engine.triage(&input).unwrap_err();

        assert!(matches!(err, TriageError::ModelFailure(_)));
    }

    #[test]
    fn test_model_failure_throws_not_default() {
        struct FailModel;
        impl TriageModel for FailModel {
            fn classify(&self, _: &TriageInput) -> Result<(TriageVerdict, f64, String), TriageError> {
                // Variante Err.
                Err(TriageError::ModelFailure("API timeout".into()))
            }
            fn name(&self) -> &str {
                "fail-model"
            }
        }

        let mut engine = TriageEngine::new();
        engine.set_model(Box::new(FailModel));

        let input = make_input("4", "Complex question", "someone@co.com", &[]);
        let err = engine.triage(&input).unwrap_err();

        match err {
            TriageError::ModelFailure(msg) => assert!(msg.contains("API timeout")),
            _ => panic!("expected ModelFailure, got {:?}", err),
        }
    }

    #[test]
    fn test_model_success_populates_decided_by() {
        struct GoodModel;
        impl TriageModel for GoodModel {
            fn classify(&self, _: &TriageInput) -> Result<(TriageVerdict, f64, String), TriageError> {
                // Variante Ok.
                Ok((TriageVerdict::NeedsYou, 0.85, "Urgent request detected".into()))
            }
            fn name(&self) -> &str {
                "gpt-triage-v1"
            }
        }

        let mut engine = TriageEngine::new();
        engine.set_model(Box::new(GoodModel));

        let input = make_input("5", "Urgent: server down", "ops@co.com", &[]);
        let decision = engine.triage(&input).unwrap();

        assert_eq!(decision.verdict, TriageVerdict::NeedsYou);
        assert_eq!(decision.decided_by, "model:gpt-triage-v1");
        assert!((decision.confidence - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rules_cannot_produce_needs_you() {
        let mut engine = TriageEngine::new();
        let bad_rule = TriageRule {
            id: "bad".into(),
            description: "Bad rule".into(),
            matcher: Box::new(|_| true),
            verdict: TriageVerdict::NeedsYou,
            priority: 1,
        };

        let err = engine.add_rule(bad_rule).unwrap_err();
        assert!(matches!(err, TriageError::RuleConfig(_)));
    }

    #[test]
    fn test_decided_by_audit_field_always_populated() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::auto_reply()).unwrap();

        struct SimpleModel;
        impl TriageModel for SimpleModel {
            fn classify(&self, _: &TriageInput) -> Result<(TriageVerdict, f64, String), TriageError> {
                // Variante Ok.
                Ok((TriageVerdict::CanWait, 0.7, "Low priority".into()))
            }
            fn name(&self) -> &str {
                "simple"
            }
        }
        engine.set_model(Box::new(SimpleModel));

        // Rule-matched item
        let input1 = make_input("a", "Auto-Reply: Away", "x@y.com", &[]);
        let d1 = engine.triage(&input1).unwrap();
        assert!(d1.decided_by.starts_with("rule:"));

        // Model-classified item
        let input2 = make_input("b", "Regular email", "a@b.com", &[]);
        let d2 = engine.triage(&input2).unwrap();
        assert!(d2.decided_by.starts_with("model:"));
    }

    #[test]
    fn test_framing_pattern_scheduled() {
        let framed = FramingPattern::wrap_scheduled("morning-checkin", "Check inbox priority");
        assert!(framed.contains("[SCHEDULED ROUTINE: morning-checkin]"));
        assert!(framed.contains("automated routine trigger"));
        assert!(FramingPattern::is_framed(&framed));
    }

    #[test]
    fn test_framing_pattern_handoff() {
        let framed = FramingPattern::wrap_handoff("email-agent", "User asked about billing");
        assert!(framed.contains("[HANDOFF FROM: email-agent]"));
        assert!(FramingPattern::is_framed(&framed));
    }

    #[test]
    fn test_unframed_text_not_detected_as_framed() {
        assert!(!FramingPattern::is_framed("Just a regular message"));
        assert!(!FramingPattern::is_framed(""));
    }

    #[test]
    fn test_initiator_tracking() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::auto_reply()).unwrap();

        let mut input = make_input("i1", "Auto-Reply: OOO", "x@y.com", &[]);
        input.initiator = Initiator::Routine("morning-checkin".into());

        let decision = engine.triage(&input).unwrap();
        assert_eq!(decision.initiator, Initiator::Routine("morning-checkin".into()));
        assert_eq!(decision.initiator.label(), "routine");
        assert_eq!(decision.initiator.detail(), "morning-checkin");
    }

    #[test]
    fn test_decision_recording_and_query() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::auto_reply()).unwrap();
        engine.add_rule(builtin_rules::bulk_promotions()).unwrap();

        let input1 = make_input("q1", "Auto-Reply: Away", "x@y.com", &[]);
        let input2 = make_input("q2", "Weekly Newsletter", "news@co.com", &["Newsletter"]);
        let input3 = make_input("q1", "Auto-Reply: Back", "x@y.com", &[]);

        engine.triage(&input1).unwrap();
        engine.triage(&input2).unwrap();
        engine.triage(&input3).unwrap();

        assert_eq!(engine.decisions().len(), 3);
        assert_eq!(engine.decisions_for_item("q1").len(), 2);
        assert_eq!(engine.decisions_for_item("q2").len(), 1);

        let counts = engine.decision_counts();
        assert_eq!(*counts.get("rule:auto-reply").unwrap(), 2);
        assert_eq!(*counts.get("rule:bulk-promotions").unwrap(), 1);
    }

    #[test]
    fn test_verdict_serialization_roundtrip() {
        let verdicts = vec![
            TriageVerdict::CanWait,
            TriageVerdict::NeedsYou,
            TriageVerdict::AutoHandle("archive".into()),
            TriageVerdict::Escalate("team-lead".into()),
        ];

        for v in verdicts {
            let db = v.to_db();
            let parsed = TriageVerdict::from_db(&db).unwrap();
            assert_eq!(parsed, v);
        }
    }

    #[test]
    fn test_verdict_from_db_invalid() {
        assert!(TriageVerdict::from_db("invalid").is_none());
        assert!(TriageVerdict::from_db("").is_none());
    }

    #[test]
    fn test_model_prompt_generation() {
        let input = make_input("p1", "Test Subject", "sender@co.com", &["inbox"]);
        let prompt = TriageEngine::build_model_prompt(&input);

        assert!(prompt.system.contains("triage classifier"));
        assert!(prompt.user.contains("Test Subject"));
        assert!(prompt.user.contains("sender@co.com"));
        assert!(prompt.expected_schema.contains("verdict"));
    }

    #[test]
    fn test_eval_harness() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::auto_reply()).unwrap();
        engine.add_rule(builtin_rules::spam_label()).unwrap();

        struct AlwaysNeedsYou;
        impl TriageModel for AlwaysNeedsYou {
            fn classify(&self, _: &TriageInput) -> Result<(TriageVerdict, f64, String), TriageError> {
                // Variante Ok.
                Ok((TriageVerdict::NeedsYou, 0.9, "Needs attention".into()))
            }
            fn name(&self) -> &str {
                "test-model"
            }
        }
        engine.set_model(Box::new(AlwaysNeedsYou));

        let corpus = vec![
            /// Variante EvalCase.
            EvalCase {
                input: make_input("e1", "Auto-Reply: OOO", "x@y.com", &[]),
                expected_verdict: TriageVerdict::AutoHandle("archive".into()),
                expected_decided_by_prefix: "rule:".into(),
            },
            /// Variante EvalCase.
            EvalCase {
                input: make_input("e2", "Spam offer", "spam@evil.com", &["Spam"]),
                expected_verdict: TriageVerdict::AutoHandle("trash".into()),
                expected_decided_by_prefix: "rule:".into(),
            },
            /// Variante EvalCase.
            EvalCase {
                input: make_input("e3", "Important question", "colleague@co.com", &[]),
                expected_verdict: TriageVerdict::NeedsYou,
                expected_decided_by_prefix: "model:".into(),
            },
        ];

        let eval = TriageEval::new(corpus);
        let result = eval.run(&engine).unwrap();

        assert_eq!(result.total, 3);
        assert_eq!(result.correct, 3);
        assert!((result.accuracy - 1.0).abs() < f64::EPSILON);
        assert!(result.failures.is_empty());
        assert!(result.by_source.contains_key("rule"));
        assert!(result.by_source.contains_key("model"));
    }

    #[test]
    fn test_bulk_promotions_via_header() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::bulk_promotions()).unwrap();

        let mut input = make_input("h1", "Sale!", "marketing@co.com", &[]);
        input
            .headers
            .insert("List-Unsubscribe".into(), "<mailto:unsub@co.com>".into());

        let decision = engine.triage(&input).unwrap();
        assert_eq!(decision.verdict, TriageVerdict::CanWait);
        assert_eq!(decision.decided_by, "rule:bulk-promotions");
    }

    #[test]
    fn test_notification_rule() {
        let mut engine = TriageEngine::new();
        engine.add_rule(builtin_rules::notifications()).unwrap();

        let input = make_input("n1", "Build #42 passed", "noreply@ci.co.com", &[]);
        let decision = engine.triage(&input).unwrap();
        assert_eq!(decision.verdict, TriageVerdict::CanWait);
        assert_eq!(decision.decided_by, "rule:notifications");
    }
}