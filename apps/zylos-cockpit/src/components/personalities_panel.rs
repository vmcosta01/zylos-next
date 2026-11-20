//! Personalities Panel — exibe registry de personalidades de agentes
//!
//! Consome: GET /api/v1/phase2/status → subsystems.personalities
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersonalityStats {
    pub total_profiles: u64,
    pub divisions: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PersonalitiesState {
    Loading,
    Empty,
    Success(PersonalityStats),
    Error(String),
}

#[component]
pub fn PersonalitiesPanel() -> impl IntoView {
    let (state, set_state) = signal(PersonalitiesState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_personality_stats().await {
                Ok(stats) => {
                    if stats.total_profiles == 0 {
                        set_state.set(PersonalitiesState::Empty);
                    } else {
                        set_state.set(PersonalitiesState::Success(stats));
                    }
                }
                Err(e) => set_state.set(PersonalitiesState::Error(e)),
            }
        });
    });

    view! {
        <div class="personalities-panel">
            {move || match &*state.read() {
                PersonalitiesState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                PersonalitiesState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhuma personalidade registrada"</p>
                        <p class="muted">"Personalidades são seedeadas automaticamente na primeira execução do gateway."</p>
                    </div>
                }.into_any(),
                PersonalitiesState::Success(stats) => {
                    let profiles = stats.total_profiles.to_string();
                    let divs = stats.divisions.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Perfis Totais"</span>
                                    <span class="metric-value">{profiles}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Divisões"</span>
                                    <span class="metric-value muted">{divs}</span>
                                </div>
                            </div>
                            <div class="personalities-list-placeholder">
                                <p>"Lista detalhada de personalidades por divisão requer endpoint dedicado (Sprint 6.2.7)"</p>
                                <p class="muted">"Divisões disponíveis: Engineering, Research, Operations, Creative, Security, Management, Support, Custom"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                PersonalitiesState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar personalidades: {}", crate::ui_error::report("Erro ao carregar personalidades", &err_msg))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_personality_stats().await {
                                        Ok(s) => if s.total_profiles == 0 {
                                            set_state.set(PersonalitiesState::Empty);
                                        } else {
                                            set_state.set(PersonalitiesState::Success(s));
                                        },
                                        Err(e) => set_state.set(PersonalitiesState::Error(e)),
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

async fn fetch_personality_stats() -> Result<PersonalityStats, String> {
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
    let ps = v
        .get("subsystems")
        .and_then(|s| s.get("personalities"))
        .ok_or("personalities not found in response")?;

    if ps.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err("personalities subsystem error".to_string());
    }

    Ok(PersonalityStats {
        total_profiles: ps
            .get("total_profiles")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        divisions: ps.get("divisions").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
    })
}
