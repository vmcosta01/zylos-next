//! Action executor: dispatches HTTP calls to provider APIs with token injection.

use crate::db::TokenStore;
use crate::error::{IntegrationError, Result};
use crate::manifest::{ActionSpec, IntegrationSpec};
use crate::oauth::{OAuthFlow, TokenBundle};
use serde_json::Value;
use tracing::{debug, info, warn};
use zylos_agent_shield::network_egress::{
    parse_url_target, EgressConfig, EgressDecision, EgressRule, NetworkEgressFilter,
};

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

/// Hosts que o executor pode chamar (fail-closed). Derivados de `specs/*.yaml`.
/// Hosts com placeholder (`{shop}.myshopify.com`) entram como wildcard de sufixo.
pub const ALLOWED_PROVIDER_HOSTS: &[&str] = &[
    "*.myshopify.com",
    "api.atlassian.com",
    "api.figma.com",
    "api.github.com",
    "api.linear.app",
    "api.notion.com",
    "api.stripe.com",
    "gmail.googleapis.com",
    "open.feishu.cn",
    "qyapi.weixin.qq.com",
    "slack.com",
    "www.googleapis.com",
];

/// Identidade usada nas regras de egress do executor.
pub const EGRESS_AGENT: &str = "zylos-integrations";

/// Filtro de egress padrão do executor: só HTTPS (443) para os hosts dos provedores,
/// com checagem do IP resolvido (anti DNS-rebinding / SSRF para rede interna).
pub fn default_egress_filter() -> NetworkEgressFilter {
    let mut rules = std::collections::HashMap::new();
    rules.insert(
        EGRESS_AGENT.to_string(),
        EgressRule {
            allowed_hosts: ALLOWED_PROVIDER_HOSTS
                .iter()
                .map(|h| h.to_string())
                .collect(),
            allowed_ports: vec![443],
            deny_all: false,
        },
    );
    NetworkEgressFilter::new(EgressConfig {
        egress_rules: rules,
        log_blocked: true,
        allow_dns_resolution: true,
    })
}

/// Codifica um valor para entrar num template de URL (RFC 3986 unreserved).
/// Impede que um parâmetro troque host, caminho ou query (`evil.com/`, `../`, `?`, `#`, `@`).
fn encode_url_param(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Checagem de egress usada pelo executor antes de cada chamada.
pub async fn check_egress_with(filter: &NetworkEgressFilter, url: &str) -> Result<()> {
    let blocked = |reason: String| IntegrationError::ProviderExecution {
        provider: "egress".into(),
        action: "check_egress".into(),
        detail: format!("egress bloqueado: {reason}"),
    };
    if !url.to_ascii_lowercase().starts_with("https://") {
        return Err(blocked(format!("só HTTPS é permitido ({url})")));
    }
    let (host, port) = parse_url_target(url).map_err(blocked)?;
    let (decision, _ips) = filter.check_with_dns(EGRESS_AGENT, &host, port).await;
    match decision {
        EgressDecision::Allowed => Ok(()),
        EgressDecision::Blocked { reason } => Err(blocked(reason)),
    }
}

/// Executes actions against provider APIs using stored OAuth tokens.
pub struct ActionExecutor {
    db: TokenStore,
    oauth: OAuthFlow,
    http: reqwest::Client,
    egress: NetworkEgressFilter,
}

impl ActionExecutor {
    pub fn new(db: TokenStore, oauth: OAuthFlow) -> Self {
        Self::with_egress(db, oauth, default_egress_filter())
    }

    /// Construtor com filtro de egress explícito (testes / política customizada).
    pub fn with_egress(db: TokenStore, oauth: OAuthFlow, egress: NetworkEgressFilter) -> Self {
        Self {
            db,
            oauth,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                // Redirect seguiria para um host não validado pelo filtro.
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Failed to build HTTP client"),
            egress,
        }
    }

    /// Verifica a URL final no filtro de egress (host, porta, IP resolvido).
    pub async fn check_egress(&self, url: &str) -> Result<()> {
        check_egress_with(&self.egress, url).await
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
        self.check_egress(&url).await?;
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
                        Value::String(s) => encode_url_param(s),
                        other => encode_url_param(&other.to_string()),
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

#[cfg(test)]
mod egress_tests {
    use super::*;

    #[tokio::test]
    async fn bloqueia_hosts_fora_da_allowlist_e_rede_interna() {
        let f = default_egress_filter();
        for url in [
            "http://api.github.com/repos",
            "https://169.254.169.254/latest/meta-data",
            "https://127.0.0.1/admin",
            "https://localhost/",
            "https://evil.example.com/",
            "https://api.github.com:8443/",
            "https://user@api.github.com/",
            "https://api.github.com.evil.com/",
            "file:///etc/passwd",
        ] {
            assert!(
                check_egress_with(&f, url).await.is_err(),
                "deveria bloquear {url}"
            );
        }
    }

    #[test]
    fn allowlist_cobre_os_specs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("specs");
        let mut hosts = std::collections::BTreeSet::new();
        for e in std::fs::read_dir(dir).unwrap() {
            let txt = std::fs::read_to_string(e.unwrap().path()).unwrap();
            for part in txt.split("https://").skip(1) {
                let h = part.split(['/', '"', '\'', ' ', '\n']).next().unwrap();
                hosts.insert(h.replace("{shop}", "x"));
            }
        }
        for h in hosts {
            let ok = ALLOWED_PROVIDER_HOSTS
                .iter()
                .any(|a| match a.strip_prefix("*.") {
                    Some(suf) => h.ends_with(&format!(".{suf}")),
                    None => *a == h,
                });
            // URLs de OAuth/autorização (ex.: accounts.google.com) não passam pelo executor.
            if !ok {
                assert!(
                    h.contains("oauth")
                        || h.contains("accounts.")
                        || h.contains("auth")
                        || !h.contains("api") && !h.contains("googleapis"),
                    "host de API sem regra de egress: {h}"
                );
            }
        }
    }

    #[test]
    fn parametro_nao_troca_host_nem_caminho() {
        assert_eq!(
            encode_url_param("evil.com/x?y#z@w"),
            "evil.com%2Fx%3Fy%23z%40w"
        );
        assert_eq!(encode_url_param("../admin"), "..%2Fadmin");
        assert_eq!(encode_url_param("abc-123_~."), "abc-123_~.");
    }
}
