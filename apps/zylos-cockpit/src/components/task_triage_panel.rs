//! TaskTriagePanel — painel de triagem de tarefas com 4 seções visuais
//!
//! Inspirado em: Rerun (`InboxSurface.tsx`) com seções needsYou/headsUps/errors/done.
//! Substitui lista plana de tarefas por visão priorizada que diferencia
//! visualmente o que requer atenção imediata do que pode esperar.
//!
//! ## Seções
//! - 🔴 Needs You: approvals pendentes, tarefas bloqueadas, perguntas do agente
//! - 🟡 Can Wait: tarefas em andamento, goals com deadline > 7d
//! - 🔵 Errors: falhas recentes, integrações offline, timeouts
//! - ✅ Done: tarefas completadas nas últimas 24h (colapsada por padrão)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.2
//! - Design System: [DESIGN_SYSTEM.md](../../docs/DESIGN_SYSTEM.md)

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Um item individual na triagem
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriageItem {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: String, // critical, high, medium, low
    pub source: String,   // approval, task, goal, integration, error
    pub action_url: Option<String>,
    pub created_at: String,
}

/// Dados agregados de triagem vindos da API
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TriageData {
    pub needs_you: Vec<TriageItem>,
    pub can_wait: Vec<TriageItem>,
    pub errors: Vec<TriageItem>,
    pub done: Vec<TriageItem>,
    pub updated_at: String,
}

/// Estado do componente
#[derive(Debug, Clone, PartialEq)]
enum TriageState {
    Loading,
    Loaded(TriageData),
    Error(String),
}

