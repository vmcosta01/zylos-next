//! E.2.17 — Task durability: maintenance loop recovers orphaned leases.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn task_durability_recovery_orphaned_leases() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Create a task with a short lease TTL
    let create_url = format!("{}/api/v1/tasks", gw.base_url());
    let task = json!({
        "task_type": "long_running_job",
        "payload": {"operation": "data_export"},
        "lease_ttl_seconds": 5,
        "max_attempts": 3
    });

    let create_resp = gw
        .client
        .post(&create_url)
        .json(&task)
        .send()
        .await
        .expect("create task");
    assert!(
        create_resp.status().is_success(),
        "task creation failed: {}",
        create_resp.status()
    );

    let created: serde_json::Value = create_resp.json().await.expect("parse created task");
    let task_id = created["task_id"].as_str().expect("task_id");

    // Simulate worker death by NOT renewing the lease and waiting for expiry
    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    // Trigger maintenance loop manually
    let maintain_url = format!("{}/api/v1/tasks/maintenance/recover", gw.base_url());
    let resp = gw
        .client
        .post(&maintain_url)
        .send()
        .await
        .expect("trigger recovery");
    assert!(
        resp.status().is_success(),
        "maintenance trigger failed: {}",
        resp.status()
    );

    let result: serde_json::Value = resp.json().await.expect("parse recovery result");
    let recovered = result["recovered_tasks"]
        .as_array()
        .map(|arr| arr.iter().any(|t| t["task_id"] == task_id))
        .unwrap_or(false);

    assert!(
        recovered,
        "orphaned task should be recovered by maintenance loop"
    );

    // Verify task is back in claimable state
    let status_url = format!("{}/api/v1/tasks/{}", gw.base_url(), task_id);
    let status: serde_json::Value = gw
        .client
        .get(&status_url)
        .send()
        .await
        .expect("get status")
        .json()
        .await
        .expect("parse status");
    assert_eq!(
        status["state"], "ready",
        "recovered task should transition back to ready state"
    );
}
