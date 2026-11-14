//! Synthetic anomaly detection tests for zylos-sentinel (T11.7.7).
//!
//! Validates that the AlertRulesEngine correctly detects various anomaly patterns:
//! - Spike detection (sudden value increase)
//! - Threshold breach (sustained high value)
//! - Multi-metric correlation
//! - Cooldown deduplication under sustained anomaly
//! - Severity classification accuracy
//! - Missing metric resilience
//! - Operator edge cases (==, !=, <=, >=)
//! - Reset cooldown recovery

use std::collections::HashMap;
use std::time::SystemTime;
use zylos_sentinel::alert_rules::{AlertRulesEngine, AlertSeverity};

const COMPREHENSIVE_RULES: &str = r##"
- name: cpu-spike
  description: "CPU usage spike detected"
  metric: cpu_percent
  operator: ">"
  threshold: 80.0
  severity: SEV-2
  cooldown_minutes: 5

- name: memory-critical
  description: "Memory usage critical"
  metric: memory_percent
  operator: ">="
  threshold: 95.0
  severity: SEV-1
  cooldown_minutes: 10

- name: error-rate-elevated
  description: "Error rate above normal"
  metric: error_rate
  operator: ">"
  threshold: 5.0
  severity: SEV-3
  cooldown_minutes: 15

- name: latency-degraded
  description: "Latency degraded below target"
  metric: requests_per_sec
  operator: "<"
  threshold: 100.0
  severity: SEV-2
  cooldown_minutes: 10

- name: disk-exact-match
  description: "Disk at exact threshold"
  metric: disk_percent
  operator: "=="
  threshold: 90.0
  severity: SEV-3
  cooldown_minutes: 30

- name: queue-not-empty
  description: "Queue has items"
  metric: queue_depth
  operator: "!="
  threshold: 0.0
  severity: SEV-4
  cooldown_minutes: 5

- name: connections-low
  description: "Active connections below minimum"
  metric: active_connections
  operator: "<="
  threshold: 5.0
  severity: SEV-2
  cooldown_minutes: 10
"##;

fn make_metrics(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

#[test]
fn test_spike_detection_cpu_above_80() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Normal CPU — no alert
    let normal = make_metrics(&[("cpu_percent", 45.0)]);
    assert!(engine.evaluate(&normal).is_empty());

    // Spike to 95% — should trigger
    let spike = make_metrics(&[("cpu_percent", 95.0)]);
    let alerts = engine.evaluate(&spike);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "cpu-spike");
    assert_eq!(alerts[0].severity, AlertSeverity::Sev2);
    assert_eq!(alerts[0].current_value, 95.0);
    assert_eq!(alerts[0].threshold, 80.0);
}

#[test]
fn test_critical_memory_at_exact_threshold() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // At exactly 95% with >= operator — should trigger
    let metrics = make_metrics(&[("memory_percent", 95.0)]);
    let alerts = engine.evaluate(&metrics);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "memory-critical");
    assert_eq!(alerts[0].severity, AlertSeverity::Sev1);
}

#[test]
fn test_below_normal_memory_no_alert() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    let metrics = make_metrics(&[("memory_percent", 70.0)]);
    assert!(engine.evaluate(&metrics).is_empty());
}

#[test]
fn test_multi_metric_simultaneous_alerts() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Multiple metrics breaching thresholds simultaneously
    let metrics = make_metrics(&[
        ("cpu_percent", 90.0),
        ("memory_percent", 97.0),
        ("error_rate", 8.0),
        ("requests_per_sec", 50.0),
    ]);

    let alerts = engine.evaluate(&metrics);
    assert_eq!(alerts.len(), 4, "Expected 4 simultaneous alerts");

    let rule_names: Vec<&str> = alerts.iter().map(|a| a.rule_name.as_str()).collect();
    assert!(rule_names.contains(&"cpu-spike"));
    assert!(rule_names.contains(&"memory-critical"));
    assert!(rule_names.contains(&"error-rate-elevated"));
    assert!(rule_names.contains(&"latency-degraded"));
}

#[test]
fn test_cooldown_dedup_under_sustained_anomaly() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    let metrics = make_metrics(&[("cpu_percent", 95.0)]);

    // First evaluation triggers
    let first = engine.evaluate(&metrics);
    assert_eq!(first.len(), 1);

    // Subsequent evaluations within cooldown do NOT trigger
    for _ in 0..10 {
        let subsequent = engine.evaluate(&metrics);
        assert!(
            subsequent.is_empty(),
            "Cooldown should prevent duplicate alerts"
        );
    }
}

#[test]
fn test_reset_cooldown_allows_re_trigger() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    let metrics = make_metrics(&[("cpu_percent", 95.0)]);

    // Trigger once
    assert_eq!(engine.evaluate(&metrics).len(), 1);

    // In cooldown
    assert!(engine.evaluate(&metrics).is_empty());

    // Reset cooldown
    engine.reset_cooldown("cpu-spike");

    // Should trigger again
    let re_triggered = engine.evaluate(&metrics);
    assert_eq!(re_triggered.len(), 1);
    assert_eq!(re_triggered[0].rule_name, "cpu-spike");
}

#[test]
fn test_clear_all_cooldowns() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Trigger multiple rules
    let metrics = make_metrics(&[
        ("cpu_percent", 95.0),
        ("memory_percent", 98.0),
        ("error_rate", 10.0),
    ]);
    assert_eq!(engine.evaluate(&metrics).len(), 3);

    // All in cooldown
    assert!(engine.evaluate(&metrics).is_empty());

    // Clear all
    engine.clear_cooldowns();

    // All should trigger again
    assert_eq!(engine.evaluate(&metrics).len(), 3);
}

