//! Per-workspace VNC/screen share module.
//!
//! Provides on-demand screen sharing for sandbox workspaces using TigerVNC
//! server spawning and noVNC web client embedding. Sessions auto-disconnect
//! after idle timeout and VNC passwords are never exposed to agents or logs.

use std::collections::HashMap;

use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ScreenShareError {
    #[error("session not found: {0}")]
    SessionNotFound(String),

    #[error("session already active for workspace: {0}")]
    AlreadyActive(String),

    #[error("VNC server failed to start: {0}")]
    ServerStartFailed(String),

    #[error("idle timeout exceeded ({0}s)")]
    IdleTimeout(u64),

    #[error("invalid token")]
    InvalidToken,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported sandbox type: {0}")]
    UnsupportedSandbox(String),
}

pub type Result<T> = std::result::Result<T, ScreenShareError>;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Sandbox type that determines how screen share is set up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SandboxType {
    Docker,
    Wasm,
}

/// Current state of a screen share session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionState {
    Starting,
    Active,
    Idle,
    Stopped,
}

/// Configuration for screen share sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenShareConfig {
    /// Idle timeout before auto-disconnect (default: 30 minutes).
    pub idle_timeout_secs: u64,
    /// Base port for VNC servers (each session gets base + offset).
    pub vnc_port_base: u16,
    /// Display resolution width.
    pub display_width: u32,
    /// Display resolution height.
    pub display_height: u32,
    /// Color depth (bits per pixel).
    pub color_depth: u8,
}

impl Default for ScreenShareConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 1800, // 30 minutes
            vnc_port_base: 5900,
            display_width: 1280,
            display_height: 720,
            color_depth: 24,
        }
    }
}

/// Information about an active screen share session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub workspace_id: String,
    pub sandbox_type: SandboxType,
    pub state: SessionState,
    pub vnc_port: u16,
    pub novnc_port: u16,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Internal session tracking
// ---------------------------------------------------------------------------

struct ActiveSession {
    info: SessionInfo,
    vnc_process: Option<Child>,
    access_token: String,
    last_activity: Instant,
}

// ---------------------------------------------------------------------------
// ScreenShareManager
// ---------------------------------------------------------------------------

/// Manages per-workspace screen share sessions.
pub struct ScreenShareManager {
    config: ScreenShareConfig,
    sessions: Arc<RwLock<HashMap<String, ActiveSession>>>,
    next_port_offset: Arc<RwLock<u16>>,
}

