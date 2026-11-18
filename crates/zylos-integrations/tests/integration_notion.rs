//! Testes de integração Notion (T11.8.7).
//!
//! - Testes `mock_*`: rodam sempre, contra servidor HTTP local (axum) que imita a API.
//! - Testes `sandbox_*`: `#[ignore]` — chamam a API real com chamadas **somente leitura**
//!   (`GET /users/me`, `POST /search` page_size=1). Rodar com `NOTION_TOKEN` definido:
//!   `cargo test -p zylos-integrations --test integration_notion -- --ignored`.
//!   Sem token, o teste imprime aviso e retorna (skip gracioso).

mod common;

use common::{env_token, MockServer};
use std::collections::HashMap;
use zylos_integrations::channel::{ChannelAdapter, OutboundMessage};
use zylos_integrations::notion::NotionAdapter;

#[tokio::test]
async fn mock_notion_health_check_ok_sends_version_header() {
    let srv = MockServer::start(vec![(
        "/users/me",
        200,
        serde_json::json!({"object": "user", "id": "bot-1", "type": "bot"}),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone());
    assert!(a.health_check().await.unwrap());
    let r = &srv.requests()[0];
    assert_eq!(r.authorization.as_deref(), Some("Bearer ntn_mock"));
    assert_eq!(r.notion_version.as_deref(), Some("2022-06-28"));
}

#[tokio::test]
async fn mock_notion_health_check_401_is_false() {
    let srv = MockServer::start(vec![(
        "/users/me",
        401,
        serde_json::json!({"object": "error", "code": "unauthorized"}),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_bad".into(), srv.base_url.clone());
    assert!(!a.health_check().await.unwrap());
}

#[tokio::test]
async fn mock_notion_send_message_appends_paragraph() {
    let srv = MockServer::start(vec![(
        "/blocks",
        200,
        serde_json::json!({"results": [{"id": "blk-42"}]}),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone());
    let r = a
        .send_message(&OutboundMessage {
            recipient: "page-1".into(),
            content: "postmortem".into(),
            msg_type: None,
            extra: HashMap::new(),
        })
        .await
        .unwrap();
    assert!(r.success);
    assert_eq!(r.message_id.0, "blk-42");
    let body = &srv.requests()[0].body;
    assert_eq!(body["parent"]["page_id"], "page-1");
    assert_eq!(
        body["children"][0]["paragraph"]["rich_text"][0]["text"]["content"],
        "postmortem"
    );
}

#[tokio::test]
async fn mock_notion_list_users_only_persons() {
    let srv = MockServer::start(vec![(
        "/users",
        200,
        serde_json::json!({"results": [
            {"id": "u1", "type": "person", "name": "Bia", "person": {"email": "bia@x.io"}},
            {"id": "b1", "type": "bot", "name": "Zylos"}
        ]}),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone());
    let users = a.list_users(None, None).await.unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].user_id, "u1");
}

#[tokio::test]
async fn mock_notion_http_error_propagates_status() {
    let srv = MockServer::start(vec![(
        "/blocks",
        400,
        serde_json::json!({"object": "error", "code": "validation_error"}),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone());
    let err = a
        .send_message(&OutboundMessage {
            recipient: "p".into(),
            content: "c".into(),
            msg_type: None,
            extra: HashMap::new(),
        })
        .await
        .unwrap_err();
    assert!(err.to_string().contains("400"), "{err}");
}

#[tokio::test]
#[ignore = "sandbox: requer NOTION_TOKEN (API real, somente leitura)"]
async fn sandbox_notion_users_me() {
    let Some(token) = env_token("NOTION_TOKEN") else {
        eprintln!("SKIP: NOTION_TOKEN ausente");
        return;
    };
    let a = NotionAdapter::new(token);
    assert!(
        a.health_check().await.unwrap(),
        "GET /users/me falhou com o token fornecido"
    );
}

#[tokio::test]
#[ignore = "sandbox: requer NOTION_TOKEN (API real, somente leitura)"]
async fn sandbox_notion_search_readonly() {
    let Some(token) = env_token("NOTION_TOKEN") else {
        eprintln!("SKIP: NOTION_TOKEN ausente");
        return;
    };
    let a = NotionAdapter::new(token);
    let dbs = a
        .list_chats(None, Some(1))
        .await
        .expect("POST /search falhou");
    eprintln!("notion: {} databases visíveis", dbs.len());
}

#[tokio::test]
async fn mock_notion_calendar_queries_database_with_date_filter() {
    let page = |id: &str, title: &str, start: &str, end: serde_json::Value| {
        serde_json::json!({
            "object": "page", "id": id, "url": format!("https://notion.so/{id}"),
            "properties": {
                "Name": {"type": "title", "title": [{"plain_text": title}]},
                "When": {"type": "date", "date": {"start": start, "end": end, "time_zone": null}},
                "Location": {"type": "rich_text", "rich_text": [{"plain_text": "Sala 1"}]},
                "Attendees": {"type": "people", "people": [
                    {"id": "u1", "name": "Bia", "person": {"email": "bia@x.io"}},
                    {"id": "u2", "name": "Rui"}
                ]}
            }
        })
    };
    let srv = MockServer::start(vec![(
        "/databases/db-cal/query",
        200,
        serde_json::json!({
            "results": [
                page("p1", "Kickoff", "2026-11-18T10:00:00Z", serde_json::json!("2026-11-18T11:00:00Z")),
                page("p2", "Feriado", "2026-11-20", serde_json::Value::Null),
                {"id": "p3", "properties": {"When": {"type": "date", "date": null}}}
            ],
            "has_more": false, "next_cursor": null
        }),
    )])
    .await;
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone())
        .with_calendar_database("db-cal", Some("When"));
    let ev = a
        .list_calendar_events("2026-11-17T00:00:00Z", "2026-11-30T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(ev.len(), 2, "page without date skipped");
    assert_eq!(ev[0].event_id, "p1");
    assert_eq!(ev[0].title, "Kickoff");
    assert_eq!(ev[0].end_time, "2026-11-18T11:00:00Z");
    assert_eq!(ev[0].location.as_deref(), Some("Sala 1"));
    assert_eq!(
        ev[0].attendees,
        vec!["bia@x.io".to_string(), "Rui".to_string()]
    );
    assert_eq!(ev[1].start_time, "2026-11-20");
    assert_eq!(
        ev[1].end_time, "2026-11-20",
        "all-day: end defaults to start"
    );

    let reqs = srv.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].method, "POST");
    let f = &reqs[0].body["filter"]["and"];
    assert_eq!(f[0]["property"], "When");
    assert_eq!(f[0]["date"]["on_or_after"], "2026-11-17T00:00:00Z");
    assert_eq!(f[1]["date"]["on_or_before"], "2026-11-30T00:00:00Z");
}

#[tokio::test]
async fn mock_notion_calendar_default_property_and_errors() {
    let srv = MockServer::start(vec![(
        "/databases/db-x/query",
        404,
        serde_json::json!({"object": "error", "code": "object_not_found"}),
    )])
    .await;
    // Not configured -> explicit error, no request.
    let a = NotionAdapter::with_base_url("ntn_mock".into(), srv.base_url.clone());
    assert!(a.list_calendar_events("a", "b").await.is_err());
    assert!(srv.requests().is_empty());
    // Default "Date" property; HTTP error propagates.
    let a = a.with_calendar_database("db-x", None);
    let err = a.list_calendar_events("a", "b").await.unwrap_err();
    assert!(err.to_string().contains("404"), "{err}");
    assert_eq!(
        srv.requests()[0].body["filter"]["and"][0]["property"],
        "Date"
    );
}
