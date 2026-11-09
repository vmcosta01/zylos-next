//! Embedded gateway process management.
//!
//! Manages the lifecycle of the `zylos-gateway` binary bundled with the
//! desktop application. Provides start/stop/status operations with proper
//! cleanup on app exit.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// Handle to a running gateway process.
#[derive(Debug)]
pub struct GatewayHandle {
    child: Child,
    pid: u32,
    started_at: std::time::Instant,
}

impl GatewayHandle {
    /// Get the PID of the running gateway.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get uptime in seconds.
    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Check if the process is still running.
    pub fn is_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false, // Process exited
            Ok(None) => true,     // Still running
            Err(e) => {
                error!("Failed to check gateway status: {e}");
                false
            }
        }
    }

    /// Gracefully stop the gateway process.
    pub async fn stop(&mut self) -> Result<(), GatewayError> {
        info!("Stopping embedded gateway (pid={})", self.pid);

        // Try graceful shutdown first (SIGTERM / Ctrl-C on Windows)
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                libc::kill(self.pid as i32, libc::SIGTERM);
            }
        }

        #[cfg(windows)]
        {
            // On Windows, send CTRL_BREAK_EVENT or just kill
            let _ = self.child.kill().await;
        }

        // Wait up to 5 seconds for graceful exit
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.child.wait(),
        );

        match timeout.await {
            Ok(Ok(_)) => {
                info!("Gateway stopped gracefully");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Error waiting for gateway exit: {e}");
                Err(GatewayError::StopFailed(e.to_string()))
            }
            Err(_) => {
                warn!("Gateway did not exit gracefully, force killing");
                self.child.kill().await.map_err(|e| {
                    GatewayError::StopFailed(format!("Force kill failed: {e}"))
                })?;
                Ok(())
            }
        }
    }
}

/// Thread-safe gateway manager.
#[derive(Clone)]
pub struct GatewayManager {
    state: Arc<Mutex<GatewayState>>,
    gateway_bin: PathBuf,
    config_path: Option<PathBuf>,
}

#[derive(Debug)]
enum GatewayState {
    Stopped,
    Running(GatewayHandle),
}

/// Errors from gateway operations.
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    /// Gateway binary does not exist at the expected path.
    #[error("Gateway binary not found at {0}")]
    BinaryNotFound(String),
    /// Attempted to start gateway when one is already running.
    #[error("Gateway already running (pid={0})")]
    AlreadyRunning(u32),
    /// Gateway process failed to spawn.
    #[error("Failed to start gateway: {0}")]
    StartFailed(String),
    /// Gateway process failed to stop gracefully or via force kill.
    #[error("Failed to stop gateway: {0}")]
    StopFailed(String),
    /// Attempted to stop or query gateway when none is running.
    #[error("Gateway not running")]
    NotRunning,
    /// Filesystem I/O error during gateway operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Status information about the gateway.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GatewayStatus {
    /// Whether the gateway process is currently running.
    pub running: bool,
    /// Process ID if running; `None` if stopped.
    pub pid: Option<u32>,
    /// Uptime in seconds if running; `None` if stopped.
    pub uptime_secs: Option<u64>,
}

impl GatewayManager {
    /// Create a new gateway manager.
    ///
    /// # Arguments
    /// * `gateway_bin` - Path to the zylos-gateway binary
    /// * `config_path` - Optional path to gateway config file
    pub fn new(gateway_bin: PathBuf, config_path: Option<PathBuf>) -> Self {
        Self {
            state: Arc::new(Mutex::new(GatewayState::Stopped)),
            gateway_bin,
            config_path,
        }
    }

    /// Resolve the gateway binary path relative to the app executable.
    pub fn resolve_binary(app_dir: &Path) -> PathBuf {
        let bin_name = if cfg!(windows) {
            "zylos-gateway.exe"
        } else {
            "zylos-gateway"
        };
        app_dir.join(bin_name)
    }

