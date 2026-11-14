//! Agent Awareness Panel — Exibe estados de consciência dos agentes em tempo real (Fase 8).
//!
//! Consome endpoints via `/api/v1/hermes/desktop/awareness/*` e exibe:
//! - Grid de agentes com ícone, estado e cor CSS
//! - Contador por estado
//! - Lista de agentes requerendo atenção (WaitingApproval, Error)

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Estado de um agente individual.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentAwarenessEntry {
    pub agent_id: String,
    pub state: String,
    pub icon: String,
    pub message: Option<String>,
}

/// Estatísticas agregadas de awareness.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AwarenessStats {
    pub total_agents: usize,
    pub idle: usize,
    pub thinking: usize,
    pub executing: usize,
    pub waiting_approval: usize,
    pub error: usize,
    pub completed: usize,
    pub attention_required: usize,
}

async fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let full_url = format!("http://localhost:3000{}", url);
    let resp = gloo_net::http::Request::get(&full_url)
        .send()
        .await
        .map_err(|e| format!("fetch error: {e}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<serde_json::Value>()
        .await
        .map_err(|e| format!("json parse: {e}"))
}

/// Componente AgentAwarenessPanel para o cockpit.
#[component]
pub fn AgentAwarenessPanel() -> impl IntoView {
    let stats = RwSignal::new(AwarenessStats::default());
    let agents = RwSignal::new(Vec::<AgentAwarenessEntry>::new());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            error_msg.set(None);

            let mut s = AwarenessStats::default();

            // Fetch awareness stats
            match fetch_json("/api/v1/hermes/desktop/awareness/states").await {
                Ok(json) => {
                    if let Some(states_map) = json.get("state_counts").and_then(|v| v.as_object()) {
                        s.idle =
                            states_map.get("idle").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                        s.thinking = states_map
                            .get("thinking")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        s.executing = states_map
                            .get("executing")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        s.waiting_approval = states_map
                            .get("waiting_approval")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        s.error = states_map
                            .get("error")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                        s.completed = states_map
                            .get("completed")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;
                    }
                    s.total_agents = json
                        .get("total_agents")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    s.attention_required = json
                        .get("attention_required")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                }
                Err(e) => {
                    error_msg.set(Some(format!("Awareness: {e}")));
                }
            }
            stats.set(s);

            // Fetch agents requiring attention
            if let Ok(json) = fetch_json("/api/v1/hermes/desktop/awareness/attention").await {
                if let Some(arr) = json.get("agents").and_then(|v| v.as_array()) {
                    let entries: Vec<AgentAwarenessEntry> = arr
                        .iter()
                        .map(|item| AgentAwarenessEntry {
                            agent_id: item
                                .get("agent_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .into(),
                            state: item
                                .get("state")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .into(),
                            icon: match item.get("state").and_then(|v| v.as_str()).unwrap_or("") {
                                "waiting_approval" => "✋".into(),
                                "error" => "❌".into(),
                                _ => "⚠️".into(),
                            },
                            message: item
                                .get("message")
                                .and_then(|v| v.as_str())
                                .map(|s| s.into()),
                        })
                        .collect();
                    agents.set(entries);
                }
            }

            loading.set(false);
        });
    });

    view! {
        <div class="agent-awareness-panel p-4 space-y-4">
            <h2 class="text-xl font-bold text-[var(--foreground)]">"🧠 Agent Awareness"</h2>

            {move || {
                if loading.get() {
                    view! { <div class="text-[var(--muted-foreground)]">"Loading agent states..."</div> }.into_any()
                } else if let Some(err) = error_msg.get() {
                    view! { <div class="text-[var(--color-error)]">{err}</div> }.into_any()
                } else {
                    let s = stats.get();
                    let attention_agents = agents.get();
                    view! {
                        <div class="space-y-4">
                            // Stats grid
                            <div class="grid grid-cols-3 gap-3">
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"💤 Idle"</div>
                                    <div class="text-lg font-bold">{s.idle}</div>
                                </div>
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"🤔 Thinking"</div>
                                    <div class="text-lg font-bold">{s.thinking}</div>
                                </div>
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"⚡ Executing"</div>
                                    <div class="text-lg font-bold">{s.executing}</div>
                                </div>
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"✋ Waiting"</div>
                                    <div class="text-lg font-bold text-[var(--color-info)]">{s.waiting_approval}</div>
                                </div>
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"❌ Error"</div>
                                    <div class="text-lg font-bold text-[var(--color-error)]">{s.error}</div>
                                </div>
                                <div class="p-2 rounded border border-[var(--border)] bg-[var(--card)] text-center">
                                    <div class="text-xs text-[var(--muted-foreground)]">"✅ Completed"</div>
                                    <div class="text-lg font-bold text-[var(--color-success)]">{s.completed}</div>
                                </div>
                            </div>

                            // Attention required section
                            {if s.attention_required > 0 {
                                view! {
                                    <div class="p-3 rounded border border-[var(--color-error)] bg-[var(--card)]">
                                        <h3 class="text-sm font-bold text-[var(--color-error)] mb-2">
                                            {"⚠️ Agents Requiring Attention ("}{s.attention_required}{")"}
                                        </h3>
                                        <ul class="space-y-1">
                                            {attention_agents.iter().map(|a| {
                                                let id = a.agent_id.clone();
                                                let state = a.state.clone();
                                                let icon = a.icon.clone();
                                                let msg = a.message.clone().unwrap_or_default();
                                                view! {
                                                    <li class="text-sm flex items-center gap-2">
                                                        <span>{icon}</span>
                                                        <span class="font-mono">{id}</span>
                                                        <span class="text-[var(--muted-foreground)]">"— "{state}</span>
                                                        {if !msg.is_empty() {
                                                            view! { <span class="text-xs text-[var(--muted-foreground)]">"("{msg}")"</span> }.into_any()
                                                        } else {
                                                            ().into_any()
                                                        }}
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="text-sm text-[var(--color-success)]">"✅ No agents require attention"</div>
                                }.into_any()
                            }}
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
