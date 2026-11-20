//! Zylos Cockpit — Root App Component
//!
//! Top-level Leptos component rendered into #app.
//! Implements Dashboard with tabs (Agents, Tasks, Memory, Skills, Logs),
//! theme toggle, i18n, and all 6 mandatory states per APP_FLOW.md and DESIGN_SYSTEM.md.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

use crate::benchmarks::BenchmarkPanel;
use crate::components::{
    provide_chat, provide_theme, provide_toast_store, ActivityPanel, AgentsPanel, ChatPanel,
    CollapsibleSidebar, CommandPalette, ExtraHost, GatewayStatus, HitlApprovalPanel, InboxPanel,
    SpaceScopedHost, ToastContainer, TranslucentHeader,
};
use crate::i18n::{provide_i18n, use_i18n, Locale};
use crate::streaming::{provide_stream, LiveEventFeed, StreamConfig};

/// Health response from backend /health endpoint
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub active_streams: usize,
    pub version: String,
}

/// Dashboard state machine (APP_FLOW.md mandatory states)
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum DashboardState {
    Loading,
    Empty,
    Success(HealthResponse),
    Error(String),
    Disabled(String),
}

pub use crate::tabs::Tab;

/// Remove o splash estático do `index.html`, se ainda estiver na página.
fn remove_splash() {
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("app-splash"))
    {
        el.remove();
    }
}

/// Root application component
#[component]
pub fn App() -> impl IntoView {
    // O app montou: o splash estático (LCP provisório) sai do DOM.
    Effect::new(move |_| remove_splash());

    // Initialize global contexts
    let _theme_ctx = provide_theme();
    let _toast_ctx = provide_toast_store();
    let _i18n_ctx = provide_i18n(Locale::PtBr);
    let stream_ctx = provide_stream(StreamConfig::default());
    stream_ctx.connect(); // abre o SSE em /api/v1/events
    let _spaces_ctx = crate::spaces::provide_spaces();
    let _chat_ctx = provide_chat();

    let (state, set_state) = signal(DashboardState::Loading);
    // O Chat é a casa do app (DESIGN_SYSTEM v2.1 §5.1); a visão geral dos agentes é uma vista.
    let (active_tab, set_active_tab) = signal(Tab::Chat);
    let palette_open = RwSignal::new(false);

    // Derive gateway status from stream connection state
    let gateway_status = Signal::derive(move || match &*state.read() {
        DashboardState::Success(_) => GatewayStatus::Healthy,
        DashboardState::Error(_) => GatewayStatus::Disconnected,
        DashboardState::Loading => GatewayStatus::Degraded,
        _ => GatewayStatus::Disconnected,
    });

    // Derive version string
    let version = Signal::derive(move || match &*state.read() {
        DashboardState::Success(h) => format!("v{}", h.version),
        _ => "v0.1.0".to_string(),
    });

    // Atalhos globais: Ctrl/Cmd+K (paleta de abas) e Ctrl/Cmd+1..9 (aba N).
    let _shortcuts = window_event_listener(leptos::ev::keydown, move |ev| {
        if let Some(sc) = crate::tabs::shortcut_for(
            ev.ctrl_key() || ev.meta_key(),
            ev.alt_key(),
            &ev.key(),
            focus_in_editable(),
        ) {
            ev.prevent_default();
            match sc {
                crate::tabs::Shortcut::OpenPalette => palette_open.set(true),
                crate::tabs::Shortcut::SwitchTab(i) => set_active_tab.set(Tab::ALL[i]),
            }
        }
    });

    // Fetch health on mount
    Effect::new(move |_| {
        spawn_local(async move {
            match fetch_health().await {
                Ok(health) => set_state.set(DashboardState::Success(health)),
                Err(e) => set_state.set(DashboardState::Error(e)),
            }
        });
    });

    view! {
        <a href="#main-content" class="skip-link">"Pular para o conteúdo"</a>
        <div class="shell-layout">
            <TranslucentHeader
                version=version
                gateway_status=gateway_status
                active_tab=active_tab
                set_active_tab=set_active_tab
            />

            <div class="shell-body">
                <aside class="shell-sidebar-wrapper">
                    <CollapsibleSidebar active_tab=active_tab set_active_tab=set_active_tab />
                </aside>

                <main id="main-content" class="shell-main" role="main">
                    {move || match &*state.read() {
                        DashboardState::Loading => view! {
                            <div class="skeleton-grid">
                                <div class="skeleton-card pulse"></div>
                                <div class="skeleton-card pulse"></div>
                                <div class="skeleton-card pulse"></div>
                            </div>
                        }.into_any(),
                        DashboardState::Empty => view! {
                            <div class="empty-state">
                                <p><EmptyAgentsText /></p>
                                <button class="cta-primary"><EmptyCtaText /></button>
                            </div>
                        }.into_any(),
                        DashboardState::Success(health) => {
                            let tab = active_tab.get();
                            view! {
                                <div class="dashboard-content">
                                    // Saúde do gateway só na visão geral; Chat, Inbox e Activity ficam limpos.
                                    {(tab == Tab::Agents).then(|| view! {
                                        <div class="metrics-bar">
                                            <div class="metric-card">
                                                <span class="metric-label"><HealthStatusLabel /></span>
                                                <span class="metric-value success">{health.status.clone()}</span>
                                            </div>
                                            <div class="metric-card">
                                                <span class="metric-label"><HealthStreamsLabel /></span>
                                                <span class="metric-value">{health.active_streams.to_string()}</span>
                                            </div>
                                            <div class="metric-card">
                                                <span class="metric-label"><HealthVersionLabel /></span>
                                                <span class="metric-value">{health.version.clone()}</span>
                                            </div>
                                        </div>
                                    })}
                                    <TabContent tab=tab />
                                </div>
                            }.into_any()
                        }
                        DashboardState::Error(msg) => {
                            let err_msg = msg.clone();
                            view! {
                                <div class="error-banner" role="alert">
                                    <p>{format!("Conexão perdida: {}", crate::ui_error::report("health", &err_msg))}</p>
                                    <button class="cta-retry" on:click=move |_| {
                                        spawn_local(async move {
                                            match fetch_health().await {
                                                Ok(h) => set_state.set(DashboardState::Success(h)),
                                                Err(e) => set_state.set(DashboardState::Error(e)),
                                            }
                                        });
                                    }><ErrorRetryText /></button>
                                </div>
                            }.into_any()
                        }
                        DashboardState::Disabled(reason) => {
                            let r = reason.clone();
                            view! {
                                <div class="disabled-overlay">
                                    <p>{format!("System maintenance: {}", r)}</p>
                                </div>
                            }.into_any()
                        }
                    }}
                </main>
            </div>

            <CommandPalette open=palette_open set_active_tab=set_active_tab />
            <ToastContainer />
        </div>
    }
}

