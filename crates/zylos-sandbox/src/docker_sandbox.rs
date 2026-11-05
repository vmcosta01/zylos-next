//! Docker Sandbox — Container-based isolation for workloads requiring a full OS
//!
//! Uses bollard (async Docker API) to spawn containers with resource limits,
//! network isolation, volume management, health checks, log streaming,
//! image pull policies, and graceful shutdown.

use bollard::container::{
    Config as ContainerConfig, CreateContainerOptions, LogsOptions,
    RemoveContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::models::HostConfig;
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::network::{ConnectNetworkOptions, CreateNetworkOptions};
use bollard::volume::{CreateVolumeOptions, RemoveVolumeOptions};
use bollard::Docker;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Errors from Docker sandbox operations
#[derive(Error, Debug)]
pub enum DockerError {
    #[error("Docker daemon unavailable: {0}")]
    DaemonUnavailable(String),
    #[error("Container creation failed: {0}")]
    CreationFailed(String),
    #[error("Container execution failed: {0}")]
    ExecutionFailed(String),
    #[error("OOM killed")]
    OomKilled,
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Cleanup failed: {0}")]
    CleanupFailed(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Image pull failed: {0}")]
    ImagePullFailed(String),
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
    #[error("Network operation failed: {0}")]
    NetworkFailed(String),
    #[error("Volume operation failed: {0}")]
    VolumeFailed(String),
    #[error("Log streaming failed: {0}")]
    LogStreamFailed(String),
}

/// When to pull container images
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImagePullPolicy {
    Always,
    IfNotPresent,
    Never,
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        Self::IfNotPresent
    }
}

/// Network mode for container isolation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum NetworkMode {
    #[default]
    None,
    Bridge,
    Host,
    Custom(String),
}

/// Volume mount specification (bind mount from host)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    pub host: String,
    pub container: String,
    pub read_only: bool,
}

/// Named volume managed by the sandbox (auto-created / auto-removed)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedVolume {
    pub name: String,
    pub container_path: String,
    pub read_only: bool,
}

/// Health-check strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub command: Vec<String>,
    pub interval_secs: u64,
    pub start_period_secs: u64,
    pub retries: u32,
    pub auto_restart: bool,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            command: vec!["true".to_string()],
            interval_secs: 5,
            start_period_secs: 2,
            retries: 3,
            auto_restart: true,
        }
    }
}

/// Configuration for Docker sandbox containers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    pub image: String,
    pub cpu_limit: f64,
    pub memory_limit: u64,
    pub network_mode: NetworkMode,
    pub auto_remove: bool,
    pub volumes: Vec<VolumeMount>,
    pub managed_volumes: Vec<ManagedVolume>,
    pub env_vars: HashMap<String, String>,
    pub timeout_secs: u64,
    pub pull_policy: ImagePullPolicy,
    pub health_check: Option<HealthCheckConfig>,
    pub stop_timeout_secs: u64,
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "alpine:3.19".to_string(),
            cpu_limit: 0.5,
            memory_limit: 256 * 1024 * 1024,
            network_mode: NetworkMode::None,
            auto_remove: false,
            volumes: Vec::new(),
            managed_volumes: Vec::new(),
            env_vars: HashMap::new(),
            timeout_secs: 30,
            pull_policy: ImagePullPolicy::IfNotPresent,
            health_check: None,
            stop_timeout_secs: 10,
        }
    }
}

/// Result of executing a command in the sandbox
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

/// A single log entry streamed from the container
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub stream: LogStream,
    pub data: String,
}

/// Which stream a log line belongs to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStream {
    Stdout,
    Stderr,
}

/// Container status as reported by health-check polling
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerStatus {
    Created,
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
    NotFound,
}

/// Docker-based sandbox with full lifecycle management.
#[derive(Debug)]
pub struct DockerSandbox {
    config: DockerConfig,
    client: Docker,
    container_id: Mutex<Option<String>>,
    network_id: Mutex<Option<String>>,
    created_volume_names: Mutex<Vec<String>>,
}

