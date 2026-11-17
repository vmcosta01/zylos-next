//! # EmployeeRateLimiter — Rate limiting por employee persona (T8.5.4)
//!
//! Controla a taxa de execução de rotinas por employee, prevenindo
//! abuso de recursos e respeitando limites operacionais.
//!
//! ## Mecanismo
//! Token bucket algorithm com refill periódico. Cada employee tem seu
//! próprio bucket independente. Limites configuráveis por role ou individualmente.
//!
//! ## Configuração
//! ```toml
//! [rate_limits]
//! developer.requests_per_minute = 30
//! researcher.requests_per_minute = 20
//! default.requests_per_minute = 10
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::roles::EmployeeRole;

/// Erros do rate limiter.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit excedido: {used}/{limit} em {window_secs}s")]
    LimitExceeded {
        used: u64,
        limit: u64,
        window_secs: u64,
    },
    #[error("Configuração inválida: {0}")]
    InvalidConfig(String),
}

/// Configuração de rate limit para um role ou employee específico.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitConfig {
    /// Máximo de requisições permitidas na janela de tempo.
    pub max_requests: u64,
    /// Janela de tempo em segundos.
    pub window_secs: u64,
    /// Burst adicional permitido acima do rate sustentável.
    pub burst_allowance: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 10,
            window_secs: 60, // 1 minuto
            burst_allowance: 5,
        }
    }
}

impl RateLimitConfig {
    /// Cria config com requests por minuto.
    pub fn per_minute(max_requests: u64) -> Self {
        Self {
            max_requests,
            window_secs: 60,
            burst_allowance: max_requests / 2,
        }
    }

    /// Cria config com requests por hora.
    pub fn per_hour(max_requests: u64) -> Self {
        Self {
            max_requests,
            window_secs: 3600,
            burst_allowance: max_requests / 4,
        }
    }

    /// Valida a configuração.
    pub fn validate(&self) -> Result<(), RateLimitError> {
        if self.max_requests == 0 {
            return Err(RateLimitError::InvalidConfig(
                "max_requests must be > 0".into(),
            ));
        }
        if self.window_secs == 0 {
            return Err(RateLimitError::InvalidConfig(
                "window_secs must be > 0".into(),
            ));
        }
        Ok(())
    }
}

/// Estado interno de um token bucket.
#[derive(Debug, Clone)]
struct TokenBucket {
    /// Tokens disponíveis.
    tokens: f64,
    /// Capacidade máxima do bucket.
    capacity: f64,
    /// Taxa de refill (tokens por segundo).
    refill_rate: f64,
    /// Último momento de refill.
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let capacity = (config.max_requests + config.burst_allowance) as f64;
        let refill_rate = config.max_requests as f64 / config.window_secs as f64;
        Self {
            tokens: capacity,
            capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Recarrega tokens baseado no tempo decorrido.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;
        self.tokens = (self.tokens + new_tokens).min(self.capacity);
        self.last_refill = now;
    }

    /// Tenta consumir um token. Retorna true se permitido.
    fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Retorna fração de capacidade restante (0.0-1.0).
    fn remaining_fraction(&mut self) -> f64 {
        self.refill();
        self.tokens / self.capacity
    }
}

/// Status atual do rate limit para um employee.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimitStatus {
    /// Employee ID ou role.
    pub identity: String,
    /// Tokens restantes.
    pub tokens_remaining: f64,
    /// Capacidade total.
    pub capacity: f64,
    /// Fração restante (0.0-1.0).
    pub remaining_fraction: f64,
    /// Se está atualmente limitado.
    pub is_limited: bool,
    /// Configuração aplicada.
    pub config: RateLimitConfig,
}

/// Rate limiter para AI Employees com suporte a limites por role e individuais.
pub struct EmployeeRateLimiter {
    /// Configurações por role.
    role_configs: HashMap<EmployeeRole, RateLimitConfig>,
    /// Configuração default para roles sem config específica.
    default_config: RateLimitConfig,
    /// Buckets por employee_id.
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    /// Overrides individuais por employee_id.
    individual_overrides: Arc<RwLock<HashMap<String, RateLimitConfig>>>,
}

