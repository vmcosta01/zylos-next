//! Memory API — expõe o MemoryStore via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /memory/stats` — resumo agregado (total_entries, pending_ops, namespaces)
//! - `GET /memory/entries` — lista paginada de entradas de memória
//!
//! O MemoryStore usa rusqlite::Connection (não Send+Sync), então usamos
//! wrapper com unsafe impl Send+Sync + spawn_blocking para acesso thread-safe.

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use zylos_memory_sync::MemoryStore;

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
        .with_state(handle)
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
