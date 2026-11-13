//! Evidence Ledger API — expõe o EvidenceLedger via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /evidence/stats` — resumo agregado (total_entries, merkle_root)
//! - `GET /evidence/entries` — lista paginada de entradas do ledger
//!
//! O EvidenceLedger usa rusqlite::Connection (não Send+Sync), então usamos
//! wrapper com unsafe impl Send+Sync + spawn_blocking para acesso thread-safe.

use axum::{
 Json, Router,
 extract::{Query, State},
 http::StatusCode,
 response::{IntoResponse, Response},
 routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use zylos_evidence_ledger::EvidenceLedger;

/// Wrapper Send+Sync para EvidenceLedger.
pub struct EvidenceHandleInner {
 pub ledger: EvidenceLedger,
}
unsafe impl Send for EvidenceHandleInner {}
unsafe impl Sync for EvidenceHandleInner {}

pub type EvidenceHandle = Arc<EvidenceHandleInner>;

#[derive(Debug, Deserialize)]
pub struct EntriesQuery {
 pub limit: Option<usize>,
 pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct EvidenceStats {
 pub total_entries: u64,
 pub merkle_root: String,
}

/// Cria o router de evidence com seu próprio handle.
pub fn create_evidence_router(handle: EvidenceHandle) -> Router {
 Router::new()
 .route("/evidence/stats", get(evidence_stats))
 .route("/evidence/entries", get(evidence_entries))
 .with_state(handle)
}

async fn evidence_stats(State(handle): State<EvidenceHandle>) -> Response {
 let rt = tokio::runtime::Handle::current();
 let result = tokio::task::spawn_blocking(move || {
 rt.block_on(handle.ledger.stats())
 })
 .await;

 match result {
 Ok(Ok(stats)) => Json(EvidenceStats {
 total_entries: stats.total_entries,
 merkle_root: stats.merkle_root.unwrap_or_default(),
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

async fn evidence_entries(
 State(handle): State<EvidenceHandle>,
 Query(params): Query<EntriesQuery>,
) -> Response {
 let limit = params.limit.unwrap_or(50) as u64;
 let offset = params.offset.unwrap_or(0) as u64;
 let rt = tokio::runtime::Handle::current();
 let result = tokio::task::spawn_blocking(move || {
 rt.block_on(handle.ledger.list_entries(limit, offset))
 })
 .await;

 match result {
 Ok(Ok(entries)) => Json(json!({ "entries": entries, "limit": limit, "offset": offset }))
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