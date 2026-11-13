//! Alert Rules Engine with YAML DSL (T11.7.3).
//!
//! Loads alert rules from YAML files, evaluates them against metrics,
//! and produces alerts with severity classification and cooldown deduplication.
//!
//! # YAML Rule Format
//!
//! ```yaml
//! name: high-error-rate
//! description: "Error rate exceeds threshold"
//! metric: error_rate
//! operator: ">"
//! threshold: 1.0
//! window_minutes: 5
//! severity: SEV-2
//! cooldown_minutes: 15
//! notify:
//!   - slack: "#alerts-zylos"
//!   - sentry: true
//! remediation:
//!   action: notify-oncall
//!   gated: true
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

/// Comparison operators for rule evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Operator {
    #[serde(rename = ">")]
    GreaterThan,
    #[serde(rename = ">=")]
    GreaterThanOrEqual,
    #[serde(rename = "<")]
    LessThan,
    #[serde(rename = "<=")]
    LessThanOrEqual,
    #[serde(rename = "==")]
    Equal,
    #[serde(rename = "!=")]
    NotEqual,
}

impl Operator {
    /// Evaluate the comparison against a numeric value.
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            Operator::GreaterThan => value > threshold,
            Operator::GreaterThanOrEqual => value >= threshold,
            Operator::LessThan => value < threshold,
            Operator::LessThanOrEqual => value <= threshold,
            Operator::Equal => (value - threshold).abs() < f64::EPSILON,
            Operator::NotEqual => (value - threshold).abs() >= f64::EPSILON,
        }
    }
}

/// Severity level for alerts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AlertSeverity {
    #[serde(rename = "SEV-1")]
    Sev1,
    #[serde(rename = "SEV-2")]
    Sev2,
    #[serde(rename = "SEV-3")]
    Sev3,
    #[serde(rename = "SEV-4")]
    Sev4,
}

impl AlertSeverity {
    /// Response time SLA for this severity.
    pub fn response_time(&self) -> Duration {
        match self {
            AlertSeverity::Sev1 => Duration::from_secs(5 * 60),
            AlertSeverity::Sev2 => Duration::from_secs(30 * 60),
            AlertSeverity::Sev3 => Duration::from_secs(2 * 60 * 60),
            AlertSeverity::Sev4 => Duration::from_secs(24 * 60 * 60),
        }
    }
}

/// Notification target for an alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NotifyTarget {
    Slack { slack: String },
    Sentry { sentry: bool },
    PagerDuty { pagerduty: String },
    Webhook { webhook: String },
}

/// Remediation action to take when alert fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Remediation {
    /// Action identifier (e.g., "restart-gateway", "notify-oncall", "none").
    pub action: String,
    /// Whether this action requires human approval before execution.
    #[serde(default)]
    pub gated: bool,
    /// Maximum automatic executions within cooldown window.
    #[serde(default)]
    pub max_auto_executions: Option<u32>,
}

/// A single alert rule loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    /// Unique rule identifier.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
    /// Metric name to evaluate (must match a key in the metrics map).
    pub metric: String,
    /// Comparison operator.
    pub operator: Operator,
    /// Threshold value for comparison.
    pub threshold: f64,
    /// Evaluation window in minutes.
    #[serde(default = "default_window")]
    pub window_minutes: u32,
    /// Alert severity.
    pub severity: AlertSeverity,
    /// Cooldown period in minutes to prevent duplicate alerts.
    #[serde(default = "default_cooldown")]
    pub cooldown_minutes: u32,
    /// Notification targets.
    #[serde(default)]
    pub notify: Vec<NotifyTarget>,
    /// Optional remediation action.
    #[serde(default)]
    pub remediation: Option<Remediation>,
}

fn default_window() -> u32 {
    5
}

fn default_cooldown() -> u32 {
    15
}

