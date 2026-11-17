//! Sentinel/AIOps Alerts Panel — exibe alertas de anomalia e saúde do sistema
//!
//! Consome: GET /api/v1/phase2/status → subsystems.agent_shield + browser_takeover
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SentinelStats {
    pub total_audit_events: u64,
    pub critical_events: u64,
    pub high_events: u64,
    pub outbound_blocked: u64,
    pub active_cdp_sessions: u64,
    pub pending_cdp_sessions: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SentinelState {
    Loading,
    Empty,
    Success(SentinelStats),
    Error(String),
}

#[component]
pub fn SentinelPanel() -> impl IntoView {
    let (state, set_state) = signal(SentinelState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_sentinel_stats().await {
                Ok(stats) => {
                    if stats.total_audit_events == 0 && stats.active_cdp_sessions == 0 {
                        set_state.set(SentinelState::Empty);
                    } else {
                        set_state.set(SentinelState::Success(stats));
                    }
                }
                Err(e) => set_state.set(SentinelState::Error(e)),
            }
        });
    });

    view! {
        <div class="sentinel-panel">
            {move || match &*state.read() {
                SentinelState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                SentinelState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhum evento de segurança registrado"</p>
                        <p class="muted">"O Agent Shield e CDP Proxy monitoram automaticamente. Alertas aparecerão aqui quando anomalias forem detectadas."</p>
                    </div>
                }.into_any(),
                SentinelState::Success(stats) => {
                    let audit = stats.total_audit_events.to_string();
                    let critical = stats.critical_events.to_string();
                    let blocked = stats.outbound_blocked.to_string();
                    let cdp_active = stats.active_cdp_sessions.to_string();
                    let cdp_pending = stats.pending_cdp_sessions.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Eventos de Auditoria"</span>
                                    <span class="metric-value">{audit}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Críticos"</span>
                                    <span class="metric-value" style="color: var(--accent-red)">{critical}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Outbound Bloqueados"</span>
                                    <span class="metric-value muted">{blocked}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"CDP Ativas"</span>
                                    <span class="metric-value muted">{cdp_active}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"CDP Pendentes"</span>
                                    <span class="metric-value muted">{cdp_pending}</span>
                                </div>
                            </div>
                            <div class="sentinel-alerts-placeholder">
                                <p>"Timeline de alertas com severidade e correlação requer endpoint dedicado (Sprint 6.2.8)"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                SentinelState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar sentinel: {}", err_msg)}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_sentinel_stats().await {
                                        Ok(s) => if s.total_audit_events == 0 && s.active_cdp_sessions == 0 {
                                            set_state.set(SentinelState::Empty);
                                        } else {
                                            set_state.set(SentinelState::Success(s));
                                        },
                                        Err(e) => set_state.set(SentinelState::Error(e)),
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

async fn fetch_sentinel_stats() -> Result<SentinelStats, String> {
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
    let subs = v.get("subsystems").ok_or("subsystems not found")?;

    let shield = subs.get("agent_shield");
    let cdp = subs.get("browser_takeover");

    Ok(SentinelStats {
        total_audit_events: shield
            .and_then(|s| s.get("total_audit_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        critical_events: shield
            .and_then(|s| s.get("critical_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        high_events: shield
            .and_then(|s| s.get("high_events"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        outbound_blocked: shield
            .and_then(|s| s.get("outbound_blocked"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        active_cdp_sessions: cdp
            .and_then(|s| s.get("active_sessions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        pending_cdp_sessions: cdp
            .and_then(|s| s.get("pending_sessions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
    })
}
