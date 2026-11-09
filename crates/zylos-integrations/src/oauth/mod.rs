//! OAuth2 PKCE flow engine with encrypted token persistence.

use crate::db::TokenStore;
use crate::error::{IntegrationError, Result};
use crate::manifest::IntegrationSpec;
use chrono::{DateTime, Utc};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Represents a complete OAuth token bundle with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBundle {
    pub provider_slug: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub scopes: Vec<String>,
    pub token_type: String,
}

impl TokenBundle {
    /// Check if the access token has expired (with 60s buffer).
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at - chrono::Duration::seconds(60)
    }
}

/// An active OAuth session awaiting code exchange.
#[derive(Debug, Clone)]
pub struct OAuthSession {
    pub session_id: String,
    pub authorization_url: String,
    pub csrf_token: String,
    pub pkce_verifier: String,
    pub provider_slug: String,
}

/// Manages OAuth2 PKCE flows for all registered providers.
#[derive(Clone)]
#[allow(dead_code)]
pub struct OAuthFlow {
    db: TokenStore,
    pending_sessions: Arc<RwLock<std::collections::HashMap<String, PendingSession>>>,
}

#[derive(Debug, Clone)]
struct PendingSession {
    pkce_verifier: String,
    client: BasicClient,
    provider_slug: String,
    created_at: DateTime<Utc>,
}

impl OAuthFlow {
    pub fn new(db: TokenStore) -> Self {
        Self {
            db,
            pending_sessions: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Initiate an OAuth2 PKCE authorization flow.
    pub async fn initiate(&self, spec: &IntegrationSpec, _redirect_uri: &str) -> Result<OAuthSession> {
        let client = build_client(spec)?;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        let mut auth_request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        for scope in &spec.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_token) = auth_request.url();

        let session_id = Uuid::new_v4().to_string();
        let pending = PendingSession {
            pkce_verifier: pkce_verifier.secret().to_string(),
            client,
            provider_slug: spec.slug.clone(),
            created_at: Utc::now(),
        };

        {
            let mut sessions = self.pending_sessions.write().await;
            sessions.insert(session_id.clone(), pending);
        }

        // Clean up stale sessions (>1 hour old)
        self.cleanup_stale_sessions().await;

        Ok(OAuthSession {
            session_id,
            authorization_url: auth_url.to_string(),
            csrf_token: csrf_token.secret().clone(),
            pkce_verifier: String::new(), // Never expose verifier to caller
            provider_slug: spec.slug.clone(),
        })
    }

    /// Complete the OAuth flow by exchanging the authorization code for tokens.
    pub async fn complete(&self, session_id: &str, code: &str) -> Result<TokenBundle> {
        let pending = {
            let mut sessions = self.pending_sessions.write().await;
            sessions.remove(session_id)
        }
        .ok_or_else(|| IntegrationError::OAuth(format!("Unknown or expired session: {}", session_id)))?;

        let pkce_verifier = PkceCodeVerifier::new(pending.pkce_verifier);
        let token_result = pending
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .set_pkce_verifier(pkce_verifier)
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| IntegrationError::OAuth(e.to_string()))?;

        let expires_at = token_result
            .expires_in()
            .map(|d| Utc::now() + chrono::Duration::from_std(d).unwrap_or_default())
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        let scopes = token_result
            .scopes()
            .map(|ss| ss.iter().map(|s| s.as_str().to_string()).collect())
            .unwrap_or_default();

        Ok(TokenBundle {
            provider_slug: pending.provider_slug,
            access_token: token_result.access_token().secret().clone(),
            refresh_token: token_result.refresh_token().map(|t| t.secret().clone()),
            expires_at,
            scopes,
            token_type: token_result.token_type().as_ref().to_string(),
        })
    }

    /// Refresh an expired token bundle using its refresh token.
    pub async fn refresh(&self, spec: &IntegrationSpec, bundle: &TokenBundle) -> Result<TokenBundle> {
        let refresh_token = bundle
            .refresh_token
            .as_ref()
            .ok_or_else(|| IntegrationError::TokenRefresh("No refresh token available".into()))?;

        let client = build_client(spec)?;
        let token_result = client
            .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
            .request_async(oauth2::reqwest::async_http_client)
            .await
            .map_err(|e| IntegrationError::TokenRefresh(e.to_string()))?;

        let expires_at = token_result
            .expires_in()
            .map(|d| Utc::now() + chrono::Duration::from_std(d).unwrap_or_default())
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        let scopes = token_result
            .scopes()
            .map(|ss| ss.iter().map(|s| s.as_str().to_string()).collect())
            .unwrap_or_else(|| bundle.scopes.clone());

        Ok(TokenBundle {
            provider_slug: bundle.provider_slug.clone(),
            access_token: token_result.access_token().secret().clone(),
            refresh_token: token_result
                .refresh_token()
                .map(|t| t.secret().clone())
                .or_else(|| bundle.refresh_token.clone()),
            expires_at,
            scopes,
            token_type: token_result.token_type().as_ref().to_string(),
        })
    }

    async fn cleanup_stale_sessions(&self) {
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let mut sessions = self.pending_sessions.write().await;
        sessions.retain(|_, s| s.created_at > cutoff);
    }
}

fn build_client(spec: &IntegrationSpec) -> Result<BasicClient> {
    let client = BasicClient::new(
        ClientId::new(spec.client_id.clone()),
        Some(ClientSecret::new(spec.client_secret.clone())),
        AuthUrl::new(spec.auth_url.clone())
            .map_err(|e| IntegrationError::OAuth(format!("Invalid auth URL: {}", e)))?,
        Some(
            TokenUrl::new(spec.token_url.clone())
                .map_err(|e| IntegrationError::OAuth(format!("Invalid token URL: {}", e)))?,
        ),
    );
    Ok(client)
}