impl ScreenShareManager {
    /// Create a new screen share manager.
    pub fn new(config: ScreenShareConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            next_port_offset: Arc::new(RwLock::new(0)),
        }
    }

    /// Start a screen share session for a workspace.
    ///
    /// Returns session info including ports and a one-time access token.
    /// The VNC password is generated internally and never returned.
    pub async fn start_session(
        &self,
        workspace_id: &str,
        sandbox_type: SandboxType,
    ) -> Result<(SessionInfo, String)> {
        // Check for existing session
        {
            let sessions = self.sessions.read().await;
            if sessions.contains_key(workspace_id) {
                return Err(ScreenShareError::AlreadyActive(workspace_id.to_string()));
            }
        }

        // Allocate ports
        let mut offset = self.next_port_offset.write().await;
        let vnc_port = self.config.vnc_port_base + *offset;
        let novnc_port = vnc_port + 1000; // noVNC WebSocket proxy port
        *offset += 2;

        // Generate secure access token (one-time use)
        let access_token = generate_token();

        // Generate VNC password (never exposed)
        let _vnc_password = generate_vnc_password();
        let session_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let info = SessionInfo {
            session_id: session_id.clone(),
            workspace_id: workspace_id.to_string(),
            sandbox_type: sandbox_type.clone(),
            state: SessionState::Starting,
            vnc_port,
            novnc_port,
            started_at: now,
            last_activity: now,
        };

        // Spawn VNC server based on sandbox type
        let vnc_process = match sandbox_type {
            SandboxType::Docker => {
                self.spawn_docker_vnc(workspace_id, vnc_port).await?
            }
            SandboxType::Wasm => {
                // WASM sandboxes use a virtual framebuffer approach
                self.spawn_wasm_vnc(workspace_id, vnc_port).await?
            }
        };

        let session = ActiveSession {
            info: info.clone(),
            vnc_process: Some(vnc_process),
            access_token: access_token.clone(),
            last_activity: Instant::now(),
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(workspace_id.to_string(), session);

        info!(
            workspace = %workspace_id,
            vnc_port,
            novnc_port,
            "Screen share session started"
        );

        Ok((info, access_token))
    }

    /// Validate a one-time access token for a session.
    pub async fn validate_token(
        &self,
        workspace_id: &str,
        token: &str,
    ) -> Result<SessionInfo> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| ScreenShareError::SessionNotFound(workspace_id.to_string()))?;

        if session.access_token != token {
            return Err(ScreenShareError::InvalidToken);
        }

        // One-time use: clear the token after validation
        session.access_token.clear();
        session.info.state = SessionState::Active;
        session.last_activity = Instant::now();
        session.info.last_activity = chrono::Utc::now();
        Ok(session.info.clone())
    }

    /// Record activity on a session (resets idle timer).
    pub async fn record_activity(&self, workspace_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(workspace_id)
            .ok_or_else(|| ScreenShareError::SessionNotFound(workspace_id.to_string()))?;

        session.last_activity = Instant::now();
        session.info.last_activity = chrono::Utc::now();
        session.info.state = SessionState::Active;

        Ok(())
    }

    /// Stop a screen share session.
    pub async fn stop_session(&self, workspace_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .remove(workspace_id)
            .ok_or_else(|| ScreenShareError::SessionNotFound(workspace_id.to_string()))?;

        // Kill VNC process
        if let Some(mut proc) = session.vnc_process {
            let _ = proc.kill().await;
        }

        info!(workspace = %workspace_id, "Screen share session stopped");
        Ok(())
    }

    /// Get info about an active session.
    pub async fn get_session(&self, workspace_id: &str) -> Result<SessionInfo> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(workspace_id)
            .ok_or_else(|| ScreenShareError::SessionNotFound(workspace_id.to_string()))?;

        Ok(session.info.clone())
    }

    /// List all active sessions.
    pub async fn list_sessions(&self) -> Vec<SessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|s| s.info.clone()).collect()
    }

    /// Check for and clean up idle sessions.
    /// Returns the IDs of sessions that were stopped due to idle timeout.
    pub async fn cleanup_idle_sessions(&self) -> Vec<String> {
        let timeout = Duration::from_secs(self.config.idle_timeout_secs);
        let now = Instant::now();
        let mut timed_out = Vec::new();

        {
            let sessions = self.sessions.read().await;
            for (ws_id, session) in sessions.iter() {
                if now.duration_since(session.last_activity) > timeout {
                    timed_out.push(ws_id.clone());
                }
            }
        }

        for ws_id in &timed_out {
            let _ = self.stop_session(ws_id).await;
            warn!(workspace = %ws_id, "Screen share session timed out");
        }

        timed_out
    }

    /// Get the number of active sessions.
    pub async fn active_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    // -- Private helpers --

    async fn spawn_docker_vnc(
        &self,
        workspace_id: &str,
        port: u16,
    ) -> Result<Child> {
        // In production, this would exec into the Docker container and start
        // Xvfb + x11vnc. For now, we spawn a placeholder that represents
        // the VNC server process.
        let child = Command::new("echo")
            .args([
                &format!("VNC server for {} on port {}", workspace_id, port),
            ])
            .spawn()
            .map_err(|e| ScreenShareError::ServerStartFailed(e.to_string()))?;

        Ok(child)
    }

    async fn spawn_wasm_vnc(
        &self,
        workspace_id: &str,
        port: u16,
    ) -> Result<Child> {
        // WASM sandboxes use a virtual display rendered via canvas.
        // The VNC server bridges the virtual framebuffer to the noVNC client.
        let child = Command::new("echo")
            .args([
                &format!("WASM VNC bridge for {} on port {}", workspace_id, port),
            ])
            .spawn()
            .map_err(|e| ScreenShareError::ServerStartFailed(e.to_string()))?;

        Ok(child)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn generate_token() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.gen();
    hex::encode(bytes)
}

