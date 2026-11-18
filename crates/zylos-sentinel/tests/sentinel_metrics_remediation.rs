//! Integração métricas → regras → gate de remediação (T11.7.2 + T11.7.5).
//!
//! Simula ticks de coleta de 5 s e verifica os critérios de aceitação da
//! Fase 11.7: CPU spike >80% detectado em <30 s, error rate >1% em <1 min,
//! e remediação só executa quando o gate aprova.

use zylos_sentinel::alert_rules::AlertRulesEngine;
use zylos_sentinel::metrics::{
    BaselineDetector, MetricHistory, MetricKind, MetricsCollector, StaticProbe, SystemProbe,
};
use zylos_sentinel::remediation::{
    GateConfig, GateMode, RecordingExecutor, RemediationAction, RemediationExecutor,
    RemediationGate, RemediationStatus,
};

use std::sync::{Arc, Mutex};

const TICK_MS: u64 = 5_000;

const RULES: &str = r#"
- name: cpu-spike
  metric: cpu_percent
  operator: ">"
  threshold: 80.0
  window_minutes: 1
  severity: SEV-2
  cooldown_minutes: 5
  remediation:
    action: restart-gateway
    gated: true
- name: error-rate
  metric: error_rate_percent
  operator: ">"
  threshold: 1.0
  window_minutes: 1
  severity: SEV-1
  cooldown_minutes: 5
  remediation:
    action: notify-oncall:pagerduty
    gated: false
    max_auto_executions: 2
"#;

/// Probe controlada pelo teste (CPU muda ao longo do tempo).
struct SharedProbe(Arc<Mutex<f64>>);
impl SystemProbe for SharedProbe {
    fn cpu_percent(&mut self) -> Option<f64> {
        self.0.lock().ok().map(|v| *v)
    }
    fn memory_percent(&mut self) -> Option<f64> {
        Some(50.0)
    }
}

struct Shared(Arc<RecordingExecutor>);
impl RemediationExecutor for Shared {
    fn execute(&self, a: &RemediationAction) -> Result<String, String> {
        self.0.execute(a)
    }
}

#[test]
fn cpu_spike_detected_within_30s() {
    let cpu = Arc::new(Mutex::new(25.0));
    let mut collector = MetricsCollector::new(Box::new(SharedProbe(cpu.clone())));
    let mut engine = AlertRulesEngine::load_from_yaml(RULES).unwrap();
    let mut detector = BaselineDetector::default();

    let spike_at_ms = 60_000;
    let mut detected_rule_at = None;
    let mut detected_z_at = None;
    for tick in 0..40u64 {
        let t = tick * TICK_MS;
        if t >= spike_at_ms {
            *cpu.lock().unwrap() = 93.0;
        } else {
            *cpu.lock().unwrap() = 25.0 + (tick % 3) as f64;
        }
        let snap = collector.snapshot(t);
        if detector
            .observe_snapshot(&snap)
            .iter()
            .any(|a| a.metric == MetricKind::CpuPercent)
        {
            detected_z_at.get_or_insert(t);
        }
        if engine
            .evaluate(&snap.to_map())
            .iter()
            .any(|a| a.rule_name == "cpu-spike")
        {
            detected_rule_at.get_or_insert(t);
        }
    }
    let rule_latency = detected_rule_at.expect("regra detectou") - spike_at_ms;
    let z_latency = detected_z_at.expect("z-score detectou") - spike_at_ms;
    assert!(rule_latency < 30_000, "rule latency {rule_latency}ms");
    assert!(z_latency < 30_000, "z latency {z_latency}ms");
}

