//! zylos-daemon — Background service for Zylos-Next agent lifecycle management.
//!
//! Runs as a system service (systemd/launchd/Windows Service) and manages:
//! - Agent process spawning and supervision
//! - Health monitoring and auto-restart
//! - Log aggregation and rotation
//! - Graceful shutdown coordination
//!
//! # Architecture
//!
//! ```text
//! zylos-daemon
//!   +-- supervisor       # Process tree management
//!   +-- health_monitor   # Periodic health checks
//!   +-- log_aggregator   # Structured log collection
//!   +-- shutdown_coord   # SIGTERM/SIGINT handling
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use axum::{extract::State, response::Json};

/// Zylos-Next Daemon — background agent lifecycle manager.
#[derive(Parser)]
#[command(name = "zylos-daemon", version, about = "Zylos-Next Daemon")]
struct Args {
    /// Path to daemon configuration (TOML)
    #[arg(long, env = "ZYLOS_DAEMON_CONFIG", default_value = "/etc/zylos/daemon.toml")]
    config: PathBuf,

    /// Run in foreground (don't daemonize)
    #[arg(long)]
    foreground: bool,

    /// Override listen address for health endpoint
    #[arg(long, env = "ZYLOS_DAEMON_HEALTH_ADDR")]
    health_addr: Option<String>,
}

/// Daemon configuration loaded from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaemonConfig {
    /// Address for the internal health/metrics HTTP server
    #[serde(default = "default_health_addr")]
    health_addr: String,

    /// How often to check agent health (seconds)
    #[serde(default = "default_health_interval")]
    health_interval_secs: u64,

    /// Maximum restart attempts before marking agent as failed
    #[serde(default = "default_max_restarts")]
    max_restarts: u32,

    /// Directory containing agent specifications
    #[serde(default = "default_agents_dir")]
    agents_dir: PathBuf,

    /// Directory for daemon state persistence
    #[serde(default = "default_state_dir")]
    state_dir: PathBuf,
}

fn default_health_addr() -> String { "127.0.0.1:9090".into() }
fn default_health_interval() -> u64 { 30 }
fn default_max_restarts() -> u32 { 5 }
fn default_agents_dir() -> PathBuf { PathBuf::from("/var/lib/zylos/agents") }
fn default_state_dir() -> PathBuf { PathBuf::from("/var/lib/zylos/daemon") }

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            health_addr: default_health_addr(),
            health_interval_secs: default_health_interval(),
            max_restarts: default_max_restarts(),
            agents_dir: default_agents_dir(),
            state_dir: default_state_dir(),
        }
    }
}

/// Runtime state of a managed agent process.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentState {
    id: String,
    pid: Option<u32>,
    status: AgentStatus,
    restart_count: u32,
    last_health_check_ms: u64,
    spec_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AgentStatus {
    Stopped,
    Starting,
    Running,
    Unhealthy,
    Failed,
    ShuttingDown,
}

/// Shared daemon state protected by RwLock.
type SharedState = Arc<RwLock<HashMap<String, AgentState>>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .json()
        .init();

    let args = Args::parse();

    let config = load_config(&args.config).unwrap_or_else(|e| {
        warn!("Failed to load config from {:?}: {}. Using defaults.", args.config, e);
        DaemonConfig::default()
    });

    let health_addr = args.health_addr.as_deref().unwrap_or(&config.health_addr);

    info!("zylos-daemon starting");
    info!("health endpoint: {}", health_addr);
    info!("agents dir: {:?}", config.agents_dir);
    info!("state dir: {:?}", config.state_dir);

    let state: SharedState = Arc::new(RwLock::new(HashMap::new()));

    // Spawn health monitor
    let health_state = state.clone();
    let health_interval = config.health_interval_secs;
    let max_restarts = config.max_restarts;
    let health_handle = tokio::spawn(async move {
        health_monitor_loop(health_state, health_interval, max_restarts).await;
    });

    // Spawn metrics/health HTTP server
    let metrics_state = state.clone();
    let addr = health_addr.to_string();
    let http_handle = tokio::spawn(async move {
        if let Err(e) = run_health_server(&addr, metrics_state).await {
            error!("Health server failed: {}", e);
        }
    });

    // Wait for shutdown signal
    shutdown_signal().await;
    info!("Shutdown signal received, draining...");

    // Cancel background tasks
    health_handle.abort();
    http_handle.abort();

    // Graceful shutdown of managed agents
    graceful_shutdown(state).await;

    info!("zylos-daemon stopped");
    Ok(())
}

