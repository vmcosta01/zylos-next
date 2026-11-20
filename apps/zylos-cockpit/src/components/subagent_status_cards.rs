//! SubagentStatusCards — cards de status de subagentes com steps, duration e tokens
//!
//! Inspirado em: OpenBot (`Subagents.tsx`) com cards mostrando progresso de subagentes.
//! Dados reais de `GET /api/v1/subagents` (tarefas filhas do swarm). Campos sem
//! fonte real chegam como `null` e são exibidos como "—". Sem subagentes →
//! estado vazio; erro → mensagem de [`crate::ui_error::report`].
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.2

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Estado de um subagente
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentState {
    Queued,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl SubagentState {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Queued => "⏳",
            Self::Running => "🔄",
            Self::WaitingApproval => "✋",
            Self::Completed => "✅",
            Self::Failed => "❌",
            Self::Cancelled => "🚫",
        }
    }

    pub fn css_class(&self) -> &'static str {
        match self {
            Self::Queued => "subagent-queued",
            Self::Running => "subagent-running",
            Self::WaitingApproval => "subagent-waiting",
            Self::Completed => "subagent-completed",
            Self::Failed => "subagent-failed",
            Self::Cancelled => "subagent-cancelled",
        }
    }
}

/// Texto exibido quando o campo não tem fonte real.
pub const MISSING: &str = "—";

/// Um subagente como devolvido por `GET /api/v1/subagents`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub goal: String,
    pub state: String,
    #[serde(default)]
    pub steps_completed: Option<usize>,
    #[serde(default)]
    pub steps_total: Option<usize>,
    #[serde(default)]
    pub tokens_used: Option<u64>,
    #[serde(default)]
    pub duration_seconds: Option<u64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SubagentsResponse {
    subagents: Vec<SubagentInfo>,
}

impl SubagentInfo {
    pub fn state_enum(&self) -> SubagentState {
        match self.state.as_str() {
            "queued" => SubagentState::Queued,
            "running" => SubagentState::Running,
            "waiting_approval" => SubagentState::WaitingApproval,
            "completed" => SubagentState::Completed,
            "failed" => SubagentState::Failed,
            "cancelled" => SubagentState::Cancelled,
            _ => SubagentState::Queued,
        }
    }

    /// Progresso 0–100; 0 quando não há passos conhecidos.
    pub fn progress_pct(&self) -> f64 {
        match (self.steps_completed, self.steps_total) {
            (Some(done), Some(total)) if total > 0 => (done as f64 / total as f64) * 100.0,
            _ => 0.0,
        }
    }

    /// `3/5`, ou "—" sem passos conhecidos.
    pub fn formatted_steps(&self) -> String {
        match (self.steps_completed, self.steps_total) {
            (Some(d), Some(t)) => format!("{d}/{t}"),
            (Some(d), None) => d.to_string(),
            _ => MISSING.to_string(),
        }
    }

    pub fn formatted_duration(&self) -> String {
        let Some(s) = self.duration_seconds else {
            return MISSING.to_string();
        };
        if s < 60 {
            format!("{s}s")
        } else if s < 3600 {
            format!("{}m {}s", s / 60, s % 60)
        } else {
            format!("{}h {}m", s / 3600, (s % 3600) / 60)
        }
    }

    pub fn formatted_tokens(&self) -> String {
        match self.tokens_used {
            None => MISSING.to_string(),
            Some(t) if t < 1000 => t.to_string(),
            Some(t) => format!("{:.1}k", t as f64 / 1000.0),
        }
    }

    pub fn formatted_model(&self) -> String {
        self.model.clone().unwrap_or_else(|| MISSING.to_string())
    }
}

