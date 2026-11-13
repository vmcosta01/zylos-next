//! Notification Router (T11.8.5)
//!
//! Dispatches notifications to configured channel adapters (Slack, Notion, Linear)
//! based on alert rules and routing configuration. Integrates with the sentinel
//! AlertRulesEngine and zylos-integrations ChannelAdapter trait.

use serde::{Deserialize, Serialize};
use tracing::info;

/// Routing rule that maps alert patterns to notification targets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Rule name for logging/debugging.
    pub name: String,
    /// Alert rule name pattern to match (exact match against TriggeredAlert.rule_name).
    pub alert_rule: String,
    /// Minimum severity to trigger this route (SEV-1 is highest priority).
    pub min_severity: SeverityFilter,
    /// Target platform ID (must match ChannelAdapter::platform_id()).
    pub target_platform: String,
    /// Target recipient (channel ID, page ID, issue ID depending on platform).
    pub target_recipient: String,
    /// Optional message template with placeholders: {rule_name}, {value}, {threshold}, {severity}, {description}.
    #[serde(default = "default_template")]
    pub message_template: String,
}

fn default_template() -> String {
    "🚨 [{severity}] {rule_name}: {description} (value={value}, threshold={threshold})".to_string()
}

/// Severity filter for routing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeverityFilter {
    #[serde(rename = "SEV-1")]
    Sev1,
    #[serde(rename = "SEV-2")]
    Sev2,
    #[serde(rename = "SEV-3")]
    Sev3,
    #[serde(rename = "SEV-4")]
    Sev4,
}

impl SeverityFilter {
    /// Returns numeric priority (lower = more severe).
    fn priority(&self) -> u8 {
        match self {
            SeverityFilter::Sev1 => 1,
            SeverityFilter::Sev2 => 2,
            SeverityFilter::Sev3 => 3,
            SeverityFilter::Sev4 => 4,
        }
    }

    /// Check if an alert severity meets this filter's minimum.
    pub fn matches(&self, alert_severity: &zylos_sentinel::alert_rules::AlertSeverity) -> bool {
        let alert_priority = match alert_severity {
            zylos_sentinel::alert_rules::AlertSeverity::Sev1 => 1,
            zylos_sentinel::alert_rules::AlertSeverity::Sev2 => 2,
            zylos_sentinel::alert_rules::AlertSeverity::Sev3 => 3,
            zylos_sentinel::alert_rules::AlertSeverity::Sev4 => 4,
        };
        alert_priority <= self.priority()
    }
}

/// Configuration for the notification router.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRouterConfig {
    /// Routing rules evaluated in order; first match wins.
    pub rules: Vec<RoutingRule>,
    /// Default platform if no rule matches (optional).
    pub default_platform: Option<String>,
    /// Default recipient for unmatched alerts (optional).
    pub default_recipient: Option<String>,
}

impl Default for NotificationRouterConfig {
    fn default() -> Self {
        Self {
            rules: vec![],
            default_platform: None,
            default_recipient: None,
        }
    }
}

/// Result of a notification dispatch attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DispatchResult {
    pub rule_name: String,
    pub target_platform: String,
    pub success: bool,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

/// Notification router that evaluates routing rules and dispatches to adapters.
pub struct NotificationRouter {
    config: NotificationRouterConfig,
}

impl NotificationRouter {
    /// Create a new router with the given configuration.
    pub fn new(config: NotificationRouterConfig) -> Self {
        info!(
            rule_count = config.rules.len(),
            "Notification router initialized"
        );
        Self { config }
    }

    /// Load configuration from a YAML string.
    pub fn from_yaml(yaml_content: &str) -> Result<Self, String> {
        let config: NotificationRouterConfig = serde_yaml::from_str(yaml_content)
            .map_err(|e| format!("Failed to parse notification router config: {}", e))?;
        Ok(Self::new(config))
    }

    /// Find the matching routing rule for an alert.
    pub fn find_route(
        &self,
        alert: &zylos_sentinel::alert_rules::TriggeredAlert,
    ) -> Option<&RoutingRule> {
        for rule in &self.config.rules {
            if rule.alert_rule == alert.rule_name
                && rule.min_severity.matches(&alert.severity)
            {
                return Some(rule);
            }
        }
        None
    }

    /// Format a notification message from a template and alert data.
    pub fn format_message(template: &str, alert: &zylos_sentinel::alert_rules::TriggeredAlert) -> String {
        let severity_str = match alert.severity {
            zylos_sentinel::alert_rules::AlertSeverity::Sev1 => "SEV-1",
            zylos_sentinel::alert_rules::AlertSeverity::Sev2 => "SEV-2",
            zylos_sentinel::alert_rules::AlertSeverity::Sev3 => "SEV-3",
            zylos_sentinel::alert_rules::AlertSeverity::Sev4 => "SEV-4",
        };

        template
            .replace("{rule_name}", &alert.rule_name)
            .replace("{value}", &format!("{:.2}", alert.current_value))
            .replace("{threshold}", &format!("{:.2}", alert.threshold))
            .replace("{severity}", severity_str)
            .replace("{description}", &alert.description)
    }