impl DockerSandbox {
    pub fn new(config: DockerConfig) -> Result<Self, DockerError> {
        Self::validate_config(&config)?;
        let client = Docker::connect_with_local_defaults()
            .map_err(|e| DockerError::DaemonUnavailable(e.to_string()))?;
        Ok(Self {
            config,
            client,
            container_id: Mutex::new(None),
            network_id: Mutex::new(None),
            created_volume_names: Mutex::new(Vec::new()),
        })
    }

    fn validate_config(config: &DockerConfig) -> Result<(), DockerError> {
        if config.image.is_empty() {
            return Err(DockerError::InvalidConfig("image cannot be empty".into()));
        }
        if config.cpu_limit <= 0.0 || config.cpu_limit > 128.0 {
            return Err(DockerError::InvalidConfig(format!(
                "cpu_limit must be >0 and <=128, got {}", config.cpu_limit
            )));
        }
        if config.memory_limit < 4 * 1024 * 1024 {
            return Err(DockerError::InvalidConfig("memory_limit must be >= 4MB".into()));
        }
        Ok(())
    }

    pub fn image(&self) -> &str { &self.config.image }
    pub fn cpu_limit(&self) -> f64 { self.config.cpu_limit }
    pub fn memory_limit(&self) -> u64 { self.config.memory_limit }
    pub fn network_mode(&self) -> &NetworkMode { &self.config.network_mode }
    pub fn auto_remove(&self) -> bool { self.config.auto_remove }
    pub async fn container_id(&self) -> Option<String> { self.container_id.lock().await.clone() }

    pub async fn ping(&self) -> Result<(), DockerError> {
        self.client.ping().await.map(|_| ()).map_err(|e| DockerError::DaemonUnavailable(e.to_string()))
    }

    async fn ensure_image(&self) -> Result<(), DockerError> {
        match self.config.pull_policy {
            ImagePullPolicy::Never => {
                self.client.inspect_image(&self.config.image).await.map(|_| ()).map_err(|_| {
                    DockerError::ImagePullFailed(format!("image {} not found locally and pull policy is Never", self.config.image))
                })
            }
            ImagePullPolicy::Always => self.pull_image().await,
            ImagePullPolicy::IfNotPresent => {
                if self.client.inspect_image(&self.config.image).await.is_err() {
                    self.pull_image().await
                } else {
                    debug!("Image {} already present", self.config.image);
                    Ok(())
                }
            }
        }
    }

    async fn pull_image(&self) -> Result<(), DockerError> {
        info!("Pulling image {}", self.config.image);
        let opts = CreateImageOptions { from_image: self.config.image.as_str(), ..Default::default() };
        let mut stream = self.client.create_image(Some(opts), None, None);
        while let Some(result) = stream.next().await {
            result.map_err(|e| DockerError::ImagePullFailed(e.to_string()))?;
        }
        info!("Pulled image {}", self.config.image);
        Ok(())
    }

    async fn create_managed_volumes(&self) -> Result<(), DockerError> {
        let mut names = self.created_volume_names.lock().await;
        for vol in &self.config.managed_volumes {
            let opts = CreateVolumeOptions { name: vol.name.as_str(), ..Default::default() };
            self.client.create_volume(opts).await.map_err(|e| DockerError::VolumeFailed(e.to_string()))?;
            names.push(vol.name.clone());
            info!("Created managed volume {}", vol.name);
        }
        Ok(())
    }

    async fn remove_managed_volumes(&self) -> Result<(), DockerError> {
        let mut names = self.created_volume_names.lock().await;
        let mut last_err: Option<DockerError> = None;
        for name in names.drain(..) {
            let opts = RemoveVolumeOptions { force: true };
            if let Err(e) = self.client.remove_volume(&name, Some(opts)).await {
                warn!("Failed to remove volume {}: {}", name, e);
                last_err = Some(DockerError::VolumeFailed(e.to_string()));
            } else {
                info!("Removed managed volume {}", name);
            }
        }
        if let Some(err) = last_err { Err(err) } else { Ok(()) }
    }

