//! E.2.20 — Full pipeline: event → SOAR → HITL → orchestrate → cockpit stream.

use crate::helpers::TestGateway;
use futures::StreamExt;
use serde_json::json;

#[tokio::test]
async fn full_pipeline_event_to_cockpit_stream() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // 1. Register a SOAR playbook that triggers HITL + orchestration
    let playbook_url = format!("{}/api/v1/soar/playbooks", gw.base_url());
    let playbook = json!({
        "name": "e2e-full-pipeline",
        "trigger": {"event_type": "deploy.request", "conditions": {}},
        "actions": [
            {"step": 1, "action": "hitl_approval", "args": {"summary": "Deploy v2.4.0", "risk": "high"}},
            {"step": 2, "action": "orchestrate_dag", "args": {"dag_id": "deploy-pipeline"}}
        ],
        "enabled": true
    });
    gw.client
        .post(&playbook_url)
        .json(&playbook)
        .send()
        .await
        .expect("register playbook");

    // 2. Pre-register the DAG that step 2 will invoke
    let orch_url = format!("{}/api/v1/orchestrate/register", gw.base_url());
    let dag = json!({
        "dag_id": "deploy-pipeline",
        "nodes": [
            {"id": "build", "action": "noop", "payload": {"phase": "build"}},
            {"id": "test", "action": "noop", "payload": {"phase": "test"}},
            {"id": "deploy", "action": "noop", "payload": {"phase": "deploy"}}
        ],
        "edges": [
            {"from": "build", "to": "test"},
            {"from": "test", "to": "deploy"}
        ]
    });
    gw.client
        .post(&orch_url)
        .json(&dag)
        .send()
        .await
        .expect("register dag");

    // 3. Connect SSE stream BEFORE emitting the event
    let stream_url = format!("{}/api/v1/stream/events", gw.base_url());
    let stream_resp = gw
        .client
        .get(&stream_url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("connect SSE");
    assert!(stream_resp.status().is_success(), "SSE connection failed");

    // 4. Emit the triggering event
    let event_url = format!("{}/api/v1/soar/events", gw.base_url());
    let event = json!({
        "event_type": "deploy.request",
        "version": "v2.4.0",
        "requester": "ci-pipeline"
    });
    gw.client
        .post(&event_url)
        .json(&event)
        .send()
        .await
        .expect("emit event");

    // 5. Auto-approve the HITL request (simulating human approval)
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    let pending_url = format!("{}/api/v1/hitl/pending-approvals", gw.base_url());
    let pending: serde_json::Value = gw
        .client
        .get(&pending_url)
        .send()
        .await
        .expect("list pending")
        .json()
        .await
        .expect("parse pending");

    if let Some(arr) = pending.as_array() {
        if let Some(approval) = arr.first() {
            let id = approval["approval_id"].as_str().unwrap_or("");
            let approve_url = format!("{}/api/v1/hitl/approve/{}", gw.base_url(), id);
            gw.client
                .post(&approve_url)
                .json(&json!({
                    "decision": "approved",
                    "approver": "e2e-auto-approver"
                }))
                .send()
                .await
                .expect("auto-approve");
        }
    }

    // 6. Collect streaming events with timeout
    let mut events = Vec::new();
    let mut stream = stream_resp.bytes_stream();
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(15);

    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(tokio::time::Duration::from_secs(3), stream.next()).await {
            Ok(Some(Ok(chunk))) => {
                let text = String::from_utf8_lossy(&chunk);
                for line in text.lines() {
                    if line.starts_with("data:") {
                        let data = line.trim_start_matches("data:").trim();
                        if let Ok(evt) = serde_json::from_str::<serde_json::Value>(data) {
                            events.push(evt);
                        }
                    }
                }
            }
            _ => break,
        }
    }

    // 7. Verify the full pipeline produced streaming events
    assert!(
        !events.is_empty(),
        "full pipeline should produce SSE events"
    );

    // Should contain events from multiple subsystems
    let event_types: Vec<&str> = events.iter().filter_map(|e| e["type"].as_str()).collect();

    assert!(
        event_types.contains(&"soar.playbook_started"),
        "should contain SOAR playbook start event"
    );
    assert!(
        event_types
            .iter()
            .any(|t| *t == "hitl.approval_requested" || *t == "hitl.approved"),
        "should contain HITL event"
    );
    assert!(
        event_types.iter().any(|t| t.starts_with("orchestrator.")),
        "should contain orchestrator events"
    );
}