/// An alert that has been triggered by rule evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggeredAlert {
 /// The rule that triggered this alert.
 pub rule_name: String,
 /// Current metric value that exceeded threshold.
 pub current_value: f64,
 /// Threshold from the rule.
 pub threshold: f64,
 /// Severity classification.
 pub severity: AlertSeverity,
 /// When the alert was triggered (Unix epoch seconds).
 pub triggered_at_epoch_secs: u64,
 /// Description from the rule.
 pub description: String,
}

/// State tracker for alert cooldowns.
#[derive(Debug)]
struct CooldownEntry {
    last_fired: Instant,
    auto_execution_count: u32,
}

/// Alert rules engine that loads YAML rules and evaluates metrics.
#[derive(Debug)]
pub struct AlertRulesEngine {
    rules: Vec<AlertRule>,
    cooldowns: HashMap<String, CooldownEntry>,
}

impl AlertRulesEngine {
    /// Create a new engine with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            cooldowns: HashMap::new(),
        }
    }

    /// Load rules from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the YAML is malformed or doesn't match the schema.
    pub fn load_from_yaml(yaml_content: &str) -> Result<Self, String> {
        let rules: Vec<AlertRule> = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse alert rules YAML: {}", e))?;

        // Validate rule names are unique
        let mut seen = std::collections::HashSet::new();
        for rule in &rules {
            if !seen.insert(&rule.name) {
                return Err(format!("Duplicate alert rule name: '{}'", rule.name));
            }
        }

        Ok(Self {
            rules,
            cooldowns: HashMap::new(),
        })
    }

    /// Load rules from a YAML file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub async fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let content = tokio::fs::read_to_string(path.as_ref())
            .await
            .map_err(|e| format!("Failed to read alert rules file: {}", e))?;
        Self::load_from_yaml(&content)
    }

    /// Add a single rule programmatically.
    pub fn add_rule(&mut self, rule: AlertRule) {
        self.rules.push(rule);
    }

    /// Get the number of loaded rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Evaluate all rules against the provided metrics map.
    ///
    /// Returns a list of triggered alerts (respecting cooldowns).
    /// Metrics keys should match `AlertRule::metric` field values.
    pub fn evaluate(&mut self, metrics: &HashMap<String, f64>) -> Vec<TriggeredAlert> {
        let now = Instant::now();
        let mut triggered = Vec::new();

        for rule in &self.rules {
            // Check if metric exists
            let Some(value) = metrics.get(&rule.metric) else {
                continue;
            };

            // Evaluate condition
            if !rule.operator.evaluate(*value, rule.threshold) {
                continue;
            }

            // Check cooldown
            if let Some(entry) = self.cooldowns.get(&rule.name) {
                let cooldown = Duration::from_secs(rule.cooldown_minutes as u64 * 60);
                if now.duration_since(entry.last_fired) < cooldown {
                    continue; // Still in cooldown
                }
            }

            // Alert triggered — update cooldown
            let entry = self.cooldowns.entry(rule.name.clone()).or_insert(CooldownEntry {
                last_fired: now,
                auto_execution_count: 0,
            });
            entry.last_fired = now;
            entry.auto_execution_count += 1;

            triggered.push(TriggeredAlert {
                        rule_name: rule.name.clone(),
                        current_value: *value,
                        threshold: rule.threshold,
                        severity: rule.severity,
                        triggered_at_epoch_secs: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                        description: rule.description.clone(),
                    });
        }

        triggered
    }

    /// Check if a rule's remediation can auto-execute (respects max_auto_executions).
    pub fn can_auto_remediate(&self, rule_name: &str) -> bool {
        let Some(rule) = self.rules.iter().find(|r| r.name == rule_name) else {
            return false;
        };

        let Some(remediation) = &rule.remediation else {
            return false;
        };

        if remediation.gated {
            return false;
        }

        if let Some(max) = remediation.max_auto_executions {
            if let Some(entry) = self.cooldowns.get(rule_name) {
                return entry.auto_execution_count <= max;
            }
        }

        true
    }

    /// Get all loaded rules (for inspection/testing).
    pub fn rules(&self) -> &[AlertRule] {
        &self.rules
    }

    /// Reset cooldown for a specific rule (useful after manual resolution).
    pub fn reset_cooldown(&mut self, rule_name: &str) {
        self.cooldowns.remove(rule_name);
    }

    /// Clear all cooldowns.
    pub fn clear_cooldowns(&mut self) {
        self.cooldowns.clear();
    }
}

