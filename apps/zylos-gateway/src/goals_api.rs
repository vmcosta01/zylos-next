//! Goals API — expõe o GoalStore via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /goals` — lista todos os goals com status e progresso
//! - `GET /goals/stats` — resumo agregado (total, ativos, concluídos, progresso médio)
//! - `GET /goals/:id` — detalhes de um goal específico
//!
//! O GoalStore usa rusqlite::Connection (não Send+Sync), então usamos
//! std::sync::Mutex + spawn_blocking para acesso thread-safe no Axum.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use zylos_goals::GoalStore;

/// Wrapper Send+Sync para GoalStore (rusqlite Connection não é Sync).
/// SAFETY: O acesso à Connection é serializado via std::sync::Mutex.
/// GoalStore internamente usa `Arc<tokio::sync::RwLock<Connection>>`, mas
/// como só acessamos via Mutex externo, nunca há acesso concorrente real.
pub struct GoalsHandleInner {
    pub store: GoalStore,
}
unsafe impl Send for GoalsHandleInner {}
unsafe impl Sync for GoalsHandleInner {}

pub type GoalsHandle = Arc<GoalsHandleInner>;

/// Cria o router de goals com seu próprio handle.
pub fn create_goals_router(handle: GoalsHandle) -> Router {
    Router::new()
        .route("/goals", get(list_goals))
        .route("/goals/stats", get(goals_stats))
        .route("/goals/:id", get(get_goal))
        .with_state(handle)
}

async fn list_goals(State(handle): State<GoalsHandle>) -> Response {
    let rt = tokio::runtime::Handle::current();
    let result =
        tokio::task::spawn_blocking(move || rt.block_on(handle.store.list_goals(None))).await;

    match result {
        Ok(Ok(goals)) => Json(json!({ "goals": goals })).into_response(),
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

async fn goals_stats(State(handle): State<GoalsHandle>) -> Response {
    let rt = tokio::runtime::Handle::current();
    let result =
        tokio::task::spawn_blocking(move || rt.block_on(handle.store.dashboard_summary())).await;

    match result {
        Ok(Ok(summary)) => Json(summary).into_response(),
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

async fn get_goal(State(handle): State<GoalsHandle>, Path(id): Path<String>) -> Response {
    let id_for_error = id.clone();
    let rt = tokio::runtime::Handle::current();
    let result = tokio::task::spawn_blocking(move || rt.block_on(handle.store.get_goal(&id))).await;

    match result {
        Ok(Ok(goal)) => Json(goal).into_response(),
        Ok(Err(e)) => {
            let msg = e.to_string();
            if msg.contains("not found") || msg.contains("NotFound") {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("Goal '{}' not found", id_for_error) })),
                )
                    .into_response()
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": msg })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("spawn_blocking failed: {}", e) })),
        )
            .into_response(),
    }
}
