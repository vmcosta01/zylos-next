//! zylos-gateway library — exposes AppState and build_router for integration tests.

pub mod bots_api;
pub mod config;
pub mod hermes_api;
pub mod local_guard;
pub mod orchestrate_api;
pub mod phase2_integration;
pub mod soar_api;

use std::sync::Arc;
use axum::{Router, routing::get};
use zylos_live_comm::{
    EventBus,
    approval::{ApprovalConfig, ApprovalStore},
    cockpit::create_cockpit_router,
    routes::create_router as create_sse_router,
    skills_api::{SkillsApiState, create_skills_router},
};
use zylos_soar::orchestrator::SoarOrchestrator;
use zylos_swarm_orchestrator::SwarmOrchestrator;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub event_bus: EventBus,
    pub skills_state: SkillsApiState,
    pub soar_orchestrator: Arc<SoarOrchestrator>,
    /// Swarm orchestrator for crew/task scheduling (Phase 2)
    pub swarm: Arc<SwarmOrchestrator>,
    /// Readiness flag — set to true after all subsystems are initialized (Task 7.1.1)
    pub ready: Arc<std::sync::atomic::AtomicBool>,
}

/// Build the consolidated API gateway router
pub fn build_router(state: AppState) -> Router {
    // Sub-routers that consume their own state internally return Router<()>
    let cockpit = create_cockpit_router(state.event_bus.clone());
    let approval_store = std::sync::Arc::new(ApprovalStore::new(ApprovalConfig::default()));
    let sse = create_sse_router(state.event_bus.clone(), approval_store);
    let skills = create_skills_router(state.skills_state.clone());
    // SOAR router needs AppState — resolve it here so it becomes Router<()> for merging
    let soar = soar_api::create_soar_router().with_state(state.clone());
    let orchestrate = orchestrate_api::create_orchestrate_router().with_state(state.clone());
    let hermes = hermes_api::create_hermes_router(std::env::var("ZYLOS_HERMES_TOKEN").ok().filter(|t| t.len() >= 16)).with_state(state.clone());
    let bots = bots_api::create_bots_router(zylos_pensar::Config::do_ambiente());

    let ready_flag = state.ready.clone();
    let root_health = Router::new()
        .route(
            "/gateway/health",
            get(|| async {
                axum::Json(serde_json::json!({
                    "service": "zylos-gateway",
                    "status": "healthy",
                    "version": env!("CARGO_PKG_VERSION"),
                }))
            }),
        )
        .route(
            "/gateway/ready",
            get(move || {
                let ready = ready_flag.clone();
                async move {
                    use std::sync::atomic::Ordering;
                    if ready.load(Ordering::Relaxed) {
                        axum::Json(serde_json::json!({"status": "ready"}))
                    } else {
                        axum::Json(serde_json::json!({"status": "not_ready"}))
                    }
                }
            }),
        )
        .route(
            "/gateway/live",
            get(|| async {
                axum::Json(serde_json::json!({"status": "alive"}))
            }),
        );

    Router::new()
        .merge(root_health)
        .nest("/api/v1", cockpit.merge(sse).merge(skills).merge(soar).merge(orchestrate).merge(hermes).merge(bots))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(
            tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::predicate(local_guard::cors_origem_local))
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        // Camada mais externa: roda antes de tudo, inclusive da resposta de CORS ao preflight.
        .layer(axum::middleware::from_fn(local_guard::so_desta_maquina))
}