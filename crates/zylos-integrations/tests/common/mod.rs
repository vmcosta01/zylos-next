//! Mock HTTP server mínimo (axum) para testes de integração offline (T11.8.7).
//!
//! Sobe um servidor em `127.0.0.1:0`, responde JSON canned por path e
//! registra cada requisição (método, path, headers relevantes, body) para asserts.
#![allow(dead_code)]

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::IntoResponse,
    Router,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Requisição capturada pelo mock.
#[derive(Debug, Clone)]
pub struct Captured {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub authorization: Option<String>,
    pub notion_version: Option<String>,
    pub body: serde_json::Value,
}

#[derive(Clone, Default)]
struct MockState {
    routes: Arc<HashMap<String, (u16, serde_json::Value)>>,
    log: Arc<Mutex<Vec<Captured>>>,
}

/// Servidor mock em execução.
pub struct MockServer {
    pub base_url: String,
    log: Arc<Mutex<Vec<Captured>>>,
}

impl MockServer {
    /// `routes`: path (sem query) -> (status, body JSON). Paths não mapeados retornam 404.
    pub async fn start(routes: Vec<(&str, u16, serde_json::Value)>) -> Self {
        let map: HashMap<String, (u16, serde_json::Value)> = routes
            .into_iter()
            .map(|(p, s, b)| (p.to_string(), (s, b)))
            .collect();
        let state = MockState {
            routes: Arc::new(map),
            log: Arc::new(Mutex::new(Vec::new())),
        };
        let log = state.log.clone();
        let app = Router::new().fallback(handler).with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("mock server");
        });
        Self {
            base_url: format!("http://{}", addr),
            log,
        }
    }

    pub fn requests(&self) -> Vec<Captured> {
        self.log.lock().expect("log lock").clone()
    }
}

async fn handler(
    State(state): State<MockState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let h = |k: &str| {
        headers
            .get(k)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
    };
    let parsed = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    state.log.lock().expect("log lock").push(Captured {
        method: method.to_string(),
        path: uri.path().to_string(),
        query: uri.query().map(|q| q.to_string()),
        authorization: h("authorization"),
        notion_version: h("notion-version"),
        body: parsed,
    });
    match state.routes.get(uri.path()) {
        Some((status, json)) => (
            StatusCode::from_u16(*status).unwrap_or(StatusCode::OK),
            axum::Json(json.clone()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "not_mapped"})),
        ),
    }
}

/// Retorna o token real do ambiente, se presente e não vazio.
pub fn env_token(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|t| !t.trim().is_empty())
}
