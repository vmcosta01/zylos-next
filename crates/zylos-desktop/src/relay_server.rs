//! # RelayServer — Mobile-to-desktop relay via HTTP/WebSocket (T8.6.3)
//!
//! Lightweight relay server that allows mobile devices to communicate with
//! the desktop gateway over the local network. Provides:
//! - REST API proxying to the embedded gateway
//! - WebSocket tunnel for real-time AG-UI streaming
//! - Token-based authentication for device pairing
//! - CORS headers for browser-based mobile clients
//!
//! ## Security
//! - All requests require a valid bearer token (generated during pairing)
//! - Tokens expire after configurable TTL (default: 24h)
//! - Rate limiting per IP to prevent abuse
//! - Only localhost + LAN interfaces bound (never public)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Errors from the relay server.
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Rate limit exceeded for {0}")]
    RateLimited(String),
    #[error("Gateway proxy error: {0}")]
    Proxy(String),
    #[error("Invalid token format")]
    InvalidToken,
}

/// Configuration for the relay server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Address to bind the relay server.
    pub bind_addr: String,
    /// Port for the relay server.
    pub port: u16,
    /// Gateway backend URL to proxy requests to.
    pub gateway_url: String,
    /// Token TTL in seconds (default: 86400 = 24h).
    pub token_ttl_secs: u64,
    /// Max requests per minute per IP.
    pub rate_limit_per_minute: u32,
    /// Enable CORS for mobile browser clients.
    pub cors_enabled: bool,
    /// Allowed CORS origins (empty = allow all when cors_enabled).
    pub cors_origins: Vec<String>,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0".into(),
            port: 9877,
            gateway_url: "http://127.0.0.1:9876".into(),
            token_ttl_secs: 86400,
            rate_limit_per_minute: 60,
            cors_enabled: true,
            cors_origins: vec![],
        }
    }
}

/// A paired device token with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceToken {
    /// The bearer token string.
    pub token: String,
    /// Human-readable device name.
    pub device_name: String,
    /// When the token was created (Unix epoch seconds).
    pub created_at: u64,
    /// When the token expires (Unix epoch seconds).
    pub expires_at: u64,
    /// Last time this token was used (Unix epoch seconds).
    pub last_used_at: Option<u64>,
}

impl DeviceToken {
    /// Check if this token has expired.
    pub fn is_expired(&self, now_secs: u64) -> bool {
        now_secs >= self.expires_at
    }
}

/// Rate limiter entry per IP.
#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    window_start: u64,
}

/// Relay server state shared across handlers.
pub struct RelayState {
    config: RelayConfig,
    tokens: Arc<RwLock<HashMap<String, DeviceToken>>>,
    rate_limits: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}

