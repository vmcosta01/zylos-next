//! Docker container sandbox for per-agent isolation.

use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::models::HostConfig;
use bollard::Docker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::approval::{ApprovalDecision, ApprovalWorkflow, RiskLevel};
use crate::error::{DockerError, Result};

/// Unique identifier for a sandbox container.
pub type SandboxId = String;

/// Lifecycle status of a sandbox container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    /// Container has been created but not yet started.
    Created,
    /// Container is currently running.
    Running,
    /// Container has been stopped gracefully.
    Stopped,
    /// Container has been removed from the Docker daemon.
    Removed,
    /// Container entered an error state with the given message.
    Error(String),
}

/// Configuration for a Docker sandbox instance.
///
/// Use [`SandboxConfig::default()`] for sensible defaults (Alpine 3.19,
/// 256 MB RAM, no network) and override only what you need.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Base image to use (e.g., `"alpine:3.19"`, `"ubuntu:24.04"`).
    pub image: String,
    /// Maximum memory in bytes (default: 256 MB).
    pub memory_limit: i64,
    /// CPU quota expressed as a fraction of one core (default: `1.0`).
    pub cpu_quota: f64,
    /// Whether to allow outbound network access from the container.
    pub network_enabled: bool,
    /// Host paths to mount into the container as read-only volumes.
    pub readonly_mounts: Vec<String>,
    /// Working directory inside the container (default: `"/workspace"`).
    pub workdir: String,
    /// Environment variables passed to the container at creation time.
    pub env: HashMap<String, String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            image: "alpine:3.19".to_string(),
            memory_limit: 256 * 1024 * 1024, // 256MB
            cpu_quota: 1.0,
            network_enabled: false,
            readonly_mounts: Vec::new(),
            workdir: "/workspace".to_string(),
            env: HashMap::new(),
        }
    }
}

/// A Docker-based sandbox container for agent isolation.
#[derive(Debug)]
pub struct DockerSandbox {
    id: SandboxId,
    config: SandboxConfig,
    docker: Docker,
    status: ContainerStatus,
    approval: ApprovalWorkflow,
}

impl DockerSandbox {
    /// Create a new Docker sandbox with the given configuration.
    ///
    /// Connects to the local Docker daemon and assigns a unique
    /// `zylos-sandbox-<uuid>` identifier. The container is **not** created or
    /// started until [`create`](Self::create) and [`start`](Self::start) are
    /// called respectively.
    ///
    /// # Errors
    /// Returns [`DockerError::DaemonUnavailable`] when the Docker daemon
    /// cannot be reached.
    pub async fn new(config: SandboxConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::DaemonUnavailable(e.to_string()))?;

        let id = format!("zylos-sandbox-{}", Uuid::new_v4());

