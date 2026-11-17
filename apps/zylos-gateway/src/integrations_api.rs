//! Integrations API (T11.8.6) — status e teste de conexão das integrações
//! Slack / Notion / Linear consumidas pelo painel `integrations_panel.rs` do cockpit.
//!
//! ## Endpoints (montados sob `/api/v1`)
//! - `GET  /integrations/status` — lista integrações e se o token está presente
//! - `POST /integrations/:name/test` — executa `health_check()` read-only no adapter
//!
//! ## Segurança
//! Valores de token **nunca** saem do processo: a resposta expõe apenas
//! `configured: bool` e um marcador fixo (`token_masked`), sem prefixo/sufixo/tamanho.
//! Tokens são lidos de variáveis de ambiente (`SLACK_BOT_TOKEN`, `NOTION_TOKEN`,
//! `LINEAR_TOKEN`) no momento da requisição.
//!
//! ## Related Documents
//! - Crate: `crates/zylos-integrations/`
//! - Componente: `apps/zylos-cockpit/src/components/integrations_panel.rs`
//! - Doc: `docs/integrations-cockpit-panel.md`

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use zylos_integrations::channel::ChannelAdapter;
use zylos_integrations::linear::LinearAdapter;
use zylos_integrations::notion::NotionAdapter;
use zylos_integrations::slack::SlackAdapter;

/// Marcador fixo exibido quando há token configurado (nunca deriva do valor real).
pub const TOKEN_MASK: &str = "••••••••";

/// Descritor estático de uma integração suportada.
#[derive(Debug, Clone, Copy)]
pub struct IntegrationDef {
    pub name: &'static str,
    pub display_name: &'static str,
    pub token_env: &'static str,
    pub base_url_env: &'static str,
    pub docs: &'static str,
}

/// Integrações suportadas pela Fase 11.8.
pub const INTEGRATIONS: [IntegrationDef; 3] = [
    IntegrationDef {
        name: "slack",
        display_name: "Slack",
        token_env: "SLACK_BOT_TOKEN",
        base_url_env: "ZYLOS_SLACK_API_URL",
        docs: "playbooks/pb-slack-integration-setup.md",
    },
    IntegrationDef {
        name: "notion",
        display_name: "Notion",
        token_env: "NOTION_TOKEN",
        base_url_env: "ZYLOS_NOTION_API_URL",
        docs: "playbooks/pb-notion-integration-setup.md",
    },
    IntegrationDef {
        name: "linear",
        display_name: "Linear",
        token_env: "LINEAR_TOKEN",
        base_url_env: "ZYLOS_LINEAR_API_URL",
        docs: "playbooks/pb-linear-integration-setup.md",
    },
];

fn find_def(name: &str) -> Option<&'static IntegrationDef> {
    INTEGRATIONS.iter().find(|d| d.name == name)
}

type Lookup = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// Estado do router: fonte de configuração (env por padrão; injetável em testes).
#[derive(Clone)]
pub struct IntegrationsState {
    lookup: Lookup,
}

impl IntegrationsState {
    /// Lê tokens/URLs das variáveis de ambiente do processo.
    pub fn from_env() -> Self {
        Self {
            lookup: Arc::new(|k: &str| std::env::var(k).ok()),
        }
    }

    /// Fonte fixa (testes).
    pub fn from_map(map: HashMap<String, String>) -> Self {
        Self {
            lookup: Arc::new(move |k: &str| map.get(k).cloned()),
        }
    }

    fn get(&self, key: &str) -> Option<String> {
        (self.lookup)(key).filter(|v| !v.trim().is_empty())
    }
}

/// Status de uma integração (sem valor de token).
#[derive(Debug, Clone, Serialize)]
pub struct IntegrationStatus {
    pub name: &'static str,
    pub display_name: &'static str,
    pub configured: bool,
    pub token_env: &'static str,
    pub token_masked: Option<&'static str>,
    pub custom_base_url: bool,
    pub docs: &'static str,
}

/// Resultado do teste de conexão.
#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub name: &'static str,
    pub configured: bool,
    pub ok: bool,
    pub latency_ms: u64,
    pub detail: String,
}

fn status_of(state: &IntegrationsState, d: &'static IntegrationDef) -> IntegrationStatus {
    let configured = state.get(d.token_env).is_some();
    IntegrationStatus {
        name: d.name,
        display_name: d.display_name,
        configured,
        token_env: d.token_env,
        token_masked: configured.then_some(TOKEN_MASK),
        custom_base_url: state.get(d.base_url_env).is_some(),
        docs: d.docs,
    }
}

async fn list_status(State(state): State<IntegrationsState>) -> impl IntoResponse {
    let items: Vec<IntegrationStatus> = INTEGRATIONS.iter().map(|d| status_of(&state, d)).collect();
    Json(serde_json::json!({ "integrations": items }))
}

