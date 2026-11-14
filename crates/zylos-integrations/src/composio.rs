//! Composio/App Marketplace integration (feature-flagged).
//!
//! Provides a unified adapter layer for 2000+ SaaS apps via the Composio API.
//! When the `composio` feature is disabled, all public functions return
//! `ComposioError::Disabled` so callers can fall back to direct integrations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ComposioError {
    #[error("composio integration is disabled (enable via config)")]
    Disabled,

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("app not found: {0}")]
    AppNotFound(String),

    #[error("authentication required for app: {0}")]
    AuthRequired(String),

    #[error("rate limited: retry after {0}s")]
    RateLimited(u64),

    #[error("cache expired")]
    CacheExpired,
}

pub type Result<T> = std::result::Result<T, ComposioError>;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A Composio app descriptor from the catalog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioApp {
    pub id: String,
    pub name: String,
    pub description: String,
    pub logo_url: Option<String>,
    pub categories: Vec<String>,
    pub auth_required: bool,
    pub actions_count: usize,
}

/// An action that can be executed on a Composio app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioAction {
    pub app_id: String,
    pub action_id: String,
    pub display_name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
}

/// Configuration for the Composio adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComposioConfig {
    /// Whether the Composio integration is enabled.
    pub enabled: bool,
    /// Composio API base URL.
    pub api_base: String,
    /// API key for authentication.
    pub api_key: Option<String>,
    /// Catalog cache TTL in seconds (default: 86400 = 24h).
    pub cache_ttl_secs: u64,
}

impl Default for ComposioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_base: "https://api.composio.dev/v1".to_string(),
            api_key: None,
            cache_ttl_secs: 86400,
        }
    }
}

// ---------------------------------------------------------------------------
// Cached catalog
// ---------------------------------------------------------------------------

struct CachedCatalog {
    apps: HashMap<String, ComposioApp>,
    fetched_at: Instant,
}

// ---------------------------------------------------------------------------
// ComposioAdapter
// ---------------------------------------------------------------------------

/// Adapter for the Composio app marketplace.
///
/// All methods check `config.enabled` first and return `ComposioError::Disabled`
/// when the feature is off, allowing callers to fall back gracefully.
pub struct ComposioAdapter {
    config: ComposioConfig,
    catalog: Arc<RwLock<Option<CachedCatalog>>>,
    http: reqwest::Client,
}

impl ComposioAdapter {
    /// Create a new Composio adapter with the given configuration.
    pub fn new(config: ComposioConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("valid HTTP client");

        Self {
            config,
            catalog: Arc::new(RwLock::new(None)),
            http,
        }
    }

    fn ensure_enabled(&self) -> Result<()> {
        if !self.config.enabled {
            Err(ComposioError::Disabled)
        } else {
            Ok(())
        }
    }

    /// List all available apps from the Composio catalog.
    /// Uses local cache if within TTL; otherwise fetches from API.
    pub async fn list_apps(&self) -> Result<Vec<ComposioApp>> {
        self.ensure_enabled()?;

        // Check cache
        {
            let cache = self.catalog.read().await;
            if let Some(cached) = cache.as_ref() {
                let age = cached.fetched_at.elapsed();
                if age < Duration::from_secs(self.config.cache_ttl_secs) {
                    return Ok(cached.apps.values().cloned().collect());
                }
            }
        }

        // Fetch fresh catalog
        self.refresh_catalog().await
    }

    /// Get details for a specific app.
    pub async fn get_app(&self, app_id: &str) -> Result<ComposioApp> {
        self.ensure_enabled()?;

        let apps = self.list_apps().await?;
        apps.into_iter()
            .find(|a| a.id == app_id)
            .ok_or_else(|| ComposioError::AppNotFound(app_id.to_string()))
    }

    /// Search apps by keyword (name or description match).
    pub async fn search_apps(&self, query: &str) -> Result<Vec<ComposioApp>> {
        self.ensure_enabled()?;

        let apps = self.list_apps().await?;
        let query_lower = query.to_lowercase();

        Ok(apps
            .into_iter()
            .filter(|a| {
                a.name.to_lowercase().contains(&query_lower)
                    || a.description.to_lowercase().contains(&query_lower)
                    || a.categories
                        .iter()
                        .any(|c| c.to_lowercase().contains(&query_lower))
            })
            .collect())
    }

    /// List available actions for an app.
    pub async fn list_actions(&self, app_id: &str) -> Result<Vec<ComposioAction>> {
        self.ensure_enabled()?;

        // Verify app exists
        let _app = self.get_app(app_id).await?;

        let url = format!("{}/apps/{}/actions", self.config.api_base, app_id);
        let mut req = self.http.get(&url);
        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ComposioError::Http(e.to_string()))?;

