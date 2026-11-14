//! # ResourceLimiter — Controle de recursos por agente (T8.3.1 + T8.3.2)
//!
//! ## T8.3.1: ResourceLimiter cgroups
//! Aplica limites de CPU, memória e PIDs via cgroups v2 (Linux).
//! Em Windows/macOS, usa Docker resource limits como fallback.
//!
//! ## T8.3.2: Platform Abstraction Layer
//! Trait `ResourceLimiterBackend` com implementações platform-specific:
//! - `CgroupsLimiter`: Linux cgroups v2 nativo
//! - `DockerLimiter`: Docker container resource limits (cross-platform)
//! - `NoopLimiter`: Fallback para testes/dev sem enforcement real
//!
//! ## Configuração
//! ```toml
//! [security.resource_limits]
//! cpu_quota_us = 100000      # 100ms por período de 1s
//! memory_bytes = 536870912   # 512 MB
//! pids_max = 64
//! backend = "auto"           # auto | cgroups | docker | noop
//! ```

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Erros do ResourceLimiter.
#[derive(Debug, Error)]
pub enum ResourceLimitError {
    #[error("cgroups v2 não disponível neste sistema")]
    CgroupsNotAvailable,
    #[error("Docker não disponível: {0}")]
    DockerNotAvailable(String),
    #[error("Container não encontrado: {0}")]
    ContainerNotFound(String),
    #[error("Limite inválido: {0}")]
    InvalidLimit(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Backend não suportado nesta plataforma")]
    UnsupportedPlatform,
}

/// Limites de recursos aplicáveis a um agente/container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    /// CPU quota em microssegundos por período (default: 100ms/1s = 10% CPU).
    pub cpu_quota_us: u64,
    /// Limite de memória em bytes (default: 512 MB).
    pub memory_bytes: u64,
    /// Número máximo de processos/PIDs (default: 64).
    pub pids_max: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_quota_us: 100_000,           // 100ms per 1s period
            memory_bytes: 512 * 1024 * 1024, // 512 MB
            pids_max: 64,
        }
    }
}

impl ResourceLimits {
    /// Valida se os limites são razoáveis.
    pub fn validate(&self) -> Result<(), ResourceLimitError> {
        if self.cpu_quota_us == 0 {
            return Err(ResourceLimitError::InvalidLimit(
                "cpu_quota_us must be > 0".into(),
            ));
        }
        if self.memory_bytes < 1024 * 1024 {
            return Err(ResourceLimitError::InvalidLimit(
                "memory_bytes must be >= 1 MB".into(),
            ));
        }
        if self.pids_max == 0 {
            return Err(ResourceLimitError::InvalidLimit(
                "pids_max must be > 0".into(),
            ));
        }
        Ok(())
    }
}

/// Backend selecionado para enforcement de limites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum LimiterBackend {
    /// Auto-detect: tenta cgroups primeiro, depois Docker, depois noop.
    #[default]
    Auto,
    /// Linux cgroups v2 nativo.
    Cgroups,
    /// Docker container resource limits.
    Docker,
    /// No-op (para testes/desenvolvimento).
    Noop,
}

/// Resultado da verificação de limites.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LimitCheckResult {
    /// Se os limites estão sendo respeitados.
    pub within_limits: bool,
    /// Uso atual de CPU (se disponível).
    pub cpu_usage_us: Option<u64>,
    /// Uso atual de memória em bytes (se disponível).
    pub memory_usage_bytes: Option<u64>,
    /// Contagem atual de PIDs (se disponível).
    pub pid_count: Option<u64>,
    /// Detalhes adicionais.
    pub details: String,
}

/// Enum dispatch wrapper para backends de resource limiting (T8.3.2).
///
/// Usa enum dispatch em vez de dyn trait para evitar problemas de
/// dyn-compatibility com async methods. Cada variante delega para
/// o backend concreto correspondente.
#[derive(Debug)]
pub enum ResourceLimiter {
    Noop(NoopLimiter),
    Cgroups(CgroupsLimiter),
    Docker(DockerLimiter),
}

