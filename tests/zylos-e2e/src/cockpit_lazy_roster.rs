//! E.2.13 — Cockpit lazy-roster renders 500 agents under 100ms.

use crate::helpers::TestGateway;
use serde_json::json;
use std::time::Instant;

#[tokio::test]
async fn cockpit_lazy_roster_500_agents_under_100ms() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Seed 500 agents into the registry
    let seed_url = format!("{}/api/v1/agents/bulk-register", gw.base_url());
    let agents: Vec<serde_json::Value> = (0..500)
        .map(|i| json!({
            "id": format!("agent-{:04}", i),
            "name": format!("Test Agent {}", i),
            "status": if i % 3 == 0 { "active" } else { "idle" },
            "tenant": "e2e-test",
            "last_heartbeat": chrono::Utc::now().to_rfc3339()
        }))
        .collect();

    let seed_resp = gw.client.post(&seed_url).json(&agents)
        .send().await.expect("bulk register agents");
    assert!(seed_resp.status().is_success(), "bulk register failed: {}", seed_resp.status());

    // Fetch first page of roster and measure latency
    let roster_url = format!("{}/api/v1/cockpit/roster?page_size=50&page=1", gw.base_url());
    let start = Instant::now();
    let resp = gw.client.get(&roster_url).send().await.expect("fetch roster");
    let elapsed = start.elapsed();

    assert!(resp.status().is_success(), "roster fetch failed: {}", resp.status());
    assert!(
        elapsed.as_millis() < 100,
        "lazy-roster first page should render in <100ms, took {}ms",
        elapsed.as_millis()
    );

    let body: serde_json::Value = resp.json().await.expect("parse roster");
    let items = body["items"].as_array().expect("roster should have items array");
    assert_eq!(items.len(), 50, "first page should return exactly 50 agents");
    assert_eq!(body["total"].as_u64(), Some(500), "total count should be 500");
}