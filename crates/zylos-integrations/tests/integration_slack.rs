//! Testes de integração Slack (T11.8.7).
//!
//! - Testes `mock_*`: rodam sempre, contra servidor HTTP local (axum) que imita a API.
//! - Testes `sandbox_*`: `#[ignore]` — chamam a API real com chamadas **somente leitura**
//!   (`auth.test`, `conversations.list`). Rodar com `SLACK_BOT_TOKEN` definido:
//!   `cargo test -p zylos-integrations --test integration_slack -- --ignored`.
//!   Sem token, o teste imprime aviso e retorna (skip gracioso).

mod common;

use common::{env_token, MockServer};
use std::collections::HashMap;
use zylos_integrations::channel::{ChannelAdapter, OutboundMessage};
use zylos_integrations::slack::SlackAdapter;

fn msg(recipient: &str, content: &str) -> OutboundMessage {
    OutboundMessage {
        recipient: recipient.to_string(),
        content: content.to_string(),
        msg_type: None,
        extra: HashMap::new(),
    }
}

#[tokio::test]
async fn mock_slack_health_check_ok_sends_bearer() {
    let srv = MockServer::start(vec![(
        "/auth.test",
        200,
        serde_json::json!({"ok": true, "team": "zylos", "user": "bot"}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-mock".into(), srv.base_url.clone());
    assert!(a.health_check().await.unwrap());
    let reqs = srv.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].authorization.as_deref(), Some("Bearer xoxb-mock"));
}

#[tokio::test]
async fn mock_slack_health_check_invalid_auth_is_false() {
    let srv = MockServer::start(vec![(
        "/auth.test",
        200,
        serde_json::json!({"ok": false, "error": "invalid_auth"}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-bad".into(), srv.base_url.clone());
    assert!(!a.health_check().await.unwrap());
}

#[tokio::test]
async fn mock_slack_send_message_posts_channel_and_text() {
    let srv = MockServer::start(vec![(
        "/chat.postMessage",
        200,
        serde_json::json!({"ok": true, "channel": "C123", "ts": "1700000000.000100"}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-mock".into(), srv.base_url.clone());
    let r = a.send_message(&msg("C123", "incidente P1")).await.unwrap();
    assert!(r.success);
    assert_eq!(r.message_id.0, "1700000000.000100");
    let reqs = srv.requests();
    assert_eq!(reqs[0].method, "POST");
    assert_eq!(reqs[0].body["channel"], "C123");
    assert_eq!(reqs[0].body["text"], "incidente P1");
}

#[tokio::test]
async fn mock_slack_send_message_api_error_propagates() {
    let srv = MockServer::start(vec![(
        "/chat.postMessage",
        200,
        serde_json::json!({"ok": false, "error": "channel_not_found"}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-mock".into(), srv.base_url.clone());
    let err = a.send_message(&msg("CX", "x")).await.unwrap_err();
    assert!(err.to_string().contains("channel_not_found"), "{err}");
}

#[tokio::test]
async fn mock_slack_list_users_filters_deleted() {
    let srv = MockServer::start(vec![(
        "/users.list",
        200,
        serde_json::json!({"ok": true, "members": [
            {"id": "U1", "real_name": "Ana", "profile": {"email": "ana@x.io"}},
            {"id": "U2", "name": "old", "deleted": true, "profile": {}}
        ]}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-mock".into(), srv.base_url.clone());
    let users = a.list_users(None, None).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].user_id, "U1");
    assert_eq!(users[0].email.as_deref(), Some("ana@x.io"));
}

#[tokio::test]
async fn mock_slack_list_chats_parses_channels() {
    let srv = MockServer::start(vec![(
        "/conversations.list",
        200,
        serde_json::json!({"ok": true, "channels": [
            {"id": "C1", "name": "incidents", "num_members": 4, "creator": "U1"}
        ]}),
    )])
    .await;
    let a = SlackAdapter::with_base_url("xoxb-mock".into(), srv.base_url.clone());
    let chats = a.list_chats(None, None).await.unwrap();
    assert_eq!(chats.len(), 1);
    assert_eq!(chats[0].chat_id, "C1");
    assert_eq!(chats[0].name, "incidents");
    let q = srv.requests()[0].query.clone().unwrap_or_default();
    assert!(q.contains("types=public_channel"), "{q}");
}

#[tokio::test]
#[ignore = "sandbox: requer SLACK_BOT_TOKEN (API real, somente leitura)"]
async fn sandbox_slack_auth_test() {
    let Some(token) = env_token("SLACK_BOT_TOKEN") else {
        eprintln!("SKIP: SLACK_BOT_TOKEN ausente");
        return;
    };
    let a = SlackAdapter::new(token);
    assert!(
        a.health_check().await.unwrap(),
        "auth.test falhou com o token fornecido"
    );
}

#[tokio::test]
#[ignore = "sandbox: requer SLACK_BOT_TOKEN (API real, somente leitura)"]
async fn sandbox_slack_list_chats_readonly() {
    let Some(token) = env_token("SLACK_BOT_TOKEN") else {
        eprintln!("SKIP: SLACK_BOT_TOKEN ausente");
        return;
    };
    let a = SlackAdapter::new(token);
    match a.list_chats(None, Some(5)).await {
        Ok(chats) => eprintln!("slack: {} canais visíveis", chats.len()),
        // Escopo channels:read pode faltar no bot — não é falha do adapter.
        Err(e) if e.to_string().contains("missing_scope") => {
            eprintln!("SKIP parcial: {e}")
        }
        Err(e) => panic!("conversations.list falhou: {e}"),
    }
}
