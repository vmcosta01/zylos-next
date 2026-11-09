//! # zylos-desktop
//!
//! Tauri v2 desktop shell for zylos-gateway. Provides:
//! - Native window management with embedded webview (cockpit UI)
//! - System tray with status indicator and quick actions
//! - Deep link handling (`zylos://` protocol)
//! - Auto-update integration
//! - IPC bridge between frontend and gateway backend
//!
//! The `gui` feature enables Tauri dependencies. Without it, this crate
//! exposes only the configuration types and IPC protocol definitions,
//! allowing headless compilation in CI.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Backend Modules
// ---------------------------------------------------------------------------

pub mod gateway_embed;
pub mod notifications;
pub mod tray;
pub mod workspace;

// ---------------------------------------------------------------------------
// Desktop Configuration
// ---------------------------------------------------------------------------

/// Desktop shell configuration loaded from `desktop-config.json` or defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesktopConfig {
    /// Window title
    pub title: String,
    /// Initial window width
    pub width: u32,
    /// Initial window height
    pub height: u32,
    /// Whether to start minimized to tray
    pub start_minimized: bool,
    /// Gateway backend URL (embedded or remote)
    pub gateway_url: String,
    /// Enable auto-update checks
    pub auto_update: bool,
    /// Deep link protocol scheme
    pub deep_link_scheme: String,
    /// Path to cockpit web assets (relative to app data dir)
    pub cockpit_assets_dir: Option<String>,
    /// Tray icon path (relative to app data dir)
    pub tray_icon_path: Option<String>,
}

impl Default for DesktopConfig {
    fn default() -> Self {
        Self {
            title: "Zylos Gateway".to_string(),
            width: 1280,
            height: 860,
            start_minimized: false,
            gateway_url: "http://127.0.0.1:9876".to_string(),
            auto_update: true,
            deep_link_scheme: "zylos".to_string(),
            cockpit_assets_dir: None,
            tray_icon_path: None,
        }
    }
}

