//! Two-Stage Message Triage Engine — Task 7C.2.1
//!
//! Implements a two-stage triage system for incoming messages/tasks:
//! - **Stage 1 (Rules):** Deterministic, zero-cost pattern matching for safe
//!   negatives (bulk mail, auto-replies, promotions, newsletters). Rules NEVER
//!   classify as "needs_you" — only "can_wait" or "auto_handle".
//! - **Stage 2 (Model):** LLM fallback with structured prompt + Zod-equivalent
//!   parsing for ambiguous cases. Model failure throws an exception rather than
//!   silently defaulting to a verdict.
//!
//! Every verdict carries a `decided_by` field for audit:
//! - `"rule:<rule_id>"` for Stage 1 decisions
//! - `"model:<model_name>"` for Stage 2 decisions
//!
//! Sources: OpenDuet `triage.ts`, `routine-firing.ts`, `initiator.ts`

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What initiated this triage request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração Initiator.
pub enum Initiator {
    // A human sent a message directly.
    Person(String),
    // A scheduled routine fired (e.g., inbox check, calendar prep).
    Routine(String),
    // An agent's responsibility triggered this (e.g., monitoring alert).
    Responsibility(String),
    // A memory consolidation or recall triggered this.
    Memory(String),
    // A handoff from another agent or system.
    Handoff { from: String, reason: String },
}

impl fmt::Display for Initiator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Person(name) => write!(f, "person:{name}"),
            Self::Routine(name) => write!(f, "routine:{name}"),
            Self::Responsibility(name) => write!(f, "responsibility:{name}"),
            Self::Memory(name) => write!(f, "memory:{name}"),
            Self::Handoff { from, reason } => write!(f, "handoff:{from}:{reason}"),
        }
    }
}

/// Triage verdict — what should happen with this item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração TriageVerdict.
pub enum TriageVerdict {
    // Item can be safely deferred; no immediate action needed.
    CanWait { reason: String },
    // Item can be automatically handled without human involvement.
    AutoHandle { action: String, reason: String },
    // Item requires human attention or decision.
    NeedsYou { urgency: Urgency, summary: String },
    // Item should be discarded (spam, irrelevant).
    Discard { reason: String },
}

/// Urgency level for items that need human attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração Urgency.
pub enum Urgency {
    /// Variante Low.
    Low,
    /// Variante Medium.
    Medium,
    /// Variante High.
    High,
    /// Variante Critical.
    Critical,
}

impl fmt::Display for Urgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// How a triage decision was made — critical for audit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Enumeração DecidedBy.
pub enum DecidedBy {
    // Stage 1 rule matched. Format: `rule:<rule_id>`.
    Rule(String),
    // Stage 2 model classified. Format: `model:<model_name>`.
    Model(String),
}

impl fmt::Display for DecidedBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(id) => write!(f, "rule:{id}"),
            Self::Model(name) => write!(f, "model:{name}"),
        }
    }
}

/// Complete triage result with audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageResult.
pub struct TriageResult {
    /// The verdict.
    pub verdict: TriageVerdict,
    /// How the decision was made (rule or model).
    pub decided_by: DecidedBy,
    /// Who/what initiated this triage.
    pub initiator: Initiator,
    /// When the decision was made.
    pub decided_at: DateTime<Utc>,
    /// Confidence score (0.0-1.0). Rules always produce 1.0.
    pub confidence: f64,
    /// The original input text (for audit/replay).
    pub input_summary: String,
}

/// A deterministic rule for Stage 1 triage.
///
/// Rules are evaluated in order; first match wins.
/// Rules MUST NOT produce `NeedsYou` verdicts — only safe negatives.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageRule.
pub struct TriageRule {
    /// Unique identifier for audit (`decided_by = "rule:<id>"`).
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Pattern to match against the input.
    pub pattern: RulePattern,
    /// The verdict to produce on match.
    pub verdict: TriageVerdict,
    /// Priority (lower = evaluated first).
    pub priority: u32,
    /// Whether this rule is enabled.
    pub enabled: bool,
}

/// Pattern matching strategy for rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Enumeração RulePattern.
pub enum RulePattern {
    // Case-insensitive substring match.
    Contains(String),
    // Regex pattern match (simplified contains-based approximation).
    Regex(String),
    // Sender/domain match (for email-like inputs).
    SenderDomain(String),
    // Header field match (key-value).
    HeaderMatch { key: String, value: String },
    // Composite: all sub-patterns must match.
    AllOf(Vec<RulePattern>),
    // Composite: any sub-pattern must match.
    AnyOf(Vec<RulePattern>),
    // Negation: sub-pattern must NOT match.
    Not(Box<RulePattern>),
}