#[test]
fn test_less_than_operator_requests_per_sec() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Below threshold — should trigger (< 100)
    let low = make_metrics(&[("requests_per_sec", 50.0)]);
    let alerts = engine.evaluate(&low);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "latency-degraded");

    // Above threshold — no alert
    let high = make_metrics(&[("requests_per_sec", 200.0)]);
    assert!(engine.evaluate(&high).is_empty());
}

#[test]
fn test_equal_operator_exact_match() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Exact match — should trigger
    let exact = make_metrics(&[("disk_percent", 90.0)]);
    let alerts = engine.evaluate(&exact);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "disk-exact-match");

    // Not equal — no alert
    let not_exact = make_metrics(&[("disk_percent", 89.9)]);
    assert!(engine.evaluate(&not_exact).is_empty());
}

#[test]
fn test_not_equal_operator_queue_depth() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Non-zero queue — should trigger (!= 0)
    let queued = make_metrics(&[("queue_depth", 5.0)]);
    let alerts = engine.evaluate(&queued);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "queue-not-empty");
    assert_eq!(alerts[0].severity, AlertSeverity::Sev4);

    // Zero queue — no alert
    let empty = make_metrics(&[("queue_depth", 0.0)]);
    assert!(engine.evaluate(&empty).is_empty());
}

#[test]
fn test_less_than_or_equal_operator() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // At threshold — should trigger (<= 5)
    let at = make_metrics(&[("active_connections", 5.0)]);
    let alerts = engine.evaluate(&at);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "connections-low");

    // Below threshold — should also trigger
    engine.clear_cooldowns();
    let below = make_metrics(&[("active_connections", 2.0)]);
    let alerts2 = engine.evaluate(&below);
    assert_eq!(alerts2.len(), 1);

    // Above threshold — no alert
    engine.clear_cooldowns();
    let above = make_metrics(&[("active_connections", 10.0)]);
    assert!(engine.evaluate(&above).is_empty());
}

#[test]
fn test_missing_metrics_gracefully_skipped() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Empty metrics map — no panics, no alerts
    let empty: HashMap<String, f64> = HashMap::new();
    assert!(engine.evaluate(&empty).is_empty());

    // Partial metrics — only matching rules evaluated
    let partial = make_metrics(&[("cpu_percent", 95.0)]);
    let alerts = engine.evaluate(&partial);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "cpu-spike");
}

#[test]
fn test_severity_classification_accuracy() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // SEV-1: memory >= 95
    let sev1 = make_metrics(&[("memory_percent", 96.0)]);
    let alerts = engine.evaluate(&sev1);
    assert_eq!(alerts[0].severity, AlertSeverity::Sev1);

    engine.clear_cooldowns();

    // SEV-2: cpu > 80
    let sev2 = make_metrics(&[("cpu_percent", 85.0)]);
    let alerts = engine.evaluate(&sev2);
    assert_eq!(alerts[0].severity, AlertSeverity::Sev2);

    engine.clear_cooldowns();

    // SEV-3: error_rate > 5
    let sev3 = make_metrics(&[("error_rate", 6.0)]);
    let alerts = engine.evaluate(&sev3);
    assert_eq!(alerts[0].severity, AlertSeverity::Sev3);

    engine.clear_cooldowns();

    // SEV-4: queue_depth != 0
    let sev4 = make_metrics(&[("queue_depth", 1.0)]);
    let alerts = engine.evaluate(&sev4);
    assert_eq!(alerts[0].severity, AlertSeverity::Sev4);
}

#[test]
fn test_triggered_alert_contains_correct_metadata() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    let metrics = make_metrics(&[("cpu_percent", 92.5)]);
    let alerts = engine.evaluate(&metrics);

    assert_eq!(alerts.len(), 1);
    let alert = &alerts[0];

    assert_eq!(alert.rule_name, "cpu-spike");
    assert_eq!(alert.current_value, 92.5);
    assert_eq!(alert.threshold, 80.0);
    assert_eq!(alert.severity, AlertSeverity::Sev2);
    assert_eq!(alert.description, "CPU usage spike detected");
    assert!(alert.triggered_at_epoch_secs > 0);

    // Verify timestamp is recent (within last minute)
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    assert!(
        now - alert.triggered_at_epoch_secs < 60,
        "Triggered timestamp should be recent"
    );
}

#[test]
fn test_boundary_value_at_threshold_no_trigger_for_gt() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // cpu-spike uses ">" operator, so exactly 80.0 should NOT trigger
    let boundary = make_metrics(&[("cpu_percent", 80.0)]);
    assert!(
        engine.evaluate(&boundary).is_empty(),
        "Value at threshold should NOT trigger '>' operator"
    );

    // But 80.001 should trigger
    let just_above = make_metrics(&[("cpu_percent", 80.001)]);
    let alerts = engine.evaluate(&just_above);
    assert_eq!(alerts.len(), 1);
}

#[test]
fn test_independent_cooldowns_per_rule() {
    let mut engine = AlertRulesEngine::load_from_yaml(COMPREHENSIVE_RULES).unwrap();

    // Trigger cpu-spike
    let cpu_metrics = make_metrics(&[("cpu_percent", 95.0)]);
    assert_eq!(engine.evaluate(&cpu_metrics).len(), 1);

    // cpu-spike is now in cooldown, but error-rate should still fire independently
    let error_metrics = make_metrics(&[("error_rate", 10.0)]);
    let alerts = engine.evaluate(&error_metrics);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].rule_name, "error-rate-elevated");

    // Both in cooldown now
    let both = make_metrics(&[("cpu_percent", 95.0), ("error_rate", 10.0)]);
    assert!(engine.evaluate(&both).is_empty());
}