#[component]
pub fn SubagentStatusCards() -> impl IntoView {
    // None = carregando; Some(Ok) = dados; Some(Err) = mensagem amigável.
    let data = RwSignal::new(None::<Result<Vec<SubagentInfo>, String>>);
    let load = move || {
        spawn_local(async move {
            let r = fetch_subagents()
                .await
                .map_err(|e| crate::ui_error::report("subagents", &e));
            data.set(Some(r));
        });
    };
    Effect::new(move |_| load());

    // Auto-refresh a cada 5 segundos (só substitui em caso de sucesso).
    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                gloo_timers::future::sleep(std::time::Duration::from_secs(5)).await;
                if let Ok(v) = fetch_subagents().await {
                    data.set(Some(Ok(v)));
                }
            }
        });
    });

    view! {
        <div class="subagent-cards-panel">
            <div class="subagent-panel-header">
                <h3 class="subagent-panel-title"><i class="ph ph-robot" aria-hidden="true"></i>" Subagentes"</h3>
                <button class="subagent-refresh-btn" on:click=move |_| load() title="Atualizar">
                    <i class="ph ph-arrows-clockwise" aria-hidden="true"></i>
                </button>
            </div>

            {move || match data.get() {
                None => view! {
                    <div class="subagent-loading">
                        <span class="spinner"></span>
                        <span>"Carregando subagentes..."</span>
                    </div>
                }.into_any(),
                Some(Err(msg)) => view! {
                    <div class="error-banner" role="alert">
                        <p>{msg}</p>
                        <button class="cta-retry" on:click=move |_| load()>"Tentar novamente"</button>
                    </div>
                }.into_any(),
                Some(Ok(agents)) if agents.is_empty() => view! {
                    <div class="subagent-empty">"Nenhum subagente no momento."</div>
                }.into_any(),
                Some(Ok(agents)) => view! {
                    <div class="subagent-grid">
                        {agents.into_iter().map(subagent_card).collect_view()}
                    </div>
                }.into_any(),
            }}
        </div>
    }
}

fn subagent_card(agent: SubagentInfo) -> impl IntoView {
    let state = agent.state_enum();
    let progress = agent.progress_pct();
    let css = format!("subagent-card {}", state.css_class());
    view! {
        <div class=css>
            <div class="subagent-card-header">
                <span class="subagent-icon">{state.icon()}</span>
                <span class="subagent-name">{agent.name.clone()}</span>
                <span class="subagent-model">{agent.formatted_model()}</span>
            </div>

            <p class="subagent-goal">{agent.goal.clone()}</p>

            <div class="subagent-stats">
                <div class="subagent-stat">
                    <span class="subagent-stat-label">"Steps"</span>
                    <span class="subagent-stat-value">{agent.formatted_steps()}</span>
                </div>
                <div class="subagent-stat">
                    <span class="subagent-stat-label">"Tokens"</span>
                    <span class="subagent-stat-value">{agent.formatted_tokens()}</span>
                </div>
                <div class="subagent-stat">
                    <span class="subagent-stat-label">"Duração"</span>
                    <span class="subagent-stat-value">{agent.formatted_duration()}</span>
                </div>
            </div>

            {(progress > 0.0).then(|| view! {
                <div class="subagent-progress-bar">
                    <div class="subagent-progress-fill" style=format!("width: {progress}%")></div>
                </div>
            })}

            {agent.error_message.clone().map(|e| view! { <div class="subagent-error">{e}</div> })}
        </div>
    }
}

// === API Function ===

async fn fetch_subagents() -> Result<Vec<SubagentInfo>, String> {
    use gloo_net::http::Request;
    let base = crate::gateway_config::gateway_base()
        .ok_or("Gateway não configurado (meta zylos-gateway-url)")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/subagents");
    let resp = crate::auth::authed(Request::get(&url))
        .send()
        .await
        .map_err(|e| format!("{e:?}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let r: SubagentsResponse = resp.json().await.map_err(|e| format!("{e:?}"))?;
    Ok(r.subagents)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campos_nulos_viram_traco() {
        let r: SubagentsResponse = serde_json::from_str(
            r#"{"subagents":[{"id":"t1","name":"Pesquisa","goal":"g","state":"running",
                "parent_id":"p","steps_completed":null,"steps_total":null,
                "tokens_used":null,"model":null,"duration_seconds":null,
                "retry_count":0,"error_message":null,"created_at":"x"}]}"#,
        )
        .unwrap();
        let a = &r.subagents[0];
        assert_eq!(a.state_enum(), SubagentState::Running);
        assert_eq!(a.formatted_steps(), MISSING);
        assert_eq!(a.formatted_tokens(), MISSING);
        assert_eq!(a.formatted_duration(), MISSING);
        assert_eq!(a.formatted_model(), MISSING);
        assert_eq!(a.progress_pct(), 0.0);
    }

    #[test]
    fn campos_presentes_formatados() {
        let a: SubagentInfo = serde_json::from_str(
            r#"{"id":"t","name":"n","state":"completed","steps_completed":3,
                "steps_total":4,"tokens_used":1500,"duration_seconds":125,"model":"m"}"#,
        )
        .unwrap();
        assert_eq!(a.formatted_steps(), "3/4");
        assert_eq!(a.formatted_tokens(), "1.5k");
        assert_eq!(a.formatted_duration(), "2m 5s");
        assert_eq!(a.progress_pct(), 75.0);
        assert_eq!(a.formatted_model(), "m");
    }
}