#[component]
pub fn TaskTriagePanel() -> impl IntoView {
    let state = RwSignal::new(TriageState::Loading);
    let done_expanded = RwSignal::new(false);

    // Fetch dados ao montar
    Effect::new(move |_| {
        spawn_local(async move {
            state.set(TriageState::Loading);

            let needs_you = fetch_needs_you().await;
            let errors = fetch_errors().await;
            let can_wait = fetch_can_wait().await;
            let done = fetch_done().await;

            match (needs_you, errors, can_wait, done) {
                (Ok(ny), Ok(err), Ok(cw), Ok(dn)) => {
                    state.set(TriageState::Loaded(TriageData {
                        needs_you: ny,
                        errors: err,
                        can_wait: cw,
                        done: dn,
                        updated_at: js_timestamp(),
                    }));
                }
                _ => {
                    state.set(TriageState::Error(
                        "Falha ao carregar dados de triagem".into(),
                    ));
                }
            }
        });
    });

    view! {
        <div class="triage-panel">
            <div class="triage-header">
                <h2 class="triage-title">"📋 Triagem de Tarefas"</h2>
                <button
                    class="triage-refresh-btn"
                    on:click=move |_| {
                        spawn_local(async move {
                            state.set(TriageState::Loading);
                            let ny = fetch_needs_you().await.unwrap_or_default();
                            let err = fetch_errors().await.unwrap_or_default();
                            let cw = fetch_can_wait().await.unwrap_or_default();
                            let dn = fetch_done().await.unwrap_or_default();
                            state.set(TriageState::Loaded(TriageData {
                                needs_you: ny,
                                errors: err,
                                can_wait: cw,
                                done: dn,
                                updated_at: js_timestamp(),
                            }));
                        });
                    }
                    title="Atualizar"
                >
                    "🔄"
                </button>
            </div>

            {move || match state.read().clone() {
                TriageState::Loading => view! {
                    <div class="triage-loading">
                        <span class="spinner"></span>
                        <span>"Carregando triagem..."</span>
                    </div>
                }.into_any(),

                TriageState::Error(msg) => view! {
                    <div class="triage-error">
                        <span>"⚠️ "</span>
                        <span>{msg.clone()}</span>
                    </div>
                }.into_any(),

                TriageState::Loaded(data) => {
                    let ny_count = data.needs_you.len();
                    let cw_count = data.can_wait.len();
                    let err_count = data.errors.len();
                    let done_count = data.done.len();
                    let _done_exp = done_expanded.get();

                    view! {
                        <div class="triage-sections">
                            // Seção 1: Needs You
                            <div class="triage-section triage-section-critical">
                                <div class="triage-section-header">
                                    <span class="triage-section-icon">"🔴"</span>
                                    <span class="triage-section-name">"Precisa de Você"</span>
                                    <span class="triage-section-count">{ny_count}</span>
                                </div>
                                {if ny_count == 0 {
                                    view! { <div class="triage-empty">"Nenhuma ação urgente pendente ✨"</div> }.into_any()
                                } else {
                                    view! {
                                        <ul class="triage-list">
                                            {data.needs_you.iter().map(|item| {
                                                let title = item.title.clone();
                                                let desc = item.description.clone();
                                                let url = item.action_url.clone();
                                                view! {
                                                    <li class="triage-item triage-item-critical">
                                                        <div class="triage-item-content">
                                                            <span class="triage-item-title">{title}</span>
                                                            {desc.map(|d| view! { <span class="triage-item-desc">{d}</span> })}
                                                        </div>
                                                        {url.map(|u| view! {
                                                            <a href=u class="triage-item-action">"Ver →"</a>
                                                        })}
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    }.into_any()
                                }}
                            </div>

                            // Seção 2: Errors
                            <div class="triage-section triage-section-error">
                                <div class="triage-section-header">
                                    <span class="triage-section-icon">"🔵"</span>
                                    <span class="triage-section-name">"Erros"</span>
                                    <span class="triage-section-count">{err_count}</span>
                                </div>
                                {if err_count == 0 {
                                    view! { <div class="triage-empty">"Sem erros recentes 👍"</div> }.into_any()
                                } else {
                                    view! {
                                        <ul class="triage-list">
                                            {data.errors.iter().map(|item| {
                                                let title = item.title.clone();
                                                let desc = item.description.clone();
                                                view! {
                                                    <li class="triage-item triage-item-error">
                                                        <div class="triage-item-content">
                                                            <span class="triage-item-title">{title}</span>
                                                            {desc.map(|d| view! { <span class="triage-item-desc">{d}</span> })}
                                                        </div>
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    }.into_any()
                                }}
                            </div>

                            // Seção 3: Can Wait
                            <div class="triage-section triage-section-warning">
                                <div class="triage-section-header">
                                    <span class="triage-section-icon">"🟡"</span>
                                    <span class="triage-section-name">"Pode Esperar"</span>
                                    <span class="triage-section-count">{cw_count}</span>
                                </div>
                                {if cw_count == 0 {
                                    view! { <div class="triage-empty">"Nenhuma tarefa em andamento"</div> }.into_any()
                                } else {
                                    view! {
                                        <ul class="triage-list">
                                            {data.can_wait.iter().map(|item| {
                                                let title = item.title.clone();
                                                let desc = item.description.clone();
                                                view! {
                                                    <li class="triage-item triage-item-warning">
                                                        <div class="triage-item-content">
                                                            <span class="triage-item-title">{title}</span>
                                                            {desc.map(|d| view! { <span class="triage-item-desc">{d}</span> })}
                                                        </div>
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    }.into_any()
                                }}
                            </div>

                            // Seção 4: Done (colapsável)
                            <div class="triage-section triage-section-success">
                                <div
                                    class="triage-section-header triage-section-collapsible"
                                    on:click=move |_| done_expanded.update(|v| *v = !*v)
                                >
                                    <span class="triage-section-icon">"✅"</span>
                                    <span class="triage-section-name">"Concluídas (24h)"</span>
                                    <span class="triage-section-count">{done_count}</span>
                                    <span class="triage-collapse-indicator">
                                        {move || if done_expanded.get() { "▼" } else { "▶" }}
                                    </span>
                                </div>
                                {move || {
                                 let expanded = done_expanded.get();
                                 if expanded && done_count > 0 {
                                    view! {
                                        <ul class="triage-list">
                                            {data.done.iter().map(|item| {
                                                let title = item.title.clone();
                                                view! {
                                                    <li class="triage-item triage-item-done">
                                                        <span class="triage-item-title">{title}</span>
                                                    </li>
                                                }
                                            }).collect_view()}
                                        </ul>
                                    }.into_any()
                                } else if expanded {
                                    view! { <div class="triage-empty">"Nenhuma tarefa concluída nas últimas 24h"</div> }.into_any()
                                } else {
                                    view! { <div></div> }.into_any()
                                }}}
                            </div>
                        </div>
                    }.into_any()
                }
            }}
        </div>
    }
}

// === Helpers ===

fn js_timestamp() -> String {
    let date = js_sys::Date::new_0();
    date.to_iso_string().as_string().unwrap_or_default()
}

// === Funções de fetch ===

async fn fetch_needs_you() -> Result<Vec<TriageItem>, String> {
    // Approvals pendentes
    let resp = crate::auth::authed(gloo_net::http::Request::get(
        "http://localhost:3000/api/v1/approvals/pending",
    ))
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !resp.ok() {
        return Ok(vec![]); // Endpoint pode não existir ainda
    }

    #[derive(Deserialize)]
    struct ApprovalSummary {
        id: String,
        bot_id: String,
        tool_name: String,
        summary: String,
        created_at: String,
    }

    let approvals: Vec<ApprovalSummary> = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(approvals
        .into_iter()
        .map(|a| TriageItem {
            id: format!("approval-{}", a.id),
            title: a.summary,
            description: Some(format!("Bot: {} | Tool: {}", a.bot_id, a.tool_name)),
            priority: "critical".into(),
            source: "approval".into(),
            action_url: Some(format!("/chat?approval={}", a.id)),
            created_at: a.created_at,
        })
        .collect())
}

async fn fetch_errors() -> Result<Vec<TriageItem>, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(
        "http://localhost:3000/api/v1/audit/actions?approval_status=refused&limit=5",
    ))
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !resp.ok() {
        return Ok(vec![]);
    }

    #[derive(Deserialize)]
    struct AuditListResp {
        entries: Vec<AuditEntrySummary>,
    }
    #[derive(Deserialize)]
    struct AuditEntrySummary {
        id: String,
        bot_id: String,
        action_type: String,
        result_summary: Option<String>,
        created_at: String,
    }

    let body: AuditListResp = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(body
        .entries
        .into_iter()
        .map(|e| TriageItem {
            id: format!("error-{}", e.id),
            title: format!("Falha em {}", e.action_type),
            description: e.result_summary.or(Some(format!("Bot: {}", e.bot_id))),
            priority: "high".into(),
            source: "error".into(),
            action_url: Some("/audit".into()),
            created_at: e.created_at,
        })
        .collect())
}

async fn fetch_can_wait() -> Result<Vec<TriageItem>, String> {
    // Placeholder: tasks em andamento virão do zylos-goals/tasks quando integrados
    // Por agora, retorna lista vazia
    Ok(vec![])
}

async fn fetch_done() -> Result<Vec<TriageItem>, String> {
    let resp = crate::auth::authed(gloo_net::http::Request::get(
        "http://localhost:3000/api/v1/audit/actions?approval_status=approved&limit=10",
    ))
    .send()
    .await
    .map_err(|e| format!("Network error: {}", e))?;

    if !resp.ok() {
        return Ok(vec![]);
    }

    #[derive(Deserialize)]
    struct AuditListResp {
        entries: Vec<AuditEntrySummary>,
    }
    #[derive(Deserialize)]
    struct AuditEntrySummary {
        id: String,
        action_type: String,
        created_at: String,
    }

    let body: AuditListResp = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(body
        .entries
        .into_iter()
        .map(|e| TriageItem {
            id: format!("done-{}", e.id),
            title: format!("{} concluído", e.action_type),
            description: None,
            priority: "low".into(),
            source: "task".into(),
            action_url: None,
            created_at: e.created_at,
        })
        .collect())
}