    /// Start the embedded gateway.
    pub async fn start(&self) -> Result<GatewayStatus, GatewayError> {
        let mut state = self.state.lock().await;

        // Check if already running
        if let GatewayState::Running(ref mut handle) = *state {
            if handle.is_running() {
                return Err(GatewayError::AlreadyRunning(handle.pid()));
            }
            // Process died, clean up
            info!("Previous gateway process died, restarting");
        }

        // Verify binary exists
        if !self.gateway_bin.exists() {
            return Err(GatewayError::BinaryNotFound(
                self.gateway_bin.display().to_string(),
            ));
        }

        info!("Starting embedded gateway: {}", self.gateway_bin.display());

        let mut cmd = Command::new(&self.gateway_bin);
        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        if let Some(ref config) = self.config_path {
            cmd.arg("--config").arg(config);
        }

        // Set environment for embedded mode
        cmd.env("ZYLOS_EMBEDDED", "1");

        let child = cmd.spawn().map_err(|e| {
            GatewayError::StartFailed(format!("Failed to spawn: {e}"))
        })?;

        let pid = child.id().unwrap_or(0);
        let handle = GatewayHandle {
            child,
            pid,
            started_at: std::time::Instant::now(),
        };

        *state = GatewayState::Running(handle);
        info!("Gateway started (pid={pid})");

        Ok(GatewayStatus {
            running: true,
            pid: Some(pid),
            uptime_secs: Some(0),
        })
    }

    /// Stop the embedded gateway.
    pub async fn stop(&self) -> Result<(), GatewayError> {
        let mut state = self.state.lock().await;

        match std::mem::replace(&mut *state, GatewayState::Stopped) {
            GatewayState::Running(mut handle) => {
                handle.stop().await?;
                Ok(())
            }
            GatewayState::Stopped => Err(GatewayError::NotRunning),
        }
    }

    /// Get current gateway status.
    pub async fn status(&self) -> GatewayStatus {
        let mut state = self.state.lock().await;

        match *state {
            GatewayState::Running(ref mut handle) => {
                if handle.is_running() {
                    GatewayStatus {
                        running: true,
                        pid: Some(handle.pid()),
                        uptime_secs: Some(handle.uptime_secs()),
                    }
                } else {
                    // Process died
                    *state = GatewayState::Stopped;
                    GatewayStatus {
                        running: false,
                        pid: None,
                        uptime_secs: None,
                    }
                }
            }
            GatewayState::Stopped => GatewayStatus {
                running: false,
                pid: None,
                uptime_secs: None,
            },
        }
    }

    /// Ensure gateway is stopped (for cleanup on app exit).
    pub async fn shutdown(&self) {
        if let Err(e) = self.stop().await {
            if !matches!(e, GatewayError::NotRunning) {
                error!("Error during gateway shutdown: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_binary_windows() {
        #[cfg(windows)]
        {
            let path = GatewayManager::resolve_binary(Path::new("C:\\Apps\\Zylos"));
            assert_eq!(path, PathBuf::from("C:\\Apps\\Zylos\\zylos-gateway.exe"));
        }
    }

    #[test]
    fn test_resolve_binary_unix() {
        #[cfg(unix)]
        {
            let path = GatewayManager::resolve_binary(Path::new("/opt/zylos"));
            assert_eq!(path, PathBuf::from("/opt/zylos/zylos-gateway"));
        }
    }

    #[tokio::test]
    async fn test_status_when_stopped() {
        let mgr = GatewayManager::new(
            PathBuf::from("/nonexistent/binary"),
            None,
        );
        let status = mgr.status().await;
        assert!(!status.running);
        assert!(status.pid.is_none());
    }

    #[tokio::test]
    async fn test_start_nonexistent_binary() {
        let mgr = GatewayManager::new(
            PathBuf::from("/nonexistent/binary"),
            None,
        );
        let result = mgr.start().await;
        assert!(matches!(result, Err(GatewayError::BinaryNotFound(_))));
    }
}
