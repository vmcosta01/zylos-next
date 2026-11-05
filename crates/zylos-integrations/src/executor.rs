//! Action executor: dispatches HTTP calls to provider APIs with token injection.

use crate::db::TokenStore;
use crate::error::{IntegrationError, Result};
use crate::manifest::{ActionSpec, IntegrationSpec};
use crate::oauth::{OAuthFlow, TokenBundle};
use serde_json::Value;
use tracing::{debug, info};

/// Executes actions against provider APIs using stored OAuth tokens.
pub struct ActionExecutor {
    db: TokenStore,
    oauth: OAuthFlow,
    http: reqwest::Client,
}

impl ActionExecutor {
    pub fn new(db: TokenStore, oauth: OAuthFlow) -> Self {
        Self {
            db,
            oauth,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }

    /// Execute an action on behalf of a user.
    /// Returns the raw JSON response from the provider API.
    pub async fn execute(
        &self,
        user_id: &str,
        spec: &IntegrationSpec,
        action: &ActionSpec,
        params: &Value,
    ) -> Result<Value> {
        // Check HITL approval requirement
        if action.requires_approval || spec.requires_approval {
            return Err(IntegrationError::ApprovalRequired {
                provider: spec.slug.clone(),
                action: action.id.clone(),
            });
        }

        // Get valid tokens (refresh if expired)
        let bundle = self.get_valid_tokens(user_id, spec).await?;

        // Build the request
        let url = self.build_url(&action.url_template, params);
        let request = self.build_request(&action.method, &url, &bundle.access_token, params)?;

        debug!(provider = %spec.slug, action = %action.id, "Executing integration action");

        let response = self.http.execute(request).await?;
        let status = response.status();

        if status.is_success() {
            let body: Value = response.json().await.unwrap_or(Value::Null);

            // Log successful action
            self.db
                            .log_action(user_id, &spec.slug, &action.id, "auto", &body)
                            .await?;

            info!(provider = %spec.slug, action = %action.id, status = %status, "Action executed successfully");
            Ok(body)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(IntegrationError::ProviderExecution {
                provider: spec.slug.clone(),
                action: action.id.clone(),
                detail: format!("HTTP {}: {}", status, error_text),
            })
        }
    }

    /// Get valid tokens, refreshing automatically if expired.
    async fn get_valid_tokens(&self, user_id: &str, spec: &IntegrationSpec) -> Result<TokenBundle> {
        let bundle = self
            .db
            .get_tokens(user_id, &spec.slug)
            .await?
            .ok_or_else(|| IntegrationError::NoTokensStored {
                user_id: user_id.to_string(),
                provider: spec.slug.clone(),
            })?;

        if bundle.is_expired() {
            debug!(provider = %spec.slug, "Token expired, refreshing");
            let refreshed = self.oauth.refresh(spec, &bundle).await?;
            self.db.store_tokens(user_id, &refreshed).await?;
            Ok(refreshed)
        } else {
            Ok(bundle)
        }
    }

    /// Substitute `{param}` placeholders in URL templates with values from params.
    fn build_url(&self, template: &str, params: &Value) -> String {
        let mut url = template.to_string();

        if let Value::Object(map) = params {
            for (key, value) in map {
                let placeholder = format!("{{{}}}", key);
                if url.contains(&placeholder) {
                    let replacement = match value {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    url = url.replace(&placeholder, &replacement);
                }
            }
        }

        url
    }

    /// Build an HTTP request with bearer auth and optional JSON body.
    fn build_request(
        &self,
        method: &str,
        url: &str,
        access_token: &str,
        params: &Value,
    ) -> Result<reqwest::Request> {
        let mut builder = match method.to_uppercase().as_str() {
            "GET" => self.http.get(url),
            "POST" => self.http.post(url),
            "PUT" => self.http.put(url),
            "PATCH" => self.http.patch(url),
            "DELETE" => self.http.delete(url),
            other => {
                return Err(IntegrationError::ProviderExecution {
                    provider: "unknown".into(),
                    action: "build_request".into(),
                    detail: format!("Unsupported HTTP method: {}", other),
                })
            }
        };

        builder = builder.bearer_auth(access_token);

        // Add JSON body for non-GET methods
        if method.to_uppercase() != "GET" && !params.is_null() {
            builder = builder.json(params);
        }

        // Notion requires special headers
        if url.contains("api.notion.com") {
            builder = builder
                .header("Notion-Version", "2022-06-28")
                .header("Content-Type", "application/json");
        }

        // Linear uses GraphQL — always POST with JSON
        if url.contains("api.linear.app") {
            builder = builder.header("Content-Type", "application/json");
        }

        builder
            .build()
            .map_err(|e| IntegrationError::Http(e))
    }
}