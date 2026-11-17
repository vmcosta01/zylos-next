//! E.2.1 — Memory consolidation: contradictory facts resolve to latest timestamp.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn memory_consolidation_resolves_by_timestamp() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Ingest two contradictory facts with different timestamps
    let fact_old = json!({
        "key": "user.timezone",
        "value": "UTC-5",
        "timestamp": "2026-01-01T10:00:00Z",
        "source": "cli"
    });
    let fact_new = json!({
        "key": "user.timezone",
        "value": "UTC-3",
        "timestamp": "2026-06-15T14:30:00Z",
        "source": "cockpit"
    });

    let url = format!("{}/api/v1/memory/ingest", gw.base_url());
    gw.client
        .post(&url)
        .json(&fact_old)
        .send()
        .await
        .expect("post old fact");
    gw.client
        .post(&url)
        .json(&fact_new)
        .send()
        .await
        .expect("post new fact");

    // Trigger consolidation
    let consolidate_url = format!("{}/api/v1/memory/consolidate", gw.base_url());
    let resp = gw
        .client
        .post(&consolidate_url)
        .send()
        .await
        .expect("consolidate");
    assert!(
        resp.status().is_success(),
        "consolidation failed: {}",
        resp.status()
    );

    // Query resolved value
    let query_url = format!("{}/api/v1/memory/query?key=user.timezone", gw.base_url());
    let body: serde_json::Value = gw
        .client
        .get(&query_url)
        .send()
        .await
        .expect("query")
        .json()
        .await
        .expect("parse response");

    assert_eq!(body["value"], "UTC-3", "should resolve to latest timestamp");
}
