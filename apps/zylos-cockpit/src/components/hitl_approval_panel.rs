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

/// Nível de risco mostrado no cartão, derivado da categoria da ação.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Risk {
    Alto,
    Medio,
    Baixo,
}

impl Risk {
    fn from_category(category: &str) -> Self {
        match category {
            "deploy" | "payment" | "sign_document" => Risk::Alto,
            "send_message" | "delete_resource" | "read_sensitive" => Risk::Medio,
            _ => Risk::Baixo,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Risk::Alto => "Risco alto",
            Risk::Medio => "Risco médio",
            Risk::Baixo => "Risco baixo",
        }
    }

    fn css(self) -> &'static str {
        match self {
            Risk::Alto => "risk-alto",
            Risk::Medio => "risk-medio",
            Risk::Baixo => "risk-baixo",
        }
    }

    /// "Sempre permitir" não existe para risco alto (DESIGN_SYSTEM v2.1, decisão D1).
    fn allows_always(self) -> bool {
        self != Risk::Alto
    }
}

const RECEIPTS_KEY: &str = "zylos_approval_receipts";
const RECEIPT_MAX_AGE_DAYS: i64 = 7;

#[component]
pub fn HitlApprovalPanel() -> impl IntoView {
    let (approvals, set_approvals) = signal(Vec::<ApprovalRequest>::new());
    let (loading, set_loading) = signal(true);
    let (error, set_error) = signal(None::<String>);

    // Busca ao montar e de novo a cada evento de aprovação do SSE (approvals_tick).
    let tick = approvals_tick();
    let mut first_load = true;
    Effect::new(move |_| {
        if let Some(t) = tick {
            let _ = t.get();
        }
        // Só a primeira carga mostra o esqueleto; as seguintes atualizam sem piscar.
        let initial = first_load;
        first_load = false;
        spawn_local(async move {
            if initial {
                set_loading.set(true);
            }
            match fetch_approvals().await {
                Ok(list) => {
                    schedule_expiry_refresh(&list, tick);
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
            <ApprovalRules />
        </div>
    }
}

// ─── Regras permanentes ("Sempre permitir") ──────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct ApprovalRuleView {
    id: String,
    bot_id: String,
    tool_name: String,
    action_category: String,
    created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RulesResponse {
    rules: Vec<ApprovalRuleView>,
}

async fn fetch_rules() -> Result<Vec<ApprovalRuleView>, String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/approvals/rules");
    let resp = crate::auth::authed(gloo_net::http::Request::get(&url))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(crate::auth::approval_error(resp.status()));
    }
    let data: RulesResponse = resp.json().await.map_err(|e| e.to_string())?;
    Ok(data.rules)
}

async fn revoke_rule(rule_id: &str) -> Result<(), String> {
    let base = crate::gateway_config::gateway_base().ok_or("Gateway não configurado")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/approvals/rules/revoke");
    let resp = crate::auth::authed(gloo_net::http::Request::post(&url))
        .json(&serde_json::json!({ "rule_id": rule_id }))
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

/// Lista as regras "Sempre permitir" ativas, com botão para revogar (o próximo pedido volta a perguntar).
#[component]
fn ApprovalRules() -> impl IntoView {
    let (rules, set_rules) = signal(Vec::<ApprovalRuleView>::new());
    let (error, set_error) = signal(None::<String>);
    let tick = approvals_tick();

    let reload = move || {
        spawn_local(async move {
            match fetch_rules().await {
                Ok(list) => {
                    set_rules.set(list);
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
        });
    };
    Effect::new(move |_| {
        if let Some(t) = tick {
            let _ = t.get();
        }
        reload();
    });

    view! {
        <section class="approval-rules" aria-labelledby="approval-rules-title">
            <h3 id="approval-rules-title">"Regras permanentes"</h3>
            {move || {
                if let Some(e) = error.get() {
                    view! { <p class="approval-hint">{format!("Não foi possível carregar as regras: {}", crate::ui_error::report("Regras", &e))}</p> }.into_any()
                } else if rules.get().is_empty() {
                    view! { <p class="approval-hint">"Nenhuma regra. \"Sempre permitir\" cria uma aqui, e você pode revogar quando quiser."</p> }.into_any()
                } else {
                    view! {
                        <ul class="approval-rules-list">
                            {rules.get().into_iter().map(|r| {
                                let id = r.id.clone();
                                view! {
                                    <li class="approval-rule-row">
                                        <span class="approval-rule-text">
                                            <strong>{format!("{} · {}", r.bot_id, r.tool_name)}</strong>
                                            <span class="approval-meta">{format!("{} · criada {}", r.action_category, r.created_at)}</span>
                                        </span>
                                        <button
                                            class="cta-secondary"
                                            aria-label=format!("Revogar regra de {}", r.tool_name)
                                            on:click=move |_| {
                                                let id = id.clone();
                                                spawn_local(async move {
                                                    match revoke_rule(&id).await {
                                                        Ok(()) => reload(),
                                                        Err(e) => set_error.set(Some(e)),
                                                    }
                                                });
                                            }
                                        >"Revogar"</button>
                                    </li>
                                }
                            }).collect::<Vec<_>>()}
                        </ul>
                    }.into_any()
                }
            }}
        </section>
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
    let on_resolve_always = on_resolve.clone();
    let risk = Risk::from_category(&approval.action_category);
    let (show_always_confirm, set_show_always_confirm) = signal(false);
    let always_scope = format!("{} · {}", approval.bot_id, approval.tool_name);

    view! {
        <div class=card_class style=scale_style>
            <div class="approval-header">
                <div class="approval-title">
                    <strong>{approval.summary.clone()}</strong>
                    <span class="approval-meta">
                        {format!("{} · {}", approval.bot_id, approval.tool_name)}
                    </span>
                </div>
                <span class=format!("approval-risk {}", risk.css())>
                    <i class="ph ph-shield-warning" aria-hidden="true"></i>
                    {risk.label()}
                </span>
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
                        let on_resolve = on_resolve_always.clone();
                        let always_handler = move |_| {
                            set_state.set(CardState::Resolving);
                            set_show_always_confirm.set(false);
                            let id = id.clone();
                            let on_resolve = on_resolve.clone();
                            spawn_local(async move {
                                match resolve_approval(&id, "allow_always", None).await {
                                    Ok(_) => {
                                        on_resolve(id, "allow_always".to_string());
                                    }
                                    Err(e) => {
                                        set_state.set(CardState::Error);
                                        set_error_msg.set(Some(crate::ui_error::report("Falha ao resolver", &e)));
                                    }
                                }
                            });
                        };
                        let always_scope = always_scope.clone();

                        view! {
                            <>
                                <button class="cta-secondary approval-btn-reject" aria-label="Recusar esta ação" on:click=reject_handler>
                                    <i class="ph ph-x" aria-hidden="true"></i>
                                    "Recusar"
                                </button>
                                <button class="cta-primary approval-btn-approve" aria-label="Permitir esta ação uma vez" on:click=approve_handler>
                                    <i class="ph ph-check" aria-hidden="true"></i>
                                    "Permitir uma vez"
                                </button>
                                <button
                                    class="cta-secondary approval-btn-always"
                                    aria-label="Sempre permitir ações como esta"
                                    disabled=!risk.allows_always()
                                    title=if risk.allows_always() { "" } else { "Indisponível para risco alto" }
                                    on:click=move |_| set_show_always_confirm.set(true)
                                >
                                    <i class="ph ph-shield-check" aria-hidden="true"></i>
                                    "Sempre permitir"
                                </button>
                                {move || if show_always_confirm.get() {
                                    let confirm = always_handler.clone();
                                    view! {
                                        <div class="approval-always-confirm" role="alertdialog" aria-label="Confirmar regra permanente">
                                            <p>{format!("Permitir sempre ações como esta ({}) sem perguntar de novo? Você pode revogar na seção Regras permanentes, nesta tela.", always_scope)}</p>
                                            <div class="approval-actions">
                                                <button class="cta-primary" on:click=confirm>"Confirmar regra"</button>
                                                <button class="cta-secondary" on:click=move |_| set_show_always_confirm.set(false)>"Cancelar"</button>
                                            </div>
                                        </div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                                {if risk.allows_always() {
                                    ().into_any()
                                } else {
                                    view! { <p class="approval-hint">"\"Sempre permitir\" fica indisponível para risco alto."</p> }.into_any()
                                }}
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
        "allow_once" => "Permitido uma vez".to_string(),
        "allow_session" => "Aprovado (sessão)".to_string(),
        "allow_always" => "Permitido sempre".to_string(),
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

// ─── Sincronização com o SSE, expiração e uso no chat ────────────────────────

/// Sinal que sobe a cada evento de aprovação do SSE (`None` sem contexto de stream, ex.: testes).
fn approvals_tick() -> Option<RwSignal<u32>> {
    crate::approvals_sync::use_approvals_tick()
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
const DAY_MS: i64 = 86_400_000;

/// Milissegundos até `expires_at` (RFC 3339), se ainda estiver no futuro e a menos de 24 h.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn expiry_delay_ms(
    expires_at: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<u32> {
    let exp = chrono::DateTime::parse_from_rfc3339(expires_at).ok()?;
    let delta = exp.signed_duration_since(now).num_milliseconds();
    (delta > 0 && delta <= DAY_MS).then_some(delta as u32)
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static EXPIRY_SCHEDULED: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Agenda, para cada pendente (uma vez por id), a atualização da lista no instante em que expira.
/// O toast "Aprovação expirou" vem do gateway (`ApprovalTimeout` pelo SSE); isto só garante que o
/// cartão some mesmo se o SSE estiver fora do ar.
#[cfg(target_arch = "wasm32")]
fn schedule_expiry_refresh(list: &[ApprovalRequest], tick: Option<RwSignal<u32>>) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };
    let now = chrono::Utc::now();
    for a in list {
        let Some(delay) = a.expires_at.as_deref().and_then(|e| expiry_delay_ms(e, now)) else {
            continue;
        };
        if !EXPIRY_SCHEDULED.with(|s| s.borrow_mut().insert(a.id.clone())) {
            continue;
        }
        let cb = Closure::once(move || {
            if let Some(t) = tick {
                crate::approvals_sync::bump(t);
            }
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            delay as i32,
        );
        cb.forget();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn schedule_expiry_refresh(list: &[ApprovalRequest], tick: Option<RwSignal<u32>>) {
    let _ = (list, tick);
}

/// Aprovações pendentes dentro do chat (ADR-001): os mesmos cartões do painel HITL, sem métricas.
#[component]
pub fn InlineApprovals() -> impl IntoView {
    let (approvals, set_approvals) = signal(Vec::<ApprovalRequest>::new());
    let tick = approvals_tick();

    Effect::new(move |_| {
        if let Some(t) = tick {
            let _ = t.get();
        }
        spawn_local(async move {
            // Sem gateway ou com erro, o chat segue sem cartões (o painel HITL mostra o erro).
            if let Ok(list) = fetch_approvals().await {
                schedule_expiry_refresh(&list, tick);
                set_approvals.set(list);
            }
        });
    });

    view! {
        <div class="chat-inline-approvals" role="region" aria-label="Aprovações pendentes">
            {move || {
                approvals
                    .get()
                    .into_iter()
                    .map(|approval| {
                        let summary = approval.summary.clone();
                        let on_resolve: Arc<dyn Fn(String, String) + Send + Sync> =
                            Arc::new(move |id: String, decision: String| {
                                set_approvals.update(|list| list.retain(|a| a.id != id));
                                save_receipt(&id, &decision, &summary);
                            });
                        view! { <InlineApprovalCard approval=approval on_resolve=on_resolve /> }
                    })
                    .collect::<Vec<_>>()
            }}
        </div>
    }
}

/// Linha de recibo para a aba Activity: o que a pessoa decidiu.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionLine {
    pub summary: String,
    pub decision: String,
    pub at: String,
}

fn decision_text(decision: &str) -> String {
    match decision {
        "allow_once" => "permitiu uma vez".to_string(),
        "allow_session" => "permitiu na sessão".to_string(),
        "allow_always" => "permitiu sempre".to_string(),
        "refuse" => "recusou".to_string(),
        other => other.to_string(),
    }
}

/// Decisões recentes (recibos guardados no navegador por 7 dias), mais novas primeiro.
pub fn recent_decisions() -> Vec<DecisionLine> {
    let mut v: Vec<DecisionLine> = load_receipts()
        .into_iter()
        .map(|r| DecisionLine {
            summary: r.summary,
            decision: decision_text(&r.decision),
            at: r.timestamp,
        })
        .collect();
    v.sort_by(|a, b| b.at.cmp(&a.at));
    v
}

/// Pendências agrupadas por agente (aba Inbox). Mesmos cartões e ações do painel HITL.
#[component]
pub fn InboxApprovals() -> impl IntoView {
    let (approvals, set_approvals) = signal(Vec::<ApprovalRequest>::new());
    let (loaded, set_loaded) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let tick = approvals_tick();

    Effect::new(move |_| {
        if let Some(t) = tick {
            let _ = t.get();
        }
        spawn_local(async move {
            match fetch_approvals().await {
                Ok(list) => {
                    schedule_expiry_refresh(&list, tick);
                    set_approvals.set(list);
                    set_error.set(None);
                }
                Err(e) => set_error.set(Some(e)),
            }
            set_loaded.set(true);
        });
    });

    view! {
        <div class="inbox-approvals">
            {move || {
                if !loaded.get() {
                    view! { <div class="skeleton-grid"><div class="skeleton-card pulse"></div></div> }.into_any()
                } else if let Some(e) = error.get() {
                    view! {
                        <div class="error-banner">
                            <p>{format!("Não foi possível carregar o Inbox: {}", crate::ui_error::report("Inbox", &e))}</p>
                            <button class="cta-retry" on:click=move |_| {
                                if let Some(t) = tick { crate::approvals_sync::bump(t); }
                            }>"Tentar novamente"</button>
                        </div>
                    }.into_any()
                } else if approvals.get().is_empty() {
                    view! {
                        <div class="empty-state">
                            <p>"Nada esperando por você"</p>
                            <p class="muted">"Quando um agente precisar de uma aprovação, ela aparece aqui e no chat."</p>
                        </div>
                    }.into_any()
                } else {
                    let mut groups: Vec<(String, Vec<ApprovalRequest>)> = Vec::new();
                    for a in approvals.get() {
                        match groups.iter_mut().find(|(bot, _)| *bot == a.bot_id) {
                            Some((_, list)) => list.push(a),
                            None => groups.push((a.bot_id.clone(), vec![a])),
                        }
                    }
                    groups
                        .into_iter()
                        .map(|(bot, list)| {
                            view! {
                                <section class="inbox-group" aria-label=format!("Pendências de {}", bot)>
                                    <h3 class="inbox-group-title">{bot.clone()}</h3>
                                    {list.into_iter().map(|approval| {
                                        let summary = approval.summary.clone();
                                        let on_resolve: Arc<dyn Fn(String, String) + Send + Sync> =
                                            Arc::new(move |id: String, decision: String| {
                                                set_approvals.update(|l| l.retain(|a| a.id != id));
                                                save_receipt(&id, &decision, &summary);
                                            });
                                        view! { <InlineApprovalCard approval=approval on_resolve=on_resolve /> }
                                    }).collect::<Vec<_>>()}
                                </section>
                            }
                        })
                        .collect_view()
                        .into_any()
                }
            }}
        </div>
    }
}

/// Aprova (uma vez) todas as pendentes. Devolve `(aprovadas, falhas)`. Quem chama deve confirmar antes.
pub async fn approve_all_pending() -> Result<(usize, usize), String> {
    let list = fetch_approvals().await?;
    let (mut ok, mut failed) = (0, 0);
    for a in list {
        match resolve_approval(&a.id, "allow_once", None).await {
            Ok(()) => {
                save_receipt(&a.id, "allow_once", &a.summary);
                ok += 1;
            }
            Err(_) => failed += 1,
        }
    }
    Ok((ok, failed))
}

/// Quantas aprovações estão pendentes agora (para o palette confirmar o lote).
pub async fn pending_approvals_count() -> Result<usize, String> {
    fetch_approvals().await.map(|l| l.len())
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
    let url = crate::gateway_config::join_url(&base, "/api/v1/approvals/resolve");

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

#[cfg(test)]
mod expiry_tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 11, 19, 12, 0, 0).unwrap()
    }

    #[test]
    fn expira_no_futuro_proximo() {
        assert_eq!(expiry_delay_ms("2026-11-19T12:05:00+00:00", now()), Some(300_000));
    }

    #[test]
    fn ignora_passado_e_muito_distante_e_invalido() {
        assert_eq!(expiry_delay_ms("2026-11-19T11:59:00+00:00", now()), None);
        assert_eq!(expiry_delay_ms("2026-11-21T12:00:00+00:00", now()), None);
        assert_eq!(expiry_delay_ms("amanha", now()), None);
    }
}

#[cfg(test)]
mod risk_tests {
    use super::*;

    #[test]
    fn risco_por_categoria() {
        assert_eq!(Risk::from_category("deploy"), Risk::Alto);
        assert_eq!(Risk::from_category("payment"), Risk::Alto);
        assert_eq!(Risk::from_category("send_message"), Risk::Medio);
        assert_eq!(Risk::from_category("low_risk"), Risk::Baixo);
        assert_eq!(Risk::from_category("desconhecida"), Risk::Baixo);
    }

    #[test]
    fn sempre_permitir_so_fora_do_risco_alto() {
        assert!(!Risk::Alto.allows_always());
        assert!(Risk::Medio.allows_always());
        assert!(Risk::Baixo.allows_always());
    }
}

#[cfg(test)]
mod decision_tests {
    use super::*;

    #[test]
    fn texto_da_decisao() {
        assert_eq!(decision_text("allow_once"), "permitiu uma vez");
        assert_eq!(decision_text("allow_always"), "permitiu sempre");
        assert_eq!(decision_text("refuse"), "recusou");
        assert_eq!(decision_text("outra"), "outra");
    }
}