        if resp.status().as_u16() == 429 {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok())
                .unwrap_or(60);
            return Err(ComposioError::RateLimited(retry));
        }

        if !resp.status().is_success() {
            return Err(ComposioError::Http(format!(
                "status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }

        let actions: Vec<ComposioAction> = resp
            .json()
            .await
            .map_err(|e| ComposioError::Serialization(e.to_string()))?;

        Ok(actions)
    }

    /// Execute an action on a Composio app.
    ///
    /// Returns the raw JSON response from Composio. The caller is responsible
    /// for interpreting the result. Token management is delegated to zylos-vault.
    pub async fn execute_action(
        &self,
        app_id: &str,
        action_id: &str,
        params: serde_json::Value,
        connected_account_id: &str,
    ) -> Result<serde_json::Value> {
        self.ensure_enabled()?;

        let url = format!(
            "{}/apps/{}/actions/{}/execute",
            self.config.api_base, app_id, action_id
        );

        let body = serde_json::json!({
            "connected_account_id": connected_account_id,
            "input": params,
        });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ComposioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ComposioError::Http(format!(
                "execute failed: status {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            )));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ComposioError::Serialization(e.to_string()))?;

        info!(app = %app_id, action = %action_id, "Composio action executed");
        Ok(result)
    }

    /// Initiate OAuth flow for a Composio app.
    /// Returns the authorization URL the user should visit.
    pub async fn initiate_oauth(&self, app_id: &str, redirect_uri: &str) -> Result<String> {
        self.ensure_enabled()?;

        let url = format!("{}/apps/{}/auth/initiate", self.config.api_base, app_id);

        let body = serde_json::json!({
            "redirect_uri": redirect_uri,
        });

        let mut req = self.http.post(&url).json(&body);
        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ComposioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(ComposioError::Http(format!(
                "oauth initiate failed: {}",
                resp.text().await.unwrap_or_default()
            )));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ComposioError::Serialization(e.to_string()))?;

        result["auth_url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ComposioError::Serialization("missing auth_url in response".into()))
    }

    /// Check whether the adapter is enabled and configured.
    pub fn is_configured(&self) -> bool {
        self.config.enabled && self.config.api_key.is_some()
    }

    /// Force-refresh the app catalog cache.
    pub async fn refresh_catalog(&self) -> Result<Vec<ComposioApp>> {
        self.ensure_enabled()?;

        let url = format!("{}/apps", self.config.api_base);
        let mut req = self.http.get(&url);
        if let Some(key) = &self.config.api_key {
            req = req.header("x-api-key", key);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ComposioError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            warn!(status = %resp.status(), "Failed to fetch Composio catalog");
            return Err(ComposioError::Http(format!(
                "catalog fetch failed: status {}",
                resp.status()
            )));
        }

        let apps: Vec<ComposioApp> = resp
            .json()
            .await
            .map_err(|e| ComposioError::Serialization(e.to_string()))?;

        let map: HashMap<String, ComposioApp> =
            apps.iter().map(|a| (a.id.clone(), a.clone())).collect();
        let count = map.len();

        let mut cache = self.catalog.write().await;
        *cache = Some(CachedCatalog {
            apps: map,
            fetched_at: Instant::now(),
        });

        info!(count, "Composio catalog refreshed");
        Ok(apps)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn disabled_adapter() -> ComposioAdapter {
        ComposioAdapter::new(ComposioConfig::default())
    }

    fn enabled_adapter() -> ComposioAdapter {
        ComposioAdapter::new(ComposioConfig {
            enabled: true,
            api_key: Some("test-key".into()),
            ..Default::default()
        })
    }

    #[test]
    fn test_disabled_by_default() {
        let adapter = disabled_adapter();
        assert!(!adapter.is_configured());
    }

    #[test]
    fn test_enabled_with_key() {
        let adapter = enabled_adapter();
        assert!(adapter.is_configured());
    }

    #[tokio::test]
    async fn test_list_apps_when_disabled() {
        let adapter = disabled_adapter();
        let result = adapter.list_apps().await;
        assert!(matches!(result, Err(ComposioError::Disabled)));
    }

    #[tokio::test]
    async fn test_get_app_when_disabled() {
        let adapter = disabled_adapter();
        let result = adapter.get_app("slack").await;
        assert!(matches!(result, Err(ComposioError::Disabled)));
    }

    #[tokio::test]
    async fn test_search_apps_when_disabled() {
        let adapter = disabled_adapter();
        let result = adapter.search_apps("slack").await;
        assert!(matches!(result, Err(ComposioError::Disabled)));
    }

    #[tokio::test]
    async fn test_execute_action_when_disabled() {
        let adapter = disabled_adapter();
        let result = adapter
            .execute_action("slack", "send_message", serde_json::json!({}), "acct-1")
            .await;
        assert!(matches!(result, Err(ComposioError::Disabled)));
    }

    #[tokio::test]
    async fn test_initiate_oauth_when_disabled() {
        let adapter = disabled_adapter();
        let result = adapter
            .initiate_oauth("slack", "http://localhost/callback")
            .await;
        assert!(matches!(result, Err(ComposioError::Disabled)));
    }

    #[test]
    fn test_default_config_values() {
        let config = ComposioConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.api_base, "https://api.composio.dev/v1");
        assert!(config.api_key.is_none());
        assert_eq!(config.cache_ttl_secs, 86400);
    }

    #[test]
    fn test_composio_app_serialization() {
        let app = ComposioApp {
            id: "slack".into(),
            name: "Slack".into(),
            description: "Team messaging".into(),
            logo_url: Some("https://example.com/slack.png".into()),
            categories: vec!["messaging".into(), "collaboration".into()],
            auth_required: true,
            actions_count: 42,
        };

        let json = serde_json::to_string(&app).unwrap();
        let deserialized: ComposioApp = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "slack");
        assert_eq!(deserialized.actions_count, 42);
    }

    #[test]
    fn test_composio_action_serialization() {
        let action = ComposioAction {
            app_id: "slack".into(),
            action_id: "send_message".into(),
            display_name: "Send Message".into(),
            description: "Send a message to a channel".into(),
            parameters_schema: serde_json::json!({"type": "object"}),
        };

        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ComposioAction = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.action_id, "send_message");
    }
}
