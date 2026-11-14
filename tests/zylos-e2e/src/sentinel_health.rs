//! E.2.19 — Sentinel health scanner detects degraded subsystem.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn sentinel_health_detects_degraded_subsystem() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Force a subsystem into degraded state by simulating high error rate
    let degrade_url = format!("{}/api/v1/sentinel/inject-degradation", gw.base_url());
    let degrade_req = json!({
        "subsystem": "memory-store",
        "error_rate": 0.8,
        "duration_seconds": 30
    });

    let degrade_resp = gw
        .client
        .post(&degrade_url)
        .json(&degrade_req)
        .send()
        .await
        .expect("inject degradation");
    assert!(
        degrade_resp.status().is_success(),
        "degradation injection failed: {}",
        degrade_resp.status()
    );

    // Wait for sentinel scan cycle (default 120s, but test config uses 5s)
    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    // Query sentinel issues
    let issues_url = format!("{}/api/v1/sentinel/issues", gw.base_url());
    let issues: serde_json::Value = gw
        .client
        .get(&issues_url)
        .send()
        .await
        .expect("list issues")
        .json()
        .await
        .expect("parse issues");

    let issue_list = issues.as_array().expect("issues should be array");
    let memory_issue = issue_list
        .iter()
        .find(|i| i["subsystem"] == "memory-store" && i["severity"] == "warning");

    assert!(
        memory_issue.is_some(),
        "sentinel should detect memory-store degradation: {:?}",
        issue_list
    );

    let issue = memory_issue.unwrap();
    assert!(
        issue.get("detected_at").is_some(),
        "issue should have detection timestamp"
    );
    assert!(
        issue.get("signature").is_some(),
        "issue should have dedup signature"
    );
}