        Ok(Self {
            id,
            config,
            docker,
            status: ContainerStatus::Created,
            approval: ApprovalWorkflow::default(),
        })
    }

    /// Get the unique sandbox identifier (e.g., `"zylos-sandbox-<uuid>"`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the current lifecycle status of the container.
    pub fn status(&self) -> &ContainerStatus {
        &self.status
    }

    /// Replace the approval workflow used to gate risky operations.
    pub fn set_approval_workflow(&mut self, workflow: ApprovalWorkflow) {
        self.approval = workflow;
    }

    /// Create the container in the Docker daemon without starting it.
    ///
    /// Applies resource limits, network mode, environment variables and
    /// working directory from the [`SandboxConfig`] provided at construction.
    ///
    /// # Errors
    /// Returns [`DockerError::CreationFailed`] when the daemon rejects the
    /// container configuration or the image cannot be pulled.
    pub async fn create(&mut self) -> Result<()> {
        let host_config = HostConfig {
            memory: Some(self.config.memory_limit),
            nano_cpus: Some((self.config.cpu_quota * 1_000_000_000.0) as i64),
            network_mode: if self.config.network_enabled {
                Some("bridge".to_string())
            } else {
                Some("none".to_string())
            },
            ..Default::default()
        };

        let env_vars: Vec<String> = self
            .config
            .env
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        let container_config = Config {
            image: Some(self.config.image.clone()),
            working_dir: Some(self.config.workdir.clone()),
            env: Some(env_vars),
            host_config: Some(host_config),
            ..Default::default()
        };

        let opts = CreateContainerOptions {
            name: self.id.clone(),
            ..Default::default()
        };

        self.docker
            .create_container(Some(opts), container_config)
            .await
            .map_err(|e| DockerError::CreationFailed(e.to_string()))?;

        self.status = ContainerStatus::Created;
        tracing::info!(id = %self.id, "Container created");
        Ok(())
    }

    /// Start the container.
    ///
    /// Gated by [`RiskLevel::Medium`] through the configured
    /// [`ApprovalWorkflow`]. Returns an error when approval is denied or
    /// still pending.
    ///
    /// # Errors
    /// - [`DockerError::ApprovalDenied`] — approval was rejected or is pending.
    /// - [`DockerError::ExecutionFailed`] — the Docker daemon refused to start
    ///   the container.
    pub async fn start(&mut self) -> Result<()> {
        let decision = self.approval.evaluate(RiskLevel::Medium, "start_container");
        match decision {
            ApprovalDecision::Approved { .. } => {}
            ApprovalDecision::Denied { reason } => {
                return Err(DockerError::ApprovalDenied(reason));
            }
            ApprovalDecision::Pending { .. } => {
                return Err(DockerError::ApprovalDenied(
                    "Container start pending approval".to_string(),
                ));
            }
        }

        self.docker
            .start_container(&self.id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;

        self.status = ContainerStatus::Running;
        tracing::info!(id = %self.id, "Container started");
        Ok(())
    }

    /// Stop the container gracefully with a 10-second timeout.
    ///
    /// Transitions the container to [`ContainerStatus::Stopped`]. Safe to call
    /// on an already-stopped container (the Docker daemon treats it as a no-op).
    ///
    /// # Errors
    /// Returns [`DockerError::ExecutionFailed`] when the daemon cannot stop
    /// the container within the timeout period.
    pub async fn stop(&mut self) -> Result<()> {
        self.docker
            .stop_container(&self.id, Some(StopContainerOptions { t: 10 }))
            .await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;

        self.status = ContainerStatus::Stopped;
        tracing::info!(id = %self.id, "Container stopped");
        Ok(())
    }

    /// Remove the container from the Docker daemon.
    ///
    /// Gated by [`RiskLevel::High`] through the configured
    /// [`ApprovalWorkflow`]. If the container is still running it will be
    /// stopped first via [`stop`](Self::stop).
    ///
    /// # Errors
    /// - [`DockerError::ApprovalDenied`] — approval was rejected or is pending.
    /// - [`DockerError::NotFound`] — the container no longer exists in the daemon.
    pub async fn remove(&mut self) -> Result<()> {
        let decision = self.approval.evaluate(RiskLevel::High, "remove_container");
        match decision {
            ApprovalDecision::Approved { .. } => {}
            ApprovalDecision::Denied { reason } => {
                return Err(DockerError::ApprovalDenied(reason));
            }
            ApprovalDecision::Pending { .. } => {
                return Err(DockerError::ApprovalDenied(
                    "Container removal pending approval".to_string(),
                ));
            }
        }

        // Stop first if running
        if self.status == ContainerStatus::Running {
            let _ = self.stop().await;
        }

        self.docker
            .remove_container(
                &self.id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| DockerError::NotFound(e.to_string()))?;

        self.status = ContainerStatus::Removed;
        tracing::info!(id = %self.id, "Container removed");
        Ok(())
    }

    /// Execute a command inside the running container and return its stdout.
    ///
    /// Both stdout and stderr are captured; the returned string contains
    /// whatever the Docker daemon streamed back. The container must be in
    /// [`ContainerStatus::Running`] — calling this on a stopped or removed
    /// sandbox returns an error immediately without contacting the daemon.
    ///
    /// # Errors
    /// - [`DockerError::ExecutionFailed`] — the container is not running, the
    ///   exec instance could not be created, or the command stream failed.
    pub async fn exec(&self, cmd: &[&str]) -> Result<String> {
        if self.status != ContainerStatus::Running {
            return Err(DockerError::ExecutionFailed(format!(
                "Container is not running (status: {:?})",
                self.status
            )));
        }

        use bollard::exec::{CreateExecOptions, StartExecResults};
        use futures::StreamExt;

        let exec_opts = CreateExecOptions {
            cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self
            .docker
            .create_exec(&self.id, exec_opts)
            .await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;

        let output = self
            .docker
            .start_exec(&exec.id, None::<bollard::exec::StartExecOptions>)
            .await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;

        let mut stdout = String::new();
        if let StartExecResults::Attached {
            output: mut stream, ..
        } = output
        {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(log) => {
                        stdout.push_str(&log.to_string());
                    }
                    Err(e) => {
                        return Err(DockerError::ExecutionFailed(e.to_string()));
                    }
                }
            }
        }

        Ok(stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SandboxConfig::default();
        assert_eq!(config.image, "alpine:3.19");
        assert_eq!(config.memory_limit, 256 * 1024 * 1024);
        assert!(!config.network_enabled);
    }

    #[test]
    fn test_container_status_equality() {
        assert_eq!(ContainerStatus::Created, ContainerStatus::Created);
        assert_ne!(ContainerStatus::Running, ContainerStatus::Stopped);
    }

    #[tokio::test]
    async fn test_sandbox_creation_without_docker() {
        // This test verifies that we get a proper error when Docker is unavailable
        let result = DockerSandbox::new(SandboxConfig::default()).await;
        // On CI or machines without Docker, this will fail with DaemonUnavailable
        // On machines with Docker, it succeeds — both are valid outcomes
        match result {
            Ok(sandbox) => {
                assert!(sandbox.id().starts_with("zylos-sandbox-"));
                assert_eq!(*sandbox.status(), ContainerStatus::Created);
            }
            Err(DockerError::DaemonUnavailable(_)) => {
                // Expected on systems without Docker
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_sandbox_config_custom_values() {
        let mut env = HashMap::new();
        env.insert("FOO".to_string(), "bar".to_string());
        let config = SandboxConfig {
            image: "ubuntu:24.04".to_string(),
            memory_limit: 512 * 1024 * 1024,
            cpu_quota: 2.0,
            network_enabled: true,
            readonly_mounts: vec!["/host/data".to_string()],
            workdir: "/app".to_string(),
            env,
        };
        assert_eq!(config.image, "ubuntu:24.04");
        assert_eq!(config.memory_limit, 512 * 1024 * 1024);
        assert_eq!(config.cpu_quota, 2.0);
        assert!(config.network_enabled);
        assert_eq!(config.readonly_mounts.len(), 1);
        assert_eq!(config.workdir, "/app");
        assert_eq!(config.env.get("FOO").map(|s| s.as_str()), Some("bar"));
    }

    #[test]
    fn test_container_status_serialization_roundtrip() {
        let statuses = vec![
            ContainerStatus::Created,
            ContainerStatus::Running,
            ContainerStatus::Stopped,
            ContainerStatus::Removed,
            ContainerStatus::Error("something broke".to_string()),
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).expect("serialize");
            let deserialized: ContainerStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(status, deserialized);
        }
    }

    #[test]
    fn test_sandbox_config_serialization_roundtrip() {
        let config = SandboxConfig {
            image: "alpine:3.19".to_string(),
            memory_limit: 128 * 1024 * 1024,
            cpu_quota: 0.5,
            network_enabled: false,
            readonly_mounts: vec!["/mnt".to_string()],
            workdir: "/workspace".to_string(),
            env: HashMap::from([("KEY".to_string(), "val".to_string())]),
        };
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: SandboxConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.image, config.image);
        assert_eq!(deserialized.memory_limit, config.memory_limit);
        assert_eq!(deserialized.cpu_quota, config.cpu_quota);
        assert_eq!(deserialized.network_enabled, config.network_enabled);
        assert_eq!(deserialized.readonly_mounts, config.readonly_mounts);
        assert_eq!(deserialized.workdir, config.workdir);
        assert_eq!(deserialized.env, config.env);
    }

    #[test]
    fn test_set_approval_workflow_replaces_previous() {
        // Verify set_approval_workflow compiles and accepts a custom workflow.
        // We cannot construct DockerSandbox without Docker, so we test the
        // ApprovalWorkflow replacement logic independently.
        let wf = ApprovalWorkflow {
            require_medium: false,
            require_low: true,
            auto_approve_after: Some(std::time::Duration::from_secs(30)),
        };
        assert!(!wf.requires_approval(RiskLevel::Medium));
        assert!(wf.requires_approval(RiskLevel::Low));
        assert_eq!(
            wf.auto_approve_after,
            Some(std::time::Duration::from_secs(30))
        );
    }
}
