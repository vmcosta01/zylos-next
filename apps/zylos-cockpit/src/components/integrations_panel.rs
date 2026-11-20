//! IntegrationsPanel — configuração e teste das integrações Slack/Notion/Linear (T11.8.6)
//!
//! Consome o gateway:
//! - `GET  /api/v1/integrations/status` — status por integração (token presente/ausente)
//! - `POST /api/v1/integrations/{name}/test` — teste de conexão read-only
//!
//! Nunca exibe valores de token: o gateway devolve apenas `configured` e um
//! marcador fixo (`token_masked`). A configuração é feita via variáveis de
//! ambiente do gateway (`SLACK_BOT_TOKEN`, `NOTION_TOKEN`, `LINEAR_TOKEN`) —
//! o painel mostra qual variável definir e o playbook de setup.
//!
//! ## CSS Classes
//! - `integrations-panel` — container raiz
//! - `integration-card` — card por integração
//! - `integration-configured` / `integration-missing` — modificador de estado
//! - `integration-test-btn` — botão "Testar conexão"
//! - `integration-test-result` — resultado do teste (`test-ok` / `test-fail`)

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// URL absoluta de `path` sob `/api/v1` do gateway.
fn gateway(path: &str) -> Result<String, String> {
    crate::gateway_http::api_url(&format!("/api/v1{path}"))
}

/// Status de uma integração como devolvido pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct IntegrationStatus {
    pub name: String,
    pub display_name: String,
    pub configured: bool,
    pub token_env: String,
    #[serde(default)]
    pub token_masked: Option<String>,
    #[serde(default)]
    pub custom_base_url: bool,
    #[serde(default)]
    pub docs: String,
}

#[derive(Debug, Clone, Deserialize)]
struct StatusResponse {
    integrations: Vec<IntegrationStatus>,
}

/// Resultado do teste de conexão.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub configured: bool,
    pub ok: bool,
    pub latency_ms: u64,
    pub detail: String,
}

/// Lista padrão (offline) — exibida até o gateway responder.
pub fn default_integrations() -> Vec<IntegrationStatus> {
    [
        ("slack", "Slack", "SLACK_BOT_TOKEN"),
        ("notion", "Notion", "NOTION_TOKEN"),
        ("linear", "Linear", "LINEAR_TOKEN"),
    ]
    .into_iter()
    .map(|(n, d, env)| IntegrationStatus {
        name: n.to_string(),
        display_name: d.to_string(),
        configured: false,
        token_env: env.to_string(),
        token_masked: None,
        custom_base_url: false,
        docs: format!("playbooks/pb-{n}-integration-setup.md"),
    })
    .collect()
}

/// Texto exibido no campo de token — nunca o valor real.
pub fn token_display(s: &IntegrationStatus) -> String {
    match (&s.configured, &s.token_masked) {
        (true, Some(m)) => m.clone(),
        (true, None) => "••••••••".to_string(),
        (false, _) => "não configurado".to_string(),
    }
}

/// Ícone por integração.
pub fn integration_icon(name: &str) -> &'static str {
    match name {
        "slack" => "\u{1f4ac}",  // 💬
        "notion" => "\u{1f4d3}", // 📓
        "linear" => "\u{1f4cb}", // 📋
        _ => "\u{1f50c}",        // 🔌
    }
}

/// Formata o resultado do teste para exibição.
pub fn format_test_result(r: &TestResult) -> String {
    if r.ok {
        format!("\u{2705} {} ({} ms)", r.detail, r.latency_ms)
    } else {
        format!("\u{274c} {}", r.detail)
    }
}

async fn fetch_status() -> Result<Vec<IntegrationStatus>, String> {
    use gloo_net::http::Request;
    let resp = crate::auth::authed(Request::get(&gateway("/integrations/status")?))
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<StatusResponse>()
        .await
        .map(|r| r.integrations)
        .map_err(|e| format!("{e:?}"))
}

async fn post_test(name: &str) -> Result<TestResult, String> {
    use gloo_net::http::Request;
    let resp = crate::auth::authed(Request::post(&gateway(&format!(
        "/integrations/{name}/test"
    ))?))
    .send()
    .await
    .map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<TestResult>()
        .await
        .map_err(|e| format!("{e:?}"))
}

