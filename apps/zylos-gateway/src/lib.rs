//! zylos-gateway library — exposes AppState and build_router for integration tests.

pub mod agents_api;
pub mod analytics_api;
pub mod api_auth;
pub mod approvals_api;
pub mod auth_api;
pub mod axiom_logging;
pub mod bots_api;
pub mod channels_api;
pub mod computer_view_api;
pub mod config;
pub mod crdt_sync;
pub mod evidence_api;
pub mod failures_api;
pub mod federation_api;
pub mod files_api;
pub mod goals_api;
pub mod hermes_api;
pub mod hermes_desktop_api;
pub mod hermes_employee_api;
pub mod hermes_security_api;
pub mod hermes_skill_evolution_api;
pub mod integrations_api;
pub mod local_guard;
pub mod memory_api;
pub mod notification_router;
pub mod orchestrate_api;
pub mod password_masking;
pub mod personalities_api;
pub mod phase2_integration;
pub mod rate_limit;
pub mod reactions_api;
pub mod relay_api;
pub mod security_headers;
pub mod sentinel_api;
pub mod sentinel_runtime;
pub mod skills_api;
pub mod soar_api;
pub mod spaces_api;
pub mod subagents_api;
pub mod threads_api;
pub mod webrtc_relay;

use axum::{routing::get, Router};
use std::sync::Arc;
use zylos_live_comm::{
    approval::{ApprovalConfig, ApprovalStore},
    cockpit::create_cockpit_router,
    routes::create_router as create_sse_router,
    skills_api::{create_skills_router, SkillsApiState},
    EventBus,
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

/// Handles for API routers that require Send+Sync wrappers around non-Sync stores.
/// Initialized in main.rs (async context) and passed to build_router.
pub struct ApiHandles {
    pub goals: goals_api::GoalsHandle,
    pub memory: memory_api::MemoryHandle,
    pub personalities: personalities_api::PersonalitiesHandle,
    pub sentinel: sentinel_api::SentinelHandle,
    pub evidence: evidence_api::EvidenceHandle,
}

/// Abre um banco SQLite tentando de novo quando outro processo/teste segura o lock
/// (`database is locked`). Para em erro de outra natureza ou após ~3 s.
async fn retry_locked<T, F, Fut>(what: &str, mut open: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let mut last = String::new();
    for attempt in 0..15 {
        match open().await {
            Ok(v) => return v,
            Err(e) if e.contains("database is locked") => {
                last = e;
                tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt + 1))).await;
            }
            Err(e) => panic!("failed to open {what}: {e}"),
        }
    }
    panic!("failed to open {what}: {last}");
}

