//! Docker container sandbox for per-agent isolation.

use bollard::Docker;
use bollard::container::{
    Config, CreateContainerOptions, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::models::HostConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::approval::{ApprovalDecision, ApprovalWorkflow, RiskLevel};
use crate::error::{DockerError, Result};

/// Unique identifier for a sandbox container.
pub type SandboxId = String;

/// Status of a sandbox container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
    Removed,
    Error(String),
}

/// Configuration for a Docker sandbox instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Base image to use (e.g., "alpine:3.19", "ubuntu:24.04").
    pub image: String,
    /// Maximum memory in bytes (default: 256MB).
    pub memory_limit: i64,
    /// CPU quota (default: 1.0 = one core).
    pub cpu_quota: f64,
    /// Whether to allow network access.
    pub network_enabled: bool,
    /// Host paths to mount read-only.
    pub readonly_mounts: Vec<String>,
    /// Working directory inside the container.
    pub workdir: String,
    /// Environment variables.
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

    /// Get the sandbox identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get current container status.
    pub fn status(&self) -> &ContainerStatus {
        &self.status
    }

    /// Set the approval workflow for this sandbox.
    pub fn set_approval_workflow(&mut self, workflow: ApprovalWorkflow) {
        self.approval = workflow;
    }

    /// Create the container without starting it.
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

    /// Start the container. Requires approval for medium-risk operations.
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

    /// Stop the container gracefully.
    pub async fn stop(&mut self) -> Result<()> {
        self.docker
            .stop_container(&self.id, Some(StopContainerOptions { t: 10 }))
            .await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;

        self.status = ContainerStatus::Stopped;
        tracing::info!(id = %self.id, "Container stopped");
        Ok(())
    }

    /// Remove the container. Requires approval for high-risk operations.
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

    /// Execute a command inside the container and return stdout.
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
        if let StartExecResults::Attached { output: mut stream, .. } = output {
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
}