//! E.2.2 — Memory decay: stale entries pruned after TTL.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn memory_decay_prunes_stale_entries() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Ingest a fact with an old timestamp (beyond default TTL)
    let stale_fact = json!({
        "key": "session.temp_token",
        "value": "abc123",
        "timestamp": "2020-01-01T00:00:00Z",
        "ttl_seconds": 3600,
        "source": "test"
    });

    let ingest_url = format!("{}/api/v1/memory/ingest", gw.base_url());
    gw.client.post(&ingest_url).json(&stale_fact).send().await.expect("ingest stale");

    // Trigger decay/prune
    let prune_url = format!("{}/api/v1/memory/decay", gw.base_url());
    let resp = gw.client.post(&prune_url).send().await.expect("decay");
    assert!(resp.status().is_success(), "decay failed: {}", resp.status());

    // Verify the stale entry is gone
    let query_url = format!("{}/api/v1/memory/query?key=session.temp_token", gw.base_url());
    let body: serde_json::Value = gw.client.get(&query_url)
        .send().await.expect("query")
        .json().await.expect("parse");

    assert!(body.is_null() || body.get("value").is_none(), "stale entry should be pruned");
}