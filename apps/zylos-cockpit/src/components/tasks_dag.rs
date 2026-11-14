//! Tasks DAG Visualization — exibe tarefas do Swarm Orchestrator como grafo
//!
//! Consome: GET /api/v1/phase2/status → subsystems.swarm_orchestrator
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwarmStats {
    pub total_tasks: u64,
    pub running_tasks: u64,
    pub total_agents: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TasksState {
    Loading,
    Empty,
    Success(SwarmStats),
    Error(String),
    Disabled(String),
}

#[component]
pub fn TasksDag() -> impl IntoView {
    let (state, set_state) = signal(TasksState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_swarm_stats().await {
                Ok(stats) => {
                    if stats.total_tasks == 0 {
                        set_state.set(TasksState::Empty);
                    } else {
                        set_state.set(TasksState::Success(stats));
                    }
                }
                Err(e) if e == "__DISABLED__" => set_state.set(TasksState::Disabled(
                    "swarm_orchestrator não inicializado no gateway".to_string(),
                )),
                Err(e) => set_state.set(TasksState::Error(e)),
            }
        });
    });

    view! {
        <div class="tasks-dag">
            {move || match &*state.read() {
                TasksState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                TasksState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhuma tarefa no orquestrador"</p>
                        <p class="muted">"As tarefas do Swarm Orchestrator aparecerão aqui quando agents forem escalonados."</p>
                    </div>
                }.into_any(),
                TasksState::Success(stats) => {
                    let total = stats.total_tasks.to_string();
                    let running = stats.running_tasks.to_string();
                    let agents = stats.total_agents.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Total de Tarefas"</span>
                                    <span class="metric-value">{total}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Em Execução"</span>
                                    <span class="metric-value success">{running}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Agents Ativos"</span>
                                    <span class="metric-value muted">{agents}</span>
                                </div>
                            </div>
                            <div class="dag-visualization-placeholder">
                                <p>"Visualização gráfica do DAG requer biblioteca de renderização SVG/WebGL (Sprint 6.2.5)"</p>
                                <p class="muted">"Dados disponíveis via API: tasks, dependencies, agent assignments"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                TasksState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar tarefas: {}", err_msg)}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_swarm_stats().await {
                                        Ok(s) => if s.total_tasks == 0 {
                                            set_state.set(TasksState::Empty);
                                        } else {
                                            set_state.set(TasksState::Success(s));
                                        },
                                        Err(e) => set_state.set(TasksState::Error(e)),
                                    }
                                });
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                }
                TasksState::Disabled(reason) => {
                    let r = reason.clone();
                    view! {
                        <div class="disabled-overlay">
                            <p>{format!("Swarm orchestrator desativado: {}", r)}</p>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

async fn fetch_swarm_stats() -> Result<SwarmStats, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request =
        Request::new_with_str_and_init("http://localhost:3000/api/v1/phase2/status", &opts)
            .map_err(|e| format!("{:?}", e))?;

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

    // Detect disabled subsystem — gateway returns {"status":"error"} when swarm is not initialized
    if sw.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err("__DISABLED__".to_string());
    }

    Ok(SwarmStats {
        total_tasks: sw.get("total_tasks").and_then(|v| v.as_u64()).unwrap_or(0),
        running_tasks: sw
            .get("running_tasks")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        total_agents: sw.get("total_agents").and_then(|v| v.as_u64()).unwrap_or(0),
    })
}
