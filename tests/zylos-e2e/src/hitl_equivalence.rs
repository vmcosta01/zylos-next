//! E.2.15 — HITL equivalence reuse: same-action skips second approval.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn hitl_equivalence_reuse_skips_second_approval() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    let submit_url = format!("{}/api/v1/hitl/request-approval", gw.base_url());
    let approval_req = json!({
        "task_id": "e2e-equiv-task-001",
        "summary": "Restart service payment-gateway",
        "tool_call": {"name": "restart_service", "args": {"service": "payment-gateway", "grace_period_s": 30}},
        "risk_level": "medium",
        "ttl_seconds": 300
    });

    // First submission — should require approval
    let resp1 = gw
        .client
        .post(&submit_url)
        .json(&approval_req)
        .send()
        .await
        .expect("first submission");
    let pending1: serde_json::Value = resp1.json().await.expect("parse first pending");
    let id1 = pending1["approval_id"].as_str().expect("first approval_id");

    // Approve the first request
    let approve_url = format!("{}/api/v1/hitl/approve/{}", gw.base_url(), id1);
    gw.client
        .post(&approve_url)
        .json(&json!({
            "decision": "approved",
            "approver": "e2e-test-user"
        }))
        .send()
        .await
        .expect("approve first");

    // Second identical submission — should auto-approve via equivalence hash
    let mut req2 = approval_req.clone();
    req2["task_id"] = json!("e2e-equiv-task-002");
    let resp2 = gw
        .client
        .post(&submit_url)
        .json(&req2)
        .send()
        .await
        .expect("second submission");
    let result2: serde_json::Value = resp2.json().await.expect("parse second result");

    assert_eq!(
        result2["state"], "auto_approved",
        "identical action should be auto-approved via equivalence reuse"
    );
    assert!(
        result2.get("equivalence_hash").is_some(),
        "auto-approved result should carry equivalence_hash"
    );
}
