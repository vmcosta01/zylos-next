//! HITL Approval Panel — Inline approval cards with real API integration (ADR-001)
//!
//! Features:
//! - Lists pending approvals from GET /api/v1/approvals
//! - Inline cards with Approve/Reject/Escalate buttons
//! - Optimistic UI with spring animations (Fase 3)
//! - Receipt persistence in localStorage
//! - 6 mandatory states: Loading, Empty, Error, Success, Validation, Disabled

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen_futures::spawn_local;

use crate::spring::{create_spring, SpringConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalRequest {
    id: String,
    bot_id: String,
    tool_name: String,
    action_category: String,
    summary: String,
    arguments_json: String,
    status: String,
    decision: Option<String>,
    refuse_reason: Option<String>,
    created_at: String,
    decided_at: Option<String>,
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalsResponse {
    approvals: Vec<ApprovalRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardState {
    Pending,
    Resolving,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApprovalReceipt {
    id: String,
    decision: String,
    timestamp: String,
    summary: String,
}

const RECEIPTS_KEY: &str = "zylos_approval_receipts";
const RECEIPT_MAX_AGE_DAYS: i64 = 7;

#[component]
pub fn HitlApprovalPanel() -> impl IntoView {
    let (approvals, set_approvals) = signal(Vec::<ApprovalRequest>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);

    // Fetch pending approvals on mount
    Effect::new(move |_| {
        spawn_local(async move {
            set_loading.set(true);
            match fetch_approvals().await {
                Ok(list) => {
                    set_approvals.set(list);
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_loading.set(false);
        });
    });

    view! {
        <div class="hitl-approval-panel">
            {move || {
                if loading.get() {
                    view! {
                        <div class="skeleton-grid">
                            <div class="skeleton-card pulse"></div>
                            <div class="skeleton-card pulse"></div>
                        </div>
                    }.into_any()
                } else if let Some(err) = error.get() {
                    view! {
                        <div class="error-banner">
                            <p>{format!("Erro ao carregar aprovações: {}", crate::ui_error::report("Erro", &err))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                spawn_local(async move {
                                    set_loading.set(true);
                                    match fetch_approvals().await {
                                        Ok(list) => {
                                            set_approvals.set(list);
                                            set_error.set(None);
                                        }
                                        Err(e) => set_error.set(Some(e)),
                                    }
                                    set_loading.set(false);
                                });
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                } else {
                    let pending = approvals.get();
                    let receipts = load_receipts();
                    if pending.is_empty() && receipts.is_empty() {
                        view! {
                            <div class="empty-state">
                                <p>"Nenhuma aprovação pendente"</p>
                                <p class="muted">"Pedidos de aprovação HITL aparecerão aqui quando agentes solicitarem ações que requerem confirmação humana."</p>
                            </div>
                        }.into_any()
                    } else {
                        let pending_count = pending.len();
                        let receipt_count = receipts.len();
                        let set_approvals_clone = set_approvals;
                        view! {
                            <div class="dashboard-content">
                                <div class="metrics-bar">
                                    <div class="metric-card">
                                        <span class="metric-label">"Aprovações Pendentes"</span>
                                        <span class="metric-value" style="color: var(--color-status-warning)">{pending_count.to_string()}</span>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Resolvidas (7 dias)"</span>
                                        <span class="metric-value" style="color: var(--color-status-success)">{receipt_count.to_string()}</span>
                                    </div>
                                </div>
                                <div class="approval-cards-list">
                                    {pending.into_iter().map(move |approval| {
                                        let approval_for_resolve = approval.clone();
                                        let set_approvals_for_callback = set_approvals_clone;
                                        let on_resolve: Arc<dyn Fn(String, String) + Send + Sync> = Arc::new(move |id: String, decision: String| {
                                            set_approvals_for_callback.update(|list| list.retain(|a| a.id != id));
                                            save_receipt(&id, &decision, &approval_for_resolve.summary);
                                        });
                                        view! {
                                            <InlineApprovalCard
                                                approval=approval.clone()
                                                on_resolve=on_resolve
                                            />
                                        }
                                    }).collect::<Vec<_>>()}
                                    {receipts.into_iter().map(|receipt| {
                                        view! { <ApprovalReceiptCard receipt=receipt /> }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}

#[component]
fn InlineApprovalCard(
    approval: ApprovalRequest,
    on_resolve: Arc<dyn Fn(String, String) + Send + Sync>,
) -> impl IntoView {
    let (state, set_state) = signal(CardState::Pending);
    let (refuse_reason, set_refuse_reason) = signal(String::new());
    let (show_reason_input, set_show_reason_input) = signal(false);
    let (error_msg, set_error_msg) = signal(None::<String>);

    // Spring for card entrance
    let (entrance_scale, entrance_spring) = create_spring(0.95, SpringConfig::GESTURE);
    let (entrance_opacity, entrance_opacity_spring) = create_spring(0.0, SpringConfig::UI);

    Effect::new(move |_| {
        entrance_spring.set_target(1.0);
        entrance_opacity_spring.set_target(1.0);
    });

    let card_class = move || {
        let base = "inline-approval-card";
        match state.get() {
            CardState::Pending => format!("{} pending", base),
            CardState::Resolving => format!("{} resolving", base),
            CardState::Error => format!("{} error", base),
        }
    };

    let scale_style = move || {
        format!(
            "transform: scale({}); opacity: {}",
            entrance_scale.get(),
            entrance_opacity.get()
        )
    };

    let id_clone = approval.id.clone();
    let on_resolve_approve = on_resolve.clone();
    let on_resolve_reject = on_resolve.clone();
    let on_resolve_escalate = on_resolve.clone();

    view! {
        <div class=card_class style=scale_style>
            <div class="approval-header">
                <div class="approval-title">
                    <strong>{approval.summary.clone()}</strong>
                    <span class="approval-meta">
                        {format!("{} · {}", approval.bot_id, approval.tool_name)}
                    </span>
                </div>
                <span class="approval-category">{approval.action_category.clone()}</span>
            </div>

            <div class="approval-details">
                <details>
                    <summary>"Ver argumentos"</summary>
                    <pre class="approval-args">{approval.arguments_json.clone()}</pre>
                </details>
                <p class="approval-time">{format!("Criado: {}", approval.created_at)}</p>
            </div>

            {move || {
                if show_reason_input.get() && state.get() == CardState::Pending {
                    view! {
                        <div class="approval-reason">
                            <label for="refuse-reason">"Motivo da recusa (opcional)"</label>
                            <input
                                id="refuse-reason"
                                type="text"
                                prop:value=move || refuse_reason.get()
                                on:input=move |ev| set_refuse_reason.set(event_target_value(&ev))
                                placeholder="Explique por que está recusando..."
                            />
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            {move || {
                if let Some(err) = error_msg.get() {
                    view! {
                        <div class="approval-error">
                            <p>{err}</p>
                            <button class="cta-retry" on:click=move |_| {
                                set_state.set(CardState::Pending);
                                set_error_msg.set(None);
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                } else {
                    view! { <div></div> }.into_any()
                }
            }}

            <div class="approval-actions">
                {move || {
                    if state.get() == CardState::Pending {
                        let id = id_clone.clone();
                        let on_resolve = on_resolve_approve.clone();
                        let approve_handler = move |_| {
                            set_state.set(CardState::Resolving);
                            let id = id.clone();
                            let on_resolve = on_resolve.clone();
                            spawn_local(async move {
                                match resolve_approval(&id, "allow_once", None).await {
                                    Ok(_) => {
                                        on_resolve(id, "allow_once".to_string());
                                    }
                                    Err(e) => {
                                        set_state.set(CardState::Error);
                                        set_error_msg.set(Some(crate::ui_error::report("Falha ao resolver", &e)));
                                    }
                                }
                            });
                        };

                        let id = id_clone.clone();
                        let on_resolve = on_resolve_reject.clone();
                        let reject_handler = move |_| {
                            if !show_reason_input.get() {
                                set_show_reason_input.set(true);
                                return;
                            }
                            set_state.set(CardState::Resolving);
                            let id = id.clone();
                            let on_resolve = on_resolve.clone();
                            let reason = refuse_reason.get_untracked();
                            spawn_local(async move {
                                match resolve_approval(&id, "refuse", if reason.is_empty() { None } else { Some(reason) }).await {
                                    Ok(_) => {
                                        on_resolve(id, "refuse".to_string());
                                    }
                                    Err(e) => {
                                        set_state.set(CardState::Error);
                                        set_error_msg.set(Some(crate::ui_error::report("Falha ao resolver", &e)));
                                    }
                                }
                            });
                        };

                        let id = id_clone.clone();
                        let on_resolve = on_resolve_escalate.clone();
                        let escalate_handler = move |_| {
                            set_state.set(CardState::Resolving);
                            let id = id.clone();
                            let on_resolve = on_resolve.clone();
                            spawn_local(async move {
                                match resolve_approval(&id, "escalate", None).await {
                                    Ok(_) => {
                                        on_resolve(id, "escalate".to_string());
                                    }
                                    Err(e) => {
                                        set_state.set(CardState::Error);
                                        set_error_msg.set(Some(crate::ui_error::report("Falha ao resolver", &e)));
                                    }
                                }
                            });
                        };

                        view! {
                            <>
                                <button class="cta-primary approval-btn-approve" aria-label="Aprovar esta ação" on:click=approve_handler>
                                    "✓ Aprovar"
                                </button>
                                <button class="cta-secondary approval-btn-reject" aria-label="Recusar esta ação" on:click=reject_handler>
                                    "✕ Recusar"
                                </button>
                                <button class="cta-secondary approval-btn-escalate" aria-label="Escalar para revisão humana" on:click=escalate_handler>
                                    "⚠ Escalar"
                                </button>
                            </>
                        }.into_any()
                    } else if state.get() == CardState::Resolving {
                        view! {
                            <div class="approval-resolving">
                                <span class="spinner"></span>
                                <span>"Resolvendo..."</span>
                            </div>
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn ApprovalReceiptCard(receipt: ApprovalReceipt) -> impl IntoView {
    let status_class = match receipt.decision.as_str() {
        "allow_once" | "allow_session" | "allow_always" => "resolved",
        "refuse" => "rejected",
        "escalate" => "escalated",
        _ => "resolved",
    };

    let decision_text = match receipt.decision.as_str() {
        "allow_once" => "Aprovado (uma vez)".to_string(),
        "allow_session" => "Aprovado (sessão)".to_string(),
        "allow_always" => "Aprovado (sempre)".to_string(),
        "refuse" => "Recusado".to_string(),
        "escalate" => "Escalado".to_string(),
        other => other.to_string(),
    };

    view! {
        <div class=format!("inline-approval-card {}", status_class)>
            <div class="approval-receipt">
                <strong>{receipt.summary}</strong>
                <span class="receipt-decision">{decision_text}</span>
                <span class="receipt-time">{receipt.timestamp}</span>
            </div>
        </div>
    }
}

async fn fetch_approvals() -> Result<Vec<ApprovalRequest>, String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/approvals");

    let resp = crate::auth::authed(gloo_net::http::Request::get(&url))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.ok() {
        return Err(crate::auth::approval_error(resp.status()));
    }

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let data: ApprovalsResponse = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(data.approvals)
}

async fn resolve_approval(
    approval_id: &str,
    decision: &str,
    refuse_reason: Option<String>,
) -> Result<(), String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/approval/resolve");

    let body = serde_json::json!({
        "approval_id": approval_id,
        "decision": decision,
        "refuse_reason": refuse_reason,
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

fn load_receipts() -> Vec<ApprovalReceipt> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return vec![],
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return vec![],
    };

    let data = match storage.get_item(RECEIPTS_KEY) {
        Ok(Some(d)) => d,
        _ => return vec![],
    };

    let mut receipts: Vec<ApprovalReceipt> = match serde_json::from_str(&data) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    // Filter out receipts older than 7 days
    let now = chrono::Utc::now();
    receipts.retain(|r| {
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&r.timestamp) {
            let age = now.signed_duration_since(ts);
            age.num_days() < RECEIPT_MAX_AGE_DAYS
        } else {
            false
        }
    });

    receipts
}

fn save_receipt(id: &str, decision: &str, summary: &str) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return,
    };

    let mut receipts = load_receipts();
    receipts.push(ApprovalReceipt {
        id: id.to_string(),
        decision: decision.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        summary: summary.to_string(),
    });

    if let Ok(json) = serde_json::to_string(&receipts) {
        let _ = storage.set_item(RECEIPTS_KEY, &json);
    }
}
