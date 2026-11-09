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

use zylos_gateway::config::GatewayConfig;
use zylos_gateway::{AppState, build_router};
use zylos_live_comm::EventBus;
use zylos_live_comm::skills_api::SkillsApiState;
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

    // --- Task 7.1.1: Configuration via TOML + env vars ---
    let config_path = std::env::var("ZYLOS_CONFIG")
        .map(|p| std::path::PathBuf::from(p))
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
    info!("EventBus initialized (capacity={})", cfg.soar.event_bus_capacity);

    let registry = SkillRegistry::empty(&cfg.skills.index_path);
    let validator =
        SkillValidator::default_schema().expect("Failed to load default skill schema");
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
        let soar_state = OrchestratorState::new(
            soar_router,
            soar_registry,
            soar_audit,
            soar_executor_config,
        )
        .with_event_bus(event_bus.clone());
        let soar_orchestrator = Arc::new(SoarOrchestrator::new(Arc::new(RwLock::new(soar_state))));
        info!("SOAR orchestrator initialized with EventBus and auto-loaded runbooks/playbooks");

    // Swarm Orchestrator — task/crew scheduling (Phase 2, in-memory)
    let swarm = SwarmOrchestrator::open(":memory:")
        .await
        .map_err(|e| anyhow::anyhow!("swarm init: {}", e))?;
    info!("Swarm orchestrator initialized (in-memory)");

    // Readiness flag — set after all subsystems are up
    let ready = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let state = AppState {
        event_bus,
        skills_state,
        soar_orchestrator,
        swarm: Arc::new(swarm),
        ready: ready.clone(),
    };

    let app = build_router(state);

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

    axum::serve(listener, app)
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