/// `true` se o foco está num campo editável (input/textarea/select/contenteditable).
fn focus_in_editable() -> bool {
    let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.active_element())
    else {
        return false;
    };
    let tag = el.tag_name().to_ascii_lowercase();
    matches!(tag.as_str(), "input" | "textarea" | "select")
        || el
            .get_attribute("contenteditable")
            .is_some_and(|v| v != "false")
}

/// Translated health labels
#[component]
fn HealthStatusLabel() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("health.status")
}

#[component]
fn HealthStreamsLabel() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("health.streams")
}

#[component]
fn HealthVersionLabel() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("health.version")
}

#[component]
fn ErrorRetryText() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("error.retry")
}

#[component]
fn EmptyAgentsText() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("empty.agents")
}

#[component]
fn EmptyCtaText() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("empty.cta")
}

/// Tab content panel — renders content based on active tab
#[component]
fn TabContent(tab: Tab) -> impl IntoView {
    match tab {
        Tab::Agents => view! {
            <div class="tab-panel tab-panel-agents">
                <AgentsPanel />
            </div>
        }
        .into_any(),
        Tab::Tasks => view! { <ExtraHost tab=Tab::Tasks /> }.into_any(),
        Tab::Memory => view! { <SpaceScopedHost tab=Tab::Memory /> }.into_any(),
        Tab::Goals => view! { <ExtraHost tab=Tab::Goals /> }.into_any(),
        Tab::Personalities => view! { <ExtraHost tab=Tab::Personalities /> }.into_any(),
        Tab::SwarmDag => view! { <ExtraHost tab=Tab::SwarmDag /> }.into_any(),
        Tab::Sentinel => view! { <ExtraHost tab=Tab::Sentinel /> }.into_any(),
        Tab::EvidenceLedger => view! { <ExtraHost tab=Tab::EvidenceLedger /> }.into_any(),
        Tab::HitlApproval => view! {
            <div class="tab-panel tab-panel-hitl">
                <HitlApprovalPanel />
            </div>
        }
        .into_any(),
        Tab::Skills => view! {
            <div class="tab-panel tab-panel-skills">
                <BenchmarkPanel />
            </div>
        }
        .into_any(),
        Tab::Logs => view! {
            <div class="tab-panel tab-panel-logs">
                <LiveEventFeed />
            </div>
        }
        .into_any(),
        Tab::Soar => view! { <ExtraHost tab=Tab::Soar /> }.into_any(),
        Tab::Security => view! { <ExtraHost tab=Tab::Security /> }.into_any(),
        Tab::Awareness => view! { <ExtraHost tab=Tab::Awareness /> }.into_any(),
        Tab::SkillEvolution => view! { <ExtraHost tab=Tab::SkillEvolution /> }.into_any(),
        Tab::EmployeeGovernance => view! { <ExtraHost tab=Tab::EmployeeGovernance /> }.into_any(),
        Tab::TaskTriage => view! { <ExtraHost tab=Tab::TaskTriage /> }.into_any(),
        Tab::Threads => view! { <SpaceScopedHost tab=Tab::Threads /> }.into_any(),
        Tab::Subagents => view! { <ExtraHost tab=Tab::Subagents /> }.into_any(),
        Tab::Files => view! { <ExtraHost tab=Tab::Files /> }.into_any(),
        Tab::Voice => view! { <ExtraHost tab=Tab::Voice /> }.into_any(),
        Tab::Reactions => view! { <ExtraHost tab=Tab::Reactions /> }.into_any(),
        Tab::Avatars => view! { <ExtraHost tab=Tab::Avatars /> }.into_any(),
        Tab::Skins => view! { <ExtraHost tab=Tab::Skins /> }.into_any(),
        Tab::Fleet => view! { <ExtraHost tab=Tab::Fleet /> }.into_any(),
        Tab::Federation => view! { <ExtraHost tab=Tab::Federation /> }.into_any(),
        Tab::Inbox => view! {
            <div class="tab-panel tab-panel-inbox">
                <InboxPanel />
            </div>
        }
        .into_any(),
        Tab::Activity => view! {
            <div class="tab-panel tab-panel-activity">
                <ActivityPanel />
            </div>
        }
        .into_any(),
        Tab::Chat => view! {
        <div class="tab-panel tab-panel-chat h-full">
        <ChatPanel />
        </div>
        }
        .into_any(),
        Tab::Channels => view! { <ExtraHost tab=Tab::Channels /> }.into_any(),
        Tab::PhoneRelay => view! { <ExtraHost tab=Tab::PhoneRelay /> }.into_any(),
        Tab::Integrations => view! { <ExtraHost tab=Tab::Integrations /> }.into_any(),
        Tab::SwarmTopology => view! { <ExtraHost tab=Tab::SwarmTopology /> }.into_any(),
        Tab::SkillEditor => view! { <ExtraHost tab=Tab::SkillEditor /> }.into_any(),
        Tab::Analytics => view! { <ExtraHost tab=Tab::Analytics /> }.into_any(),
        Tab::Collab => view! { <ExtraHost tab=Tab::Collab /> }.into_any(),
    }
}

async fn fetch_health() -> Result<HealthResponse, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let base = crate::gateway_config::gateway_base()
        .ok_or("Gateway não configurado (meta zylos-gateway-url)")?;
    let url = crate::gateway_config::join_url(&base, "/api/v1/health");
    let request = Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{:?}", e))?;

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

    serde_json::from_str(&text).map_err(|e| e.to_string())
}