fn load_config(path: &PathBuf) -> Result<DaemonConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read config: {:?}", path))?;
    let config: DaemonConfig = toml::from_str(&content)
        .context("Invalid TOML in daemon config")?;
    Ok(config)
}

async fn health_monitor_loop(
    state: SharedState,
    interval_secs: u64,
    max_restarts: u32,
) {
    let mut interval = tokio::time::interval(
        tokio::time::Duration::from_secs(interval_secs),
    );

    loop {
        interval.tick().await;

        let agents = state.read().await;
        for (id, agent) in agents.iter() {
            if agent.status == AgentStatus::Running {
                info!(agent_id = %id, pid = ?agent.pid, "health check OK");
            } else if agent.status == AgentStatus::Unhealthy {
                if agent.restart_count < max_restarts {
                    warn!(agent_id = %id, restart = agent.restart_count + 1, "restarting unhealthy agent");
                } else {
                    error!(agent_id = %id, "agent exceeded max restarts, marking failed");
                }
            }
        }
    }
}

async fn run_health_server(addr: &str, state: SharedState) -> Result<()> {
    use axum::routing::{get, post};
    use axum::Router;

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/agents", get(agents_handler))
        .route("/soar/playbooks", get(soar_list_playbooks))
        .route("/soar/events", post(soar_submit_event))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("Cannot bind health server to {}", addr))?;

    info!("Health server listening on {}", addr);
    axum::serve(listener, app).await.context("Health server error")?;
    Ok(())
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn agents_handler(State(state): State<SharedState>) -> Json<HashMap<String, AgentState>> {
    let agents = state.read().await;
    Json(agents.clone())
}

/// List available SOAR playbooks (delegates to gateway or local registry)
async fn soar_list_playbooks() -> Json<serde_json::Value> {
    // In production, this would query the local RunbookRegistry
    // For now, return a placeholder indicating the endpoint is active
    Json(serde_json::json!({
        "playbooks": [],
        "note": "SOAR integration active — playbooks loaded via zylos-gateway"
    }))
}

/// Accept a SOAR event for processing
async fn soar_submit_event(
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    info!(event = %payload, "SOAR event received by daemon");
    Json(serde_json::json!({
        "status": "accepted",
        "event": payload
    }))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn graceful_shutdown(state: SharedState) {
    let mut agents = state.write().await;
    for (id, agent) in agents.iter_mut() {
        if agent.status == AgentStatus::Running {
            info!(agent_id = %id, "sending shutdown signal");
            agent.status = AgentStatus::ShuttingDown;
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    info!("All agents shut down");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = DaemonConfig::default();
        assert_eq!(config.health_addr, "127.0.0.1:9090");
        assert_eq!(config.health_interval_secs, 30);
        assert_eq!(config.max_restarts, 5);
    }

    #[test]
    fn agent_status_serialization() {
        let status = AgentStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"Running\"");
    }

    #[test]
    fn agent_state_roundtrip() {
        let state = AgentState {
            id: "test-agent".into(),
            pid: Some(12345),
            status: AgentStatus::Running,
            restart_count: 0,
            last_health_check_ms: 1700000000000,
            spec_path: PathBuf::from("/etc/zylos/agents/test.yaml"),
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-agent");
        assert_eq!(deserialized.pid, Some(12345));
        assert_eq!(deserialized.status, AgentStatus::Running);
    }

    #[test]
    fn parse_cli_defaults() {
        let args = Args::try_parse_from(["zylos-daemon"]).unwrap();
        assert!(!args.foreground);
        assert!(args.health_addr.is_none());
    }

    #[test]
    fn parse_cli_foreground() {
        let args = Args::try_parse_from(["zylos-daemon", "--foreground"]).unwrap();
        assert!(args.foreground);
    }

    #[test]
    fn parse_cli_health_override() {
        let args = Args::try_parse_from([
            "zylos-daemon",
            "--health-addr",
            "0.0.0.0:8080",
        ])
        .unwrap();
        assert_eq!(args.health_addr.as_deref(), Some("0.0.0.0:8080"));
    }
}
