//! E.2.9 — Sandbox Docker execution with network isolation.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn sandbox_docker_network_isolation() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Submit a Docker task that attempts external network access
    let exec_url = format!("{}/api/v1/sandbox/docker/execute", gw.base_url());
    let exec_req = json!({
        "image": "alpine:3.19",
        "command": ["sh", "-c", "wget -qO- http://example.com || echo 'NETWORK_BLOCKED'"],
        "network_mode": "none",
        "limits": {
            "memory_mb": 128,
            "cpu_seconds": 10
        }
    });

    let resp = gw
        .client
        .post(&exec_url)
        .json(&exec_req)
        .send()
        .await
        .expect("execute docker");
    assert!(
        resp.status().is_success(),
        "docker exec failed: {}",
        resp.status()
    );

    let result: serde_json::Value = resp.json().await.expect("parse docker result");
    let stdout = result["stdout"].as_str().unwrap_or("");

    // With network_mode=none, external access should fail
    assert!(
        stdout.contains("NETWORK_BLOCKED") || result["exit_code"] != 0,
        "Docker container should not reach external network: stdout={}",
        stdout
    );
}