/// Painel principal de integrações.
#[component]
pub fn IntegrationsPanel() -> impl IntoView {
    let items = RwSignal::new(default_integrations());
    let gateway_error = RwSignal::new(None::<String>);
    // (name, texto do resultado, ok)
    let results = RwSignal::new(Vec::<(String, String, bool)>::new());
    let testing = RwSignal::new(None::<String>);

    let reload = move || {
        spawn_local(async move {
            match fetch_status().await {
                Ok(list) => {
                    items.set(list);
                    gateway_error.set(None);
                }
                Err(e) => gateway_error.set(Some(e)),
            }
        });
    };
    Effect::new(move |_| reload());

    let run_test = move |name: String| {
        testing.set(Some(name.clone()));
        spawn_local(async move {
            let (text, ok) = match post_test(&name).await {
                Ok(r) => (format_test_result(&r), r.ok),
                Err(e) => (format!("\u{274c} gateway indisponível: {e}"), false),
            };
            results.update(|v| {
                v.retain(|(n, _, _)| n != &name);
                v.push((name.clone(), text, ok));
            });
            testing.set(None);
        });
    };

    view! {
        <div class="integrations-panel">
            <header class="panel-header">
                <h2>{"\u{1f50c} Integrações"}</h2>
                <p class="panel-subtitle">
                    {"Slack, Notion e Linear — status, configuração e teste de conexão"}
                </p>
                <button class="integration-reload-btn" on:click=move |_| reload()>
                    {"Recarregar"}
                </button>
            </header>
            {move || {
                gateway_error
                    .get()
                    .map(|e| {
                        view! {
                            <p class="integrations-gateway-error">
                                {format!("Gateway indisponível ({e}) — exibindo padrão offline")}
                            </p>
                        }
                    })
            }}
            <div class="integration-list">
                <For
                    each=move || items.get()
                    key=|s| (s.name.clone(), s.configured)
                    children=move |s: IntegrationStatus| {
                        let name = s.name.clone();
                        let name_for_btn = name.clone();
                        let name_for_busy = name.clone();
                        let name_for_result = name.clone();
                        let card_class = if s.configured {
                            "integration-card integration-configured"
                        } else {
                            "integration-card integration-missing"
                        };
                        let status_label = if s.configured { "Configurado" } else { "Não configurado" };
                        view! {
                            <div class=card_class data-integration=name.clone()>
                                <h3>
                                    {integration_icon(&s.name)} " " {s.display_name.clone()}
                                </h3>
                                <p class="integration-status">{status_label}</p>
                                <label class="integration-token-label">
                                    {format!("Token ({})", s.token_env)}
                                    <input
                                        class="integration-token"
                                        type="text"
                                        readonly=true
                                        value=token_display(&s)
                                    />
                                </label>
                                <p class="integration-hint">
                                    {if s.configured {
                                        "Token presente no gateway (valor oculto).".to_string()
                                    } else {
                                        format!(
                                            "Defina {} no ambiente do gateway e recarregue. Guia: {}",
                                            s.token_env, s.docs,
                                        )
                                    }}
                                </p>
                                {s
                                    .custom_base_url
                                    .then(|| view! { <p class="integration-sandbox">{"URL customizada (sandbox/mock) ativa"}</p> })}
                                <button
                                    class="integration-test-btn"
                                    disabled=move || testing.get().as_deref() == Some(name_for_busy.as_str())
                                    on:click=move |_| run_test(name_for_btn.clone())
                                >
                                    {"Testar conexão"}
                                </button>
                                {move || {
                                    results
                                        .get()
                                        .into_iter()
                                        .find(|(n, _, _)| n == &name_for_result)
                                        .map(|(_, text, ok)| {
                                            let cls = if ok {
                                                "integration-test-result test-ok"
                                            } else {
                                                "integration-test-result test-fail"
                                            };
                                            view! { <p class=cls>{text}</p> }
                                        })
                                }}
                            </div>
                        }
                    }
                />
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_three_unconfigured() {
        let d = default_integrations();
        assert_eq!(d.len(), 3);
        assert!(d.iter().all(|s| !s.configured));
        assert_eq!(d[0].token_env, "SLACK_BOT_TOKEN");
    }

    #[test]
    fn token_display_never_shows_value() {
        let mut s = default_integrations().remove(0);
        assert_eq!(token_display(&s), "não configurado");
        s.configured = true;
        s.token_masked = Some("••••••••".into());
        assert_eq!(token_display(&s), "••••••••");
    }

    #[test]
    fn parses_gateway_status_json() {
        let json = r#"{"integrations":[{"name":"slack","display_name":"Slack","configured":true,
            "token_env":"SLACK_BOT_TOKEN","token_masked":"••••••••","custom_base_url":false,
            "docs":"playbooks/pb-slack-integration-setup.md"}]}"#;
        let r: StatusResponse = serde_json::from_str(json).unwrap();
        assert!(r.integrations[0].configured);
    }

    #[test]
    fn format_result_ok_and_fail() {
        let mut r = TestResult {
            name: "slack".into(),
            configured: true,
            ok: true,
            latency_ms: 42,
            detail: "conexão OK".into(),
        };
        assert!(format_test_result(&r).contains("42 ms"));
        r.ok = false;
        r.detail = "token ausente".into();
        assert!(format_test_result(&r).contains("token ausente"));
    }
}