fn build_adapter(
    d: &IntegrationDef,
    token: String,
    base: Option<String>,
) -> Box<dyn ChannelAdapter> {
    match (d.name, base) {
        ("slack", Some(b)) => Box::new(SlackAdapter::with_base_url(token, b)),
        ("slack", None) => Box::new(SlackAdapter::new(token)),
        ("notion", Some(b)) => Box::new(NotionAdapter::with_base_url(token, b)),
        ("notion", None) => Box::new(NotionAdapter::new(token)),
        (_, Some(b)) => Box::new(LinearAdapter::with_base_url(token, b)),
        (_, None) => Box::new(LinearAdapter::new(token)),
    }
}

async fn test_connection(
    State(state): State<IntegrationsState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let Some(d) = find_def(&name) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "integração desconhecida", "name": name })),
        )
            .into_response();
    };
    let Some(token) = state.get(d.token_env) else {
        return Json(TestResult {
            name: d.name,
            configured: false,
            ok: false,
            latency_ms: 0,
            detail: format!("token ausente: defina {}", d.token_env),
        })
        .into_response();
    };
    let adapter = build_adapter(d, token, state.get(d.base_url_env));
    let started = std::time::Instant::now();
    let (ok, detail) = match adapter.health_check().await {
        Ok(true) => (true, "conexão OK".to_string()),
        Ok(false) => (
            false,
            "falha de autenticação ou API indisponível".to_string(),
        ),
        Err(e) => (false, format!("erro: {e}")),
    };
    Json(TestResult {
        name: d.name,
        configured: true,
        ok,
        latency_ms: started.elapsed().as_millis() as u64,
        detail,
    })
    .into_response()
}

/// Cria o router de integrações (Router<()> pronto para merge em `/api/v1`).
pub fn create_integrations_router(state: IntegrationsState) -> Router {
    Router::new()
        .route("/integrations/status", get(list_status))
        .route("/integrations/:name/test", post(test_connection))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn call(app: Router, method: &str, uri: &str) -> (StatusCode, serde_json::Value) {
        let resp = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    fn state(pairs: &[(&str, &str)]) -> IntegrationsState {
        IntegrationsState::from_map(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    /// Mock local que responde `auth.test` (Slack) com o JSON dado.
    async fn mock_slack(json: serde_json::Value) -> String {
        let app = Router::new().route(
            "/auth.test",
            get(move || {
                let j = json.clone();
                async move { Json(j) }
            }),
        );
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn status_lists_three_and_never_leaks_token() {
        let secret = "xoxb-SUPER-SECRET-123";
        let app = create_integrations_router(state(&[("SLACK_BOT_TOKEN", secret)]));
        let (st, body) = call(app, "GET", "/integrations/status").await;
        assert_eq!(st, StatusCode::OK);
        let items = body["integrations"].as_array().unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0]["name"], "slack");
        assert_eq!(items[0]["configured"], true);
        assert_eq!(items[0]["token_masked"], TOKEN_MASK);
        assert_eq!(items[1]["configured"], false);
        assert!(items[1]["token_masked"].is_null());
        let raw = body.to_string();
        assert!(!raw.contains("SUPER-SECRET"), "token vazou: {raw}");
        assert!(!raw.contains("xoxb-"), "prefixo vazou: {raw}");
    }

    #[tokio::test]
    async fn blank_token_counts_as_not_configured() {
        let app = create_integrations_router(state(&[("NOTION_TOKEN", "   ")]));
        let (_, body) = call(app, "GET", "/integrations/status").await;
        assert_eq!(body["integrations"][1]["configured"], false);
    }

    #[tokio::test]
    async fn test_unknown_integration_is_404() {
        let app = create_integrations_router(state(&[]));
        let (st, _) = call(app, "POST", "/integrations/jira/test").await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_without_token_reports_not_configured() {
        let app = create_integrations_router(state(&[]));
        let (st, body) = call(app, "POST", "/integrations/linear/test").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["configured"], false);
        assert_eq!(body["ok"], false);
        assert!(body["detail"].as_str().unwrap().contains("LINEAR_TOKEN"));
    }

    #[tokio::test]
    async fn test_with_mock_ok_returns_ok_without_token() {
        let base = mock_slack(serde_json::json!({"ok": true})).await;
        let secret = "xoxb-mock-secret";
        let app = create_integrations_router(state(&[
            ("SLACK_BOT_TOKEN", secret),
            ("ZYLOS_SLACK_API_URL", &base),
        ]));
        let (st, body) = call(app, "POST", "/integrations/slack/test").await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["ok"], true, "{body}");
        assert_eq!(body["configured"], true);
        assert!(!body.to_string().contains(secret));
    }

    #[tokio::test]
    async fn test_with_mock_invalid_auth_returns_not_ok() {
        let base = mock_slack(serde_json::json!({"ok": false, "error": "invalid_auth"})).await;
        let app = create_integrations_router(state(&[
            ("SLACK_BOT_TOKEN", "xoxb-bad"),
            ("ZYLOS_SLACK_API_URL", &base),
        ]));
        let (_, body) = call(app, "POST", "/integrations/slack/test").await;
        assert_eq!(body["ok"], false);
        assert_eq!(body["configured"], true);
    }
}
