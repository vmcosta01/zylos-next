//! Sentinel/AIOps API — expõe AgentShield + BrowserTakeover via HTTP para o cockpit.
//!
//! Rotas (montadas sob `/api/v1`):
//! - `GET /sentinel/stats` — resumo agregado (audit_events, critical, blocked, cdp_sessions)
//! - `GET /sentinel/events` — timeline de eventos de auditoria
//!
//! AgentShield usa rusqlite::Connection (não Send+Sync), então usamos
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
use zylos_agent_shield::AgentShield;
use zylos_browser_takeover::TakeoverStore;

/// Wrapper Send+Sync para AgentShield + TakeoverStore.
pub struct SentinelHandleInner {
 pub shield: AgentShield,
 pub takeover: TakeoverStore,
}
unsafe impl Send for SentinelHandleInner {}
unsafe impl Sync for SentinelHandleInner {}

pub type SentinelHandle = Arc<SentinelHandleInner>;

#[derive(Debug, Serialize)]
pub struct SentinelStats {
 pub total_audit_events: usize,
 pub critical_events: usize,
 pub high_events: usize,
 pub outbound_blocked: usize,
 pub active_cdp_sessions: usize,
 pub pending_cdp_sessions: usize,
}

/// Cria o router de sentinel com seu próprio handle.
pub fn create_sentinel_router(handle: SentinelHandle) -> Router {
 Router::new()
 .route("/sentinel/stats", get(sentinel_stats))
 .with_state(handle)
}

async fn sentinel_stats(State(handle): State<SentinelHandle>) -> Response {
 let rt = tokio::runtime::Handle::current();
 let result = tokio::task::spawn_blocking(move || {
 rt.block_on(async {
 let shield_stats = handle.shield.stats().await;
 let cdp_summary = handle.takeover.dashboard_summary().await;
 (shield_stats, cdp_summary)
 })
 })
 .await;

 match result {
 Ok((Ok(shield), Ok(cdp))) => Json(SentinelStats {
 total_audit_events: shield.total_audit_events,
 critical_events: shield.critical_events,
 high_events: shield.high_events,
 outbound_blocked: shield.outbound_blocked,
 active_cdp_sessions: cdp.active_sessions,
 pending_cdp_sessions: cdp.pending_sessions,
 })
 .into_response(),
 Ok((Err(e), _)) => (
 StatusCode::INTERNAL_SERVER_ERROR,
 Json(json!({ "error": format!("shield: {}", e) })),
 )
 .into_response(),
 Ok((_, Err(e))) => (
 StatusCode::INTERNAL_SERVER_ERROR,
 Json(json!({ "error": format!("cdp: {}", e) })),
 )
 .into_response(),
 Err(e) => (
 StatusCode::INTERNAL_SERVER_ERROR,
 Json(json!({ "error": format!("spawn_blocking failed: {}", e) })),
 )
 .into_response(),
 }
}