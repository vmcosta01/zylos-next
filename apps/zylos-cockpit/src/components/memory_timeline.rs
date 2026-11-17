//! Memory Timeline Component — visualiza entradas de memória cross-harness
//!
//! Consome: GET /api/v1/phase2/status → subsystems.memory_sync
//! Estados obrigatórios: Loading, Empty, Success, Error, Disabled, Validation

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_entries: u64,
    pub pending_sync_ops: u64,
    pub namespaces: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MemoryState {
    Loading,
    Empty,
    Success(MemoryStats),
    Error(String),
    Disabled(String),
}

#[component]
pub fn MemoryTimeline() -> impl IntoView {
    let (state, set_state) = signal(MemoryState::Loading);

    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_memory_stats().await {
                Ok(stats) => {
                    if stats.total_entries == 0 {
                        set_state.set(MemoryState::Empty);
                    } else {
                        set_state.set(MemoryState::Success(stats));
                    }
                }
                Err(e) if e == "__DISABLED__" => set_state.set(MemoryState::Disabled(
                    "memory_sync não inicializado no gateway".to_string(),
                )),
                Err(e) => set_state.set(MemoryState::Error(e)),
            }
        });
    });

    view! {
        <div class="memory-timeline">
            {move || match &*state.read() {
                MemoryState::Loading => view! {
                    <div class="skeleton-grid">
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                        <div class="skeleton-card pulse"></div>
                    </div>
                }.into_any(),
                MemoryState::Empty => view! {
                    <div class="empty-state">
                        <p>"Nenhuma entrada de memória sincronizada"</p>
                        <p class="muted">"As memórias dos agentes aparecerão aqui após a primeira sincronização CRDT."</p>
                    </div>
                }.into_any(),
                MemoryState::Success(stats) => {
                    let entries = stats.total_entries.to_string();
                    let pending = stats.pending_sync_ops.to_string();
                    let ns = stats.namespaces.to_string();
                    view! {
                        <div class="dashboard-content">
                            <div class="metrics-bar">
                                <div class="metric-card">
                                    <span class="metric-label">"Entradas Totais"</span>
                                    <span class="metric-value">{entries}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Ops Pendentes"</span>
                                    <span class="metric-value muted">{pending}</span>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Namespaces"</span>
                                    <span class="metric-value muted">{ns}</span>
                                </div>
                            </div>
                            <div class="memory-entries-placeholder">
                                <p>"Timeline detalhada de entradas individuais requer endpoint dedicado (Sprint 6.2.4)"</p>
                            </div>
                        </div>
                    }.into_any()
                }
                MemoryState::Error(msg) => {
                    let err_msg = msg.clone();
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar memória: {}", err_msg)}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    match fetch_memory_stats().await {
                                        Ok(s) => if s.total_entries == 0 {
                                            set_state.set(MemoryState::Empty);
                                        } else {
                                            set_state.set(MemoryState::Success(s));
                                        },
                                        Err(e) => set_state.set(MemoryState::Error(e)),
                                    }
                                });
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                }
                MemoryState::Disabled(reason) => {
                    let r = reason.clone();
                    view! {
                        <div class="disabled-overlay">
                            <p>{format!("Memory sync desativado: {}", r)}</p>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

async fn fetch_memory_stats() -> Result<MemoryStats, String> {
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
    let ms = v
        .get("subsystems")
        .and_then(|s| s.get("memory_sync"))
        .ok_or("memory_sync not found in response")?;

    // Detect disabled subsystem — gateway returns {"status":"error"} when memory_sync is not initialized
    if ms.get("status").and_then(|s| s.as_str()) == Some("error") {
        return Err("__DISABLED__".to_string());
    }

    Ok(MemoryStats {
        total_entries: ms
            .get("total_entries")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        pending_sync_ops: ms
            .get("pending_sync_ops")
            .and_then(|v| v.as_u64())
            .unwrap_or(0),
        namespaces: ms.get("namespaces").and_then(|v| v.as_u64()).unwrap_or(0) as usize,
    })
}