#[test]
fn error_rate_above_1pct_detected_within_1min() {
    let mut collector = MetricsCollector::new(Box::new(StaticProbe::default()));
    let mut engine = AlertRulesEngine::load_from_yaml(RULES).unwrap();
    let burst_at_ms = 120_000;
    let mut detected_at = None;
    for tick in 0..60u64 {
        let t = tick * TICK_MS;
        // 200 req/tick; burst: 3% de erro.
        for i in 0..200 {
            let err = t >= burst_at_ms && i % 33 == 0;
            collector.record_request(20.0, err);
        }
        let snap = collector.snapshot(t);
        if engine
            .evaluate(&snap.to_map())
            .iter()
            .any(|a| a.rule_name == "error-rate")
        {
            detected_at.get_or_insert(t);
        }
    }
    let latency = detected_at.expect("detectou") - burst_at_ms;
    assert!(latency < 60_000, "latency {latency}ms");
}

#[test]
fn steady_traffic_has_no_false_positives() {
    // 24 h simuladas em ticks de 5 s (17 280 amostras) com ruído determinístico.
    let mut engine = AlertRulesEngine::load_from_yaml(RULES).unwrap();
    let mut detector = BaselineDetector::new(120, 30, 4.0);
    let mut alerts = 0usize;
    let mut z_flags = 0usize;
    let samples = 24 * 60 * 60 / 5;
    let mut seed: u64 = 42;
    for i in 0..samples as u64 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let noise = ((seed >> 33) % 1000) as f64 / 100.0 - 5.0; // ±5
        let mut c = MetricsCollector::new(Box::new(StaticProbe {
            cpu: Some(40.0 + noise),
            memory: Some(55.0),
        }));
        for _ in 0..200 {
            c.record_request(20.0 + noise.abs(), false);
        }
        c.record_request(20.0, (i % 400) == 0); // erro esporádico ~0.5%
        let snap = c.snapshot(i * TICK_MS);
        engine.clear_cooldowns();
        alerts += engine.evaluate(&snap.to_map()).len();
        z_flags += detector
            .observe_snapshot(&snap)
            .iter()
            .filter(|a| a.metric == MetricKind::CpuPercent)
            .count();
    }
    assert_eq!(alerts, 0, "regras não devem disparar em tráfego estável");
    let fp_rate = z_flags as f64 / samples as f64 * 100.0;
    assert!(fp_rate < 5.0, "FP rate z-score {fp_rate:.3}%");
}

#[test]
fn remediation_executes_only_when_gate_approves() {
    let cpu = Arc::new(Mutex::new(95.0));
    let mut collector = MetricsCollector::new(Box::new(SharedProbe(cpu)));
    let mut engine = AlertRulesEngine::load_from_yaml(RULES).unwrap();
    let exec = Arc::new(RecordingExecutor::default());
    let mut gate = RemediationGate::new(
        GateConfig {
            mode: GateMode::Auto,
            approval_ttl_secs: 600,
        },
        Box::new(Shared(exec.clone())),
    );
    let mut history = MetricHistory::new(10);

    for _ in 0..10 {
        collector.record_request(10.0, true);
    }
    let snap = collector.snapshot(0);
    history.push(&snap, &[]);
    let alerts = engine.evaluate(&snap.to_map());
    assert_eq!(alerts.len(), 2);

    let mut restart_id = None;
    for a in &alerts {
        let p = gate.propose(&engine, a, 0).unwrap().unwrap();
        match &p.action {
            RemediationAction::RestartService { .. } => {
                assert_eq!(p.status, RemediationStatus::PendingApproval);
                restart_id = Some(p.id.clone());
            }
            RemediationAction::NotifyOncall { .. } => {
                assert_eq!(p.status, RemediationStatus::Executed);
            }
            other => panic!("ação inesperada {other:?}"),
        }
    }
    // Só notify executou.
    assert_eq!(exec.calls().len(), 1);
    let id = restart_id.unwrap();
    gate.approve(&id, "sre-oncall", 30).unwrap();
    assert_eq!(exec.calls().len(), 2);
    assert!(matches!(
        exec.calls()[1],
        RemediationAction::RestartService { .. }
    ));
    assert!(gate.drain_soar_events().len() >= 3);
    assert_eq!(history.series(MetricKind::CpuPercent).len(), 1);
}
