//! Zylos Cockpit — Root App Component
//!
//! Top-level Leptos component rendered into #app.
//! Implements Dashboard with tabs (Agents, Tasks, Memory, Skills, Logs),
//! theme toggle, i18n, and all 6 mandatory states per APP_FLOW.md and DESIGN_SYSTEM.md.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

use crate::benchmarks::BenchmarkPanel;
use crate::components::analytics_panel::AnalyticsPanel;
use crate::components::{
    provide_theme, AgentAvatarsGrid, AgentAwarenessPanel, AgentsPanel, AuthHeaderButton,
    ChannelPanel, ChatPanel, CollabPanel, EmojiReactions, EmployeeGovernancePanel,
    EvidenceLedgerPanel, FederationDashboard, FileManager, FleetAuditorPanel, GoalsPanel,
    HitlApprovalPanel, IntegrationsPanel, MemoryTimeline, PersonalitiesPanel, PhoneRelayPanel,
    SecurityPanel, SentinelPanel, SkillEditorPanel, SkillEvolutionPanel, SkinThemesPanel,
    SoarDashboard, SubagentStatusCards, SwarmDagPanel, SwarmTopologyPanel, TaskTriagePanel,
    TasksDag, ThemeToggle, ThreadManager, VoicePanel,
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

/// Root application component
#[component]
pub fn App() -> impl IntoView {
    // Initialize global contexts
    let _theme_ctx = provide_theme();
    let _i18n_ctx = provide_i18n(Locale::PtBr);
    let stream_ctx = provide_stream(StreamConfig::default());
    stream_ctx.connect(); // abre o SSE em /api/v1/events

    let (state, set_state) = signal(DashboardState::Loading);
    let (active_tab, set_active_tab) = signal(Tab::Agents);
    let palette_open = RwSignal::new(false);

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
            <div class="cockpit-root">
                <header class="cockpit-header">
                    <h1><AppTitle /></h1>
                    <div class="header-actions">
                        <span class="version">{move || match &*state.read() {
                            DashboardState::Success(h) => format!("v{}", h.version),
                            _ => "v0.1.0".to_string(),
                        }}</span>
                        <ThemeToggle />
                        <AuthHeaderButton />
                    </div>
                </header>

                <nav class="cockpit-tabs" aria-label="Abas do cockpit">
                                <TabButton tab=Tab::Agents active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Tasks active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Memory active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Goals active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Personalities active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::SwarmDag active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Sentinel active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::EvidenceLedger active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::HitlApproval active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Skills active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Logs active_tab=active_tab set_active_tab=set_active_tab />
                                <TabButton tab=Tab::Soar active_tab=active_tab set_active_tab=set_active_tab />
                                                            <TabButton tab=Tab::Security active_tab=active_tab set_active_tab=set_active_tab />
                                                            <TabButton tab=Tab::Awareness active_tab=active_tab set_active_tab=set_active_tab />
                                                            <TabButton tab=Tab::SkillEvolution active_tab=active_tab set_active_tab=set_active_tab />
                                                            <TabButton tab=Tab::EmployeeGovernance active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                    <TabButton tab=Tab::TaskTriage active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                    <TabButton tab=Tab::Threads active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                    <TabButton tab=Tab::Subagents active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                     <TabButton tab=Tab::Files active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                      <TabButton tab=Tab::Voice active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                       <TabButton tab=Tab::Reactions active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                       <TabButton tab=Tab::Avatars active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                        <TabButton tab=Tab::Skins active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                         <TabButton tab=Tab::Fleet active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                          <TabButton tab=Tab::Federation active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                          <TabButton tab=Tab::Chat active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                <TabButton tab=Tab::Channels active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                                                                                                                                                                                                <TabButton tab=Tab::PhoneRelay active_tab=active_tab set_active_tab=set_active_tab />
    <TabButton tab=Tab::Integrations active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            <TabButton tab=Tab::SwarmTopology active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    <TabButton tab=Tab::SkillEditor active_tab=active_tab set_active_tab=set_active_tab />
                    <TabButton tab=Tab::Analytics active_tab=active_tab set_active_tab=set_active_tab />
                    <TabButton tab=Tab::Collab active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   </nav>

                <TabPalette open=palette_open set_active_tab=set_active_tab />

                <main class="cockpit-main">
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
                            let status = health.status.clone();
                            let streams = health.active_streams.to_string();
                            let version = health.version.clone();
                            let tab = active_tab.get();
                            view! {
                                <div class="dashboard-content">
                                    <div class="metrics-bar">
                                        <div class="metric-card">
                                            <span class="metric-label"><HealthStatusLabel /></span>
                                            <span class="metric-value success">{status}</span>
                                        </div>
                                        <div class="metric-card">
                                            <span class="metric-label"><HealthStreamsLabel /></span>
                                            <span class="metric-value">{streams}</span>
                                        </div>
                                        <div class="metric-card">
                                            <span class="metric-label"><HealthVersionLabel /></span>
                                            <span class="metric-value">{version}</span>
                                        </div>
                                    </div>
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

/// Paleta simples de abas (Ctrl+K): filtra por nome; Enter abre a primeira, Esc fecha.
#[component]
fn TabPalette(open: RwSignal<bool>, set_active_tab: WriteSignal<Tab>) -> impl IntoView {
    // StoredValue é Copy: as closures internas do <Show> continuam `Fn`.
    let i18n = StoredValue::new_local(use_i18n());
    let query = RwSignal::new(String::new());
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let matches = move || {
        let q = query.get().to_lowercase();
        Tab::ALL
            .iter()
            .copied()
            .filter(|t| {
                q.is_empty()
                    || i18n
                        .with_value(|i| i.t(t.i18n_key()))
                        .to_lowercase()
                        .contains(&q)
            })
            .collect::<Vec<_>>()
    };
    let choose = move |t: Tab| {
        set_active_tab.set(t);
        open.set(false);
        query.set(String::new());
    };

    Effect::new(move |_| {
        if open.get() {
            if let Some(el) = input_ref.get() {
                let _ = el.focus();
            }
        }
    });

    view! {
        <Show when=move || open.get()>
            <div class="tab-palette-backdrop" on:click=move |_| open.set(false)>
                <div
                    class="tab-palette"
                    role="dialog"
                    aria-modal="true"
                    aria-label="Paleta de abas"
                    on:click=|ev| ev.stop_propagation()
                >
                    <input
                        node_ref=input_ref
                        type="search"
                        placeholder="Ir para aba... (Esc fecha)"
                        prop:value=move || query.get()
                        on:input=move |ev| query.set(event_target_value(&ev))
                        on:keydown=move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
                            "Escape" => open.set(false),
                            "Enter" => {
                                if let Some(t) = matches().first().copied() {
                                    choose(t);
                                }
                            }
                            _ => {}
                        }
                    />
                    <ul class="tab-palette-list">
                        {move || {
                            matches()
                                .into_iter()
                                .map(|t| {
                                    let label = i18n.with_value(|i| i.t(t.i18n_key()));
                                    let pos = Tab::ALL.iter().position(|x| *x == t).unwrap_or(0);
                                    let hint = if pos < 9 { format!("Ctrl+{}", pos + 1) } else { String::new() };
                                    view! {
                                        <li>
                                            <button class="tab-palette-item" on:click=move |_| choose(t)>
                                                <span>{label}</span>
                                                <kbd>{hint}</kbd>
                                            </button>
                                        </li>
                                    }
                                })
                                .collect_view()
                        }}
                    </ul>
                </div>
            </div>
        </Show>
    }
}

/// Translated app title
#[component]
fn AppTitle() -> impl IntoView {
    let i18n = use_i18n();
    move || i18n.t("app.title")
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

/// Tab button with active state
#[component]
fn TabButton(
    tab: Tab,
    active_tab: ReadSignal<Tab>,
    set_active_tab: WriteSignal<Tab>,
) -> impl IntoView {
    let i18n = use_i18n();
    let label = move || i18n.t(tab.i18n_key());
    let is_active = move || active_tab.get() == tab;
    let css = move || {
        if is_active() {
            format!("tab-btn {} active", tab.css_class())
        } else {
            format!("tab-btn {}", tab.css_class())
        }
    };

    view! {
        <button
            class=css
            aria-current=move || is_active().then_some("page")
            on:click=move |_| set_active_tab.set(tab)
        >
            {label}
        </button>
    }
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
        Tab::Tasks => view! {
            <div class="tab-panel tab-panel-tasks">
                <TasksDag />
            </div>
        }
        .into_any(),
        Tab::Memory => view! {
            <div class="tab-panel tab-panel-memory">
                <MemoryTimeline />
            </div>
        }
        .into_any(),
        Tab::Goals => view! {
            <div class="tab-panel tab-panel-goals">
                <GoalsPanel />
            </div>
        }
        .into_any(),
        Tab::Personalities => view! {
            <div class="tab-panel tab-panel-personalities">
                <PersonalitiesPanel />
            </div>
        }
        .into_any(),
        Tab::SwarmDag => view! {
            <div class="tab-panel tab-panel-swarm-dag">
                <SwarmDagPanel />
            </div>
        }
        .into_any(),
        Tab::Sentinel => view! {
            <div class="tab-panel tab-panel-sentinel">
                <SentinelPanel />
            </div>
        }
        .into_any(),
        Tab::EvidenceLedger => view! {
            <div class="tab-panel tab-panel-evidence-ledger">
                <EvidenceLedgerPanel />
            </div>
        }
        .into_any(),
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
        Tab::Soar => view! {
            <div class="tab-panel tab-panel-soar">
                <SoarDashboard />
            </div>
        }
        .into_any(),
        Tab::Security => view! {
            <div class="tab-panel tab-panel-security">
                <SecurityPanel />
            </div>
        }
        .into_any(),
        Tab::Awareness => view! {
            <div class="tab-panel tab-panel-awareness">
                <AgentAwarenessPanel />
            </div>
        }
        .into_any(),
        Tab::SkillEvolution => view! {
            <div class="tab-panel tab-panel-skill-evolution">
                <SkillEvolutionPanel />
            </div>
        }
        .into_any(),
        Tab::EmployeeGovernance => view! {
        <div class="tab-panel tab-panel-employee-governance">
        <EmployeeGovernancePanel />
        </div>
        }
        .into_any(),
        Tab::TaskTriage => view! {
        <div class="tab-panel tab-panel-task-triage">
        <TaskTriagePanel />
        </div>
        }
        .into_any(),
        Tab::Threads => view! {
        <div class="tab-panel tab-panel-threads">
        <ThreadManager />
        </div>
        }
        .into_any(),
        Tab::Subagents => view! {
        <div class="tab-panel tab-panel-subagents">
        <SubagentStatusCards />
        </div>
        }
        .into_any(),
        Tab::Files => view! {
        <div class="tab-panel tab-panel-files">
        <FileManager />
        </div>
        }
        .into_any(),
        Tab::Voice => view! {
        <div class="tab-panel tab-panel-voice">
        <VoicePanel />
        </div>
        }
        .into_any(),
        Tab::Reactions => view! {
        <div class="tab-panel tab-panel-reactions">
        <EmojiReactions _message_id="demo-msg-1" />
        </div>
        }
        .into_any(),
        Tab::Avatars => view! {
        <div class="tab-panel tab-panel-avatars">
        <AgentAvatarsGrid />
        </div>
        }
        .into_any(),
        Tab::Skins => view! {
        <div class="tab-panel tab-panel-skins">
        <SkinThemesPanel />
        </div>
        }
        .into_any(),
        Tab::Fleet => view! {
        <div class="tab-panel tab-panel-fleet">
        <FleetAuditorPanel />
        </div>
        }
        .into_any(),
        Tab::Federation => view! {
        <div class="tab-panel tab-panel-federation">
        <FederationDashboard />
        </div>
        }
        .into_any(),
        Tab::Chat => view! {
        <div class="tab-panel tab-panel-chat h-full">
        <ChatPanel />
        </div>
        }
        .into_any(),
        Tab::Channels => view! {
        <div class="tab-panel tab-panel-channels h-full">
        <ChannelPanel />
        </div>
        }
        .into_any(),
        Tab::PhoneRelay => view! {
        <div class="tab-panel tab-panel-phone-relay">
        <PhoneRelayPanel />
        </div>
        }
        .into_any(),
        Tab::Integrations => view! {
        <div class="tab-panel tab-panel-integrations">
        <IntegrationsPanel />
        </div>
        }
        .into_any(),
        Tab::SwarmTopology => view! {
        <div class="tab-panel tab-panel-swarm-topology">
        <SwarmTopologyPanel />
        </div>
        }
        .into_any(),
        Tab::SkillEditor => view! {
        <div class="tab-panel tab-panel-skill-editor">
        <SkillEditorPanel />
        </div>
        }
        .into_any(),
        Tab::Analytics => view! {
        <div class="tab-panel tab-panel-analytics">
        <AnalyticsPanel />
        </div>
        }
        .into_any(),
        Tab::Collab => view! {
        <div class="tab-panel tab-panel-collab">
        <CollabPanel />
        </div>
        }
        .into_any(),
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
