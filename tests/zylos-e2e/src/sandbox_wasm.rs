//! E.2.8 — Sandbox WASM execution with resource limits.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn sandbox_wasm_resource_limits() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Submit a WASM module that attempts to exceed memory limits
    let exec_url = format!("{}/api/v1/sandbox/wasm/execute", gw.base_url());
    let exec_req = json!({
        "module_name": "test_memory_hog",
        "entry_point": "main",
        "args": [],
        "limits": {
            "memory_mb": 64,
            "cpu_seconds": 5,
            "max_allocations": 1000
        },
        "wasm_bytes_base64": null  // Will use built-in test module
    });

    let resp = gw
        .client
        .post(&exec_url)
        .json(&exec_req)
        .send()
        .await
        .expect("execute wasm");

    let result: serde_json::Value = resp.json().await.expect("parse wasm result");

    // Should either succeed within limits or fail with resource_exceeded
    let status = result["status"].as_str().unwrap_or("");
    assert!(
        status == "completed" || status == "resource_exceeded",
        "WASM should complete or be killed, not hang: got {}",
        status
    );

    if status == "resource_exceeded" {
        assert!(
            result.get("exceeded_resource").is_some(),
            "should report which resource was exceeded"
        );
    }
}
