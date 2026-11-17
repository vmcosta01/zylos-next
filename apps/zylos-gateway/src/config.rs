//! Gateway configuration — TOML file + env vars + CLI flags.
//!
//! Priority: CLI > env > TOML file > defaults.
//! Supports hot-reload via SIGHUP / file watcher (Task 7.1.1).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

/// Top-level gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Network settings
    #[serde(default)]
    pub server: ServerConfig,

    /// Health check settings
    #[serde(default)]
    pub health: HealthConfig,

    /// SOAR orchestrator settings
    #[serde(default)]
    pub soar: SoarConfig,

    /// Skills registry settings
    #[serde(default)]
    pub skills: SkillsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Listen address (e.g. "127.0.0.1:3000"; use ZYLOS_LISTEN=0.0.0.0:3000 para expor)
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    /// Graceful shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout_secs")]
    pub shutdown_timeout_secs: u64,

    /// Max concurrent connections
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Enable readiness probe
    #[serde(default = "default_true")]
    pub ready_enabled: bool,

    /// Enable liveness probe
    #[serde(default = "default_true")]
    pub live_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoarConfig {
    /// Audit trail key (hex or base64)
    #[serde(default = "default_audit_key")]
    pub audit_key: String,

    /// EventBus channel capacity
    #[serde(default = "default_event_bus_capacity")]
    pub event_bus_capacity: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    /// Path to skills index.json directory
    #[serde(default = "default_skills_path")]
    pub index_path: PathBuf,
}

// --- Defaults ---

fn default_listen_addr() -> String {
    "127.0.0.1:3000".to_string()
}
fn default_shutdown_timeout_secs() -> u64 {
    5
}
fn default_max_connections() -> usize {
    1024
}
fn default_true() -> bool {
    true
}
fn default_audit_key() -> String {
    "zylos-gateway-soar-key".to_string()
}
fn default_event_bus_capacity() -> usize {
    256
}
fn default_skills_path() -> PathBuf {
    PathBuf::from(".")
}

#[allow(clippy::derivable_impls)]
impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            health: HealthConfig::default(),
            soar: SoarConfig::default(),
            skills: SkillsConfig::default(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            shutdown_timeout_secs: default_shutdown_timeout_secs(),
            max_connections: default_max_connections(),
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            ready_enabled: true,
            live_enabled: true,
        }
    }
}

impl Default for SoarConfig {
    fn default() -> Self {
        Self {
            audit_key: default_audit_key(),
            event_bus_capacity: default_event_bus_capacity(),
        }
    }
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            index_path: default_skills_path(),
        }
    }
}

impl GatewayConfig {
    /// Load config with priority: env overrides > TOML file > defaults.
    pub fn load(toml_path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        let mut config = if let Some(path) = toml_path {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                let parsed: GatewayConfig = toml::from_str(&content)?;
                info!("Loaded gateway config from {}", path.display());
                parsed
            } else {
                info!("Config file {} not found, using defaults", path.display());
                GatewayConfig::default()
            }
        } else {
            GatewayConfig::default()
        };

        // Env overrides
        if let Ok(addr) = std::env::var("ZYLOS_LISTEN") {
            config.server.listen_addr = addr;
        }
        if let Ok(v) = std::env::var("ZYLOS_SHUTDOWN_TIMEOUT_SECS") {
            if let Ok(n) = v.parse() {
                config.server.shutdown_timeout_secs = n;
            }
        }
        if let Ok(v) = std::env::var("ZYLOS_EVENT_BUS_CAPACITY") {
            if let Ok(n) = v.parse() {
                config.soar.event_bus_capacity = n;
            }
        }
        if let Ok(v) = std::env::var("ZYLOS_SKILLS_PATH") {
            config.skills.index_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("ZYLOS_SOAR_AUDIT_KEY") {
            config.soar.audit_key = v;
        }

        Ok(config)
    }

    /// Reload config from the same source (hot-reload support).
    pub fn reload(&self, toml_path: Option<&std::path::Path>) -> anyhow::Result<Self> {
        info!("Hot-reloading gateway configuration");
        Self::load(toml_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize tests that mutate process-wide env vars to prevent cross-test races
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config() {
        let config = GatewayConfig::default();
        assert_eq!(config.server.listen_addr, "127.0.0.1:3000");
        assert_eq!(config.server.shutdown_timeout_secs, 5);
        assert!(config.health.ready_enabled);
        assert!(config.health.live_enabled);
        assert_eq!(config.soar.event_bus_capacity, 256);
    }

    #[test]
    fn test_load_missing_file_uses_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("ZYLOS_LISTEN");
        let config =
            GatewayConfig::load(Some(std::path::Path::new("/nonexistent/config.toml"))).unwrap();
        assert_eq!(config.server.listen_addr, "127.0.0.1:3000");
    }

    #[test]
    fn test_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("ZYLOS_LISTEN", "127.0.0.1:9999");
        let config = GatewayConfig::load(None).unwrap();
        assert_eq!(config.server.listen_addr, "127.0.0.1:9999");
        std::env::remove_var("ZYLOS_LISTEN");
    }
}
