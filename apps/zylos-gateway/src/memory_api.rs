//! Memory API — expõe o MemoryStore via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /memory/stats` — resumo agregado (total_entries, pending_ops, namespaces)
//! - `GET /memory/entries?space=<id>` — notas do Space (Geral ou sem `space` = todas)
//! - `POST /memory/entries` — cria uma nota `{space?, content, title?}`
//! - `DELETE /memory/entries/:key?space=<id>` — remove uma nota
//!
//! Escopo por Space: a nota vive no namespace `space:<id>`; o Space "geral" enxerga tudo.
//!
//! O MemoryStore usa rusqlite::Connection (não Send+Sync), então usamos
//! wrapper com unsafe impl Send+Sync + spawn_blocking para acesso thread-safe.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use zylos_memory_sync::{MemoryEntry, MemoryQuery, MemoryStore};

/// Wrapper Send+Sync para MemoryStore (rusqlite Connection não é Sync).
/// SAFETY: O acesso à Connection é serializado via std::sync::Mutex externo
/// no caller; aqui apenas garantimos que o wrapper seja Send+Sync para o Axum.
pub struct MemoryHandleInner {
    pub store: MemoryStore,
}
unsafe impl Send for MemoryHandleInner {}
unsafe impl Sync for MemoryHandleInner {}

pub type MemoryHandle = Arc<MemoryHandleInner>;

#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_entries: u64,
    pub pending_sync_ops: u64,
    pub namespaces: usize,
}

/// Cria o router de memory com seu próprio handle.
pub fn create_memory_router(handle: MemoryHandle) -> Router {
    Router::new()
        .route("/memory/stats", get(memory_stats))
        .route("/memory/entries", get(list_entries).post(create_entry))
        .route("/memory/entries/:key", delete(delete_entry))
        .with_state(handle)
}

/// Space que enxerga todas as notas (espelha `spaces_api::GENERAL_ID`).
const GERAL: &str = "geral";
/// Prefixo do namespace das notas por Space.
const NS_PREFIX: &str = "space:";
const MAX_CONTENT: usize = 8_000;
const MAX_TITLE: usize = 120;

fn namespace_do_space(space: &str) -> String {
    format!("{NS_PREFIX}{space}")
}

/// Space de um namespace (`space:x` → `x`; qualquer outro namespace aparece no Geral).
fn space_do_namespace(ns: &str) -> &str {
    ns.strip_prefix(NS_PREFIX).unwrap_or(GERAL)
}

fn space_valido(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn erro(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(json!({ "error": msg.into() }))).into_response()
}

/// Nota como devolvida pela API.
#[derive(Debug, Serialize)]
pub struct EntryView {
    pub key: String,
    pub space_id: String,
    pub title: String,
    pub content: String,
    pub updated_at: String,
}

