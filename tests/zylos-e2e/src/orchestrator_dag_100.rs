//! E.2.3 — Orchestrator DAG with 100 nodes executes in topological order.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn orchestrator_dag_100_nodes_topological_order() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Build a linear chain of 100 nodes: node_0 → node_1 → ... → node_99
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for i in 0..100 {
        nodes.push(json!({
            "id": format!("node_{}", i),
            "action": "noop",
            "payload": {"step": i}
        }));
        if i > 0 {
            edges.push(json!({
                "from": format!("node_{}", i - 1),
                "to": format!("node_{}", i)
            }));
        }
    }

    let dag = json!({
        "dag_id": "e2e-chain-100",
        "nodes": nodes,
        "edges": edges,
        "max_concurrency": 1
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

    let result: serde_json::Value = resp.json().await.expect("parse dag result");
    assert_eq!(
        result["status"], "completed",
        "DAG should complete successfully"
    );

    // Verify execution order via trace
    let trace = result["trace"].as_array().expect("trace should be array");
    assert_eq!(trace.len(), 100, "all 100 nodes should execute");
    for (idx, entry) in trace.iter().enumerate() {
        assert_eq!(
            entry["node_id"],
            format!("node_{}", idx),
            "node at position {} should be node_{}",
            idx,
            idx
        );
    }
}
