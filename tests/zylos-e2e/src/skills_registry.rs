//! E.2.10 — Skills registry: register, list, eval, export.

use crate::helpers::TestGateway;
use serde_json::json;

#[tokio::test]
async fn skills_registry_crud_lifecycle() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Register a skill
    let reg_url = format!("{}/api/v1/skills", gw.base_url());
    let skill = json!({
        "name": "e2e-test-skill",
        "version": "0.1.0",
        "description": "Skill created during E2E testing",
        "entry_point": "main.py",
        "tags": ["test", "e2e"],
        "schema": {
            "input": {"type": "object", "properties": {"query": {"type": "string"}}},
            "output": {"type": "object", "properties": {"result": {"type": "string"}}}
        }
    });

    let reg_resp = gw.client.post(&reg_url).json(&skill)
        .send().await.expect("register skill");
    assert!(reg_resp.status().is_success(), "skill registration failed: {}", reg_resp.status());

    // List skills and verify ours is present
    let list_resp = gw.client.get(&reg_url).send().await.expect("list skills");
    let skills: serde_json::Value = list_resp.json().await.expect("parse skills list");
    let found = skills.as_array()
        .map(|arr| arr.iter().any(|s| s["name"] == "e2e-test-skill"))
        .unwrap_or(false);
    assert!(found, "registered skill should appear in listing");

    // Eval the skill (dry run)
    let eval_url = format!("{}/api/v1/skills/e2e-test-skill/eval", gw.base_url());
    let eval_resp = gw.client.post(&eval_url).json(&json!({"dry_run": true}))
        .send().await.expect("eval skill");
    assert!(eval_resp.status().is_success(), "skill eval failed: {}", eval_resp.status());

    // Export the skill as archive
    let export_url = format!("{}/api/v1/skills/e2e-test-skill/export", gw.base_url());
    let export_resp = gw.client.get(&export_url).send().await.expect("export skill");
    assert!(export_resp.status().is_success(), "skill export failed: {}", export_resp.status());
    assert_eq!(
        export_resp.headers().get("content-type").map(|v| v.to_str().unwrap_or("")),
        Some("application/octet-stream"),
        "export should return binary archive"
    );
}