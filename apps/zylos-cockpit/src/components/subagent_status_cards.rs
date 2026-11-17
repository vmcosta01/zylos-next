//! SubagentStatusCards — cards de status de subagentes com steps, duration e tokens
//!
//! Inspirado em: OpenBot (`Subagents.tsx`) com cards mostrando progresso de subagentes.
//! Exibe estado, duração, tokens consumidos e steps completados de cada subagente ativo.
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

/// Um subagente com seu estado atual
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentInfo {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub state: String,
    pub steps_completed: usize,
    pub steps_total: Option<usize>,
    pub tokens_used: usize,
    pub duration_seconds: u64,
    pub model: String,
    pub error_message: Option<String>,
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

    pub fn progress_pct(&self) -> f64 {
        if let Some(total) = self.steps_total {
            if total > 0 {
                (self.steps_completed as f64 / total as f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    pub fn formatted_duration(&self) -> String {
        if self.duration_seconds < 60 {
            format!("{}s", self.duration_seconds)
        } else if self.duration_seconds < 3600 {
            format!(
                "{}m {}s",
                self.duration_seconds / 60,
                self.duration_seconds % 60
            )
        } else {
            format!(
                "{}h {}m",
                self.duration_seconds / 3600,
                (self.duration_seconds % 3600) / 60
            )
        }
    }

    pub fn formatted_tokens(&self) -> String {
        if self.tokens_used < 1000 {
            format!("{}", self.tokens_used)
        } else {
            format!("{:.1}k", self.tokens_used as f64 / 1000.0)
        }
    }
}

#[component]
pub fn SubagentStatusCards() -> impl IntoView {
    let subagents = RwSignal::new(Vec::<SubagentInfo>::new());
    let loading = RwSignal::new(true);

    // Fetch subagentes ao montar
    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            if let Ok(data) = fetch_subagents().await {
                subagents.set(data);
            }
            loading.set(false);
        });
    });

    // Auto-refresh a cada 5 segundos para agentes running
    Effect::new(move |_| {
        spawn_local(async move {
            loop {
                gloo_timers::future::sleep(std::time::Duration::from_secs(5)).await;
                if let Ok(data) = fetch_subagents().await {
                    subagents.set(data);
                }
            }
        });
    });

    view! {
        <div class="subagent-cards-panel">
            <div class="subagent-panel-header">
                <h3 class="subagent-panel-title">"🤖 Subagentes Ativos"</h3>
                <button
                    class="subagent-refresh-btn"
                    on:click=move |_| {
                        spawn_local(async move {
                            if let Ok(data) = fetch_subagents().await {
                                subagents.set(data);
                            }
                        });
                    }
                    title="Atualizar"
                >
                    "🔄"
                </button>
            </div>

            {move || {
                if loading.get() {
                    view! {
                        <div class="subagent-loading">
                            <span class="spinner"></span>
                            <span>"Carregando subagentes..."</span>
                        </div>
                    }.into_any()
                } else {
                    let agents = subagents.get();
                    if agents.is_empty() {
                        view! {
                            <div class="subagent-empty">
                                "Nenhum subagente ativo no momento."
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="subagent-grid">
                                <For
                                    each=move || subagents.get()
                                    key=|a| a.id.clone()
                                    children=move |agent| {
                                        let state = agent.state_enum();
                                        let progress = agent.progress_pct();
                                        let duration = agent.formatted_duration();
                                        let tokens = agent.formatted_tokens();
                                        let goal = agent.goal.clone();
                                        let name = agent.name.clone();
                                        let model = agent.model.clone();
                                        let steps = agent.steps_completed;
                                        let total = agent.steps_total;
                                        let error = agent.error_message.clone();
                                        let css = state.css_class().to_string();
                                        let icon = state.icon().to_string();

                                        view! {
                                            <div class=format!("subagent-card {}", css)>
                                                <div class="subagent-card-header">
                                                    <span class="subagent-icon">{icon.clone()}</span>
                                                    <span class="subagent-name">{name.clone()}</span>
                                                    <span class="subagent-model">{model.clone()}</span>
                                                </div>

                                                <p class="subagent-goal">{goal.clone()}</p>

                                                <div class="subagent-stats">
                                                    <div class="subagent-stat">
                                                        <span class="subagent-stat-label">"Steps"</span>
                                                        <span class="subagent-stat-value">
                                                            {steps}
                                                            {total.map(|t| format!("/{}", t)).unwrap_or_default()}
                                                        </span>
                                                    </div>
                                                    <div class="subagent-stat">
                                                        <span class="subagent-stat-label">"Tokens"</span>
                                                        <span class="subagent-stat-value">{tokens.clone()}</span>
                                                    </div>
                                                    <div class="subagent-stat">
                                                        <span class="subagent-stat-label">"Duração"</span>
                                                        <span class="subagent-stat-value">{duration.clone()}</span>
                                                    </div>
                                                </div>

                                                {if progress > 0.0 {
                                                    view! {
                                                        <div class="subagent-progress-bar">
                                                            <div
                                                                class="subagent-progress-fill"
                                                                style=format!("width: {}%", progress)
                                                            ></div>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <div></div> }.into_any()
                                                }}

                                                {error.map(|e| view! {
                                                    <div class="subagent-error">{e}</div>
                                                })}
                                            </div>
                                        }
                                    }
                                />
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

// === API Function ===

async fn fetch_subagents() -> Result<Vec<SubagentInfo>, String> {
    // Placeholder: subagentes virão do backend quando orquestração multi-agente estiver integrada
    // Por agora, retorna dados mock para demonstração do componente
    Ok(vec![
        SubagentInfo {
            id: "sa-1".into(),
            name: "Research Agent".into(),
            goal: "Analisar documentação técnica do projeto Ruflo".into(),
            state: "running".into(),
            steps_completed: 7,
            steps_total: Some(12),
            tokens_used: 15420,
            duration_seconds: 145,
            model: "claude-3.5-sonnet".into(),
            error_message: None,
        },
        SubagentInfo {
            id: "sa-2".into(),
            name: "Code Reviewer".into(),
            goal: "Revisar PR #142 para aprovação".into(),
            state: "waiting_approval".into(),
            steps_completed: 3,
            steps_total: Some(5),
            tokens_used: 8900,
            duration_seconds: 67,
            model: "gpt-4o".into(),
            error_message: None,
        },
        SubagentInfo {
            id: "sa-3".into(),
            name: "Test Runner".into(),
            goal: "Executar suite de testes E2E".into(),
            state: "completed".into(),
            steps_completed: 15,
            steps_total: Some(15),
            tokens_used: 3200,
            duration_seconds: 234,
            model: "gpt-4o-mini".into(),
            error_message: None,
        },
        SubagentInfo {
            id: "sa-4".into(),
            name: "Deploy Agent".into(),
            goal: "Deploy staging environment".into(),
            state: "failed".into(),
            steps_completed: 2,
            steps_total: Some(8),
            tokens_used: 1500,
            duration_seconds: 45,
            model: "claude-3-haiku".into(),
            error_message: Some("Connection timeout to staging server".into()),
        },
    ])
}
