//! E.2.4 — Orchestrator rollback on mid-DAG failure.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn orchestrator_rollback_on_failure() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // DAG: A → B(fail) → C  — C should never run, A should be rolled back
    let dag = json!({
        "dag_id": "e2e-rollback",
        "nodes": [
            {"id": "A", "action": "write_state", "payload": {"key": "rollback_test", "value": "set"}},
            {"id": "B", "action": "fail_intentionally", "payload": {}},
            {"id": "C", "action": "noop", "payload": {}}
        ],
        "edges": [
            {"from": "A", "to": "B"},
            {"from": "B", "to": "C"}
        ],
        "rollback_policy": "full"
    });

    let url = format!("{}/api/v1/orchestrate", gw.base_url());
    let resp = gw.client.post(&url).json(&dag).send().await.expect("submit dag");
    let result: serde_json::Value = resp.json().await.expect("parse result");

    assert_eq!(result["status"], "failed", "DAG should report failure");
    assert!(result["rolled_back"].as_bool().unwrap_or(false),
        "state from node A should be rolled back");

    // Verify node C never executed
    let trace = result["trace"].as_array().expect("trace");
    let c_executed = trace.iter().any(|e| e["node_id"] == "C" && e["status"] == "completed");
    assert!(!c_executed, "node C should not have executed after B failed");
}