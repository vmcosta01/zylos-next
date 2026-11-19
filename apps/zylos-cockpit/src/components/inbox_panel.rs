//! Inbox e Activity (DESIGN_SYSTEM v2.1 §6.5, APP_FLOW v2.1 §6 e §7).
//!
//! - **Inbox**: o que espera a pessoa (aprovações), agrupado por agente.
//! - **Activity**: o que a pessoa decidiu (recibos de 7 dias) e a linha do tempo dos avisos do
//!   gateway recebidos pelo SSE nesta sessão (aprovações, erros de agente, alertas do Sentinela).

use leptos::prelude::*;

use super::hitl_approval_panel::{recent_decisions, InboxApprovals};
use crate::approvals_sync::{use_activity_log, ActivityEntry, ActivityKind};

/// Aba Inbox.
#[component]
pub fn InboxPanel() -> impl IntoView {
    view! {
        <section class="inbox-panel" aria-labelledby="inbox-title">
            <h2 id="inbox-title" class="page-title">"Inbox"</h2>
            <p class="page-sub">"O que está esperando por você"</p>
            <InboxApprovals />
        </section>
    }
}

/// Filtro da linha do tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Approvals,
    Alerts,
}

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Filter::All => "Tudo",
            Filter::Approvals => "Aprovações",
            Filter::Alerts => "Alertas e erros",
        }
    }

    fn matches(self, kind: ActivityKind) -> bool {
        match self {
            Filter::All => true,
            Filter::Approvals => kind == ActivityKind::Approval,
            Filter::Alerts => kind != ActivityKind::Approval,
        }
    }
}

/// Entradas que passam no filtro, na ordem recebida (mais novas primeiro).
fn filtered(entries: &[ActivityEntry], filter: Filter) -> Vec<ActivityEntry> {
    entries
        .iter()
        .filter(|e| filter.matches(e.kind))
        .cloned()
        .collect()
}

/// `2026-11-19T20:42:49Z` para a hora local (HH:MM); fora do navegador, mostra o trecho UTC.
fn short_time(iso: &str) -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
        if !d.get_time().is_nan() {
            return format!("{:02}:{:02}", d.get_hours(), d.get_minutes());
        }
    }
    iso.get(11..16).unwrap_or(iso).to_string()
}

/// Aba Activity.
#[component]
pub fn ActivityPanel() -> impl IntoView {
    let log = use_activity_log();
    let (filter, set_filter) = signal(Filter::All);

    view! {
        <section class="activity-panel" aria-labelledby="activity-title">
            <h2 id="activity-title" class="page-title">"Activity"</h2>
            <p class="page-sub">"O que você decidiu e o que o gateway avisou"</p>

            <h3 class="activity-heading">"Decisões suas (7 dias)"</h3>
            {move || {
                let decisions = recent_decisions();
                if decisions.is_empty() {
                    view! { <p class="muted">"Nenhuma decisão ainda. Aprovações que você resolver aparecem aqui."</p> }.into_any()
                } else {
                    view! {
                        <ul class="activity-list">
                            {decisions.into_iter().map(|d| view! {
                                <li class="activity-row">
                                    <i class="ph ph-check" aria-hidden="true"></i>
                                    <span class="activity-text">{format!("Você {}: {}", d.decision, d.summary)}</span>
                                    <time class="activity-time">{short_time(&d.at)}</time>
                                </li>
                            }).collect::<Vec<_>>()}
                        </ul>
                    }.into_any()
                }
            }}

            <h3 class="activity-heading">"Linha do tempo (esta sessão)"</h3>
            <div class="activity-filters" role="group" aria-label="Filtrar a linha do tempo">
                {[Filter::All, Filter::Approvals, Filter::Alerts].into_iter().map(|f| view! {
                    <button
                        class="activity-filter"
                        aria-pressed=move || (filter.get() == f).to_string()
                        on:click=move |_| set_filter.set(f)
                    >{f.label()}</button>
                }).collect::<Vec<_>>()}
            </div>
            {move || {
                let entries = log.map(|l| l.get()).unwrap_or_default();
                let shown = filtered(&entries, filter.get());
                if shown.is_empty() {
                    view! { <p class="muted">"Sem atividade ainda. Os avisos do gateway aparecem aqui assim que chegarem."</p> }.into_any()
                } else {
                    view! {
                        <ul class="activity-list">
                            {shown.into_iter().map(|e| {
                                let icon = match e.kind {
                                    ActivityKind::Approval => "ph ph-hand-palm",
                                    ActivityKind::Alert => "ph ph-shield-warning",
                                    ActivityKind::Agent => "ph ph-warning-circle",
                                };
                                view! {
                                    <li class="activity-row">
                                        <i class=icon aria-hidden="true"></i>
                                        <span class="activity-text">{e.text.clone()}</span>
                                        <time class="activity-time">{short_time(&e.at)}</time>
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

#[cfg(test)]
mod tests {
    use super::*;

    fn e(kind: ActivityKind, text: &str) -> ActivityEntry {
        ActivityEntry { kind, text: text.into(), at: "2026-11-19T12:00:00Z".into() }
    }

    #[test]
    fn filtro_separa_aprovacoes_de_alertas() {
        let all = vec![
            e(ActivityKind::Approval, "a"),
            e(ActivityKind::Alert, "b"),
            e(ActivityKind::Agent, "c"),
        ];
        assert_eq!(filtered(&all, Filter::All).len(), 3);
        assert_eq!(filtered(&all, Filter::Approvals).len(), 1);
        let alertas = filtered(&all, Filter::Alerts);
        assert_eq!(alertas.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(), ["b", "c"]);
    }

    #[test]
    fn hora_curta_fora_do_navegador() {
        assert_eq!(short_time("2026-11-19T20:42:49Z"), "20:42");
        assert_eq!(short_time("x"), "x");
    }
}
