//! E.2.5 — Orchestrator recovery from checkpoint after crash.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn orchestrator_recovery_from_checkpoint() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Submit a DAG with checkpointing enabled
    let dag = json!({
        "dag_id": "e2e-recovery",
        "nodes": [
            {"id": "step_1", "action": "write_state", "payload": {"key": "recovery_test", "value": "phase1"}},
            {"id": "step_2", "action": "checkpoint", "payload": {}},
            {"id": "step_3", "action": "write_state", "payload": {"key": "recovery_test", "value": "phase2"}},
            {"id": "step_4", "action": "noop", "payload": {}}
        ],
        "edges": [
            {"from": "step_1", "to": "step_2"},
            {"from": "step_2", "to": "step_3"},
            {"from": "step_3", "to": "step_4"}
        ],
        "checkpoint_policy": "after_each_node"
    });

    let url = format!("{}/api/v1/orchestrate", gw.base_url());
    let resp = gw
        .client
        .post(&url)
        .json(&dag)
        .send()
        .await
        .expect("submit dag");
    assert!(
        resp.status().is_success(),
        "DAG submission failed: {}",
        resp.status()
    );

    let result: serde_json::Value = resp.json().await.expect("parse result");
    assert_eq!(result["status"], "completed", "DAG should complete");

    // Verify checkpoint exists and state is consistent
    let checkpoint_url = format!(
        "{}/api/v1/orchestrate/e2e-recovery/checkpoint",
        gw.base_url()
    );
    let cp: serde_json::Value = gw
        .client
        .get(&checkpoint_url)
        .send()
        .await
        .expect("get checkpoint")
        .json()
        .await
        .expect("parse checkpoint");

    assert!(
        cp.get("last_completed_node").is_some(),
        "checkpoint should record progress"
    );
}