impl Default for EmployeeRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl EmployeeRateLimiter {
    /// Cria um rate limiter com configs padrão por role.
    pub fn new() -> Self {
        let mut role_configs = HashMap::new();
        role_configs.insert(EmployeeRole::Developer, RateLimitConfig::per_minute(30));
        role_configs.insert(EmployeeRole::Researcher, RateLimitConfig::per_minute(20));
        role_configs.insert(EmployeeRole::DevOps, RateLimitConfig::per_minute(15));
        role_configs.insert(
            EmployeeRole::SecurityAuditor,
            RateLimitConfig::per_minute(10),
        );
        role_configs.insert(EmployeeRole::Designer, RateLimitConfig::per_minute(20));
        role_configs.insert(
            EmployeeRole::ProjectManager,
            RateLimitConfig::per_minute(25),
        );
        role_configs.insert(EmployeeRole::SupportAgent, RateLimitConfig::per_minute(40));
        role_configs.insert(
            EmployeeRole::ContentCreator,
            RateLimitConfig::per_minute(15),
        );

        Self {
            role_configs,
            default_config: RateLimitConfig::per_minute(10),
            buckets: Arc::new(RwLock::new(HashMap::new())),
            individual_overrides: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Define limite para um role específico.
    pub fn set_role_limit(&mut self, role: EmployeeRole, config: RateLimitConfig) {
        self.role_configs.insert(role, config);
    }

    /// Define override individual para um employee específico.
    pub async fn set_individual_limit(
        &self,
        employee_id: impl Into<String>,
        config: RateLimitConfig,
    ) {
        let id = employee_id.into();
        let mut overrides = self.individual_overrides.write().await;
        overrides.insert(id.clone(), config.clone());

        // Resetar bucket existente para aplicar nova config
        let mut buckets = self.buckets.write().await;
        buckets.insert(id, TokenBucket::new(&config));
    }

    /// Verifica e consome um token para um employee.
    ///
    /// Retorna Ok(()) se permitido, Err(LimitExceeded) se bloqueado.
    pub async fn check_and_consume(
        &self,
        employee_id: &str,
        role: &EmployeeRole,
    ) -> Result<(), RateLimitError> {
        let config = self.get_effective_config(employee_id, role).await;

        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(employee_id.to_string())
            .or_insert_with(|| TokenBucket::new(&config));

        if bucket.try_consume() {
            Ok(())
        } else {
            Err(RateLimitError::LimitExceeded {
                used: config.max_requests,
                limit: config.max_requests,
                window_secs: config.window_secs,
            })
        }
    }

    /// Verifica status atual sem consumir token.
    pub async fn status(&self, employee_id: &str, role: &EmployeeRole) -> RateLimitStatus {
        let config = self.get_effective_config(employee_id, role).await;

        let mut buckets = self.buckets.write().await;
        let bucket = buckets
            .entry(employee_id.to_string())
            .or_insert_with(|| TokenBucket::new(&config));

        let remaining = bucket.remaining_fraction();
        RateLimitStatus {
            identity: employee_id.to_string(),
            tokens_remaining: bucket.tokens,
            capacity: bucket.capacity,
            remaining_fraction: remaining,
            is_limited: remaining < 0.1,
            config,
        }
    }

    /// Reseta o bucket de um employee específico.
    pub async fn reset(&self, employee_id: &str) {
        let mut buckets = self.buckets.write().await;
        buckets.remove(employee_id);
    }

    /// Reseta todos os buckets.
    pub async fn reset_all(&self) {
        let mut buckets = self.buckets.write().await;
        buckets.clear();
    }

    /// Retorna a config efetiva para um employee (override > role > default).
    async fn get_effective_config(
        &self,
        employee_id: &str,
        role: &EmployeeRole,
    ) -> RateLimitConfig {
        // Check individual override first
        let overrides = self.individual_overrides.read().await;
        if let Some(config) = overrides.get(employee_id) {
            return config.clone();
        }
        drop(overrides);

        // Then role config
        self.role_configs
            .get(role)
            .cloned()
            .unwrap_or_else(|| self.default_config.clone())
    }

    /// Retorna estatísticas globais do rate limiter.
    pub async fn stats(&self) -> RateLimiterStats {
        let buckets = self.buckets.read().await;
        let overrides = self.individual_overrides.read().await;

        let limited_count = buckets
            .values()
            .filter(|b| {
                // Calcular fração restante sem mutabilidade:
                // tokens / capacity (sem refill, apenas snapshot atual)
                b.tokens / b.capacity < 0.1
            })
            .count();

        RateLimiterStats {
            active_buckets: buckets.len(),
            individual_overrides: overrides.len(),
            currently_limited: limited_count,
        }
    }
}

/// Estatísticas globais do rate limiter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RateLimiterStats {
    pub active_buckets: usize,
    pub individual_overrides: usize,
    pub currently_limited: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_config_default() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests, 10);
        assert_eq!(config.window_secs, 60);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_rate_limit_config_per_minute() {
        let config = RateLimitConfig::per_minute(30);
        assert_eq!(config.max_requests, 30);
        assert_eq!(config.window_secs, 60);
        assert_eq!(config.burst_allowance, 15);
    }

