//! # zylos-integrations
//!
//! SaaS OAuth integration catalog for zylos-gateway.
//! Provides encrypted token storage, auto-refresh, and declarative provider specs.

pub mod oauth;
pub mod manifest;
pub mod error;
pub mod db;
pub mod executor;
pub mod composio;
pub mod channel;

pub use error::{IntegrationError, Result};
pub use channel::{ChannelAdapter, ChannelRegistry, MessageId, OutboundMessage, SendResult};
pub use manifest::IntegrationSpec;
pub use oauth::{OAuthFlow, OAuthSession, TokenBundle};
pub use executor::ActionExecutor;

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Central registry for all SaaS integrations.
/// Loads provider specs from YAML files and manages OAuth flows.
pub struct IntegrationRegistry {
    specs: Arc<RwLock<Vec<IntegrationSpec>>>,
    oauth: OAuthFlow,
    db: db::TokenStore,
}

impl IntegrationRegistry {
    /// Create a new registry by loading all YAML specs from the given directory.
    pub async fn from_specs_dir(path: &Path) -> Result<Self> {
        let specs = manifest::load_specs(path)?;
        info!(count = specs.len(), "Loaded integration specs");

        let db = db::TokenStore::new().await?;
        let oauth = OAuthFlow::new(db.clone());

        Ok(Self {
            specs: Arc::new(RwLock::new(specs)),
            oauth,
            db,
        })
    }

    /// Get a specific integration spec by slug.
    pub async fn get(&self, slug: &str) -> Option<IntegrationSpec> {
        let specs = self.specs.read().await;
        specs.iter().find(|s| s.slug == slug).cloned()
    }

    /// List all registered integration specs.
    pub async fn list(&self) -> Vec<IntegrationSpec> {
        self.specs.read().await.clone()
    }

    /// Initiate an OAuth2 PKCE flow for the given provider.
    pub async fn initiate_oauth(
        &self,
        slug: &str,
        redirect_uri: &str,
    ) -> Result<OAuthSession> {
        let spec = self.get(slug).await
            .ok_or_else(|| IntegrationError::UnknownProvider(slug.to_string()))?;

        self.oauth.initiate(&spec, redirect_uri).await
    }

    /// Complete an OAuth flow by exchanging the authorization code for tokens.
    pub async fn complete_oauth(
        &self,
        session_id: &str,
        code: &str,
        user_id: &str,
    ) -> Result<TokenBundle> {
        let bundle = self.oauth.complete(session_id, code).await?;

        // Persist encrypted tokens
        self.db.store_tokens(user_id, &bundle).await?;
        info!(user_id, provider = %bundle.provider_slug, "OAuth tokens stored");

        Ok(bundle)
    }

    /// Execute an action against a provider using stored tokens.
    /// Automatically refreshes expired tokens before execution.
    pub async fn execute_action(
        &self,
        user_id: &str,
        slug: &str,
        action: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let spec = self.get(slug).await
            .ok_or_else(|| IntegrationError::UnknownProvider(slug.to_string()))?;

        // Get or refresh tokens
        let mut bundle = self.db.get_tokens(user_id, slug).await?
            .ok_or_else(|| IntegrationError::NoTokensStored {
                user_id: user_id.to_string(),
                provider: slug.to_string(),
            })?;

        if bundle.is_expired() {
            bundle = self.oauth.refresh(&spec, &bundle).await?;
            self.db.store_tokens(user_id, &bundle).await?;
        }

        // Find the action spec
        let action_spec = spec.actions.iter()
            .find(|a| a.id == action)
            .ok_or_else(|| IntegrationError::ProviderExecution {
                provider: slug.to_string(),
                action: action.to_string(),
                detail: format!("Unknown action: {}", action),
            })?;

        // Dispatch to provider implementation via ActionExecutor
        let executor = ActionExecutor::new(self.db.clone(), self.oauth.clone());
        let result = executor.execute(user_id, &spec, action_spec, &params).await?;

        // Audit log
        self.db.log_action(user_id, slug, action, "auto", &result).await?;

        Ok(result)
    }

    /// Revoke stored tokens for a user+provider combination.
    pub async fn revoke(&self, user_id: &str, slug: &str) -> Result<()> {
        self.db.delete_tokens(user_id, slug).await?;
        info!(user_id, provider = slug, "Tokens revoked");
        Ok(())
    }

    /// Initialize the channel registry with all available platform adapters.
    /// Reads credentials from environment variables or token store.
    pub fn init_channel_registry(&self) -> crate::channel::ChannelRegistry {
        use crate::channel::{ChannelRegistry, wechat_work::WeChatWorkAdapter, feishu::FeishuAdapter};
        
        let mut registry = ChannelRegistry::new();
        
        // WeChat Work adapter
        if let (Ok(corp_id), Ok(agent_id)) = (
            std::env::var("WECHAT_CORP_ID"),
            std::env::var("WECHAT_AGENT_ID"),
        ) {
            let adapter = WeChatWorkAdapter::new(corp_id, agent_id, None);
            registry.register(Box::new(adapter));
            tracing::info!("Registered WeChat Work channel adapter");
        }
        
        // Feishu adapter
        if let (Ok(app_id), Ok(app_secret)) = (
            std::env::var("FEISHU_APP_ID"),
            std::env::var("FEISHU_APP_SECRET"),
        ) {
            let adapter = FeishuAdapter::new(app_id, app_secret, None);
            registry.register(Box::new(adapter));
            tracing::info!("Registered Feishu channel adapter");
        }
        
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        // Basic smoke test - full integration tests in tests/ dir
        let dir = tempfile::tempdir().unwrap();
        let registry = IntegrationRegistry::from_specs_dir(dir.path()).await;
        assert!(registry.is_ok());
    }
}