fn generate_vnc_password() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
    hex::encode(bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> ScreenShareManager {
        ScreenShareManager::new(ScreenShareConfig {
            idle_timeout_secs: 60,
            vnc_port_base: 15900,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn test_start_session() {
        let mgr = test_manager();
        let (info, token) = mgr.start_session("ws-1", SandboxType::Docker).await.unwrap();

        assert_eq!(info.workspace_id, "ws-1");
        assert_eq!(info.sandbox_type, SandboxType::Docker);
        assert_eq!(info.state, SessionState::Starting);
        assert!(!token.is_empty());
        assert_eq!(info.vnc_port, 15900);
        assert_eq!(info.novnc_port, 16900);
    }

    #[tokio::test]
    async fn test_duplicate_session_rejected() {
        let mgr = test_manager();
        mgr.start_session("ws-dup", SandboxType::Docker).await.unwrap();

        let result = mgr.start_session("ws-dup", SandboxType::Docker).await;
        assert!(matches!(result, Err(ScreenShareError::AlreadyActive(_))));
    }

    #[tokio::test]
    async fn test_validate_token() {
        let mgr = test_manager();
        let (info, token) = mgr.start_session("ws-auth", SandboxType::Docker).await.unwrap();

        let validated = mgr.validate_token("ws-auth", &token).await.unwrap();
        assert_eq!(validated.session_id, info.session_id);
        assert_eq!(validated.state, SessionState::Active);

        // Token is one-time: second use should fail
        let result = mgr.validate_token("ws-auth", &token).await;
        assert!(matches!(result, Err(ScreenShareError::InvalidToken)));
    }

    #[tokio::test]
    async fn test_invalid_token_rejected() {
        let mgr = test_manager();
        mgr.start_session("ws-bad", SandboxType::Docker).await.unwrap();

        let result = mgr.validate_token("ws-bad", "wrong-token").await;
        assert!(matches!(result, Err(ScreenShareError::InvalidToken)));
    }

    #[tokio::test]
    async fn test_stop_session() {
        let mgr = test_manager();
        mgr.start_session("ws-stop", SandboxType::Docker).await.unwrap();

        mgr.stop_session("ws-stop").await.unwrap();
        let result = mgr.get_session("ws-stop").await;
        assert!(matches!(result, Err(ScreenShareError::SessionNotFound(_))));
    }

    #[tokio::test]
    async fn test_record_activity() {
        let mgr = test_manager();
        mgr.start_session("ws-act", SandboxType::Docker).await.unwrap();

        mgr.record_activity("ws-act").await.unwrap();

        let info = mgr.get_session("ws-act").await.unwrap();
        assert_eq!(info.state, SessionState::Active);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let mgr = test_manager();
        mgr.start_session("ws-a", SandboxType::Docker).await.unwrap();
        mgr.start_session("ws-b", SandboxType::Wasm).await.unwrap();
        let sessions = mgr.list_sessions().await;
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_active_count() {
        let mgr = test_manager();
        assert_eq!(mgr.active_count().await, 0);

        mgr.start_session("ws-cnt", SandboxType::Docker).await.unwrap();
        assert_eq!(mgr.active_count().await, 1);

        mgr.stop_session("ws-cnt").await.unwrap();
        assert_eq!(mgr.active_count().await, 0);
    }

    #[tokio::test]
    async fn test_wasm_sandbox_session() {
        let mgr = test_manager();
        let (info, _token) = mgr.start_session("ws-wasm", SandboxType::Wasm).await.unwrap();

        assert_eq!(info.sandbox_type, SandboxType::Wasm);
    }

    #[tokio::test]
    async fn test_cleanup_idle_sessions() {
        let mgr = ScreenShareManager::new(ScreenShareConfig {
            idle_timeout_secs: 0, // Immediate timeout for testing
            vnc_port_base: 15900,
            ..Default::default()
        });

        mgr.start_session("ws-idle", SandboxType::Docker).await.unwrap();
        // Small delay to ensure idle time exceeds 0
        tokio::time::sleep(Duration::from_millis(10)).await;

        let cleaned = mgr.cleanup_idle_sessions().await;
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0], "ws-idle");
        assert_eq!(mgr.active_count().await, 0);
    }

    #[test]
    fn test_default_config() {
        let config = ScreenShareConfig::default();
        assert_eq!(config.idle_timeout_secs, 1800);
        assert_eq!(config.vnc_port_base, 5900);
        assert_eq!(config.display_width, 1280);
        assert_eq!(config.display_height, 720);
        assert_eq!(config.color_depth, 24);
    }

    #[test]
    fn test_generate_token_uniqueness() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert_eq!(t1.len(), 64); // 32 bytes hex-encoded
    }

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            session_id: "sess-1".into(),
            workspace_id: "ws-1".into(),
            sandbox_type: SandboxType::Docker,
            state: SessionState::Active,
            vnc_port: 5900,
            novnc_port: 6900,
            started_at: chrono::Utc::now(),
            last_activity: chrono::Utc::now(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.session_id, "sess-1");
        assert_eq!(deserialized.vnc_port, 5900);
    }
}