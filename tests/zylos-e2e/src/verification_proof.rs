//! E.2.6 — Verification proof generation and validation round-trip.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
#[ignore]
async fn verification_proof_roundtrip() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Generate a proof for a sample output
    let gen_url = format!("{}/api/v1/verification/generate-proof", gw.base_url());
    let proof_req = json!({
        "output_id": "test-output-001",
        "content": "The analysis shows 42% improvement in throughput.",
        "claim_type": "quantitative",
        "evidence_sources": ["benchmark_suite_v3", "production_metrics_q3"]
    });

    let gen_resp = gw
        .client
        .post(&gen_url)
        .json(&proof_req)
        .send()
        .await
        .expect("generate proof");
    assert!(
        gen_resp.status().is_success(),
        "proof generation failed: {}",
        gen_resp.status()
    );

    let proof: serde_json::Value = gen_resp.json().await.expect("parse proof");
    assert!(proof.get("proof_hash").is_some(), "proof should have hash");
    assert!(
        proof.get("verification_chain").is_some(),
        "proof should have chain"
    );

    // Validate the proof
    let validate_url = format!("{}/api/v1/verification/validate-proof", gw.base_url());
    let val_resp = gw
        .client
        .post(&validate_url)
        .json(&proof)
        .send()
        .await
        .expect("validate proof");
    assert!(
        val_resp.status().is_success(),
        "proof validation failed: {}",
        val_resp.status()
    );

    let val_result: serde_json::Value = val_resp.json().await.expect("parse validation");
    assert_eq!(
        val_result["valid"], true,
        "proof should validate successfully"
    );
}
