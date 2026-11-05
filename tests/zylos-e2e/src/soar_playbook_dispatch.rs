//! E.2.18 — SOAR playbook dispatch on matching event trigger.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn soar_playbook_dispatch_on_event() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Register a SOAR playbook
    let reg_url = format!("{}/api/v1/soar/playbooks", gw.base_url());
    let playbook = json!({
        "name": "e2e-incident-response",
        "trigger": {
            "event_type": "security.alert",
            "conditions": {"severity": {"gte": "high"}}
        },
        "actions": [
            {"step": 1, "action": "isolate_host", "args": {"host": "{{event.source_ip}}"}},
            {"step": 2, "action": "notify_soc", "args": {"channel": "#incidents"}},
            {"step": 3, "action": "create_ticket", "args": {"priority": "P1"}}
        ],
        "enabled": true
    });

    let reg_resp = gw.client.post(&reg_url).json(&playbook)
        .send().await.expect("register playbook");
    assert!(reg_resp.status().is_success(), "playbook registration failed: {}", reg_resp.status());

    // Emit a matching event
    let event_url = format!("{}/api/v1/soar/events", gw.base_url());
    let event = json!({
        "event_type": "security.alert",
        "source_ip": "10.0.1.42",
        "severity": "critical",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "payload": {"description": "Suspicious outbound connection detected"}
    });

    let event_resp = gw.client.post(&event_url).json(&event)
        .send().await.expect("emit event");
    assert!(event_resp.status().is_success(), "event emission failed: {}", event_resp.status());

    // Allow async dispatch to process
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Check that the playbook was dispatched
    let runs_url = format!("{}/api/v1/soar/runs?playbook=e2e-incident-response", gw.base_url());
    let runs: serde_json::Value = gw.client.get(&runs_url)
        .send().await.expect("list runs")
        .json().await.expect("parse runs");

    let run_list = runs.as_array().expect("runs should be array");
    assert!(!run_list.is_empty(), "at least one playbook run should be triggered");

    let latest_run = &run_list[0];
    assert_eq!(latest_run["status"], "completed", "playbook run should complete");
    assert_eq!(
        latest_run["steps_executed"].as_u64(),
        Some(3),
        "all 3 playbook steps should execute"
    );
}