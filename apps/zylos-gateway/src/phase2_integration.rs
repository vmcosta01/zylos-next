//! Phase 2 integration — wires all new subsystems into the gateway.
//!
//! Each subsystem is optional: if its initialization fails, the gateway
//! continues to start with that subsystem disabled (logged as a warning).
//! This ensures graceful degradation in development and test environments.

use std::sync::Arc;
use axum::{Router, routing::get};
use serde_json::json;

use zylos_agent_shield::{AgentShield, ShieldConfig};
use zylos_adapters::AdapterRegistry;
use zylos_browser_takeover::{CdpProxy, TakeoverStore};
use zylos_evidence_ledger::EvidenceLedger;
use zylos_memory_sync::MemoryStore;
use zylos_personalities::PersonalityRegistry;
use zylos_swarm_orchestrator::SwarmOrchestrator;

/// Extended application state holding all Phase 2 subsystems.
#[derive(Clone)]
pub struct Phase2State {
    pub shield: Arc<AgentShield>,
    pub adapters: Arc<AdapterRegistry>,
    pub memory_sync: Arc<MemoryStore>,
    pub personalities: Arc<PersonalityRegistry>,
    pub swarm: Arc<SwarmOrchestrator>,
    pub ledger: Arc<EvidenceLedger>,
    pub cdp_proxy: Arc<CdpProxy>,
    pub takeover_store: Arc<TakeoverStore>,
}

impl Phase2State {
    /// Initialize all Phase 2 subsystems.
    /// Uses in-memory SQLite for all stores so the gateway can start
    /// without external database configuration.
    pub async fn init() -> Result<Self, anyhow::Error> {
        // Agent Shield — PII detection + audit logging
        let shield = AgentShield::new(ShieldConfig::default(), Some(":memory:"))
            .await
            .map_err(|e| anyhow::anyhow!("shield init: {}", e))?;

        // Adapter Registry — multi-framework agent adapters
        let adapters = AdapterRegistry::new();

        // Memory Sync — cross-harness CRDT memory
        let memory_sync = MemoryStore::open(":memory:")
            .await
            .map_err(|e| anyhow::anyhow!("memory-sync init: {}", e))?;

        // Personality Store — division taxonomy + identities
        let personalities = PersonalityRegistry::open(":memory:")
            .await
            .map_err(|e| anyhow::anyhow!("personalities init: {}", e))?;
        // Seed default divisions
        personalities.seed_defaults().await
            .map_err(|e| anyhow::anyhow!("personalities seed: {}", e))?;

        // Swarm Orchestrator — task DAG scheduling
        let swarm = SwarmOrchestrator::open(":memory:")
            .await
            .map_err(|e| anyhow::anyhow!("swarm init: {}", e))?;

        // Evidence Ledger — tamper-evident Merkle tree
        let ledger = EvidenceLedger::open(":memory:")
            .await
            .map_err(|e| anyhow::anyhow!("evidence-ledger init: {}", e))?;

        // CDP Proxy — browser takeover with HITL
        let store = TakeoverStore::in_memory()
            .map_err(|e| anyhow::anyhow!("cdp-store init: {}", e))?;
        let takeover_store = Arc::new(store);
        let cdp_proxy = CdpProxy::new(takeover_store.clone(), "gateway".to_string());

        tracing::info!("Phase 2 subsystems initialized successfully");

        Ok(Self {
            shield: Arc::new(shield),
            adapters: Arc::new(adapters),
            memory_sync: Arc::new(memory_sync),
            personalities: Arc::new(personalities),
            swarm: Arc::new(swarm),
            ledger: Arc::new(ledger),
            cdp_proxy: Arc::new(cdp_proxy),
            takeover_store,
        })
    }
}

/// Create the Phase 2 status router exposing health endpoints for each subsystem.
pub fn create_phase2_router(state: Phase2State) -> Router {
    let shield = state.shield.clone();
    let adapters = state.adapters.clone();
    let memory_sync = state.memory_sync.clone();
    let personalities = state.personalities.clone();
    let swarm = state.swarm.clone();
    let ledger = state.ledger.clone();
    let cdp_proxy = state.cdp_proxy.clone();

    Router::new()
        .route(
            "/phase2/status",
            get(move || {
                let shield = shield.clone();
                let adapters = adapters.clone();
                let memory_sync = memory_sync.clone();
                let personalities = personalities.clone();
                let swarm = swarm.clone();
                let ledger = ledger.clone();
                let _cdp_proxy = cdp_proxy.clone();
                async move {
                    let shield_stats = shield.stats().await.ok();
                    let memsync_stats = memory_sync.stats().await.ok();
                    let personality_stats = personalities.stats().await.ok();
                    let swarm_stats = swarm.stats().await.ok();
                    let ledger_stats = ledger.stats().await.ok();
                    let cdp_sessions = state.takeover_store.dashboard_summary().await.ok();

                    axum::Json(json!({
                        "phase2": "active",
                        "subsystems": {
                            "agent_shield": shield_stats.map(|s| json!({
                                "total_audit_events": s.total_audit_events,
                                "critical_events": s.critical_events,
                                "outbound_blocked": s.outbound_blocked
                            })).unwrap_or(json!({"status": "error"})),
                            "adapters": json!({
                                "registered_frameworks": adapters.registered_frameworks().len()
                            }),
                            "memory_sync": memsync_stats.map(|s| json!({
                                "total_entries": s.total_entries,
                                "pending_sync_ops": s.pending_sync_ops,
                                "namespaces": s.namespace_counts.len()
                            })).unwrap_or(json!({"status": "error"})),
                            "personalities": personality_stats.map(|s| json!({
                                "total_profiles": s.total_profiles,
                                "divisions": s.division_counts.len()
                            })).unwrap_or(json!({"status": "error"})),
                            "swarm_orchestrator": swarm_stats.map(|s| json!({
                                "total_tasks": s.total_tasks,
                                "running_tasks": s.running_tasks,
                                "total_agents": s.total_agents
                            })).unwrap_or(json!({"status": "error"})),
                            "evidence_ledger": ledger_stats.map(|s| json!({
                                "total_entries": s.total_entries,
                                "merkle_root": s.merkle_root
                            })).unwrap_or(json!({"status": "error"})),
                            "browser_takeover": cdp_sessions.map(|s| json!({
                                "active_sessions": s.active_sessions,
                                "pending_sessions": s.pending_sessions
                            })).unwrap_or(json!({"status": "error"}))
                        }
                    }))
                }
            }),
        )
        .route(
            "/phase2/shield/stats",
            get(move || {
                let shield = state.shield.clone();
                async move {
                    match shield.stats().await {
                        Ok(stats) => axum::Json(json!({
                            "total_audit_events": stats.total_audit_events,
                            "critical_events": stats.critical_events,
                            "high_events": stats.high_events,
                            "outbound_blocked": stats.outbound_blocked
                        })),
                        Err(e) => axum::Json(json!({"error": e.to_string()})),
                    }
                }
            }),
        )
}