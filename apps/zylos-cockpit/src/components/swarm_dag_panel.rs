//! Swarm Orchestrator DAG Panel — visualização detalhada do grafo de tarefas
//!
//! Consome: GET /api/v1/phase2/status → subsystems.swarm_orchestrator
//! Complementa TasksDag com visão detalhada de dependências e agent assignments
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmDetailStats {
    pub total_tasks: u64,
    pub running_tasks: u64,
    pub total_agents: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwarmDagState {
    Loading,
    Empty,
    Success(SwarmDetailStats),
    Error(String),
}

#[component]
pub fn SwarmDagPanel() -> impl IntoView {
    let (state, set_state) = signal(SwarmDagState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_swarm_detail().await {
                Ok(stats) => {
                    if stats.total_tasks == 0 {
                        set_state.set(SwarmDagState::Empty);
                    } else {
                        set_state.set(SwarmDagState::Success(stats));
                    }
                }
                Err(e) => set_state.set(SwarmDagState::Error(e)),
            }
        });
    });

    view! {
        <div class="swarm-dag-panel">
            {move || match &*state.read() {
                SwarmDagState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                SwarmDagState::Empty => view! {
                    <div class="empty-state">
                        <p>"Orquestrador vazio"</p>
                        <p class="muted">"O DAG de tarefas será visualizado aqui quando o Swarm Orchestrator escalonar agents."</p>
                    </div>
                }.into_any(),
                SwarmDagState::Success(stats) => {
                    let total = stats.total_tasks.to_string();
                    let running = stats.running_tasks.to_string();
                    let agents = stats.total_agents.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Tasks no DAG"</span>
                                    <span class="metric-value">{total}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Executando"</span>
                                    <span class="metric-value success">{running}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Agents Escalonados"</span>
                                    <span class="metric-value muted">{agents}</span>
                                </div>
                            </div>
                            <div class="dag-detail-placeholder">
                                <p>"Visualização interativa do DAG com nós, arestas e estado de cada task requer biblioteca SVG (Sprint 6.2.11)"</p>
                                <p class="muted">"API disponível: tasks, dependencies, agent assignments, retry counts"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                SwarmDagState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar DAG: {}", crate::ui_error::report("Erro ao carregar DAG", &err_msg))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_swarm_detail().await {
                                        Ok(s) => if s.total_tasks == 0 {
                                            set_state.set(SwarmDagState::Empty);
                                        } else {
                                            set_state.set(SwarmDagState::Success(s));
                                        },
                                        Err(e) => set_state.set(SwarmDagState::Error(e)),
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

async fn fetch_swarm_detail() -> Result<SwarmDetailStats, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init(
        &crate::gateway_http::api_url("/api/v1/phase2/status")?,
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
    let sw = v
        .get("subsystems")
        .and_then(|s| s.get("swarm_orchestrator"))
        .ok_or("swarm_orchestrator not found in response")?;

    if sw.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err("swarm_orchestrator subsystem error".to_string());
    }

    Ok(SwarmDetailStats {
        total_tasks: sw.get("total_tasks").and_then(|v| v.as_u64()).unwrap_or(0),
        running_tasks: sw
            .get("running_tasks")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        total_agents: sw.get("total_agents").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}