    /// Get all configured routing rules (for inspection/testing).
    pub fn rules(&self) -> &[RoutingRule] {
        &self.config.rules
    }

    /// Get the number of routing rules.
    pub fn rule_count(&self) -> usize {
        self.config.rules.len()
    }
}

impl Default for NotificationRouter {
    fn default() -> Self {
        Self::new(NotificationRouterConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zylos_sentinel::alert_rules::{AlertSeverity, TriggeredAlert};
    use std::time::SystemTime;

    fn make_alert(rule_name: &str, severity: AlertSeverity, value: f64) -> TriggeredAlert {
        TriggeredAlert {
            rule_name: rule_name.to_string(),
            current_value: value,
            threshold: 1.0,
            severity,
            triggered_at_epoch_secs: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            description: "Test alert".to_string(),
        }
    }

    const SAMPLE_CONFIG: &str = r##"
rules:
  - name: critical-to-slack
    alert_rule: health-check-down
    min_severity: SEV-1
    target_platform: slack
    target_recipient: "#alerts-critical"
    message_template: "🔴 CRITICAL: {rule_name} — {description}"

  - name: warnings-to-slack
    alert_rule: high-error-rate
    min_severity: SEV-2
    target_platform: slack
    target_recipient: "#alerts-zylos"

  - name: latency-to-notion
    alert_rule: high-latency-p95
    min_severity: SEV-3
    target_platform: notion
    target_recipient: "page-id-123"
"##;

    #[test]
    fn test_load_config_from_yaml() {
        let router = NotificationRouter::from_yaml(SAMPLE_CONFIG).unwrap();
        assert_eq!(router.rule_count(), 3);
        assert_eq!(router.rules()[0].name, "critical-to-slack");
    }

    #[test]
    fn test_find_route_exact_match() {
        let router = NotificationRouter::from_yaml(SAMPLE_CONFIG).unwrap();
        let alert = make_alert("health-check-down", AlertSeverity::Sev1, 5.0);
        let route = router.find_route(&alert);
        assert!(route.is_some());
        assert_eq!(route.unwrap().target_platform, "slack");
        assert_eq!(route.unwrap().target_recipient, "#alerts-critical");
    }

    #[test]
    fn test_find_route_severity_filter() {
        let router = NotificationRouter::from_yaml(SAMPLE_CONFIG).unwrap();

        // SEV-2 alert should NOT match SEV-1-only rule
        let alert = make_alert("health-check-down", AlertSeverity::Sev2, 5.0);
        let route = router.find_route(&alert);
        assert!(route.is_none(), "SEV-2 should not match SEV-1 min_severity rule");

        // SEV-2 alert SHOULD match SEV-2 rule
        let alert2 = make_alert("high-error-rate", AlertSeverity::Sev2, 2.5);
        let route2 = router.find_route(&alert2);
        assert!(route2.is_some());
        assert_eq!(route2.unwrap().target_recipient, "#alerts-zylos");
    }

    #[test]
    fn test_find_route_no_match() {
        let router = NotificationRouter::from_yaml(SAMPLE_CONFIG).unwrap();
        let alert = make_alert("unknown-rule", AlertSeverity::Sev1, 99.0);
        assert!(router.find_route(&alert).is_none());
    }

    #[test]
    fn test_format_message() {
        let alert = make_alert("high-error-rate", AlertSeverity::Sev2, 2.5);
        let template = "[{severity}] {rule_name}: value={value} > threshold={threshold}";
        let msg = NotificationRouter::format_message(template, &alert);
        assert_eq!(msg, "[SEV-2] high-error-rate: value=2.50 > threshold=1.00");
    }

    #[test]
    fn test_format_message_default_template() {
        let alert = make_alert("test-rule", AlertSeverity::Sev3, 42.0);
        let template = default_template();
        let msg = NotificationRouter::format_message(&template, &alert);
        assert!(msg.contains("SEV-3"));
        assert!(msg.contains("test-rule"));
        assert!(msg.contains("42.00"));
    }

    #[test]
    fn test_severity_filter_matches() {
        assert!(SeverityFilter::Sev1.matches(&AlertSeverity::Sev1));
        assert!(!SeverityFilter::Sev1.matches(&AlertSeverity::Sev2));
        assert!(SeverityFilter::Sev2.matches(&AlertSeverity::Sev1));
        assert!(SeverityFilter::Sev2.matches(&AlertSeverity::Sev2));
        assert!(!SeverityFilter::Sev2.matches(&AlertSeverity::Sev3));
        assert!(SeverityFilter::Sev4.matches(&AlertSeverity::Sev1));
        assert!(SeverityFilter::Sev4.matches(&AlertSeverity::Sev4));
    }

    #[test]
    fn test_empty_config() {
        let router = NotificationRouter::default();
        assert_eq!(router.rule_count(), 0);
        let alert = make_alert("any", AlertSeverity::Sev1, 1.0);
        assert!(router.find_route(&alert).is_none());
    }
}