    async fn setup_network(&self) -> Result<Option<String>, DockerError> {
        match &self.config.network_mode {
            NetworkMode::Custom(name) => {
                let opts = CreateNetworkOptions { name: name.as_str(), driver: "bridge", ..Default::default() };
                match self.client.create_network(opts).await {
                    Ok(resp) => { info!("Created network {}", name); Ok(resp.id) }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("already exists") { Ok(None) } else { Err(DockerError::NetworkFailed(msg)) }
                    }
                }
            }
            _ => Ok(None),
        }
    }

    async fn teardown_network(&self) -> Result<(), DockerError> {
        let mut guard = self.network_id.lock().await;
        if let Some(id) = guard.take() {
            if let Err(e) = self.client.remove_network(&id).await {
                warn!("Failed to remove network {}: {}", id, e);
                return Err(DockerError::NetworkFailed(e.to_string()));
            }
        }
        Ok(())
    }

    pub async fn start(&self) -> Result<String, DockerError> {
        self.ensure_image().await?;
        self.create_managed_volumes().await?;
        let network_id = self.setup_network().await?;
        if let Some(nid) = network_id { *self.network_id.lock().await = Some(nid); }

        let nano_cpus = (self.config.cpu_limit * 1_000_000_000.0) as i64;
        let mem_limit = self.config.memory_limit as i64;
        let mut binds: Vec<String> = Vec::new();
        for v in &self.config.volumes {
            let ro = if v.read_only { ":ro" } else { "" };
            binds.push(format!("{}:{}{}", v.host, v.container, ro));
        }
        for v in &self.config.managed_volumes {
            let ro = if v.read_only { ":ro" } else { "" };
            binds.push(format!("{}:{}{}", v.name, v.container_path, ro));
        }
        let network_mode_str = match &self.config.network_mode {
            NetworkMode::None => "none", NetworkMode::Bridge => "bridge",
            NetworkMode::Host => "host", NetworkMode::Custom(_) => "bridge",
        }.to_string();

        let host_config = HostConfig {
            nano_cpus: Some(nano_cpus), memory: Some(mem_limit), memory_swap: Some(mem_limit),
            binds: if binds.is_empty() { None } else { Some(binds) },
            network_mode: Some(network_mode_str), auto_remove: Some(self.config.auto_remove),
            ..Default::default()
        };
        let env: Vec<String> = self.config.env_vars.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        let container_config = ContainerConfig {
            image: Some(self.config.image.clone()),
            env: if env.is_empty() { None } else { Some(env) },
            host_config: Some(host_config), ..Default::default()
        };
        let create_opts = CreateContainerOptions { name: format!("zylos-sandbox-{}", uuid::Uuid::new_v4()), ..Default::default() };
        let response = self.client.create_container(Some(create_opts), container_config).await
            .map_err(|e| DockerError::CreationFailed(e.to_string()))?;
        let cid = response.id.clone();

        if let NetworkMode::Custom(net_name) = &self.config.network_mode {
            let connect_opts = ConnectNetworkOptions { container: cid.as_str(), ..Default::default() };
            self.client.connect_network(net_name, connect_opts).await.map_err(|e| DockerError::NetworkFailed(e.to_string()))?;
        }
        self.client.start_container(&cid, None::<StartContainerOptions<String>>).await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;
        *self.container_id.lock().await = Some(cid.clone());
        info!("Started container {}", &cid[..12.min(cid.len())]);
        Ok(cid)
    }

    pub async fn exec(&self, cmd: &[&str]) -> Result<ExecResult, DockerError> {
        let cid = self.container_id.lock().await.clone()
            .ok_or_else(|| DockerError::ExecutionFailed("no container running".into()))?;
        let exec_opts = CreateExecOptions {
            cmd: Some(cmd.iter().map(|s| s.to_string()).collect()),
            attach_stdout: Some(true), attach_stderr: Some(true), ..Default::default()
        };
        let exec = self.client.create_exec(&cid, exec_opts).await.map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;
        let output = self.client.start_exec(&exec.id, None).await.map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let StartExecResults::Attached { output: mut stream, .. } = output {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(log) => match log {
                        bollard::container::LogOutput::StdOut { message } => stdout.push_str(&String::from_utf8_lossy(&message)),
                        bollard::container::LogOutput::StdErr { message } => stderr.push_str(&String::from_utf8_lossy(&message)),
                        _ => {}
                    },
                    Err(e) => return Err(DockerError::ExecutionFailed(e.to_string())),
                }
            }
        }
        let inspect = self.client.inspect_exec(&exec.id).await.map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;
        Ok(ExecResult { exit_code: inspect.exit_code.unwrap_or(-1), stdout, stderr })
    }

    pub async fn logs(&self, follow: bool) -> Result<impl futures_util::Stream<Item = Result<LogEntry, DockerError>>, DockerError> {
        let cid = self.container_id.lock().await.clone()
            .ok_or_else(|| DockerError::LogStreamFailed("no container running".into()))?;
        let opts = LogsOptions::<String> { follow, stdout: true, stderr: true, timestamps: false, ..Default::default() };
        let stream = self.client.logs(&cid, Some(opts));
        Ok(stream.map(|chunk| chunk.map(|log| match log {
            bollard::container::LogOutput::StdOut { message } => LogEntry { stream: LogStream::Stdout, data: String::from_utf8_lossy(&message).to_string() },
            bollard::container::LogOutput::StdErr { message } => LogEntry { stream: LogStream::Stderr, data: String::from_utf8_lossy(&message).to_string() },
            _ => LogEntry { stream: LogStream::Stdout, data: String::new() },
        }).map_err(|e| DockerError::LogStreamFailed(e.to_string()))))
    }

    pub async fn status(&self) -> Result<ContainerStatus, DockerError> {
        let cid = match self.container_id.lock().await.clone() { Some(id) => id, None => return Ok(ContainerStatus::NotFound) };
        match self.client.inspect_container(&cid, None).await {
            Ok(info) => {
                let state = info.state.unwrap_or_default();
                Ok(match state.status {
                    Some(bollard::secret::ContainerStateStatusEnum::CREATED) => ContainerStatus::Created,
                    Some(bollard::secret::ContainerStateStatusEnum::RUNNING) => ContainerStatus::Running,
                    Some(bollard::secret::ContainerStateStatusEnum::PAUSED) => ContainerStatus::Paused,
                    Some(bollard::secret::ContainerStateStatusEnum::RESTARTING) => ContainerStatus::Restarting,
                    Some(bollard::secret::ContainerStateStatusEnum::EXITED) => ContainerStatus::Exited,
                    Some(bollard::secret::ContainerStateStatusEnum::DEAD) => ContainerStatus::Dead,
                    _ => ContainerStatus::Exited,
                })
            }
            Err(_) => Ok(ContainerStatus::NotFound),
        }
    }

    pub async fn health_probe(&self) -> Result<bool, DockerError> {
        let hc = self.config.health_check.as_ref()
            .ok_or_else(|| DockerError::HealthCheckFailed("no health check configured".into()))?;
        let cmd: Vec<&str> = hc.command.iter().map(|s| s.as_str()).collect();
        match self.exec(&cmd).await { Ok(r) => Ok(r.exit_code == 0), Err(_) => Ok(false) }
    }

    pub async fn run_health_check_loop(&self) -> Result<(), DockerError> {
        let hc = self.config.health_check.as_ref()
            .ok_or_else(|| DockerError::HealthCheckFailed("no health check configured".into()))?.clone();
        tokio::time::sleep(std::time::Duration::from_secs(hc.start_period_secs)).await;
        let mut consecutive_failures: u32 = 0;
        loop {
            if self.container_id.lock().await.is_none() { break; }
            match self.health_probe().await {
                Ok(true) => { consecutive_failures = 0; }
                Ok(false) => {
                    consecutive_failures += 1;
                    warn!("Health check failed ({}/{})", consecutive_failures, hc.retries);
                    if consecutive_failures >= hc.retries && hc.auto_restart {
                        warn!("Container unhealthy - restarting");
                        self.restart().await?;
                        consecutive_failures = 0;
                        tokio::time::sleep(std::time::Duration::from_secs(hc.start_period_secs)).await;
                        continue;
                    }
                }
                Err(e) => { error!("Health probe error: {}", e); consecutive_failures += 1; }
            }
            tokio::time::sleep(std::time::Duration::from_secs(hc.interval_secs)).await;
        }
        Ok(())
    }

    pub async fn restart(&self) -> Result<(), DockerError> {
        let cid = self.container_id.lock().await.clone()
            .ok_or_else(|| DockerError::ExecutionFailed("no container to restart".into()))?;
        let _ = self.client.stop_container(&cid, Some(StopContainerOptions { t: 5 })).await;
        self.client.start_container(&cid, None::<StartContainerOptions<String>>).await
            .map_err(|e| DockerError::ExecutionFailed(e.to_string()))?;
        info!("Restarted container {}", &cid[..12.min(cid.len())]);
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), DockerError> {
        let cid = match self.container_id.lock().await.clone() { Some(id) => id, None => return Ok(()) };
        let stop_opts = StopContainerOptions { t: self.config.stop_timeout_secs as i64 };
        info!("Stopping container {} (timeout={}s)", &cid[..12.min(cid.len())], self.config.stop_timeout_secs);
        if let Err(e) = self.client.stop_container(&cid, Some(stop_opts)).await {
            let msg = e.to_string();
            if !msg.contains("is not running") && !msg.contains("not found") { warn!("Stop error: {}", msg); }
        }
        Ok(())
    }

    pub async fn destroy(&self) -> Result<(), DockerError> {
        self.stop().await?;
        let cid = self.container_id.lock().await.take();
        if let Some(id) = cid {
            if !self.config.auto_remove {
                let opts = RemoveContainerOptions { force: true, ..Default::default() };
                if let Err(e) = self.client.remove_container(&id, Some(opts)).await {
                    let msg = e.to_string();
                    if !msg.contains("not found") && !msg.contains("already") { warn!("Remove failed: {}", e); }
                }
            }
        }
        self.remove_managed_volumes().await?;
        self.teardown_network().await?;
        Ok(())
    }
}

