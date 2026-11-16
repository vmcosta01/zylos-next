//! E.2.16 — Task durability: lease steal prevented by CAS.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn task_durability_cas_prevents_lease_steal() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Create a durable task
    let create_url = format!("{}/api/v1/tasks", gw.base_url());
    let task = json!({
        "task_type": "data_processing",
        "payload": {"input_file": "large_dataset.csv"},
        "lease_ttl_seconds": 60,
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
    let lease_id_1 = created["lease_id"].as_str().expect("initial lease_id");

    // Attempt to claim the same task with a different worker (simulating steal)
    let claim_url = format!("{}/api/v1/tasks/{}/claim", gw.base_url(), task_id);
    let claim_resp = gw
        .client
        .post(&claim_url)
        .json(&json!({
            "worker_id": "worker-B",
            "expected_lease_id": "nonexistent-lease-id"
        }))
        .send()
        .await
        .expect("attempt steal");

    // CAS should reject the steal attempt
    assert!(
        !claim_resp.status().is_success() || {
            let body: serde_json::Value = claim_resp.json().await.unwrap_or(json!({}));
            body.get("error").is_some() || body["lease_id"] != "nonexistent-lease-id"
        },
        "CAS should prevent lease steal with wrong expected_lease_id"
    );

    // Original lease holder should still own the task
    let status_url = format!("{}/api/v1/tasks/{}", gw.base_url(), task_id);
    let status: serde_json::Value = gw
        .client
        .get(&status_url)
        .send()
        .await
        .expect("get task status")
        .json()
        .await
        .expect("parse status");
    assert_eq!(
        status["lease_id"].as_str(),
        Some(lease_id_1),
        "original lease holder should retain ownership after failed steal"
    );
}
