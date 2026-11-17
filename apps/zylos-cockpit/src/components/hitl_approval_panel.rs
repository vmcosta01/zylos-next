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
                            <p>{format!("Erro ao carregar aprovações: {}", crate::ui_error::report("Erro ao carregar aprovações", &err_msg))}</p>
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
            <ResolveApprovalForm />
        </div>
    }
}

/// Formulário para resolver um pedido HITL (POST /api/v1/approval/resolve).
#[component]
fn ResolveApprovalForm() -> impl IntoView {
    let approval_id = RwSignal::new(String::new());
    let msg = RwSignal::new(None::<String>);

    let decide = move |decision: &'static str| {
        let id = approval_id.get_untracked();
        if id.trim().is_empty() {
            msg.set(Some("Informe o ID da aprovação.".into()));
            return;
        }
        spawn_local(async move {
            let text = match resolve_approval(&id, decision).await {
                Ok(()) => format!("{id}: {decision}"),
                Err(e) => crate::ui_error::report("approval resolve", &e),
            };
            msg.set(Some(text));
        });
    };

    view! {
        <div class="hitl-resolve">
            <label for="hitl-approval-id">"ID da aprovação"</label>
            <input
                id="hitl-approval-id"
                type="text"
                prop:value=move || approval_id.get()
                on:input=move |ev| approval_id.set(event_target_value(&ev))
            />
            <button class="cta-primary" on:click=move |_| decide("allow_once")>"Aprovar"</button>
            <button class="cta-retry" on:click=move |_| decide("refuse")>"Recusar"</button>
            {move || msg.get().map(|m| view! { <p class="hitl-resolve-msg" role="status">{m}</p> })}
        </div>
    }
}

/// POST /api/v1/approval/resolve. 401/403 → pede login de administrador.
async fn resolve_approval(approval_id: &str, decision: &str) -> Result<(), String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/approval/resolve");
    let body = serde_json::json!({
        "approval_id": approval_id,
        "decision": decision,
        "resolved_by": "cockpit",
    });
    let resp = crate::auth::authed(gloo_net::http::Request::post(&url))
        .json(&body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(crate::auth::approval_error(resp.status()))
    }
}

async fn fetch_pending_approvals() -> Result<u64, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request =
        Request::new_with_str_and_init("http://localhost:3000/api/v1/phase2/status", &opts)
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
    let pending = v
        .get("subsystems")
        .and_then(|s| s.get("browser_takeover"))
        .and_then(|s| s.get("pending_sessions"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    Ok(pending)
}