impl Default for AlertRulesEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RULES: &str = r##"
- name: high-error-rate
  description: "Error rate exceeds 1%"
  metric: error_rate
  operator: ">"
  threshold: 1.0
  window_minutes: 5
  severity: SEV-2
  cooldown_minutes: 15
  notify:
    - slack: "#alerts-zylos"
  remediation:
    action: notify-oncall
    gated: true

- name: high-latency
  description: "P95 latency exceeds 2s"
  metric: latency_p95_ms
  operator: ">"
  threshold: 2000.0
  severity: SEV-3
  cooldown_minutes: 30
"##;

    #[test]
    fn test_load_rules_from_yaml() {
        let engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        assert_eq!(engine.rule_count(), 2);
        assert_eq!(engine.rules()[0].name, "high-error-rate");
        assert_eq!(engine.rules()[1].name, "high-latency");
    }

    #[test]
    fn test_evaluate_triggers_alert() {
        let mut engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        let mut metrics = HashMap::new();
        metrics.insert("error_rate".to_string(), 2.5);

        let alerts = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_name, "high-error-rate");
        assert_eq!(alerts[0].current_value, 2.5);
        assert_eq!(alerts[0].severity, AlertSeverity::Sev2);
    }

    #[test]
    fn test_evaluate_no_trigger_below_threshold() {
        let mut engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        let mut metrics = HashMap::new();
        metrics.insert("error_rate".to_string(), 0.5);

        let alerts = engine.evaluate(&metrics);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_cooldown_prevents_duplicate() {
        let mut engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        let mut metrics = HashMap::new();
        metrics.insert("error_rate".to_string(), 5.0);

        // First evaluation triggers
        let alerts = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1);

        // Second evaluation within cooldown does not trigger
        let alerts = engine.evaluate(&metrics);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_missing_metric_skipped() {
        let mut engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        let metrics = HashMap::new(); // Empty — no metrics

        let alerts = engine.evaluate(&metrics);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_duplicate_rule_name_rejected() {
        let yaml = r#"
- name: same-name
  metric: x
  operator: ">"
  threshold: 1.0
  severity: SEV-1
- name: same-name
  metric: y
  operator: ">"
  threshold: 2.0
  severity: SEV-2
"#;
        let result = AlertRulesEngine::load_from_yaml(yaml);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_operator_evaluation() {
        assert!(Operator::GreaterThan.evaluate(2.0, 1.0));
        assert!(!Operator::GreaterThan.evaluate(1.0, 1.0));
        assert!(Operator::GreaterThanOrEqual.evaluate(1.0, 1.0));
        assert!(Operator::LessThan.evaluate(0.5, 1.0));
        assert!(Operator::Equal.evaluate(1.0, 1.0));
        assert!(Operator::NotEqual.evaluate(1.0, 2.0));
    }

    #[test]
    fn test_can_auto_remediate_gated() {
        let engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        // high-error-rate has gated: true
        assert!(!engine.can_auto_remediate("high-error-rate"));
        // high-latency has no remediation
        assert!(!engine.can_auto_remediate("high-latency"));
    }

    #[test]
    fn test_reset_cooldown() {
        let mut engine = AlertRulesEngine::load_from_yaml(SAMPLE_RULES).unwrap();
        let mut metrics = HashMap::new();
        metrics.insert("error_rate".to_string(), 5.0);

        // Trigger alert
        engine.evaluate(&metrics);
        // Should be in cooldown
        assert!(engine.evaluate(&metrics).is_empty());

        // Reset cooldown
        engine.reset_cooldown("high-error-rate");
        // Should trigger again
        let alerts = engine.evaluate(&metrics);
        assert_eq!(alerts.len(), 1);
    }
}