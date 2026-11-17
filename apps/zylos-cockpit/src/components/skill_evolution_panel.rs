//! Skill Evolution Panel — Exibe contratos, grafo de evolução e hierarquia de skills (Fase 8).
//!
//! Consome endpoints via `/api/v1/hermes/skill-evolution/*` e exibe:
//! - Contagem de contratos ativos por tier (Planning/Functional/Atomic)
//! - Resumo do grafo de evolução (nós e arestas)
//! - Status do miner e trajectory compiler

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Estado do painel de evolução de skills.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SkillEvolutionState {
    pub total_contracts: usize,
    pub planning_tier: usize,
    pub functional_tier: usize,
    pub atomic_tier: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub edge_types: Vec<String>,
    pub miner_status: String,
    pub trajectory_status: String,
}

async fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let full_url = format!("http://localhost:3000{}", url);
    let resp = crate::auth::authed(gloo_net::http::Request::get(&full_url))
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

/// Componente SkillEvolutionPanel para o cockpit.
#[component]
pub fn SkillEvolutionPanel() -> impl IntoView {
    let state = RwSignal::new(SkillEvolutionState::default());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            error_msg.set(None);

            let mut s = SkillEvolutionState::default();

            // Fetch hierarchy stats
            match fetch_json("/api/v1/hermes/skill-evolution/hierarchy").await {
                Ok(json) => {
                    s.total_contracts =
                        json.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    s.planning_tier = json
                        .get("planning_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    s.functional_tier = json
                        .get("functional_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    s.atomic_tier = json
                        .get("atomic_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                }
                Err(e) => {
                    error_msg.set(Some(format!("Hierarchy: {e}")));
                }
            }

            // Fetch evolution graph summary
            if let Ok(json) = fetch_json("/api/v1/hermes/skill-evolution/graph").await {
                s.graph_nodes = json.get("nodes").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                s.graph_edges = json.get("edges").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                if let Some(types) = json.get("edge_types").and_then(|v| v.as_array()) {
                    s.edge_types = types
                        .iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect();
                }
            }

            // Check contracts endpoint availability
            match fetch_json("/api/v1/hermes/skill-evolution/contracts").await {
                Ok(_) => {
                    s.miner_status = "available".into();
                    s.trajectory_status = "available".into();
                }
                Err(_) => {
                    s.miner_status = "unavailable".into();
                    s.trajectory_status = "unavailable".into();
                }
            }

            state.set(s);
            loading.set(false);
        });
    });

    view! {
        <div class="skill-evolution-panel p-4 space-y-4">
            <h2 class="text-xl font-bold text-[var(--foreground)]">"🔄 Skill Evolution"</h2>

            {move || {
                if loading.get() {
                    view! { <div class="text-[var(--muted-foreground)]">"Loading skill evolution data..."</div> }.into_any()
                } else if let Some(err) = error_msg.get() {
                    view! { <div class="text-[var(--color-error)]">{err}</div> }.into_any()
                } else {
                    let s = state.get();
                    view! {
                        <div class="space-y-4">
                            // Tier distribution
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <h3 class="text-sm font-bold text-[var(--foreground)] mb-2">"Skill Hierarchy ("{s.total_contracts}" contracts)"</h3>
                                <div class="grid grid-cols-3 gap-2">
                                    <div class="text-center p-2 rounded bg-[var(--accent)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"🎯 Planning"</div>
                                        <div class="text-lg font-bold">{s.planning_tier}</div>
                                    </div>
                                    <div class="text-center p-2 rounded bg-[var(--accent)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"⚙️ Functional"</div>
                                        <div class="text-lg font-bold">{s.functional_tier}</div>
                                    </div>
                                    <div class="text-center p-2 rounded bg-[var(--accent)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"🔧 Atomic"</div>
                                        <div class="text-lg font-bold">{s.atomic_tier}</div>
                                    </div>
                                </div>
                            </div>

                            // Evolution graph summary
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <h3 class="text-sm font-bold text-[var(--foreground)] mb-2">"Evolution Graph"</h3>
                                <div class="flex gap-4 text-sm">
                                    <div>
                                        <span class="text-[var(--muted-foreground)]">"Nodes: "</span>
                                        <span class="font-bold">{s.graph_nodes}</span>
                                    </div>
                                    <div>
                                        <span class="text-[var(--muted-foreground)]">"Edges: "</span>
                                        <span class="font-bold">{s.graph_edges}</span>
                                    </div>
                                </div>
                                {if !s.edge_types.is_empty() {
                                    view! {
                                        <div class="mt-2 flex flex-wrap gap-1">
                                            {s.edge_types.iter().map(|et| {
                                                let label = et.clone();
                                                view! {
                                                    <span class="px-2 py-0.5 text-xs rounded bg-[var(--accent)] bg-opacity-30 text-[var(--foreground)]">
                                                        {label}
                                                    </span>
                                                }
                                            }).collect_view()}
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            </div>

                            // Pipeline status
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <h3 class="text-sm font-bold text-[var(--foreground)] mb-2">"Pipeline Status"</h3>
                                <div class="space-y-1 text-sm">
                                    <div class="flex justify-between">
                                        <span class="text-[var(--muted-foreground)]">"Skill Miner"</span>
                                        <span class={if s.miner_status == "available" { "text-[var(--color-success)]" } else { "text-[var(--color-warning)]" }}>
                                            {if s.miner_status == "available" { "✅ Available" } else { "⚠️ Unavailable" }}
                                        </span>
                                    </div>
                                    <div class="flex justify-between">
                                        <span class="text-[var(--muted-foreground)]">"Trajectory Compiler"</span>
                                        <span class={if s.trajectory_status == "available" { "text-[var(--color-success)]" } else { "text-[var(--color-warning)]" }}>
                                            {if s.trajectory_status == "available" { "✅ Available" } else { "⚠️ Unavailable" }}
                                        </span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