impl Drop for DockerSandbox {
    fn drop(&mut self) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let container_id = self.container_id.try_lock().ok().and_then(|g| g.clone());
            let volume_names = self.created_volume_names.try_lock().ok().map(|g| g.clone()).unwrap_or_default();
            let network_id = self.network_id.try_lock().ok().and_then(|g| g.clone());
            let auto_remove = self.config.auto_remove;
            handle.spawn(async move {
                if let Ok(client) = Docker::connect_with_local_defaults() {
                    if let Some(id) = container_id {
                        if !auto_remove {
                            let _ = client.remove_container(&id, Some(RemoveContainerOptions { force: true, ..Default::default() })).await;
                        }
                    }
                    for name in volume_names {
                        let _ = client.remove_volume(&name, Some(RemoveVolumeOptions { force: true })).await;
                    }
                    if let Some(nid) = network_id { let _ = client.remove_network(&nid).await; }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let c = DockerConfig::default();
        assert_eq!(c.image, "alpine:3.19");
        assert_eq!(c.cpu_limit, 0.5);
        assert_eq!(c.memory_limit, 256 * 1024 * 1024);
        assert_eq!(c.network_mode, NetworkMode::None);
        assert!(!c.auto_remove);
        assert!(c.volumes.is_empty());
        assert!(c.managed_volumes.is_empty());
        assert_eq!(c.pull_policy, ImagePullPolicy::IfNotPresent);
        assert!(c.health_check.is_none());
        assert_eq!(c.stop_timeout_secs, 10);
    }

    #[test]
    fn test_reject_empty_image() {
        assert!(matches!(DockerSandbox::new(DockerConfig { image: "".into(), ..Default::default() }).unwrap_err(), DockerError::InvalidConfig(_)));
    }

    #[test]
    fn test_reject_invalid_cpu() {
        assert!(DockerSandbox::new(DockerConfig { cpu_limit: 0.0, ..Default::default() }).is_err());
        assert!(DockerSandbox::new(DockerConfig { cpu_limit: -1.0, ..Default::default() }).is_err());
    }

    #[test]
    fn test_reject_low_memory() {
        assert!(matches!(DockerSandbox::new(DockerConfig { memory_limit: 1024, ..Default::default() }).unwrap_err(), DockerError::InvalidConfig(_)));
    }

    #[test]
    fn test_status_not_found_when_no_container() {
        let s = DockerSandbox::new(DockerConfig::default()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert_eq!(rt.block_on(s.status()).unwrap(), ContainerStatus::NotFound);
    }

    #[test]
    fn test_stop_noop_when_no_container() {
        let s = DockerSandbox::new(DockerConfig::default()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(s.stop()).is_ok());
    }

    #[test]
    fn test_destroy_noop_when_no_container() {
        let s = DockerSandbox::new(DockerConfig::default()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        assert!(rt.block_on(s.destroy()).is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn integration_lifecycle() {
        let config = DockerConfig { image: "alpine:3.19".into(), cpu_limit: 0.25, memory_limit: 64*1024*1024, ..Default::default() };
        let sandbox = DockerSandbox::new(config).unwrap();
        sandbox.ping().await.expect("Docker daemon must be reachable");
        let cid = sandbox.start().await.expect("start failed");
        assert!(!cid.is_empty());
        assert_eq!(sandbox.status().await.unwrap(), ContainerStatus::Running);
        let r = sandbox.exec(&["echo", "hello"]).await.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.stdout.contains("hello"));
        sandbox.destroy().await.unwrap();
        assert_eq!(sandbox.status().await.unwrap(), ContainerStatus::NotFound);
    }

    #[tokio::test]
    #[ignore]
    async fn integration_log_streaming() {
        let sandbox = DockerSandbox::new(DockerConfig::default()).unwrap();
        sandbox.start().await.unwrap();
        sandbox.exec(&["sh", "-c", "echo line1; echo line2 >&2"]).await.unwrap();
        let mut stream = sandbox.logs(false).await.unwrap();
        let mut entries = Vec::new();
        while let Some(e) = stream.next().await { entries.push(e.unwrap()); }
        assert!(!entries.is_empty());
        sandbox.destroy().await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn integration_volume_cleanup() {
        let vol = format!("zylos-test-{}", uuid::Uuid::new_v4());
        let config = DockerConfig { managed_volumes: vec![ManagedVolume { name: vol.clone(), container_path: "/data".into(), read_only: false }], ..Default::default() };
        let sandbox = DockerSandbox::new(config).unwrap();
        sandbox.start().await.unwrap();
        sandbox.destroy().await.unwrap();
        let client = Docker::connect_with_local_defaults().unwrap();
        assert!(client.inspect_volume(&vol).await.is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn integration_graceful_shutdown() {
        let config = DockerConfig { stop_timeout_secs: 5, ..Default::default() };
        let sandbox = DockerSandbox::new(config).unwrap();
        sandbox.start().await.unwrap();
        sandbox.stop().await.unwrap();
        assert_ne!(sandbox.status().await.unwrap(), ContainerStatus::Running);
        sandbox.destroy().await.unwrap();
    }
}