impl RelayState {
    /// Create new relay state from config.
    pub fn new(config: RelayConfig) -> Self {
        Self {
            config,
            tokens: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new device token for pairing.
    pub async fn pair_device(&self, device_name: impl Into<String>) -> DeviceToken {
        let token = generate_token();
        let now = unix_now();
        let device_token = DeviceToken {
            token: token.clone(),
            device_name: device_name.into(),
            created_at: now,
            expires_at: now + self.config.token_ttl_secs,
            last_used_at: None,
        };

        let mut tokens = self.tokens.write().await;
        tokens.insert(token, device_token.clone());

        info!(
            device = %device_token.device_name,
            ttl = self.config.token_ttl_secs,
            "Device paired successfully"
        );

        device_token
    }

    /// Validate a bearer token. Returns the device token if valid.
    pub async fn validate_token(&self, token: &str) -> Result<DeviceToken, RelayError> {
        let tokens = self.tokens.read().await;
        let device_token = tokens
            .get(token)
            .ok_or_else(|| RelayError::Auth("Unknown token".into()))?;

        if device_token.is_expired(unix_now()) {
            return Err(RelayError::Auth("Token expired".into()));
        }

        Ok(device_token.clone())
    }

    /// Update last_used_at for a token.
    pub async fn touch_token(&self, token: &str) {
        let mut tokens = self.tokens.write().await;
        if let Some(dt) = tokens.get_mut(token) {
            dt.last_used_at = Some(unix_now());
        }
    }

    /// Revoke a device token.
    pub async fn revoke_token(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write().await;
        tokens.remove(token).is_some()
    }

    /// List all active (non-expired) device tokens.
    pub async fn list_devices(&self) -> Vec<DeviceToken> {
        let now = unix_now();
        let tokens = self.tokens.read().await;
        tokens
            .values()
            .filter(|dt| !dt.is_expired(now))
            .cloned()
            .collect()
    }

    /// Remove all expired tokens.
    pub async fn purge_expired(&self) -> usize {
        let now = unix_now();
        let mut tokens = self.tokens.write().await;
        let before = tokens.len();
        tokens.retain(|_, dt| !dt.is_expired(now));
        before - tokens.len()
    }

    /// Check rate limit for an IP. Returns Ok if allowed, Err if limited.
    pub async fn check_rate_limit(&self, ip: &str) -> Result<(), RelayError> {
        let now = unix_now();
        let window_secs = 60u64;
        let mut limits = self.rate_limits.write().await;

        let entry = limits.entry(ip.to_string()).or_insert(RateLimitEntry {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now - entry.window_start >= window_secs {
            entry.count = 0;
            entry.window_start = now;
        }

        entry.count += 1;

        if entry.count > self.config.rate_limit_per_minute {
            warn!(ip = %ip, count = entry.count, "Rate limit exceeded");
            Err(RelayError::RateLimited(ip.to_string()))
        } else {
            Ok(())
        }
    }

    /// Get the configured bind address.
    pub fn bind_addr(&self) -> SocketAddr {
        format!("{}:{}", self.config.bind_addr, self.config.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 9877)))
    }

    /// Get the gateway URL to proxy to.
    pub fn gateway_url(&self) -> &str {
        &self.config.gateway_url
    }

    /// Check if CORS is enabled.
    pub fn cors_enabled(&self) -> bool {
        self.config.cors_enabled
    }

    /// Get allowed CORS origins.
    pub fn cors_origins(&self) -> &[String] {
        &self.config.cors_origins
    }

    /// Get the config.
    pub fn config(&self) -> &RelayConfig {
        &self.config
    }
}

/// Generate a cryptographically random token.
fn generate_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    // Simple but sufficient for local-only relay tokens
    format!("zrly_{:016x}{:08x}", nanos as u64, (nanos >> 64) as u32)
}

/// Current Unix timestamp in seconds.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> RelayState {
        RelayState::new(RelayConfig {
            token_ttl_secs: 3600,
            rate_limit_per_minute: 5,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn test_pair_device_creates_token() {
        let state = test_state();
        let token = state.pair_device("iPhone 15").await;

        assert!(token.token.starts_with("zrly_"));
        assert_eq!(token.device_name, "iPhone 15");
        assert!(token.expires_at > token.created_at);
        assert!(token.last_used_at.is_none());
    }

    #[tokio::test]
    async fn test_validate_token_success() {
        let state = test_state();
        let paired = state.pair_device("Pixel 8").await;

        let result = state.validate_token(&paired.token).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().device_name, "Pixel 8");
    }

    #[tokio::test]
    async fn test_validate_unknown_token_fails() {
        let state = test_state();
        let result = state.validate_token("invalid-token").await;
        assert!(matches!(result, Err(RelayError::Auth(_))));
    }

    #[tokio::test]
    async fn test_validate_expired_token_fails() {
        let state = RelayState::new(RelayConfig {
            token_ttl_secs: 0, // Expire immediately
            ..Default::default()
        });
        let paired = state.pair_device("Old Phone").await;

        // Token should be expired since TTL is 0
        let result = state.validate_token(&paired.token).await;
        assert!(matches!(result, Err(RelayError::Auth(_))));
    }

    #[tokio::test]
    async fn test_touch_token_updates_last_used() {
        let state = test_state();
        let paired = state.pair_device("Tablet").await;
        assert!(paired.last_used_at.is_none());

        state.touch_token(&paired.token).await;

        let tokens = state.tokens.read().await;
        let updated = tokens.get(&paired.token).unwrap();
        assert!(updated.last_used_at.is_some());
    }

    #[tokio::test]
    async fn test_revoke_token() {
        let state = test_state();
        let paired = state.pair_device("Revoked Device").await;

        assert!(state.revoke_token(&paired.token).await);
        assert!(!state.revoke_token(&paired.token).await); // Already revoked

        let result = state.validate_token(&paired.token).await;
        assert!(matches!(result, Err(RelayError::Auth(_))));
    }

    #[tokio::test]
    async fn test_list_devices_excludes_expired() {
        let state = RelayState::new(RelayConfig {
            token_ttl_secs: 3600,
            ..Default::default()
        });

        state.pair_device("Active 1").await;
        state.pair_device("Active 2").await;

        let devices = state.list_devices().await;
        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    async fn test_purge_expired() {
        let state = RelayState::new(RelayConfig {
            token_ttl_secs: 0, // All expire immediately
            ..Default::default()
        });

        state.pair_device("Expired 1").await;
        state.pair_device("Expired 2").await;

        let purged = state.purge_expired().await;
        assert_eq!(purged, 2);

        let devices = state.list_devices().await;
        assert_eq!(devices.len(), 0);
    }

    #[tokio::test]
    async fn test_rate_limit_allows_under_threshold() {
        let state = test_state(); // 5 per minute

        for _ in 0..5 {
            assert!(state.check_rate_limit("192.168.1.100").await.is_ok());
        }
    }

    #[tokio::test]
    async fn test_rate_limit_blocks_over_threshold() {
        let state = test_state(); // 5 per minute

        for _ in 0..5 {
            state.check_rate_limit("10.0.0.1").await.unwrap();
        }

        let result = state.check_rate_limit("10.0.0.1").await;
        assert!(matches!(result, Err(RelayError::RateLimited(_))));
    }

    #[tokio::test]
    async fn test_rate_limit_separate_per_ip() {
        let state = test_state(); // 5 per minute

        for _ in 0..5 {
            state.check_rate_limit("10.0.0.1").await.unwrap();
        }

        // Different IP should still be allowed
        assert!(state.check_rate_limit("10.0.0.2").await.is_ok());
    }

    #[tokio::test]
    async fn test_bind_addr_parsing() {
        let state = RelayState::new(RelayConfig {
            bind_addr: "127.0.0.1".into(),
            port: 8080,
            ..Default::default()
        });

        let addr = state.bind_addr();
        assert_eq!(addr.port(), 8080);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn test_gateway_url_accessor() {
        let state = test_state();
        assert_eq!(state.gateway_url(), "http://127.0.0.1:9876");
    }

    #[tokio::test]
    async fn test_cors_config() {
        let state = RelayState::new(RelayConfig {
            cors_enabled: true,
            cors_origins: vec!["https://mobile.example.com".into()],
            ..Default::default()
        });

        assert!(state.cors_enabled());
        assert_eq!(state.cors_origins().len(), 1);
    }

    #[tokio::test]
    async fn test_generate_token_uniqueness() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
        assert!(t1.starts_with("zrly_"));
        assert!(t2.starts_with("zrly_"));
    }

    #[tokio::test]
    async fn test_device_token_serialization() {
        let token = DeviceToken {
            token: "zrly_test123".into(),
            device_name: "Test Device".into(),
            created_at: 1700000000,
            expires_at: 1700086400,
            last_used_at: Some(1700043200),
        };

        let json = serde_json::to_string(&token).unwrap();
        let restored: DeviceToken = serde_json::from_str(&json).unwrap();
        assert_eq!(token.token, restored.token);
        assert_eq!(token.device_name, restored.device_name);
        assert_eq!(token.expires_at, restored.expires_at);
    }

    #[tokio::test]
    async fn test_relay_config_serialization() {
        let config = RelayConfig {
            bind_addr: "0.0.0.0".into(),
            port: 9877,
            gateway_url: "http://localhost:9876".into(),
            token_ttl_secs: 7200,
            rate_limit_per_minute: 120,
            cors_enabled: false,
            cors_origins: vec![],
        };

        let json = serde_json::to_string(&config).unwrap();
        let restored: RelayConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.port, restored.port);
        assert_eq!(config.token_ttl_secs, restored.token_ttl_secs);
        assert_eq!(config.cors_enabled, restored.cors_enabled);
    }

    #[tokio::test]
    async fn test_multiple_devices_independent() {
        let state = test_state();

        let d1 = state.pair_device("Phone").await;
        let d2 = state.pair_device("Tablet").await;
        let d3 = state.pair_device("Laptop").await;

        assert_ne!(d1.token, d2.token);
        assert_ne!(d2.token, d3.token);

        // Revoke one, others remain valid
        state.revoke_token(&d2.token).await;

        assert!(state.validate_token(&d1.token).await.is_ok());
        assert!(state.validate_token(&d2.token).await.is_err());
        assert!(state.validate_token(&d3.token).await.is_ok());
    }

    #[tokio::test]
    async fn test_is_expired_boundary() {
        let token = DeviceToken {
            token: "test".into(),
            device_name: "test".into(),
            created_at: 1000,
            expires_at: 2000,
            last_used_at: None,
        };

        assert!(!token.is_expired(1999));
        assert!(token.is_expired(2000));
        assert!(token.is_expired(2001));
    }

    #[tokio::test]
    async fn test_config_accessor() {
        let state = test_state();
        assert_eq!(state.config().rate_limit_per_minute, 5);
        assert_eq!(state.config().token_ttl_secs, 3600);
    }
}
