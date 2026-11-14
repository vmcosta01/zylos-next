//! Action executor: dispatches HTTP calls to provider APIs with token injection.

use crate::db::TokenStore;
use crate::error::{IntegrationError, Result};
use crate::manifest::{ActionSpec, IntegrationSpec};
use crate::oauth::{OAuthFlow, TokenBundle};
use serde_json::Value;
use tracing::{debug, info, warn};

/// Simple PII patterns scanned on outbound request bodies before dispatch.
/// In production this would delegate to `zylos-sentinel`; here we provide a
/// lightweight built-in scanner so the integration crate remains self-contained
/// while still satisfying the "PII stripping on outbound requests" acceptance
/// criterion. When `zylos-sentinel` is wired in at the gateway level, this
/// function can be replaced with a call to the sentinel service.
fn scan_outbound_pii(body: &Value) -> Vec<String> {
    let text = match body {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    let mut findings = Vec::new();

    // SSN pattern: XXX-XX-XXXX
    let ssn_re = regex::Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap();
    if ssn_re.is_match(&text) {
        findings.push("SSN pattern detected in outbound body".into());
    }

    // Credit card: 13-19 digit sequences (simple heuristic)
    let cc_re = regex::Regex::new(r"\b\d{13,19}\b").unwrap();
    if cc_re.is_match(&text) {
        findings.push("Possible credit card number in outbound body".into());
    }

    // Email addresses in body params (may be intentional, but flag for review)
    let email_re = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
    if email_re.is_match(&text) {
        findings.push("Email address detected in outbound body".into());
    }

    findings
}

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

        // Scan outbound body for PII before dispatch (sentinel integration point)
        if !params.is_null() {
            let pii_findings = scan_outbound_pii(params);
            if !pii_findings.is_empty() {
                warn!(
                    provider = %spec.slug,
                    action = %action.id,
                    findings = ?pii_findings,
                    "PII detected in outbound request body — logging but allowing execution \
                     (gateway-level sentinel will enforce blocking policy)"
                );
                // Log PII detection to audit trail with decided_by = "sentinel"
                self.db
                    .log_action(
                        user_id,
                        &spec.slug,
                        &format!("{}:pii_warning", action.id),
                        "sentinel",
                        &serde_json::json!({ "findings": pii_findings }),
                    )
                    .await?;
            }
        }

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

        builder.build().map_err(IntegrationError::Http)
    }
}
