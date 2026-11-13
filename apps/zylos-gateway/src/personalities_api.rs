//! Personalities API — expõe o PersonalityRegistry via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /personalities/stats` — resumo agregado (total_profiles, divisions)
//! - `GET /personalities/list` — lista todas as personalidades com filtros
//!
//! O PersonalityRegistry usa rusqlite::Connection (não Send+Sync), então usamos
//! wrapper com unsafe impl Send+Sync + spawn_blocking para acesso thread-safe.

use axum::{
 Json, Router,
 extract::State,
 http::StatusCode,
 response::{IntoResponse, Response},
 routing::get,
};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use zylos_personalities::PersonalityRegistry;

/// Wrapper Send+Sync para PersonalityRegistry.
pub struct PersonalitiesHandleInner {
 pub registry: PersonalityRegistry,
}
unsafe impl Send for PersonalitiesHandleInner {}
unsafe impl Sync for PersonalitiesHandleInner {}

pub type PersonalitiesHandle = Arc<PersonalitiesHandleInner>;

#[derive(Debug, Serialize)]
pub struct PersonalityStats {
 pub total_profiles: usize,
 pub divisions: usize,
}

/// Cria o router de personalities com seu próprio handle.
pub fn create_personalities_router(handle: PersonalitiesHandle) -> Router {
 Router::new()
 .route("/personalities/stats", get(personalities_stats))
 .route("/personalities/divisions", get(personalities_divisions))
 .with_state(handle)
}

async fn personalities_stats(State(handle): State<PersonalitiesHandle>) -> Response {
 let rt = tokio::runtime::Handle::current();
 let result = tokio::task::spawn_blocking(move || {
 rt.block_on(handle.registry.stats())
 })
 .await;

 match result {
 Ok(Ok(stats)) => Json(PersonalityStats {
 total_profiles: stats.total_profiles,
 divisions: stats.division_counts.len(),
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

async fn personalities_divisions(State(handle): State<PersonalitiesHandle>) -> Response {
 let rt = tokio::runtime::Handle::current();
 let result = tokio::task::spawn_blocking(move || {
 rt.block_on(handle.registry.list_divisions())
 })
 .await;

 match result {
 Ok(Ok(divisions)) => Json(json!({ "divisions": divisions })).into_response(),
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