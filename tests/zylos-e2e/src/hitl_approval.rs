//! E.2.14 — HITL approval lifecycle: request → approve → execute.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn hitl_approval_full_lifecycle() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Submit a task that requires human approval
    let submit_url = format!("{}/api/v1/hitl/request-approval", gw.base_url());
    let approval_req = json!({
        "task_id": "e2e-hitl-task-001",
        "summary": "Deploy new model to production cluster us-east-1",
        "tool_call": {"name": "deploy_model", "args": {"cluster": "us-east-1", "version": "v2.3.1"}},
        "risk_level": "high",
        "ttl_seconds": 300
    });

    let submit_resp = gw.client.post(&submit_url).json(&approval_req)
        .send().await.expect("submit approval request");
    assert!(submit_resp.status().is_success(), "approval submission failed: {}", submit_resp.status());

    let pending: serde_json::Value = submit_resp.json().await.expect("parse pending approval");
    let approval_id = pending["approval_id"].as_str().expect("should have approval_id");

    // Verify it appears in pending list
    let list_url = format!("{}/api/v1/hitl/pending-approvals", gw.base_url());
    let list_resp = gw.client.get(&list_url).send().await.expect("list pending");
    let list_body: serde_json::Value = list_resp.json().await.expect("parse pending list");
    let found = list_body.as_array()
        .map(|arr| arr.iter().any(|a| a["approval_id"] == approval_id))
        .unwrap_or(false);
    assert!(found, "submitted approval should appear in pending list");

    // Approve the request
    let approve_url = format!("{}/api/v1/hitl/approve/{}", gw.base_url(), approval_id);
    let approve_resp = gw.client.post(&approve_url).json(&json!({
        "decision": "approved",
        "approver": "e2e-test-user",
        "comment": "Approved for E2E test"
    })).send().await.expect("approve");
    assert!(approve_resp.status().is_success(), "approval failed: {}", approve_resp.status());

    // Verify the task was executed after approval
    let status_url = format!("{}/api/v1/hitl/status/{}", gw.base_url(), approval_id);
    let status: serde_json::Value = gw.client.get(&status_url)
        .send().await.expect("get status")
        .json().await.expect("parse status");
    assert_eq!(status["state"], "executed", "approved task should transition to executed");
}