//! Testes de integração Linear (T11.8.7).
//!
//! - Testes `mock_*`: rodam sempre, contra servidor HTTP local (axum) que imita a API.
//! - Testes `sandbox_*`: `#[ignore]` — chamam a API real com chamadas **somente leitura**
//!   (query `viewer`, `teams`). Rodar com `LINEAR_TOKEN` definido:
//!   `cargo test -p zylos-integrations --test integration_linear -- --ignored`.
//!   Sem token, o teste imprime aviso e retorna (skip gracioso).

mod common;

use common::{env_token, MockServer};
use std::collections::HashMap;
use zylos_integrations::channel::{ChannelAdapter, OutboundMessage};
use zylos_integrations::linear::LinearAdapter;

#[tokio::test]
async fn mock_linear_health_check_ok_raw_token_header() {
    let srv = MockServer::start(vec![(
        "/graphql",
        200,
        serde_json::json!({"data": {"viewer": {"id": "v1", "name": "Bot"}}}),
    )])
    .await;
    let a =
        LinearAdapter::with_base_url("lin_api_mock".into(), format!("{}/graphql", srv.base_url));
    assert!(a.health_check().await.unwrap());
    let r = &srv.requests()[0];
    assert_eq!(r.method, "POST");
    // Linear PAT vai sem prefixo Bearer
    assert_eq!(r.authorization.as_deref(), Some("lin_api_mock"));
    assert!(r.body["query"].as_str().unwrap_or("").contains("viewer"));
}

#[tokio::test]
async fn mock_linear_graphql_errors_make_health_false() {
    let srv = MockServer::start(vec![(
        "/graphql",
        200,
        serde_json::json!({"errors": [{"message": "Authentication required"}]}),
    )])
    .await;
    let a = LinearAdapter::with_base_url("bad".into(), format!("{}/graphql", srv.base_url));
    assert!(!a.health_check().await.unwrap());
}

#[tokio::test]
async fn mock_linear_send_message_creates_comment() {
    let srv = MockServer::start(vec![(
        "/graphql",
        200,
        serde_json::json!({"data": {"commentCreate": {"success": true, "comment": {"id": "c-9"}}}}),
    )])
    .await;
    let a =
        LinearAdapter::with_base_url("lin_api_mock".into(), format!("{}/graphql", srv.base_url));
    let r = a
        .send_message(&OutboundMessage {
            recipient: "ISS-1".into(),
            content: "status: mitigado".into(),
            msg_type: None,
            extra: HashMap::new(),
        })
        .await
        .unwrap();
    assert!(r.success);
    assert_eq!(r.message_id.0, "c-9");
    let body = &srv.requests()[0].body;
    assert_eq!(body["variables"]["issueId"], "ISS-1");
    assert_eq!(body["variables"]["body"], "status: mitigado");
}

#[tokio::test]
async fn mock_linear_comment_success_false_reports_error() {
    let srv = MockServer::start(vec![(
        "/graphql",
        200,
        serde_json::json!({"data": {"commentCreate": {"success": false, "comment": null}}}),
    )])
    .await;
    let a = LinearAdapter::with_base_url("t".into(), format!("{}/graphql", srv.base_url));
    let r = a
        .send_message(&OutboundMessage {
            recipient: "ISS-2".into(),
            content: "x".into(),
            msg_type: None,
            extra: HashMap::new(),
        })
        .await
        .unwrap();
    assert!(!r.success);
    assert!(r.error.is_some());
}

#[tokio::test]
async fn mock_linear_list_chats_maps_teams() {
    let srv = MockServer::start(vec![(
        "/graphql",
        200,
        serde_json::json!({"data": {"teams": {"nodes": [
            {"id": "t1", "key": "ENG", "name": "Engineering"}
        ]}}}),
    )])
    .await;
    let a = LinearAdapter::with_base_url("t".into(), format!("{}/graphql", srv.base_url));
    let chats = a.list_chats(None, None).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].chat_id, "t1");
}

#[tokio::test]
#[ignore = "sandbox: requer LINEAR_TOKEN (API real, somente leitura)"]
async fn sandbox_linear_viewer() {
    let Some(token) = env_token("LINEAR_TOKEN") else {
        eprintln!("SKIP: LINEAR_TOKEN ausente");
        return;
    };
    let a = LinearAdapter::new(token);
    assert!(
        a.health_check().await.unwrap(),
        "query viewer falhou com o token fornecido"
    );
}

#[tokio::test]
#[ignore = "sandbox: requer LINEAR_TOKEN (API real, somente leitura)"]
async fn sandbox_linear_teams_readonly() {
    let Some(token) = env_token("LINEAR_TOKEN") else {
        eprintln!("SKIP: LINEAR_TOKEN ausente");
        return;
    };
    let a = LinearAdapter::new(token);
    let teams = a.list_chats(None, None).await.expect("query teams falhou");
    eprintln!("linear: {} times visíveis", teams.len());
}