/// Errors from the triage engine.
#[derive(Debug, Clone, thiserror::Error)]
/// Enumeração TriageError.
pub enum TriageError {
    #[error("model classification failed: {0}")]
    // Variante ModelFailure.
    ModelFailure(String),
    #[error("invalid rule pattern: {0}")]
    // Variante InvalidPattern.
    InvalidPattern(String),
    #[error("no rules configured and model unavailable")]
    /// Variante NoFallbackAvailable.
    NoFallbackAvailable,
    #[error("input too large for triage: {size} bytes (max {max})")]
    // Variante InputTooLarge.
    InputTooLarge { size: usize, max: usize },
}

/// Configuration for the two-stage triage engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageConfig.
pub struct TriageConfig {
    /// Maximum input size in bytes.
    pub max_input_bytes: usize,
    /// Model name for Stage 2 (used in `decided_by`).
    pub model_name: String,
    /// Whether Stage 2 is enabled.
    pub model_enabled: bool,
    /// Default verdict when no rules match and model is disabled.
    pub default_verdict: TriageVerdict,
}

impl Default for TriageConfig {
    fn default() -> Self {
        // Variante Self.
        Self {
            max_input_bytes: 100_000,
            model_name: "triage-llm".to_string(),
            model_enabled: false,
            default_verdict: TriageVerdict::CanWait {
                reason: "No rules matched and model disabled".to_string(),
            },
        }
    }
}

/// The two-stage message triage engine.
pub struct MessageTriageEngine {
    config: TriageConfig,
    rules: Vec<TriageRule>,
}

impl MessageTriageEngine {
    /// Função new.
    pub fn new(config: TriageConfig, mut rules: Vec<TriageRule>) -> Self {
        rules.sort_by_key(|r| r.priority);
        // Variante Self.
        Self { config, rules }
    }

    /// Add a rule at runtime.
    pub fn add_rule(&mut self, rule: TriageRule) {
        self.rules.push(rule);
        self.rules.sort_by_key(|r| r.priority);
    }

