//! zylos-gateway — API Gateway Consolidada (Task 6.1.1 + Task 7.1.1)
//!
//! Binary entry point. All shared types and router construction live in lib.rs
//! so integration tests can import them via `use zylos_gateway::{AppState, build_router}`.

use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use zylos_agent_shield::{AgentShield, ShieldConfig};
use zylos_browser_takeover::TakeoverStore;
use zylos_evidence_ledger::EvidenceLedger;
use zylos_gateway::config::GatewayConfig;
use zylos_gateway::{build_router, ApiHandles, AppState};
use zylos_goals::GoalStore;
use zylos_live_comm::skills_api::SkillsApiState;
use zylos_live_comm::EventBus;
use zylos_memory_sync::MemoryStore;
use zylos_personalities::PersonalityRegistry;
use zylos_skills::{EvalFlywheel, SkillRegistry, SkillValidator};
use zylos_soar::{
    audit::AuditTrail,
    executor::ExecutorConfig,
    loader::{load_all_playbooks, load_all_runbook_specs},
    orchestrator::{OrchestratorState, SoarOrchestrator},
    registry::RunbookRegistry,
    router::EventRouter,
    yaml_runbook::YamlRunbook,
};
use zylos_swarm_orchestrator::SwarmOrchestrator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    // === Fase 11.4 T11.4.4: Sentry error tracking ===
    let _sentry_guard = std::env::var("SENTRY_DSN")
        .ok()
        .filter(|dsn| !dsn.is_empty())
        .map(|dsn| {
            sentry::init((
                dsn,
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    traces_sample_rate: 0.1,
                    ..Default::default()
                },
            ))
        });
    if _sentry_guard.is_some() {
        info!("Sentry initialized for error tracking");
    } else {
        info!("Sentry disabled (SENTRY_DSN not set)");
    }

    // --- Task 7.1.1: Configuration via TOML + env vars ---
    let config_path = std::env::var("ZYLOS_CONFIG")
        .map(std::path::PathBuf::from)
        .ok();
    let cfg = GatewayConfig::load(config_path.as_deref())?;

    info!(
        listen = %cfg.server.listen_addr,
        shutdown_timeout_secs = cfg.server.shutdown_timeout_secs,
        event_bus_capacity = cfg.soar.event_bus_capacity,
        "Gateway configuration loaded"
    );

    // --- Startup order: EventBus → Skills → SOAR → Router ---
    let event_bus = EventBus::new(cfg.soar.event_bus_capacity, None);
    info!(
        "EventBus initialized (capacity={})",
        cfg.soar.event_bus_capacity
    );

    let registry = SkillRegistry::empty(&cfg.skills.index_path);
    let validator = SkillValidator::default_schema().expect("Failed to load default skill schema");
    let eval_flywheel = EvalFlywheel::empty();

    let skills_state = SkillsApiState {
        registry: Arc::new(RwLock::new(registry)),
        validator: Arc::new(validator),
        eval_flywheel: Arc::new(RwLock::new(eval_flywheel)),
    };
    info!("Skills registry initialized");

    let mut soar_router = EventRouter::new();
    let mut soar_registry = RunbookRegistry::new();

    // --- Auto-load runbooks and playbooks from filesystem ---
    let runbook_specs = load_all_runbook_specs();
    info!("Discovered {} runbook specs on disk", runbook_specs.len());

    for spec in runbook_specs {
        let name = spec.metadata.name.clone();
        let yaml_runbook = Box::new(YamlRunbook::new(spec));
        soar_registry.register(yaml_runbook);
        info!("Registered YAML runbook: {}", name);
    }

    let playbooks = load_all_playbooks();
    info!("Discovered {} playbooks on disk", playbooks.len());

    for playbook in playbooks {
        let name = playbook.name.clone();
        soar_router.register_playbook(playbook);
        info!("Registered playbook: {}", name);
    }

    let soar_audit = AuditTrail::new(cfg.soar.audit_key.as_bytes());
    let soar_executor_config = ExecutorConfig::default();
    let soar_state =
        OrchestratorState::new(soar_router, soar_registry, soar_audit, soar_executor_config)
            .with_event_bus(event_bus.clone());
    let soar_orchestrator = Arc::new(SoarOrchestrator::new(Arc::new(RwLock::new(soar_state))));
    info!("SOAR orchestrator initialized with EventBus and auto-loaded runbooks/playbooks");

    // Swarm Orchestrator — task/crew scheduling (Phase 2, in-memory)
    let swarm = SwarmOrchestrator::open(":memory:")
        .await
        .map_err(|e| anyhow::anyhow!("swarm init: {}", e))?;
    info!("Swarm orchestrator initialized (in-memory)");

    // --- Initialize dedicated API stores (async, Send+Sync wrappers) ---
    let goals_store = GoalStore::open(std::path::Path::new(":memory:"))
        .map_err(|e| anyhow::anyhow!("goals init: {}", e))?;
    info!("Goals store initialized (in-memory)");

    // Notas por Space precisam sobreviver a reinício: arquivo próprio (`ZYLOS_MEMORY_DB`).
    let memory_path =
        std::env::var("ZYLOS_MEMORY_DB").unwrap_or_else(|_| "sqlite:data/memory.db".to_string());
    if let Some(dir) = std::path::Path::new(memory_path.trim_start_matches("sqlite:")).parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    let memory_store = MemoryStore::open(&memory_path)
        .await
        .map_err(|e| anyhow::anyhow!("memory init: {}", e))?;
    info!("Memory store initialized ({})", memory_path);

    let personalities_registry = PersonalityRegistry::open(":memory:")
        .await
        .map_err(|e| anyhow::anyhow!("personalities init: {}", e))?;
    info!("Personalities registry initialized (in-memory)");

    let shield = AgentShield::new(ShieldConfig::default(), Some(":memory:"))
        .await
        .map_err(|e| anyhow::anyhow!("shield init: {}", e))?;
    let takeover_store =
        TakeoverStore::in_memory().map_err(|e| anyhow::anyhow!("takeover store init: {}", e))?;
    info!("Sentinel subsystems initialized (in-memory)");

    let evidence_ledger = EvidenceLedger::open(":memory:")
        .await
        .map_err(|e| anyhow::anyhow!("evidence ledger init: {}", e))?;
    info!("Evidence ledger initialized (in-memory)");

    let api_handles = ApiHandles {
        goals: Arc::new(zylos_gateway::goals_api::GoalsHandleInner { store: goals_store }),
        memory: Arc::new(zylos_gateway::memory_api::MemoryHandleInner {
            store: memory_store,
        }),
        personalities: Arc::new(zylos_gateway::personalities_api::PersonalitiesHandleInner {
            registry: personalities_registry,
        }),
        sentinel: Arc::new(zylos_gateway::sentinel_api::SentinelHandleInner {
            shield,
            takeover: takeover_store,
        }),
        evidence: Arc::new(zylos_gateway::evidence_api::EvidenceHandleInner {
            ledger: evidence_ledger,
        }),
    };

    // Readiness flag — set after all subsystems are up
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let state = AppState {
        event_bus,
        skills_state,
        soar_orchestrator,
        swarm: Arc::new(swarm),
        ready: ready.clone(),
    };

    let app = build_router(state, api_handles);

    // --- Graceful shutdown via CancellationToken ---
    let shutdown_token = CancellationToken::new();
    let shutdown_clone = shutdown_token.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        info!("Shutdown signal received, initiating graceful shutdown");
        shutdown_clone.cancel();
    });

    let listener = TcpListener::bind(&cfg.server.listen_addr).await?;
    info!("zylos-gateway listening on {}", cfg.server.listen_addr);

    // Mark as ready after bind succeeds
    ready.store(true, std::sync::atomic::Ordering::Relaxed);
    info!("Gateway marked as READY");

    // ConnectInfo: o rate limiter (F15) precisa do IP real do cliente.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_token.cancelled().await;
        info!("Graceful shutdown initiated, draining requests...");
        tokio::time::sleep(std::time::Duration::from_secs(
            cfg.server.shutdown_timeout_secs,
        ))
        .await;
        info!("Shutdown timeout reached, forcing stop");
    })
    .await?;

    info!("zylos-gateway stopped cleanly");
    Ok(())
}
