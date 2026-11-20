//! Security Panel — Exibe status de guardrails, capability tokens, egress e sanitizer (Fase 8).
//!
//! Consome endpoints via `/api/v1/hermes/security/*` e exibe:
//! - Estatísticas do GuardrailEnforcer
//! - Tokens de capacidade ativos
//! - Regras de egress de rede
//! - Pipeline de sanitização de output

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Estado do painel de segurança.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SecurityState {
    pub guardrails_active: usize,
    pub guardrails_blocked: usize,
    pub capability_tokens_active: usize,
    pub egress_rules_count: usize,
    pub sanitizer_rules_count: usize,
}

async fn fetch_json(url: &str) -> Result<serde_json::Value, String> {
    let full_url = crate::gateway_http::api_url(url)?;
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

/// Componente SecurityPanel para o cockpit.
#[component]
pub fn SecurityPanel() -> impl IntoView {
    let state = RwSignal::new(SecurityState::default());
    let loading = RwSignal::new(true);
    let error_msg = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        spawn_local(async move {
            loading.set(true);
            error_msg.set(None);

            let mut s = SecurityState::default();

            // Guardrails stats
            match fetch_json("/api/v1/hermes/security/guardrails/stats").await {
                Ok(json) => {
                    s.guardrails_active = json
                        .get("active_rules")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    s.guardrails_blocked = json
                        .get("blocked_count")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                }
                Err(e) => {
                    error_msg.set(Some(format!("Guardrails: {e}")));
                }
            }

            // Capability tokens
            if let Ok(json) = fetch_json("/api/v1/hermes/security/capability-tokens").await {
                s.capability_tokens_active = json
                    .get("tokens")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
            }

            // Egress rules
            if let Ok(json) = fetch_json("/api/v1/hermes/security/egress/rules").await {
                s.egress_rules_count = json
                    .get("rules")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
            }

            // Sanitizer rules
            if let Ok(json) = fetch_json("/api/v1/hermes/security/sanitizer/rules").await {
                s.sanitizer_rules_count = json
                    .get("rules")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
            }

            state.set(s);
            loading.set(false);
        });
    });

    view! {
        <div class="security-panel p-4 space-y-4">
            <h2 class="text-xl font-bold text-[var(--foreground)]"><i class="ph ph-shield-check" aria-hidden="true"></i>" Security Dashboard"</h2>

            {move || {
                if loading.get() {
                    view! { <div class="text-[var(--muted-foreground)]">"Loading security stats..."</div> }.into_any()
                } else if let Some(err) = error_msg.get() {
                    view! { <div class="text-[var(--color-error)]">{err}</div> }.into_any()
                } else {
                    let s = state.get();
                    view! {
                        <div class="grid grid-cols-2 gap-4">
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <div class="text-sm text-[var(--muted-foreground)]">"Guardrails Active"</div>
                                <div class="text-2xl font-bold text-[var(--foreground)]">{s.guardrails_active}</div>
                            </div>
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <div class="text-sm text-[var(--muted-foreground)]">"Blocked Decisions"</div>
                                <div class="text-2xl font-bold text-[var(--color-error)]">{s.guardrails_blocked}</div>
                            </div>
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <div class="text-sm text-[var(--muted-foreground)]">"Capability Tokens"</div>
                                <div class="text-2xl font-bold text-[var(--foreground)]">{s.capability_tokens_active}</div>
                            </div>
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <div class="text-sm text-[var(--muted-foreground)]">"Egress Rules"</div>
                                <div class="text-2xl font-bold text-[var(--foreground)]">{s.egress_rules_count}</div>
                            </div>
                            <div class="p-3 rounded border border-[var(--border)] bg-[var(--card)]">
                                <div class="text-sm text-[var(--muted-foreground)]">"Sanitizer Rules"</div>
                                <div class="text-2xl font-bold text-[var(--foreground)]">{s.sanitizer_rules_count}</div>
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}