/// Build the consolidated API gateway router
pub fn build_router(state: AppState, handles: ApiHandles) -> Router {
    // Sub-routers that consume their own state internally return Router<()>
    let cockpit = create_cockpit_router(state.event_bus.clone());
    let approval_store = std::sync::Arc::new(ApprovalStore::new(ApprovalConfig::default()));
    // Separate SQLite-backed approval store for the REST API (list/resolve)
    let approval_sqlite_store = std::sync::Arc::new(tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let path = std::env::var("ZYLOS_APPROVALS_DB")
                .unwrap_or_else(|_| "sqlite:data/approvals.db".to_string());
            // Ensure parent directory exists
            if let Some(parent) = std::path::Path::new(&path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            // Vários gateways/testes abrindo o mesmo arquivo juntos disputam o lock da troca para
            // WAL e da migração ("database is locked"): tenta de novo antes de desistir.
            retry_locked("approvals store", || async {
                let store = zylos_approval::ApprovalStore::connect(&path)
                    .await
                    .map_err(|e| e.to_string())?;
                // Banco novo não tem a tabela: sem isto, criar/listar/resolver davam 500.
                store.run_migrations().await.map_err(|e| e.to_string())?;
                Ok(store)
            })
            .await
        })
    }));
    let approvals = approvals_api::create_approvals_router(
        approvals_api::ApprovalsState::new(approval_sqlite_store)
            .with_event_bus(state.event_bus.clone()),
    );
    // Spaces: grupos de agentes que filtram Inbox e chat no cockpit (banco próprio, `ZYLOS_SPACES_DB`).
    let spaces_store = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let path = std::env::var("ZYLOS_SPACES_DB")
                .unwrap_or_else(|_| "sqlite:data/spaces.db".to_string());
            if let Some(parent) = std::path::Path::new(path.trim_start_matches("sqlite:")).parent()
            {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            retry_locked("spaces store", || async {
                spaces_api::SpacesStore::connect(&path)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
        })
    });
    let spaces = spaces_api::create_spaces_router(spaces_store);
    // Falhas de agentes: o gravador assina o barramento e guarda o que virou toast (`ZYLOS_FAILURES_DB`).
    let failures_store = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let path = std::env::var("ZYLOS_FAILURES_DB")
                .unwrap_or_else(|_| "sqlite:data/failures.db".to_string());
            if let Some(parent) = std::path::Path::new(path.trim_start_matches("sqlite:")).parent()
            {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            retry_locked("failures store", || async {
                failures_api::FailuresStore::connect(&path)
                    .await
                    .map_err(|e| e.to_string())
            })
            .await
        })
    });
    if let Err(e) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(failures_store.spawn_recorder(&state.event_bus))
    }) {
        tracing::warn!(error = %e, "gravador de falhas não iniciou");
    }
    let failures = failures_api::create_failures_router(failures_store);
    // Registro de contratos de skills: pasta `ZYLOS_SKILLS_DIR` (padrão `data/skills`).
    let skills_registry = skills_api::create_skills_router(skills_api::SkillsState::do_ambiente());
    // Federação: nós de `ZYLOS_FEDERATION_PEERS`, sondados pelo health de cada um.
    let federation =
        federation_api::create_federation_router(federation_api::FederationState::do_ambiente());
    // Arquivos do cockpit: pasta `ZYLOS_FILES_DIR` (padrão `data/files`).
    let files = files_api::create_files_router(files_api::FilesState::do_ambiente());
    let sse = create_sse_router(state.event_bus.clone(), approval_store);
    let skills = create_skills_router(state.skills_state.clone());
    // SOAR router needs AppState — resolve it here so it becomes Router<()> for merging
    let soar = soar_api::create_soar_router().with_state(state.clone());
    let orchestrate = orchestrate_api::create_orchestrate_router().with_state(state.clone());
    let hermes = hermes_api::create_hermes_router(
        std::env::var("ZYLOS_HERMES_TOKEN")
            .ok()
            .filter(|t| t.len() >= 16),
    )
    .with_state(state.clone());
    let hermes_security =
        hermes_security_api::create_hermes_security_router().with_state(state.clone());
    let hermes_desktop =
        hermes_desktop_api::create_hermes_desktop_router().with_state(state.clone());
    let hermes_skill_evolution = hermes_skill_evolution_api::create_hermes_skill_evolution_router()
        .with_state(state.clone());
    let hermes_employee =
        hermes_employee_api::create_hermes_employee_router().with_state(state.clone());
    let bots = bots_api::create_bots_router_with_bus(
        zylos_pensar::Config::do_ambiente(),
        Some(state.event_bus.clone()),
    );

    // Dedicated API routers with Send+Sync handles
    let goals = goals_api::create_goals_router(handles.goals);
    let memory = memory_api::create_memory_router(handles.memory);
    let personalities = personalities_api::create_personalities_router(handles.personalities);
    let sentinel = sentinel_api::create_sentinel_router(handles.sentinel);
    let evidence = evidence_api::create_evidence_router(handles.evidence);
    // Fase 11.7: runtime do Sentinel (métricas → regras → gate HITL → SOAR/Axiom)
    let sentinel_rt = std::sync::Arc::new(
        sentinel_runtime::SentinelRuntime::from_env(Box::new(
            sentinel_runtime::OrchestratorForwarder(state.soar_orchestrator.clone()),
        ))
        .with_event_bus(state.event_bus.clone()),
    );
    sentinel_rt.spawn_loop(sentinel_runtime::tick_interval_from_env());
    let sentinel_runtime_routes =
        sentinel_runtime::create_sentinel_runtime_router(sentinel_rt.clone());

    // Phase 2 aggregated status endpoint (used by cockpit components: memory, tasks, personalities, sentinel, swarm, evidence, hitl)
    let phase2_state = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(phase2_integration::Phase2State::init())
    });
    let phase2 = match phase2_state {
        Ok(ps) => phase2_integration::create_phase2_router(ps),
        Err(e) => {
            tracing::warn!(error = %e, "Phase2 subsystems failed to init; /phase2/status will be unavailable");
            Router::new()
        }
    };

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
            get(|| async { axum::Json(serde_json::json!({"status": "alive"})) }),
        );

    // === Fase 11.4 T11.4.6: Health check endpoints padronizados ===
    // `/api/v1/health` vem do router do cockpit (zylos-live-comm), que devolve
    // status/active_streams/version/service — formato que o cockpit consome.
    // Não registrar `/health` aqui de novo: o axum entra em pânico com rota duplicada.
    let api_health = Router::new().route(
        "/cockpit/health",
        get(|| async {
            axum::Json(serde_json::json!({
            "status": "ok",
            "component": "cockpit",
            "endpoints_available": [
            "/api/v1/channels",
            "/api/v1/computer-view/sessions",
            "/api/v1/hermes/skills",
            "/api/v1/goals",
            "/api/v1/memory",
            "/api/v1/personalities",
            "/api/v1/sentinel",
            "/api/v1/evidence",
            "/api/v1/phase2/status",
            "/api/v1/integrations/status"
            ]
            }))
        }),
    );

    // New Fase 9 routers (channels + computer-view)
    let channels_store = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let path = std::env::var("ZYLOS_CHANNELS_DB")
                .unwrap_or_else(|_| "sqlite://data/channels.db".to_string());
            // Testes em paralelo abrem o mesmo arquivo: `database is locked` é passageiro.
            // Migração e semente são idempotentes, então a sequência inteira pode repetir.
            retry_locked("channels store", || async {
                let store = zylos_channels::ChannelsStore::connect(&path)
                    .await
                    .map_err(|e| e.to_string())?;
                store.run_migrations().await.map_err(|e| e.to_string())?;
                store.seed_defaults().await.map_err(|e| e.to_string())?;
                Ok(store)
            })
            .await
        })
    });
    let channels = channels_api::create_channels_router(channels_api::ChannelsHandle::from_store(
        channels_store,
    ));
    // O mesmo relay serve o WebSocket do cockpit e a rota onde o agente empurra frames.
    let relay_state = webrtc_relay::WebRtcRelayState::new();
    let computer_view_handle =
        computer_view_api::ComputerViewHandle::new().with_relay(relay_state.clone());
    // Opt-in: fotografa o monitor deste computador para quem assiste uma sessão local.
    if computer_view_api::captura_local_ligada() {
        computer_view_api::spawn_captura_local(
            computer_view_handle.clone(),
            computer_view_api::intervalo_captura(),
        );
    }
    let computer_view = computer_view_api::create_computer_view_router(computer_view_handle);
    let webrtc_relay = webrtc_relay::create_webrtc_relay_router(relay_state);
    let auth_state = auth_api::AuthState::from_env();
    let api_auth_state = api_auth::ApiAuth::new(auth_state.token_service());
    if !api_auth_state.enforced() {
        tracing::warn!(
            "ZYLOS_AUTH_SECRET ausente: /api/v1 sem login (só local); aprovações bloqueadas"
        );
    }
    let auth = auth_api::create_auth_router_with(auth_state.clone());
    let integrations = integrations_api::create_integrations_router(
        integrations_api::IntegrationsState::from_env(),
    );
    let analytics =
        analytics_api::create_analytics_router(analytics_api::AnalyticsHandle::from_env());
    let crdt_state = crdt_sync::CrdtSyncState::with_store_path(std::path::Path::new(
        &std::env::var("ZYLOS_CRDT_DB").unwrap_or_else(|_| "data/crdt.db".to_string()),
    ))
    .with_token_service(auth_state.token_service());
    crdt_sync::spawn_gc_task(&crdt_state);
    let crdt = crdt_sync::create_crdt_sync_router(crdt_state);

    Router::new()
        .merge(root_health)
        .nest(
            "/api/v1",
            api_health
                .merge(cockpit)
                .merge(sse)
                .merge(skills)
                .merge(soar)
                .merge(orchestrate)
                .merge(hermes)
                .merge(hermes_security)
                .merge(hermes_desktop)
                .merge(hermes_skill_evolution)
                .merge(hermes_employee)
                .merge(bots)
                .merge(goals)
                .merge(memory)
                .merge(personalities)
                .merge(sentinel)
                .merge(sentinel_runtime_routes)
                .merge(evidence)
                .merge(phase2)
                .merge(channels)
                .merge(computer_view)
                .merge(webrtc_relay)
                .merge(auth)
                .merge(integrations)
                .merge(analytics)
                .merge(crdt)
                .merge(approvals)
                .merge(spaces)
                .merge(failures)
                .merge(skills_registry)
                .merge(federation)
                .merge(files)
                .merge(agents_api::create_agents_router(state.swarm.clone()))
                .merge(subagents_api::create_subagents_router(state.swarm.clone()))
                .merge(threads_api::create_threads_router(
                    threads_api::ThreadsStore::from_env().expect("threads store"),
                ))
                .merge(relay_api::create_relay_router(
                    relay_api::RelayState::from_env(),
                ))
                .merge(reactions_api::create_reactions_router(
                    reactions_api::ReactionsState::from_env(),
                )),
        )
        .layer(axum::middleware::from_fn_with_state(
            api_auth_state,
            api_auth::exigir_auth,
        ))
        .layer(axum::middleware::from_fn_with_state(
            sentinel_rt,
            sentinel_runtime::track_requests,
        ))
        .layer(axum::middleware::from_fn(
            security_headers::security_headers,
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        // F15: per-IP token-bucket rate limiter (ZYLOS_RATE_LIMIT_RPS, default 100).
        .layer(axum::middleware::from_fn_with_state(
            rate_limit::RateLimiter::from_env(),
            rate_limit::RateLimiter::middleware,
        ))
        .layer({
            // F19: CORS com métodos/headers explícitos + origens via ZYLOS_CORS_ORIGINS.
            use axum::http::{HeaderName, Method};
            let mut cors = tower_http::cors::CorsLayer::new()
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                    HeaderName::from_static("x-requested-with"),
                ]);
            if let Ok(origins_env) = std::env::var("ZYLOS_CORS_ORIGINS") {
                let origins: Vec<_> = origins_env
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if origins.is_empty() {
                    tracing::warn!(
                        "ZYLOS_CORS_ORIGINS definido mas vazio — usando apenas origens locais"
                    );
                    cors = cors.allow_origin(tower_http::cors::AllowOrigin::predicate(
                        local_guard::cors_origem_local,
                    ));
                } else {
                    cors = cors.allow_origin(tower_http::cors::AllowOrigin::list(origins));
                }
            } else {
                cors = cors.allow_origin(tower_http::cors::AllowOrigin::predicate(
                    local_guard::cors_origem_local,
                ));
            }
            cors
        })
        // Camada mais externa: roda antes de tudo, inclusive da resposta de CORS ao preflight.
        .layer(axum::middleware::from_fn(local_guard::so_desta_maquina))
}