    /// Remove a rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let before = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        self.rules.len() < before
    }

    /// Get all active rules.
    pub fn active_rules(&self) -> impl Iterator<Item = &TriageRule> {
        self.rules.iter().filter(|r| r.enabled)
    }

    /// Classify an input through the two-stage pipeline.
    ///
    /// Stage 1: Try all enabled rules in priority order. First match wins.
    /// Stage 2: If no rule matches and model is enabled, call model.
    ///          Model failure → exception (never silent default).
    pub fn classify<F>(
        &self,
        input: &str,
        initiator: Initiator,
        model_fn: Option<F>,
    ) -> Result<TriageResult, TriageError>
    where
        F: FnOnce(&str) -> Result<TriageVerdict, String>,
    {
        if input.len() > self.config.max_input_bytes {
            return Err(TriageError::InputTooLarge {
                size: input.len(),
                max: self.config.max_input_bytes,
            });
        }

        let now = Utc::now();
        let input_summary = if input.len() > 500 {
            format!("{}...", &input[..500])
        } else {
            input.to_string()
        };

        // Stage 1: Rule matching
        for rule in self.active_rules() {
            if matches_pattern(&rule.pattern, input) {
                assert!(
                    !matches!(rule.verdict, TriageVerdict::NeedsYou { .. }),
                    "Rule '{}' produced NeedsYou verdict — rules must only produce safe negatives",
                    rule.id
                );

                return Ok(TriageResult {
                    verdict: rule.verdict.clone(),
                    decided_by: DecidedBy::Rule(rule.id.clone()),
                    initiator,
                    decided_at: now,
                    confidence: 1.0,
                    input_summary,
                });
            }
        }

        // Stage 2: Model fallback
        if self.config.model_enabled {
            let model_fn = model_fn.ok_or_else(|| {
                TriageError::ModelFailure(
                    "Model enabled but no model function provided".to_string(),
                )
            })?;

            let verdict = model_fn(input).map_err(TriageError::ModelFailure)?;

            return Ok(TriageResult {
                verdict,
                decided_by: DecidedBy::Model(self.config.model_name.clone()),
                initiator,
                decided_at: now,
                confidence: 0.85,
                input_summary,
            });
        }

        // No rules matched, model disabled → use configured default
        // Variante Ok.
        Ok(TriageResult {
            verdict: self.config.default_verdict.clone(),
            decided_by: DecidedBy::Rule("default".to_string()),
            initiator,
            decided_at: now,
            confidence: 1.0,
            input_summary,
        })
    }

    /// Create a standard set of built-in rules for common safe negatives.
    pub fn default_rules() -> Vec<TriageRule> {
        vec![
            // Variante TriageRule.
            TriageRule {
                id: "auto-reply".into(),
                description: "Auto-reply / out-of-office messages".into(),
                pattern: RulePattern::AnyOf(vec![
                    RulePattern::Contains("out of office".into()),
                    RulePattern::Contains("automatic reply".into()),
                    RulePattern::Contains("auto-reply".into()),
                    RulePattern::Contains("autoreply".into()),
                    RulePattern::Contains("I am currently out of the office".into()),
                ]),
                verdict: TriageVerdict::Discard {
                    reason: "Auto-reply message".into(),
                },
                priority: 10,
                enabled: true,
            },
            // Variante TriageRule.
            TriageRule {
                id: "newsletter".into(),
                description: "Newsletter / mailing list messages".into(),
                pattern: RulePattern::AnyOf(vec![
                    RulePattern::Contains("unsubscribe".into()),
                    RulePattern::Contains("newsletter".into()),
                    RulePattern::Contains("mailing list".into()),
                ]),
                verdict: TriageVerdict::CanWait {
                    reason: "Newsletter — read later".into(),
                },
                priority: 20,
                enabled: true,
            },
            // Variante TriageRule.
            TriageRule {
                id: "promotion".into(),
                description: "Promotional / marketing emails".into(),
                pattern: RulePattern::AnyOf(vec![
                    RulePattern::Contains("limited time offer".into()),
                    RulePattern::Contains("act now".into()),
                    RulePattern::Contains("special deal".into()),
                    RulePattern::Contains("don't miss out".into()),
                    RulePattern::Contains("exclusive offer".into()),
                ]),
                verdict: TriageVerdict::Discard {
                    reason: "Promotional content".into(),
                },
                priority: 30,
                enabled: true,
            },
            // Variante TriageRule.
            TriageRule {
                id: "notification".into(),
                description: "Automated notifications (CI, monitoring, etc.)".into(),
                pattern: RulePattern::AnyOf(vec![
                    RulePattern::Contains("build succeeded".into()),
                    RulePattern::Contains("build failed".into()),
                    RulePattern::Contains("deployment complete".into()),
                    RulePattern::Contains("alert resolved".into()),
                    RulePattern::SenderDomain("notifications.github.com".into()),
                    RulePattern::SenderDomain("noreply".into()),
                ]),
                verdict: TriageVerdict::CanWait {
                    reason: "Automated notification".into(),
                },
                priority: 40,
                enabled: true,
            },
            // Variante TriageRule.
            TriageRule {
                id: "bulk-mail".into(),
                description: "Bulk / mass mail indicators".into(),
                pattern: RulePattern::AllOf(vec![RulePattern::HeaderMatch {
                    key: "Precedence".into(),
                    value: "bulk".into(),
                }]),
                verdict: TriageVerdict::Discard {
                    reason: "Bulk mail".into(),
                },
                priority: 5,
                enabled: true,
            },
        ]
    }
}

/// Evaluate a rule pattern against an input string.
fn matches_pattern(pattern: &RulePattern, input: &str) -> bool {
    match pattern {
        RulePattern::Contains(substring) => {
            input.to_lowercase().contains(&substring.to_lowercase())
        }
        RulePattern::Regex(re_str) => input.contains(re_str.as_str()),
        RulePattern::SenderDomain(domain) => {
            input.to_lowercase().contains(&domain.to_lowercase())
        }
        RulePattern::HeaderMatch { key, value } => {
            let lower_input = input.to_lowercase();
            let lower_key = key.to_lowercase();
            if value.is_empty() {
                lower_input.contains(&lower_key)
            } else {
                lower_input.contains(&format!("{}: {}", lower_key, value.to_lowercase()))
            }
        }
        RulePattern::AllOf(patterns) => patterns.iter().all(|p| matches_pattern(p, input)),
        RulePattern::AnyOf(patterns) => patterns.iter().any(|p| matches_pattern(p, input)),
        RulePattern::Not(inner) => !matches_pattern(inner, input),
    }
}