impl ResourceLimiter {
    /// Aplica limites a um target (container ID, cgroup path, etc.).
    pub async fn apply(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceLimitError> {
        match self {
            Self::Noop(l) => l.apply(target_id, limits).await,
            Self::Cgroups(l) => l.apply(target_id, limits).await,
            Self::Docker(l) => l.apply(target_id, limits).await,
        }
    }

    /// Verifica se os limites estão sendo respeitados.
    pub async fn verify(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<LimitCheckResult, ResourceLimitError> {
        match self {
            Self::Noop(l) => l.verify(target_id, limits).await,
            Self::Cgroups(l) => l.verify(target_id, limits).await,
            Self::Docker(l) => l.verify(target_id, limits).await,
        }
    }

    /// Remove limites aplicados a um target.
    pub async fn remove(&self, target_id: &str) -> Result<(), ResourceLimitError> {
        match self {
            Self::Noop(l) => l.remove(target_id).await,
            Self::Cgroups(l) => l.remove(target_id).await,
            Self::Docker(l) => l.remove(target_id).await,
        }
    }

    /// Nome do backend para logging/debugging.
    pub fn name(&self) -> &str {
        match self {
            Self::Noop(l) => l.name(),
            Self::Cgroups(l) => l.name(),
            Self::Docker(l) => l.name(),
        }
    }
}

// ---------------------------------------------------------------------------
// NoopLimiter — Fallback para testes/desenvolvimento
// ---------------------------------------------------------------------------

/// Backend no-op que aceita todos os limites sem enforcement real.
/// Usado em testes e quando nenhum backend real está disponível.
#[derive(Debug, Clone)]
pub struct NoopLimiter;

impl NoopLimiter {
    pub async fn apply(
        &self,
        _target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceLimitError> {
        limits.validate()?;
        tracing::debug!(
            target = _target_id,
            cpu_quota = limits.cpu_quota_us,
            memory = limits.memory_bytes,
            pids = limits.pids_max,
            "NoopLimiter: limits accepted (no enforcement)"
        );
        Ok(())
    }

    pub async fn verify(
        &self,
        _target_id: &str,
        _limits: &ResourceLimits,
    ) -> Result<LimitCheckResult, ResourceLimitError> {
        Ok(LimitCheckResult {
            within_limits: true,
            cpu_usage_us: None,
            memory_usage_bytes: None,
            pid_count: None,
            details: "NoopLimiter: no enforcement active".into(),
        })
    }

    pub async fn remove(&self, _target_id: &str) -> Result<(), ResourceLimitError> {
        Ok(())
    }

    pub fn name(&self) -> &str {
        "noop"
    }
}

// ---------------------------------------------------------------------------
// CgroupsLimiter — Linux cgroups v2
// ---------------------------------------------------------------------------

/// Backend Linux cgroups v2 para enforcement nativo de recursos.
///
/// Requer cgroups v2 montado em `/sys/fs/cgroup`.
/// Em plataformas não-Linux, `new()` retorna `Err(CgroupsNotAvailable)`.
#[derive(Debug, Clone)]
pub struct CgroupsLimiter {
    /// Path base do cgroup (ex: "/sys/fs/cgroup/zylos-agents").
    #[allow(dead_code)]
    base_path: String,
}

impl CgroupsLimiter {
    /// Cria um novo CgroupsLimiter.
    ///
    /// Retorna erro se cgroups v2 não estiver disponível.
    pub fn new(base_path: Option<&str>) -> Result<Self, ResourceLimitError> {
        let path = base_path
            .unwrap_or("/sys/fs/cgroup/zylos-agents")
            .to_string();

        // Verificar se estamos em Linux e cgroups v2 está montado
        #[cfg(target_os = "linux")]
        {
            if !std::path::Path::new(&path).exists() {
                // Tentar criar o diretório do cgroup
                std::fs::create_dir_all(&path)
                    .map_err(|_| ResourceLimitError::CgroupsNotAvailable)?;
            }
            Ok(Self { base_path: path })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = path;
            Err(ResourceLimitError::CgroupsNotAvailable)
        }
    }

    /// Retorna o path do cgroup para um target específico.
    #[allow(dead_code)]
    fn cgroup_path(&self, target_id: &str) -> String {
        format!("{}/{}", self.base_path, target_id)
    }
}

impl CgroupsLimiter {
    async fn apply(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceLimitError> {
        limits.validate()?;

        #[cfg(target_os = "linux")]
        {
            let cg_path = self.cgroup_path(target_id);
            std::fs::create_dir_all(&cg_path)?;

            // CPU quota: cpu.max = "$quota $period"
            let cpu_max = format!("{} 1000000", limits.cpu_quota_us);
            std::fs::write(format!("{}/cpu.max", cg_path), &cpu_max)?;

            // Memory limit: memory.max
            std::fs::write(
                format!("{}/memory.max", cg_path),
                limits.memory_bytes.to_string(),
            )?;

            // PID limit: pids.max
            std::fs::write(format!("{}/pids.max", cg_path), limits.pids_max.to_string())?;

            tracing::info!(
                target = target_id,
                cgroup = %cg_path,
                cpu_quota = limits.cpu_quota_us,
                memory = limits.memory_bytes,
                pids = limits.pids_max,
                "CgroupsLimiter: limits applied"
            );
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (target_id, limits);
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    async fn verify(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<LimitCheckResult, ResourceLimitError> {
        #[cfg(target_os = "linux")]
        {
            let cg_path = self.cgroup_path(target_id);

            let memory_current = std::fs::read_to_string(format!("{}/memory.current", cg_path))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());

            let pid_current = std::fs::read_to_string(format!("{}/pids.current", cg_path))
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok());

            let mem_ok = memory_current.is_none_or(|m| m <= limits.memory_bytes);
            let pid_ok = pid_current.is_none_or(|p| p <= limits.pids_max);

            Ok(LimitCheckResult {
                within_limits: mem_ok && pid_ok,
                cpu_usage_us: None, // CPU usage requires more complex accounting
                memory_usage_bytes: memory_current,
                pid_count: pid_current,
                details: format!("cgroup={}, mem_ok={}, pid_ok={}", cg_path, mem_ok, pid_ok),
            })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (target_id, limits);
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    async fn remove(&self, target_id: &str) -> Result<(), ResourceLimitError> {
        #[cfg(target_os = "linux")]
        {
            let cg_path = self.cgroup_path(target_id);
            if std::path::Path::new(&cg_path).exists() {
                std::fs::remove_dir_all(&cg_path)?;
            }
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = target_id;
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    fn name(&self) -> &str {
        "cgroups-v2"
    }
}

// ---------------------------------------------------------------------------
// DockerLimiter — Cross-platform via Docker API
// ---------------------------------------------------------------------------

/// Backend Docker para enforcement de recursos via container limits.
///
/// Funciona em qualquer plataforma com Docker instalado.
/// Usa comandos `docker update` para aplicar limites em containers existentes.
#[derive(Debug, Clone)]
pub struct DockerLimiter {
    /// Path do binário docker.
    docker_bin: String,
}

impl DockerLimiter {
    /// Cria um novo DockerLimiter.
    ///
    /// Verifica se o binário `docker` está disponível no PATH.
    pub fn new() -> Result<Self, ResourceLimitError> {
        let docker_bin = if cfg!(windows) {
            "docker.exe"
        } else {
            "docker"
        };

        // Verificar disponibilidade do docker
        let check = std::process::Command::new(docker_bin)
            .arg("--version")
            .output();

        match check {
            Ok(output) if output.status.success() => Ok(Self {
                docker_bin: docker_bin.to_string(),
            }),
            Ok(_) => Err(ResourceLimitError::DockerNotAvailable(
                "docker command failed".into(),
            )),
            Err(e) => Err(ResourceLimitError::DockerNotAvailable(e.to_string())),
        }
    }
}

impl DockerLimiter {
    async fn apply(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<(), ResourceLimitError> {
        limits.validate()?;

        // Converter cpu_quota_us para --cpus (fração de CPU)
        // cpu_quota_us / 1_000_000 = fração de CPU
        let cpus = limits.cpu_quota_us as f64 / 1_000_000.0;
        let memory_mb = limits.memory_bytes / (1024 * 1024);

        let output = tokio::process::Command::new(&self.docker_bin)
            .args([
                "update",
                "--cpus",
                &format!("{:.2}", cpus),
                "--memory",
                &format!("{}m", memory_mb),
                "--pids-limit",
                &limits.pids_max.to_string(),
                target_id,
            ])
            .output()
            .await
            .map_err(|e| ResourceLimitError::DockerNotAvailable(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such container") {
                return Err(ResourceLimitError::ContainerNotFound(target_id.into()));
            }
            return Err(ResourceLimitError::DockerNotAvailable(stderr.to_string()));
        }

        tracing::info!(
            container = target_id,
            cpus = cpus,
            memory_mb = memory_mb,
            pids = limits.pids_max,
            "DockerLimiter: limits applied"
        );
        Ok(())
    }

    async fn verify(
        &self,
        target_id: &str,
        limits: &ResourceLimits,
    ) -> Result<LimitCheckResult, ResourceLimitError> {
        // Usar docker stats para verificar uso atual
        let output = tokio::process::Command::new(&self.docker_bin)
            .args([
                "stats",
                "--no-stream",
                "--format",
                "{{.MemUsage}}|{{.PIDs}}",
                target_id,
            ])
            .output()
            .await
            .map_err(|e| ResourceLimitError::DockerNotAvailable(e.to_string()))?;

        if !output.status.success() {
            return Err(ResourceLimitError::ContainerNotFound(target_id.into()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = stdout.trim().split('|').collect();

        let memory_usage = parts.first().and_then(|mem_str| {
            // Parse "123MiB / 512MiB" → extrair primeiro valor
            mem_str.split('/').next().and_then(|v| {
                let v = v.trim();
                if v.ends_with("GiB") {
                    v.trim_end_matches("GiB")
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .map(|g| (g * 1024.0 * 1024.0 * 1024.0) as u64)
                } else if v.ends_with("MiB") {
                    v.trim_end_matches("MiB")
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .map(|m| (m * 1024.0 * 1024.0) as u64)
                } else if v.ends_with("KiB") {
                    v.trim_end_matches("KiB")
                        .trim()
                        .parse::<f64>()
                        .ok()
                        .map(|k| (k * 1024.0) as u64)
                } else {
                    None
                }
            })
        });

        let pid_count = parts.get(1).and_then(|p| p.trim().parse::<u64>().ok());

        let mem_ok = memory_usage.is_none_or(|m| m <= limits.memory_bytes);
        let pid_ok = pid_count.is_none_or(|p| p <= limits.pids_max);

        Ok(LimitCheckResult {
            within_limits: mem_ok && pid_ok,
            cpu_usage_us: None,
            memory_usage_bytes: memory_usage,
            pid_count,
            details: format!("docker container={}", target_id),
        })
    }

    async fn remove(&self, _target_id: &str) -> Result<(), ResourceLimitError> {
        // Docker não suporta "remover" limites; resetar para defaults
        // seria necessário via docker update com valores ilimitados
        tracing::debug!(
            container = _target_id,
            "DockerLimiter: remove is a no-op (reset to defaults manually)"
        );
        Ok(())
    }

    fn name(&self) -> &str {
        "docker"
    }
}

// ---------------------------------------------------------------------------
// Auto-detection factory
// ---------------------------------------------------------------------------

/// Cria o limiter apropriado baseado na configuração e plataforma.
pub fn create_limiter(backend: LimiterBackend) -> Result<ResourceLimiter, ResourceLimitError> {
    match backend {
        LimiterBackend::Noop => Ok(ResourceLimiter::Noop(NoopLimiter)),
        LimiterBackend::Cgroups => Ok(ResourceLimiter::Cgroups(CgroupsLimiter::new(None)?)),
        LimiterBackend::Docker => Ok(ResourceLimiter::Docker(DockerLimiter::new()?)),
        LimiterBackend::Auto => {
            // Tentar cgroups primeiro (Linux only)
            if let Ok(cg) = CgroupsLimiter::new(None) {
                tracing::info!("ResourceLimiter: using cgroups v2 backend");
                return Ok(ResourceLimiter::Cgroups(cg));
            }
            // Tentar Docker
            if let Ok(dk) = DockerLimiter::new() {
                tracing::info!("ResourceLimiter: using Docker backend");
                return Ok(ResourceLimiter::Docker(dk));
            }
            // Fallback para noop
            tracing::warn!("ResourceLimiter: no real backend available, using noop");
            Ok(ResourceLimiter::Noop(NoopLimiter))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_quota_us, 100_000);
        assert_eq!(limits.memory_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.pids_max, 64);
        assert!(limits.validate().is_ok());
    }

    #[test]
    fn test_resource_limits_validation() {
        let invalid_cpu = ResourceLimits {
            cpu_quota_us: 0,
            ..Default::default()
        };
        assert!(invalid_cpu.validate().is_err());

        let invalid_mem = ResourceLimits {
            memory_bytes: 100, // < 1 MB
            ..Default::default()
        };
        assert!(invalid_mem.validate().is_err());

        let invalid_pids = ResourceLimits {
            pids_max: 0,
            ..Default::default()
        };
        assert!(invalid_pids.validate().is_err());
    }

    #[tokio::test]
    async fn test_noop_limiter_apply() {
        let limiter = NoopLimiter;
        let limits = ResourceLimits::default();
        assert!(limiter.apply("test-container", &limits).await.is_ok());
    }

    #[tokio::test]
    async fn test_noop_limiter_verify() {
        let limiter = NoopLimiter;
        let limits = ResourceLimits::default();
        let result = limiter.verify("test-container", &limits).await.unwrap();
        assert!(result.within_limits);
        assert_eq!(result.details, "NoopLimiter: no enforcement active");
    }

    #[tokio::test]
    async fn test_noop_limiter_remove() {
        let limiter = NoopLimiter;
        assert!(limiter.remove("test-container").await.is_ok());
    }

    #[test]
    fn test_noop_limiter_name() {
        assert_eq!(NoopLimiter.name(), "noop");
    }

    #[tokio::test]
    async fn test_noop_rejects_invalid_limits() {
        let limiter = NoopLimiter;
        let invalid = ResourceLimits {
            cpu_quota_us: 0,
            ..Default::default()
        };
        assert!(limiter.apply("test", &invalid).await.is_err());
    }

    #[test]
    fn test_create_limiter_noop() {
        let limiter = create_limiter(LimiterBackend::Noop).unwrap();
        assert_eq!(limiter.name(), "noop");
        assert!(matches!(limiter, ResourceLimiter::Noop(_)));
    }

    #[test]
    fn test_create_limiter_auto_fallback() {
        // Em CI/test environments sem cgroups ou Docker, auto deve fallback para noop
        let limiter = create_limiter(LimiterBackend::Auto);
        // Deve sempre succeeder (fallback para noop)
        assert!(limiter.is_ok());
        let limiter = limiter.unwrap();
        // Em Windows/CI sem Docker, deve ser Noop
        assert!(matches!(
            limiter,
            ResourceLimiter::Noop(_) | ResourceLimiter::Docker(_)
        ));
    }

    #[tokio::test]
    async fn test_enum_dispatch_apply_verify_remove() {
        let limiter = create_limiter(LimiterBackend::Noop).unwrap();
        let limits = ResourceLimits::default();

        // apply via enum dispatch
        assert!(limiter.apply("test-target", &limits).await.is_ok());

        // verify via enum dispatch
        let result = limiter.verify("test-target", &limits).await.unwrap();
        assert!(result.within_limits);

        // remove via enum dispatch
        assert!(limiter.remove("test-target").await.is_ok());
    }

    #[test]
    fn test_resource_limits_serialization() {
        let limits = ResourceLimits {
            cpu_quota_us: 200_000,
            memory_bytes: 1024 * 1024 * 1024,
            pids_max: 128,
        };
        let json = serde_json::to_string(&limits).unwrap();
        let restored: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(limits, restored);
    }

    #[test]
    fn test_limit_check_result_serialization() {
        let result = LimitCheckResult {
            within_limits: true,
            cpu_usage_us: Some(50_000),
            memory_usage_bytes: Some(256 * 1024 * 1024),
            pid_count: Some(10),
            details: "all ok".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: LimitCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, restored);
    }

    #[test]
    fn test_limiter_backend_serialization() {
        for backend in [
            LimiterBackend::Auto,
            LimiterBackend::Cgroups,
            LimiterBackend::Docker,
            LimiterBackend::Noop,
        ] {
            let json = serde_json::to_string(&backend).unwrap();
            let restored: LimiterBackend = serde_json::from_str(&json).unwrap();
            assert_eq!(backend, restored);
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_cgroups_not_available_on_non_linux() {
        let result = CgroupsLimiter::new(None);
        assert!(result.is_err());
    }
}
