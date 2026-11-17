//! Evidence Ledger Audit Trail Panel — exibe entradas do ledger Merkle
//!
//! Consome: GET /api/v1/phase2/status → subsystems.evidence_ledger
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerStats {
    pub total_entries: u64,
    pub merkle_root: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LedgerState {
    Loading,
    Empty,
    Success(LedgerStats),
    Error(String),
}

#[component]
pub fn EvidenceLedgerPanel() -> impl IntoView {
    let (state, set_state) = signal(LedgerState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_ledger_stats().await {
                Ok(stats) => {
                    if stats.total_entries == 0 {
                        set_state.set(LedgerState::Empty);
                    } else {
                        set_state.set(LedgerState::Success(stats));
                    }
                }
                Err(e) => set_state.set(LedgerState::Error(e)),
            }
        });
    });

    view! {
        <div class="evidence-ledger-panel">
            {move || match &*state.read() {
                LedgerState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                LedgerState::Empty => view! {
                    <div class="empty-state">
                        <p>"Ledger vazio"</p>
                        <p class="muted">"Entradas são adicionadas automaticamente quando eventos de auditoria são registrados."</p>
                    </div>
                }.into_any(),
                LedgerState::Success(stats) => {
                    let entries = stats.total_entries.to_string();
                    let root = stats.merkle_root.clone();
                    let root_display = if root.len() > 16 {
                        format!("{}...{}", &root[..8], &root[root.len()-8..])
                    } else {
                        root.clone()
                    };
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Entradas Totais"</span>
                                    <span class="metric-value">{entries}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Merkle Root"</span>
                                    <span class="metric-value muted" title={root.clone()}>{root_display}</span>
                                </div>
                            </div>
                            <div class="ledger-entries-placeholder">
                                <p>"Lista de entradas com hash, timestamp e tipo requer endpoint dedicado (Sprint 6.2.9)"</p>
                                <p class="muted">"Integridade verificável via SHA-256 chain"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                LedgerState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar ledger: {}", crate::ui_error::report("Erro ao carregar ledger", &err_msg))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_ledger_stats().await {
                                        Ok(s) => if s.total_entries == 0 {
                                            set_state.set(LedgerState::Empty);
                                        } else {
                                            set_state.set(LedgerState::Success(s));
                                        },
                                        Err(e) => set_state.set(LedgerState::Error(e)),
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

async fn fetch_ledger_stats() -> Result<LedgerStats, String> {
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
    let el = v
        .get("subsystems")
        .and_then(|s| s.get("evidence_ledger"))
        .ok_or("evidence_ledger not found in response")?;

    if el.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err("evidence_ledger subsystem error".to_string());
    }

    Ok(LedgerStats {
        total_entries: el
            .get("total_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        merkle_root: el
            .get("merkle_root")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}