/// Framing wrapper for scheduled instructions.
///
/// Prevents misinterpretation of scheduled/routine messages as urgent requests.
pub fn frame_scheduled_instruction(instruction: &str, routine_name: &str) -> String {
    format!(
        "[SCHEDULED ROUTINE: {}]\n\
         This is an automated instruction from a scheduled routine.\n\
         Do NOT treat this as an urgent human request.\n\
         ---\n\
         {}\n\
         ---\n\
         [END SCHEDULED ROUTINE]",
        routine_name, instruction
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_engine() -> MessageTriageEngine {
        MessageTriageEngine::new(TriageConfig::default(), MessageTriageEngine::default_rules())
    }

    #[test]
    fn test_auto_reply_discarded() {
        let engine = test_engine();
        let result = engine
            .classify(
                "I am currently out of the office until Monday.",
                Initiator::Person("boss".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();

        assert!(matches!(result.verdict, TriageVerdict::Discard { .. }));
        assert!(matches!(result.decided_by, DecidedBy::Rule(ref id) if id == "auto-reply"));
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_newsletter_can_wait() {
        let engine = test_engine();
        let result = engine
            .classify(
                "This week's newsletter: top stories. Click unsubscribe to opt out.",
                Initiator::Routine("inbox-check".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();

        assert!(matches!(result.verdict, TriageVerdict::CanWait { .. }));
        assert!(matches!(result.decided_by, DecidedBy::Rule(ref id) if id == "newsletter"));
    }

    #[test]
    fn test_promotion_discarded() {
        let engine = test_engine();
        let result = engine
            .classify(
                "Limited time offer! Don't miss out on our special deal!",
                Initiator::Person("unknown".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();

        assert!(matches!(result.verdict, TriageVerdict::Discard { .. }));
    }

    #[test]
    fn test_no_match_uses_default() {
        let engine = test_engine();
        let result = engine
            .classify(
                "Hey, can you review this PR when you get a chance?",
                Initiator::Person("colleague".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();

        assert!(matches!(result.verdict, TriageVerdict::CanWait { .. }));
        assert!(matches!(result.decided_by, DecidedBy::Rule(ref id) if id == "default"));
    }

    #[test]
    fn test_model_fallback_when_enabled() {
        let config = TriageConfig {
            model_enabled: true,
            model_name: "gpt-triage".into(),
            ..Default::default()
        };
        let engine = MessageTriageEngine::new(config, MessageTriageEngine::default_rules());

        let model_fn = |_input: &str| -> Result<TriageVerdict, String> {
            // Variante Ok.
            Ok(TriageVerdict::NeedsYou {
                urgency: Urgency::High,
                summary: "PR review request from colleague".into(),
            })
        };

        let result = engine
            .classify(
                "Hey, can you review this PR when you get a chance?",
                Initiator::Person("colleague".into()),
                // Variante Some.
                Some(model_fn),
            )
            .unwrap();

        assert!(matches!(result.verdict, TriageVerdict::NeedsYou { .. }));
        assert!(matches!(result.decided_by, DecidedBy::Model(ref name) if name == "gpt-triage"));
    }

    #[test]
    fn test_model_failure_throws_not_defaults() {
        let config = TriageConfig {
            model_enabled: true,
            model_name: "gpt-triage".into(),
            ..Default::default()
        };
        let engine = MessageTriageEngine::new(config, MessageTriageEngine::default_rules());

        let model_fn = |_input: &str| -> Result<TriageVerdict, String> {
            // Variante Err.
            Err("API rate limit exceeded".into())
        };

        let result = engine.classify(
            "Important message that doesn't match any rule",
            Initiator::Person("ceo".into()),
            Some(model_fn),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TriageError::ModelFailure(_)));
    }

    #[test]
    fn test_model_enabled_but_no_fn_provided() {
        let config = TriageConfig {
            model_enabled: true,
            ..Default::default()
        };
        let engine = MessageTriageEngine::new(config, MessageTriageEngine::default_rules());

        let result = engine.classify(
            "Some message",
            Initiator::Person("x".into()),
            None::<fn(&str) -> Result<TriageVerdict, String>>,
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TriageError::ModelFailure(_)));
    }

    #[test]
    fn test_input_too_large_rejected() {
        let config = TriageConfig {
            max_input_bytes: 100,
            ..Default::default()
        };
        let engine = MessageTriageEngine::new(config, vec![]);

        let result = engine.classify(
            &"x".repeat(200),
            Initiator::Person("x".into()),
            None::<fn(&str) -> Result<TriageVerdict, String>>,
        );

        assert!(matches!(
            result.unwrap_err(),
            TriageError::InputTooLarge { .. }
        ));
    }

    #[test]
    fn test_decided_by_always_populated() {
        let engine = test_engine();

        let r1 = engine
            .classify(
                "unsubscribe from this newsletter",
                Initiator::Routine("check".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();
        match &r1.decided_by {
            DecidedBy::Rule(id) => assert!(!id.is_empty()),
            _ => panic!("Expected Rule decided_by"),
        }

        let r2 = engine
            .classify(
                "random text that matches nothing",
                Initiator::Person("x".into()),
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();
        match &r2.decided_by {
            DecidedBy::Rule(id) => assert_eq!(id, "default"),
            _ => panic!("Expected Rule(default) decided_by"),
        }
    }

    #[test]
    fn test_initiator_tracking() {
        let engine = test_engine();

        let r = engine
            .classify(
                "unsubscribe",
                Initiator::Handoff {
                    from: "email-agent".into(),
                    reason: "inbox overflow".into(),
                },
                None::<fn(&str) -> Result<TriageVerdict, String>>,
            )
            .unwrap();

        assert!(matches!(r.initiator, Initiator::Handoff { .. }));
    }

    #[test]
    fn test_framing_wrapper() {
        let framed = frame_scheduled_instruction("Check inbox and triage", "morning-routine");
        assert!(framed.contains("[SCHEDULED ROUTINE: morning-routine]"));
        assert!(framed.contains("Do NOT treat this as an urgent human request"));
        assert!(framed.contains("Check inbox and triage"));
        assert!(framed.contains("[END SCHEDULED ROUTINE]"));
    }

    #[test]
    fn test_add_remove_rule() {
        let mut engine = test_engine();
        let initial_count = engine.active_rules().count();

        engine.add_rule(TriageRule {
            id: "custom-spam".into(),
            description: "Custom spam filter".into(),
            pattern: RulePattern::Contains("buy now".into()),
            verdict: TriageVerdict::Discard {
                reason: "Spam".into(),
            },
            priority: 100,
            enabled: true,
        });

        assert_eq!(engine.active_rules().count(), initial_count + 1);

        assert!(engine.remove_rule("custom-spam"));
        assert_eq!(engine.active_rules().count(), initial_count);

        assert!(!engine.remove_rule("nonexistent"));
    }

    #[test]
    fn test_all_of_pattern() {
        let pattern = RulePattern::AllOf(vec![
            RulePattern::Contains("hello".into()),
            RulePattern::Contains("world".into()),
        ]);

        assert!(matches_pattern(&pattern, "hello world"));
        assert!(!matches_pattern(&pattern, "hello there"));
        assert!(!matches_pattern(&pattern, "goodbye world"));
    }

    #[test]
    fn test_any_of_pattern() {
        let pattern = RulePattern::AnyOf(vec![
            RulePattern::Contains("foo".into()),
            RulePattern::Contains("bar".into()),
        ]);

        assert!(matches_pattern(&pattern, "foo baz"));
        assert!(matches_pattern(&pattern, "baz bar"));
        assert!(!matches_pattern(&pattern, "baz qux"));
    }

    #[test]
    fn test_not_pattern() {
        let pattern = RulePattern::Not(Box::new(RulePattern::Contains("spam".into())));

        assert!(matches_pattern(&pattern, "hello world"));
        assert!(!matches_pattern(&pattern, "this is spam"));
    }

    #[test]
    fn test_case_insensitive_contains() {
        let pattern = RulePattern::Contains("UNSUBSCRIBE".into());
        assert!(matches_pattern(&pattern, "click here to unsubscribe"));
        assert!(matches_pattern(&pattern, "Click Here To UNSUBSCRIBE"));
    }

    #[test]
    fn test_display_formatting() {
        assert_eq!(format!("{}", Urgency::Critical), "critical");
        assert_eq!(format!("{}", DecidedBy::Rule("r1".into())), "rule:r1");
        assert_eq!(
            format!("{}", DecidedBy::Model("gpt-4".into())),
            "model:gpt-4"
        );
        assert_eq!(
            format!("{}", Initiator::Person("alice".into())),
            "person:alice"
        );
        assert_eq!(
            format!("{}", Initiator::Routine("morning".into())),
            "routine:morning"
        );
    }

    #[test]
    fn test_rules_sorted_by_priority() {
        let rules = vec![
            /// Variante TriageRule.
            TriageRule {
                id: "low".into(),
                description: "".into(),
                pattern: RulePattern::Contains("x".into()),
                verdict: TriageVerdict::Discard { reason: "".into() },
                priority: 100,
                enabled: true,
            },
            /// Variante TriageRule.
            TriageRule {
                id: "high".into(),
                description: "".into(),
                pattern: RulePattern::Contains("x".into()),
                verdict: TriageVerdict::CanWait { reason: "".into() },
                priority: 1,
                enabled: true,
            },
        ];

        let engine = MessageTriageEngine::new(TriageConfig::default(), rules);
        let ids: Vec<&str> = engine.active_rules().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, vec!["high", "low"]);
    }
}