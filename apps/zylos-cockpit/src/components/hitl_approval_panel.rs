//! HITL Approval Panel — exibe pedidos de aprovação pendentes e resolve
//!
//! Consome: GET /api/v1/phase2/status → subsystems.browser_takeover.pending_sessions
//! Estados obrigatórios: Loading, Empty, HasPending, Error, Disabled, Validation

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq)]
pub enum HitlState {
    Loading,
    Empty,
    HasPending(u64),
    Error(String),
}

#[component]
pub fn HitlApprovalPanel() -> impl IntoView {
    let (state, set_state) = signal(HitlState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_pending_approvals().await {
                Ok(0) => set_state.set(HitlState::Empty),
                Ok(n) => set_state.set(HitlState::HasPending(n)),
                Err(e) => set_state.set(HitlState::Error(e)),
            }
        });
    });

    view! {
        <div class="hitl-approval-panel">
            {move || match &*state.read() {
                HitlState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                HitlState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhuma aprovação pendente"</p>
                        <p class="muted">"Pedidos de aprovação HITL aparecerão aqui quando agentes solicitarem ações que requerem confirmação humana."</p>
                        <p class="muted">"Endpoint disponível: POST /api/v1/approval/resolve"</p>
                    </div>
                }.into_any(),
                HitlState::HasPending(count) => {
                    let n = count.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Aprovações Pendentes"</span>
                                    <span class="metric-value" style="color: var(--accent-yellow)">{n}</span>
                                </div>
                            </div>
                            <div class="hitl-cards-placeholder">
                                <p>"Cards interativos de aprovação com detalhes da solicitação requerem integração completa com approval store (Sprint 6.2.10)"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                HitlState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar aprovações: {}", err_msg)}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_pending_approvals().await {
                                        Ok(0) => set_state.set(HitlState::Empty),
                                        Ok(n) => set_state.set(HitlState::HasPending(n)),
                                        Err(e) => set_state.set(HitlState::Error(e)),
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

async fn fetch_pending_approvals() -> Result<u64, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init("http://localhost:3000/api/v1/phase2/status", &opts)
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

    let text_js = response.text().map_err(|e| format!("{:?}", e))?.await.map_err(|e| format!("{:?}", e))?;
    let text: String = text_js.as_string().ok_or("response text is not a string")?;

    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let pending = v.get("subsystems")
        .and_then(|s| s.get("browser_takeover"))
        .and_then(|s| s.get("pending_sessions"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Ok(pending)
}