    #[test]
    fn test_rate_limit_config_per_hour() {
        let config = RateLimitConfig::per_hour(100);
        assert_eq!(config.max_requests, 100);
        assert_eq!(config.window_secs, 3600);
        assert_eq!(config.burst_allowance, 25);
    }

    #[test]
    fn test_rate_limit_config_validation() {
        let invalid_zero_requests = RateLimitConfig {
            max_requests: 0,
            window_secs: 60,
            burst_allowance: 0,
        };
        assert!(invalid_zero_requests.validate().is_err());

        let invalid_zero_window = RateLimitConfig {
            max_requests: 10,
            window_secs: 0,
            burst_allowance: 0,
        };
        assert!(invalid_zero_window.validate().is_err());
    }

    #[tokio::test]
    async fn test_check_and_consume_within_limit() {
        let limiter = EmployeeRateLimiter::new();
        let role = EmployeeRole::Developer; // 30/min

        // Primeiras requisições devem passar
        for _ in 0..10 {
            assert!(limiter.check_and_consume("dev-1", &role).await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_check_and_consume_exceeds_limit() {
        let mut limiter = EmployeeRateLimiter::new();
        // Configurar limite baixo para teste
        limiter.set_role_limit(
            EmployeeRole::Developer,
            RateLimitConfig {
                max_requests: 3,
                window_secs: 60,
                burst_allowance: 0,
            },
        );

        let role = EmployeeRole::Developer;

        // 3 requisições devem passar
        assert!(limiter
            .check_and_consume("dev-limited", &role)
            .await
            .is_ok());
        assert!(limiter
            .check_and_consume("dev-limited", &role)
            .await
            .is_ok());
        assert!(limiter
            .check_and_consume("dev-limited", &role)
            .await
            .is_ok());

        // 4ª deve falhar
        let result = limiter.check_and_consume("dev-limited", &role).await;
        assert!(matches!(result, Err(RateLimitError::LimitExceeded { .. })));
    }

    #[tokio::test]
    async fn test_different_employees_independent() {
        let mut limiter = EmployeeRateLimiter::new();
        limiter.set_role_limit(
            EmployeeRole::Developer,
            RateLimitConfig {
                max_requests: 2,
                window_secs: 60,
                burst_allowance: 0,
            },
        );

        let role = EmployeeRole::Developer;

        // dev-1 esgota seu limite
        assert!(limiter.check_and_consume("dev-1", &role).await.is_ok());
        assert!(limiter.check_and_consume("dev-1", &role).await.is_ok());
        assert!(limiter.check_and_consume("dev-1", &role).await.is_err());

        // dev-2 ainda tem tokens
        assert!(limiter.check_and_consume("dev-2", &role).await.is_ok());
    }

    #[tokio::test]
    async fn test_individual_override() {
        let limiter = EmployeeRateLimiter::new();
        let role = EmployeeRole::Developer;

        // Override individual com limite muito baixo
        limiter
            .set_individual_limit(
                "special-dev",
                RateLimitConfig {
                    max_requests: 1,
                    window_secs: 60,
                    burst_allowance: 0,
                },
            )
            .await;

        assert!(limiter
            .check_and_consume("special-dev", &role)
            .await
            .is_ok());
        assert!(limiter
            .check_and_consume("special-dev", &role)
            .await
            .is_err());

        // Outro dev usa limite do role (30/min)
        assert!(limiter.check_and_consume("normal-dev", &role).await.is_ok());
    }

    #[tokio::test]
    async fn test_status_reporting() {
        let limiter = EmployeeRateLimiter::new();
        let role = EmployeeRole::Researcher;

        let status = limiter.status("researcher-1", &role).await;
        assert_eq!(status.identity, "researcher-1");
        assert!(!status.is_limited);
        assert!(status.remaining_fraction > 0.5);
        assert_eq!(status.config.max_requests, 20);
    }

    #[tokio::test]
    async fn test_reset_employee() {
        let mut limiter = EmployeeRateLimiter::new();
        limiter.set_role_limit(
            EmployeeRole::Developer,
            RateLimitConfig {
                max_requests: 2,
                window_secs: 60,
                burst_allowance: 0,
            },
        );

        let role = EmployeeRole::Developer;

        // Esgotar limite
        limiter
            .check_and_consume("reset-test", &role)
            .await
            .unwrap();
        limiter
            .check_and_consume("reset-test", &role)
            .await
            .unwrap();
        assert!(limiter
            .check_and_consume("reset-test", &role)
            .await
            .is_err());

        // Resetar
        limiter.reset("reset-test").await;

        // Deve funcionar novamente
        assert!(limiter.check_and_consume("reset-test", &role).await.is_ok());
    }

    #[tokio::test]
    async fn test_reset_all() {
        let limiter = EmployeeRateLimiter::new();
        let role = EmployeeRole::Developer;

        limiter.check_and_consume("a", &role).await.unwrap();
        limiter.check_and_consume("b", &role).await.unwrap();

        limiter.reset_all().await;

        let stats = limiter.stats().await;
        assert_eq!(stats.active_buckets, 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let limiter = EmployeeRateLimiter::new();
        let role = EmployeeRole::Developer;

        limiter.check_and_consume("s1", &role).await.unwrap();
        limiter.check_and_consume("s2", &role).await.unwrap();

        limiter
            .set_individual_limit("s3", RateLimitConfig::per_minute(5))
            .await;

        let stats = limiter.stats().await;
        assert_eq!(stats.active_buckets, 3); // s1, s2, s3
        assert_eq!(stats.individual_overrides, 1); // s3
    }

    #[tokio::test]
    async fn test_burst_allowance() {
        let mut limiter = EmployeeRateLimiter::new();
        limiter.set_role_limit(
            EmployeeRole::DevOps,
            RateLimitConfig {
                max_requests: 5,
                window_secs: 60,
                burst_allowance: 5, // Total capacity = 10
            },
        );

        let role = EmployeeRole::DevOps;

        // Deve permitir até capacity (max + burst = 10)
        for i in 0..10 {
            let result = limiter.check_and_consume("burst-test", &role).await;
            assert!(
                result.is_ok(),
                "Request {} should succeed within burst capacity",
                i + 1
            );
        }

        // 11ª deve falhar
        assert!(limiter
            .check_and_consume("burst-test", &role)
            .await
            .is_err());
    }

    #[test]
    fn test_rate_limit_error_display() {
        let err = RateLimitError::LimitExceeded {
            used: 30,
            limit: 30,
            window_secs: 60,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("30"));
        assert!(msg.contains("60"));
    }

    #[test]
    fn test_rate_limit_config_serialization() {
        let config = RateLimitConfig::per_minute(25);
        let json = serde_json::to_string(&config).unwrap();
        let restored: RateLimitConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, restored);
    }

    #[test]
    fn test_rate_limit_status_serialization() {
        let status = RateLimitStatus {
            identity: "emp-1".into(),
            tokens_remaining: 15.0,
            capacity: 30.0,
            remaining_fraction: 0.5,
            is_limited: false,
            config: RateLimitConfig::per_minute(30),
        };
        let json = serde_json::to_string(&status).unwrap();
        let restored: RateLimitStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, restored);
    }

    #[test]
    fn test_rate_limiter_stats_serialization() {
        let stats = RateLimiterStats {
            active_buckets: 5,
            individual_overrides: 2,
            currently_limited: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let restored: RateLimiterStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }
}
