//! E.2.12 — Cockpit SSE streaming delivers events in order.

use crate::helpers::TestGateway;
use futures::StreamExt;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn cockpit_streaming_delivers_events_in_order() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Trigger an agent run that emits streaming events
    let run_url = format!("{}/api/v1/orchestrate", gw.base_url());
    let dag = json!({
        "dag_id": "e2e-stream-test",
        "nodes": [
            {"id": "emit_1", "action": "emit_event", "payload": {"seq": 1}},
            {"id": "emit_2", "action": "emit_event", "payload": {"seq": 2}},
            {"id": "emit_3", "action": "emit_event", "payload": {"seq": 3}}
        ],
        "edges": [
            {"from": "emit_1", "to": "emit_2"},
            {"from": "emit_2", "to": "emit_3"}
        ]
    });
    gw.client
        .post(&run_url)
        .json(&dag)
        .send()
        .await
        .expect("trigger run");

    // Connect to SSE stream
    let stream_url = format!(
        "{}/api/v1/stream/events?dag_id=e2e-stream-test",
        gw.base_url()
    );
    let resp = gw
        .client
        .get(&stream_url)
        .header("Accept", "text/event-stream")
        .send()
        .await
        .expect("connect SSE");
    assert!(
        resp.status().is_success(),
        "SSE connection failed: {}",
        resp.status()
    );

    // Collect first 10 events with timeout
    let mut events = Vec::new();
    let mut stream = resp.bytes_stream();
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);

    while events.len() < 10 && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(tokio::time::Duration::from_secs(2), stream.next()).await {
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

    assert!(!events.is_empty(), "should receive at least one SSE event");

    // Verify sequence ordering where applicable
    let seq_events: Vec<u64> = events
        .iter()
        .filter_map(|e| e.get("sequence").and_then(|s| s.as_u64()))
        .collect();
    if seq_events.len() > 1 {
        for window in seq_events.windows(2) {
            assert!(
                window[1] > window[0],
                "SSE events must arrive in ascending sequence order: {:?}",
                seq_events
            );
        }
    }
}
