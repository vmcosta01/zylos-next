//! E.2.7 — Verification adversarial panel rejects weak output.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn verification_panel_rejects_weak_output() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Submit a deliberately weak output for adversarial review
    let panel_url = format!("{}/api/v1/verification/adversarial-panel", gw.base_url());
    let panel_req = json!({
        "output_id": "weak-output-001",
        "content": "This is probably correct I think maybe.",
        "claim_type": "factual",
        "panel_config": {
            "judges": ["skeptic", "fact_checker", "domain_expert"],
            "consensus_threshold": 0.66
        }
    });

    let resp = gw
        .client
        .post(&panel_url)
        .json(&panel_req)
        .send()
        .await
        .expect("submit to panel");
    assert!(
        resp.status().is_success(),
        "panel submission failed: {}",
        resp.status()
    );

    let result: serde_json::Value = resp.json().await.expect("parse panel result");

    // Weak output should be rejected by at least one judge
    assert_eq!(
        result["verdict"], "rejected",
        "weak output should be rejected"
    );
    assert!(
        result["dissenting_judges"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "at least one judge should dissent on weak output"
    );
}
