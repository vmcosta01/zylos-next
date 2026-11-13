//! Employee Governance Panel — Exibe roles, rotinas, plan-first gate e rate limiter (Fase 8).
//!
//! Consome endpoints via `/api/v1/hermes/employee/*` e exibe:
//! - Grid dos 8 roles padronizados com contagem de employees
//! - Status do Plan-First Gate (pendentes/aprovados/rejeitados)
//! - Estatísticas do Rate Limiter por role

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Estado do painel de governança de employees.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EmployeeGovernanceState {
 pub roles: Vec<RoleEntry>,
 pub pending_plans: usize,
 pub approved_plans: usize,
 pub rejected_plans: usize,
 pub rate_limited_employees: usize,
 pub total_routines: usize,
}

/// Entrada de um role individual.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoleEntry {
 pub name: String,
 pub display_name: String,
 pub employee_count: usize,
 pub routine_count: usize,
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

/// Componente EmployeeGovernancePanel para o cockpit.
#[component]
pub fn EmployeeGovernancePanel() -> impl IntoView {
 let state = RwSignal::new(EmployeeGovernanceState::default());
 let loading = RwSignal::new(true);
 let error_msg = RwSignal::new(None::<String>);

 Effect::new(move |_| {
 spawn_local(async move {
 loading.set(true);
 error_msg.set(None);

 let mut s = EmployeeGovernanceState::default();

 // Fetch roles
 match fetch_json("/api/v1/hermes/employee/roles").await {
 Ok(json) => {
 if let Some(roles_arr) = json.get("roles").and_then(|v| v.as_array()) {
 s.roles = roles_arr
 .iter()
 .filter_map(|r| {
 r.as_str().map(|name| RoleEntry {
 name: name.into(),
 display_name: name.replace('_', " "),
 employee_count: 0,
 routine_count: 0,
 })
 })
 .collect();
 }
 }
 Err(e) => { error_msg.set(Some(format!("Roles: {e}"))); }
 }

 // Fetch routine catalog summary
 if let Ok(json) = fetch_json("/api/v1/hermes/employee/routines").await {
 s.total_routines = json.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
 }

 // Fetch rate limiter stats
 if let Ok(json) = fetch_json("/api/v1/hermes/employee/rate-limiter/stats").await {
 s.rate_limited_employees = json.get("throttled_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
 }

 state.set(s);
 loading.set(false);
 });
 });

    view! {
        <div class="employee-governance-panel p-4 space-y-4">
            <h2 class="text-xl font-bold text-[var(--foreground)]">"👥 Employee Governance"</h2>

            {move || {
                if loading.get() {
                    view! { <div class="text-[var(--muted-foreground)]">"Loading employee data..."</div> }
                        .into_any()
                } else if let Some(err) = error_msg.get() {
                    view! { <div class="text-[var(--color-error)]">{err}</div> }.into_any()
                } else {
                    let s = state.get();
                    view! {
                        <div class="space-y-4">
                            // Roles grid
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <h3 class="text-sm font-bold text-[var(--foreground)] mb-2">
                                    "Standardized Roles ("{s.roles.len()}{")"}
                                </h3>
                                <div class="grid grid-cols-2 gap-2">
                                    {s.roles.iter().map(|role| {
                                        let name = role.display_name.clone();
                                        let count = role.employee_count;
                                        view! {
                                            <div class="flex justify-between items-center p-2 rounded bg-[var(--accent)] bg-opacity-10 text-sm">
                                                <span class="capitalize text-[var(--foreground)]">{name}</span>
                                                <span class="text-[var(--muted-foreground)]">{count}" emp"</span>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>

                            // Plan-First Gate status
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <h3 class="text-sm font-bold text-[var(--foreground)] mb-2">"Plan-First Gate"</h3>
                                <div class="grid grid-cols-3 gap-2 text-center">
                                    <div class="p-2 rounded bg-[var(--color-warning)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"⏳ Pending"</div>
                                        <div class="text-lg font-bold">{s.pending_plans}</div>
                                    </div>
                                    <div class="p-2 rounded bg-[var(--color-success)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"✅ Approved"</div>
                                        <div class="text-lg font-bold">{s.approved_plans}</div>
                                    </div>
                                    <div class="p-2 rounded bg-[var(--color-error)] bg-opacity-20">
                                        <div class="text-xs text-[var(--muted-foreground)]">"❌ Rejected"</div>
                                        <div class="text-lg font-bold">{s.rejected_plans}</div>
                                    </div>
                                </div>
                            </div>

                            // Rate limiter + routines summary
                            <div class="grid grid-cols-2 gap-3">
                                <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                    <div class="text-sm text-[var(--muted-foreground)]">"🚦 Rate Limited"</div>
                                    <div class="text-2xl font-bold text-[var(--foreground)]">
                                        {s.rate_limited_employees}
                                    </div>
                                </div>
                                <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                    <div class="text-sm text-[var(--muted-foreground)]">"📋 Total Routines"</div>
                                    <div class="text-2xl font-bold text-[var(--foreground)]">
                                        {s.total_routines}
                                    </div>
                                </div>
                            </div>
                        </div>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}