impl DesktopConfig {
    /// Load config from a JSON file, falling back to defaults
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
        serde_json::from_str(&content).map_err(ConfigError::Parse)
    }

    /// Save config to a JSON file
    pub fn save(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let json = serde_json::to_string_pretty(self).map_err(ConfigError::Serialize)?;
        std::fs::write(path, json).map_err(ConfigError::Io)
    }

    /// Resolve the cockpit assets directory relative to app data
    pub fn resolve_cockpit_dir(&self) -> PathBuf {
        if let Some(ref dir) = self.cockpit_assets_dir {
            PathBuf::from(dir)
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("zylos")
                .join("cockpit")
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Filesystem I/O error during config load/save operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON parsing error when reading config file.
    #[error("JSON parse error: {0}")]
    Parse(#[from] serde_json::Error),
    /// JSON serialization error when writing config file.
    #[error("Serialization error: {0}")]
    Serialize(serde_json::Error),
}

// ---------------------------------------------------------------------------
// IPC Protocol Types
// ---------------------------------------------------------------------------

/// Messages from frontend → backend via Tauri IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcRequest {
    /// Get current gateway health/status.
    ///
    /// Backend responds with [`IpcResponse::Status`] containing health,
    /// version, uptime, and active agent count.
    #[serde(rename = "get_status")]
    GetStatus,
    /// Send a prompt to the orchestrator.
    ///
    /// Backend responds with [`IpcResponse::PromptAck`] containing the
    /// run ID for tracking the agent execution.
    #[serde(rename = "send_prompt")]
    SendPrompt { message: String, thread_id: Option<String> },
    /// List available skills from the consolidated registry.
    ///
    /// Backend responds with [`IpcResponse::Skills`] containing summaries
    /// of all registered skills across agents.
    #[serde(rename = "list_skills")]
    ListSkills,
    /// List SOAR playbooks available for execution.
    ///
    /// Backend responds with [`IpcResponse::Playbooks`] containing
    /// playbook summaries with step counts.
    #[serde(rename = "list_playbooks")]
    ListPlaybooks,
    /// Execute an integration action (e.g., send email, create issue).
    ///
    /// Backend responds with [`IpcResponse::ActionResult`] containing
    /// success status, result payload, or error message.
    #[serde(rename = "execute_action")]
    ExecuteAction {
        provider: String,
        action: String,
        params: serde_json::Value,
    },
    /// Open external URL in system default browser.
    ///
    /// No response is sent; the frontend should handle UI updates locally.
    #[serde(rename = "open_external")]
    OpenExternal { url: String },
    /// Minimize the main window to the system tray.
    ///
    /// No response is sent; window visibility is managed by the backend.
    #[serde(rename = "minimize_to_tray")]
    MinimizeToTray,
    /// Check for application updates from the configured update server.
    ///
    /// Backend responds with [`IpcResponse::UpdateAvailable`] if a newer
    /// version exists, or [`IpcResponse::NoUpdate`] otherwise.
    #[serde(rename = "check_update")]
    CheckUpdate,
}

/// Responses from backend → frontend via Tauri IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResponse {
    /// Response to [`IpcRequest::GetStatus`] with gateway health metrics.
    #[serde(rename = "status")]
    Status {
        /// Whether the gateway backend is healthy and responsive.
        healthy: bool,
        /// Semantic version string of the gateway backend.
        version: String,
        /// Gateway uptime in seconds since last start.
        uptime_secs: u64,
        /// Number of currently active agent instances.
        active_agents: usize,
    },
    /// Acknowledgment for [`IpcRequest::SendPrompt`] with the assigned run ID.
    #[serde(rename = "prompt_ack")]
    PromptAck { run_id: String },
    /// Response to [`IpcRequest::ListSkills`] with skill summaries.
    #[serde(rename = "skills")]
    Skills { skills: Vec<SkillSummary> },
    /// Response to [`IpcRequest::ListPlaybooks`] with playbook summaries.
    #[serde(rename = "playbooks")]
    Playbooks { playbooks: Vec<PlaybookSummary> },
    /// Response to [`IpcRequest::ExecuteAction`] with execution result.
    #[serde(rename = "action_result")]
    ActionResult {
        /// Whether the action completed successfully.
        success: bool,
        /// Result payload on success; `None` on failure.
        result: Option<serde_json::Value>,
        /// Error message on failure; `None` on success.
        error: Option<String>,
    },
    /// Response to [`IpcRequest::CheckUpdate`] when a newer version is available.
    #[serde(rename = "update_available")]
    UpdateAvailable { version: String, changelog: String },
    /// Response to [`IpcRequest::CheckUpdate`] when already on latest version.
    #[serde(rename = "no_update")]
    NoUpdate,
    /// Generic error response for failed IPC operations.
    #[serde(rename = "error")]
    Error { code: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    /// Unique skill identifier (e.g., `"web_search"`).
    pub name: String,
    /// Human-readable description of what the skill does.
    pub description: String,
    /// Optional category for grouping skills in UI (e.g., `"research"`, `"coding"`).
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookSummary {
    /// Unique playbook identifier.
    pub id: String,
    /// Human-readable playbook name.
    pub name: String,
    /// Description of the playbook's purpose and behavior.
    pub description: String,
    /// Number of discrete steps in the playbook execution sequence.
    pub steps: usize,
}

// ---------------------------------------------------------------------------
// Deep Link Handling
// ---------------------------------------------------------------------------

/// Parsed deep link from `zylos://` protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum DeepLink {
    /// Open a specific thread/conversation in the cockpit UI.
    ///
    /// URL format: `zylos://open_thread?thread_id=<uuid>`
    #[serde(rename = "open_thread")]
    OpenThread { thread_id: String },
    /// Trigger execution of a registered skill with optional parameters.
    ///
    /// URL format: `zylos://run_skill?skill=<name>&params=<json>`
    #[serde(rename = "run_skill")]
    RunSkill { skill: String, params: Option<serde_json::Value> },
    /// Open the settings/preferences page, optionally at a specific section.
    ///
    /// URL format: `zylos://settings?page=<section>`
    #[serde(rename = "settings")]
    Settings { page: Option<String> },
    /// OAuth callback redirect after external provider authentication.
    ///
    /// URL format: `zylos://oauth_callback?provider=<p>&code=<c>&state=<s>`
    #[serde(rename = "oauth_callback")]
    OAuthCallback {
        provider: String,
        code: String,
        state: String,
    },
}

impl DeepLink {
    /// Parse a deep link URL string
    pub fn parse(url: &str) -> Result<Self, DeepLinkError> {
        // Expected format: zylos://action?key=value&key2=value2
        let stripped = url
            .strip_prefix("zylos://")
            .ok_or(DeepLinkError::InvalidScheme)?;

        let (action, query) = match stripped.split_once('?') {
            Some((a, q)) => (a, q),
            None => (stripped, ""),
        };

        let params: std::collections::HashMap<String, String> =
            serde_urlencoded::from_str(query).unwrap_or_default();

        match action {
            "open_thread" => {
                let thread_id = params
                    .get("thread_id")
                    .cloned()
                    .ok_or(DeepLinkError::MissingParam("thread_id"))?;
                Ok(DeepLink::OpenThread { thread_id })
            }
            "run_skill" => {
                let skill = params
                    .get("skill")
                    .cloned()
                    .ok_or(DeepLinkError::MissingParam("skill"))?;
                let p = params.get("params").and_then(|s| serde_json::from_str(s).ok());
                Ok(DeepLink::RunSkill { skill, params: p })
            }
            "settings" => Ok(DeepLink::Settings {
                page: params.get("page").cloned(),
            }),
            "oauth_callback" => {
                let provider = params
                    .get("provider")
                    .cloned()
                    .ok_or(DeepLinkError::MissingParam("provider"))?;
                let code = params
                    .get("code")
                    .cloned()
                    .ok_or(DeepLinkError::MissingParam("code"))?;
                let state = params
                    .get("state")
                    .cloned()
                    .ok_or(DeepLinkError::MissingParam("state"))?;
                Ok(DeepLink::OAuthCallback {
                    provider,
                    code,
                    state,
                })
            }
            _ => Err(DeepLinkError::UnknownAction(action.to_string())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeepLinkError {
    /// URL scheme is not `zylos://`.
    #[error("Invalid scheme: expected zylos://")]
    InvalidScheme,
    /// Required query parameter is missing from the URL.
    #[error("Missing required parameter: {0}")]
    MissingParam(&'static str),
    /// Deep link action is not recognized by the parser.
    #[error("Unknown deep link action: {0}")]
    UnknownAction(String),
}

// ---------------------------------------------------------------------------
// Tray Actions
// ---------------------------------------------------------------------------

/// System tray menu actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrayAction {
    /// Show the main application window.
    ShowWindow,
    /// Hide the main application window (minimize to tray).
    HideWindow,
    /// Open the cockpit dashboard view.
    OpenDashboard,
    /// Restart the embedded gateway backend process.
    RestartGateway,
    /// Check for application updates.
    CheckUpdates,
    /// Quit the application completely.
    Quit,
}

// ---------------------------------------------------------------------------
// GUI Module (feature-gated)
// ---------------------------------------------------------------------------

#[cfg(feature = "gui")]
pub mod gui {
    use super::*;

    /// Initialize the Tauri application with all plugins
    pub fn build_app(config: DesktopConfig) -> Result<tauri::AppHandle, tauri::Error> {
        let builder = tauri::Builder::default();

        // TODO: Register plugins when tauri-plugin-* crates are added
        // .plugin(tauri_plugin_shell::init())
        // .plugin(tauri_plugin_deep_link::init())
        // .plugin(tauri_plugin_updater::Builder::new().build())

        let _handle = builder.build(tauri::generate_context!())?;
        Ok(_handle.handle().clone())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = DesktopConfig::default();
        assert_eq!(cfg.title, "Zylos Gateway");
        assert_eq!(cfg.width, 1280);
        assert_eq!(cfg.gateway_url, "http://127.0.0.1:9876");
        assert!(cfg.auto_update);
    }

    #[test]
    fn test_deep_link_open_thread() {
        let link = DeepLink::parse("zylos://open_thread?thread_id=abc-123").unwrap();
        match link {
            DeepLink::OpenThread { thread_id } => assert_eq!(thread_id, "abc-123"),
            _ => panic!("expected OpenThread"),
        }
    }

    #[test]
    fn test_deep_link_oauth_callback() {
        let link = DeepLink::parse(
            "zylos://oauth_callback?provider=gmail&code=4/xyz&state=pkce-verifier",
        )
        .unwrap();
        match link {
            DeepLink::OAuthCallback {
                provider,
                code,
                state,
            } => {
                assert_eq!(provider, "gmail");
                assert_eq!(code, "4/xyz");
                assert_eq!(state, "pkce-verifier");
            }
            _ => panic!("expected OAuthCallback"),
        }
    }

    #[test]
    fn test_deep_link_invalid_scheme() {
        let result = DeepLink::parse("https://example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_deep_link_unknown_action() {
        let result = DeepLink::parse("zylos://unknown_action");
        assert!(matches!(result, Err(DeepLinkError::UnknownAction(_))));
    }

    #[test]
    fn test_ipc_request_serialization() {
        let req = IpcRequest::SendPrompt {
            message: "hello".into(),
            thread_id: Some("t1".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("send_prompt"));
        assert!(json.contains("hello"));

        let parsed: IpcRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            IpcRequest::SendPrompt { message, thread_id } => {
                assert_eq!(message, "hello");
                assert_eq!(thread_id.as_deref(), Some("t1"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_config_roundtrip() {
        let cfg = DesktopConfig {
            title: "Test".into(),
            width: 800,
            height: 600,
            ..Default::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let parsed: DesktopConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.title, "Test");
        assert_eq!(parsed.width, 800);
    }
}