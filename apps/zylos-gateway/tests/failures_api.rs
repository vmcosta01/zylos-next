//! `GET /api/v1/failures`: a falha publicada no barramento chega à lista.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use serde_json::Value;
use tower::ServiceExt;
use zylos_gateway::failures_api::{create_failures_router, FailuresStore};
use zylos_live_comm::event_types::AgentStateChanged;
use zylos_live_comm::{EventBus, ZylosEvent};

async fn listar(app: &Router, uri: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .unwrap();
    (st, serde_json::from_slice(&bytes).unwrap())
}

fn evento(agent: &str, novo: &str, motivo: Option<&str>) -> ZylosEvent {
    ZylosEvent::AgentStateChanged(AgentStateChanged {
        agent_id: agent.to_string(),
        previous_state: "running".to_string(),
        new_state: novo.to_string(),
        reason: motivo.map(str::to_string),
    })
}

#[tokio::test]
async fn falha_publicada_no_barramento_aparece_na_lista() {
    let d = tempfile::tempdir().unwrap();
    let path = format!("sqlite:{}", d.path().join("f.db").display());
    let store = FailuresStore::connect(&path).await.unwrap();
    let bus = EventBus::new(64, None);
    store.spawn_recorder(&bus).await.unwrap();
    let app = Router::new().nest("/api/v1", create_failures_router(store));

    let (st, v) = listar(&app, "/api/v1/failures").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["failures"].as_array().unwrap().len(), 0);

    // Só `new_state = "error"` vira falha.
    bus.publish("agent.state", evento("vigia", "running", None), 1)
        .await
        .unwrap();
    bus.publish(
        "agent.state",
        evento("vigia", "error", Some("Chave de API faltando")),
        1,
    )
    .await
    .unwrap();

    let mut achou = Value::Null;
    for _ in 0..50 {
        let (_, v) = listar(&app, "/api/v1/failures").await;
        if !v["failures"].as_array().unwrap().is_empty() {
            achou = v;
            break;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    let lista = achou["failures"].as_array().expect("falha não chegou");
    assert_eq!(lista.len(), 1, "{achou}");
    assert_eq!(lista[0]["agent_id"], "vigia");
    assert_eq!(lista[0]["title"], "Chave de API faltando");
    assert!(lista[0]["created_at"].as_str().unwrap().contains('T'));

    let (st, v) = listar(&app, "/api/v1/failures?limit=1").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["failures"].as_array().unwrap().len(), 1);
}