fn to_view(e: MemoryEntry) -> EntryView {
    EntryView {
        space_id: space_do_namespace(&e.namespace).to_string(),
        title: e.tags.first().cloned().unwrap_or_default(),
        key: e.key,
        content: e.content,
        updated_at: e.updated_at.to_rfc3339(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SpaceQuery {
    #[serde(default)]
    pub space: Option<String>,
}

async fn list_entries(State(h): State<MemoryHandle>, Query(q): Query<SpaceQuery>) -> Response {
    let space = q.space.unwrap_or_default();
    let filtro = if space.is_empty() || space == GERAL {
        None
    } else if space_valido(&space) {
        Some(namespace_do_space(&space))
    } else {
        return erro(StatusCode::BAD_REQUEST, "space inválido");
    };
    let query = MemoryQuery {
        namespace: filtro,
        limit: Some(200),
        ..Default::default()
    };
    match h.store.query(&query).await {
        Ok(v) => Json(json!({ "entries": v.into_iter().map(to_view).collect::<Vec<_>>() }))
            .into_response(),
        Err(e) => erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateEntry {
    #[serde(default)]
    pub space: Option<String>,
    pub content: String,
    #[serde(default)]
    pub title: Option<String>,
}

async fn create_entry(State(h): State<MemoryHandle>, Json(req): Json<CreateEntry>) -> Response {
    let space = req.space.unwrap_or_else(|| GERAL.to_string());
    if !space_valido(&space) {
        return erro(StatusCode::BAD_REQUEST, "space inválido");
    }
    let content = req.content.trim().to_string();
    if content.is_empty() || content.chars().count() > MAX_CONTENT {
        return erro(
            StatusCode::BAD_REQUEST,
            format!("conteúdo obrigatório (até {MAX_CONTENT} caracteres)"),
        );
    }
    let title = req.title.unwrap_or_default().trim().to_string();
    if title.chars().count() > MAX_TITLE {
        return erro(
            StatusCode::BAD_REQUEST,
            format!("título com até {MAX_TITLE} caracteres"),
        );
    }
    let mut entry = MemoryEntry::new(
        namespace_do_space(&space),
        uuid::Uuid::new_v4().to_string(),
        content,
        "cockpit",
    );
    if !title.is_empty() {
        entry.tags = vec![title];
    }
    match h.store.upsert(&entry).await {
        Ok(_) => (StatusCode::CREATED, Json(to_view(entry))).into_response(),
        Err(e) => erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn delete_entry(
    State(h): State<MemoryHandle>,
    Path(key): Path<String>,
    Query(q): Query<SpaceQuery>,
) -> Response {
    let space = q.space.unwrap_or_else(|| GERAL.to_string());
    if !space_valido(&space) {
        return erro(StatusCode::BAD_REQUEST, "space inválido");
    }
    match h.store.delete(&namespace_do_space(&space), &key).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => erro(StatusCode::NOT_FOUND, "nota não encontrada"),
        Err(e) => erro(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

async fn memory_stats(State(handle): State<MemoryHandle>) -> Response {
    let rt = tokio::runtime::Handle::current();
    let result = tokio::task::spawn_blocking(move || rt.block_on(handle.store.stats())).await;

    match result {
        Ok(Ok(stats)) => Json(MemoryStats {
            total_entries: stats.total_entries as u64,
            pending_sync_ops: stats.pending_sync_ops as u64,
            namespaces: stats.namespace_counts.len(),
        })
        .into_response(),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("spawn_blocking failed: {}", e) })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn app() -> Router {
        let store = MemoryStore::open(":memory:").await.unwrap();
        create_memory_router(Arc::new(MemoryHandleInner { store }))
    }

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(
                body.map(|s| Body::from(s.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        let v = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, v)
    }

    #[tokio::test]
    async fn notas_ficam_no_space_e_geral_ve_tudo() {
        let app = app().await;
        let (st, a) = call(
            &app,
            "POST",
            "/memory/entries",
            Some(r#"{"space":"juridico","content":"prazo 15 dias","title":"Prazo"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        assert_eq!(a["space_id"], "juridico");
        assert_eq!(a["title"], "Prazo");
        call(
            &app,
            "POST",
            "/memory/entries",
            Some(r#"{"space":"ops","content":"deploy sexta"}"#),
        )
        .await;

        let (_, j) = call(&app, "GET", "/memory/entries?space=juridico", None).await;
        assert_eq!(j["entries"].as_array().unwrap().len(), 1);
        assert_eq!(j["entries"][0]["content"], "prazo 15 dias");
        let (_, g) = call(&app, "GET", "/memory/entries?space=geral", None).await;
        assert_eq!(g["entries"].as_array().unwrap().len(), 2);
        let (_, t) = call(&app, "GET", "/memory/entries", None).await;
        assert_eq!(t["entries"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn remove_so_no_space_certo() {
        let app = app().await;
        let (_, a) = call(
            &app,
            "POST",
            "/memory/entries",
            Some(r#"{"space":"ops","content":"x"}"#),
        )
        .await;
        let key = a["key"].as_str().unwrap().to_string();
        let (st, _) = call(
            &app,
            "DELETE",
            &format!("/memory/entries/{key}?space=juridico"),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _) = call(
            &app,
            "DELETE",
            &format!("/memory/entries/{key}?space=ops"),
            None,
        )
        .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn valida_entrada() {
        let app = app().await;
        let (st, _) = call(
            &app,
            "POST",
            "/memory/entries",
            Some(r#"{"content":"   "}"#),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        let (st, _) = call(
            &app,
            "POST",
            "/memory/entries",
            Some(r#"{"space":"a b","content":"x"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        let (st, _) = call(&app, "GET", "/memory/entries?space=../x", None).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }
}
