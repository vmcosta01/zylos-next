//! Goals/OKRs Panel — exibe objetivos, milestones e key results
//!
//! Consome: GET /api/v1/goals/stats → DashboardSummary
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoalsStats {
    pub total_goals: u64,
    pub active_goals: u64,
    pub completed_goals: u64,
    pub avg_kr_progress_pct: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoalsState {
    Loading,
    Empty,
    Success(GoalsStats),
    Error(String),
}

#[component]
pub fn GoalsPanel() -> impl IntoView {
    let (state, set_state) = signal(GoalsState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_goals_stats().await {
                Ok(stats) => {
                    if stats.total_goals == 0 {
                        set_state.set(GoalsState::Empty);
                    } else {
                        set_state.set(GoalsState::Success(stats));
                    }
                }
                Err(e) => set_state.set(GoalsState::Error(e)),
            }
        });
    });

    view! {
        <div class="goals-panel">
            {move || match &*state.read() {
                GoalsState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                GoalsState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhum objetivo definido"</p>
                        <p class="muted">"Crie OKRs via CLI ou API para rastrear metas do sistema."</p>
                        <p class="muted">"Exemplo: zylos-cli goals create --title 'Lançar MVP' --owner tech-lead"</p>
                    </div>
                }.into_any(),
                GoalsState::Success(stats) => {
                    let total = stats.total_goals.to_string();
                    let active = stats.active_goals.to_string();
                    let completed = stats.completed_goals.to_string();
                    let progress = format!("{:.1}%", stats.avg_kr_progress_pct);
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Total de Objetivos"</span>
                                    <span class="metric-value">{total}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Ativos"</span>
                                    <span class="metric-value success">{active}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Concluídos"</span>
                                    <span class="metric-value muted">{completed}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Progresso Médio KR"</span>
                                    <span class="metric-value">{progress}</span>
                                </div>
                            </div>
                            <div class="goals-list-placeholder">
                                <p>"Lista detalhada de goals com milestones e KRs requer endpoint dedicado (Sprint 6.2.6)"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                GoalsState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar goals: {}", crate::ui_error::report("Erro ao carregar goals", &err_msg))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_goals_stats().await {
                                        Ok(s) => if s.total_goals == 0 {
                                            set_state.set(GoalsState::Empty);
                                        } else {
                                            set_state.set(GoalsState::Success(s));
                                        },
                                        Err(e) => set_state.set(GoalsState::Error(e)),
                                    }
                                });
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

async fn fetch_goals_stats() -> Result<GoalsStats, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init(
        &crate::gateway_http::api_url("/api/v1/goals/stats")?,
        &opts,
    )
    .map_err(|e| format!("{:?}", e))?;

    crate::auth::authed_web_request(&request);

    let window = web_sys::window().ok_or("no window")?;
    let resp_value = window
        .fetch_with_request(&request)
        .await
        .map_err(|e| format!("{:?}", e))?;

    let response: Response = resp_value.dyn_into().map_err(|_| "not a Response")?;

    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }

    let text_js = response
        .text()
        .map_err(|e| format!("{:?}", e))?
        .await
        .map_err(|e| format!("{:?}", e))?;
    let text: String = text_js.as_string().ok_or("response text is not a string")?;

    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    Ok(GoalsStats {
        total_goals: v.get("total_goals").and_then(|v| v.as_u64()).unwrap_or(0),
        active_goals: v.get("active_goals").and_then(|v| v.as_u64()).unwrap_or(0),
        completed_goals: v
            .get("completed_goals")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        avg_kr_progress_pct: v
            .get("avg_kr_progress_pct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
    })
}
