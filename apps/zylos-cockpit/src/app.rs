//! Zylos Cockpit — Root App Component
//!
//! Top-level Leptos component rendered into #app.
//! Implements Dashboard with tabs (Agents, Tasks, Memory, Skills, Logs),
//! theme toggle, i18n, and all 6 mandatory states per APP_FLOW.md and DESIGN_SYSTEM.md.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

use crate::benchmarks::BenchmarkPanel;
use crate::components::{AgentInfo, AgentStatus, LazyRoster, SoarDashboard, ThemeToggle, provide_theme};
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
pub enum DashboardState {
    Loading,
    Empty,
    Success(HealthResponse),
    Error(String),
    Disabled(String),
}

/// Active tab enum for dashboard navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Tasks,
    Memory,
    Skills,
    Logs,
    Soar,
}

impl Tab {
    pub fn css_class(&self) -> &'static str {
        match self {
            Tab::Agents => "tab-agents",
            Tab::Tasks => "tab-tasks",
            Tab::Memory => "tab-memory",
            Tab::Skills => "tab-skills",
            Tab::Logs => "tab-logs",
            Tab::Soar => "tab-soar",
        }
    }

    pub fn i18n_key(&self) -> &'static str {
        match self {
            Tab::Agents => "tab.agents",
            Tab::Tasks => "tab.tasks",
            Tab::Memory => "tab.memory",
            Tab::Skills => "tab.skills",
            Tab::Logs => "tab.logs",
            Tab::Soar => "tab.soar",
        }
    }
}

/// Sample agents for demo/empty-state testing
fn sample_agents() -> Vec<AgentInfo> {
    vec![
        AgentInfo {
            id: "agent-001".to_string(),
            name: "Chief of Staff".to_string(),
            status: AgentStatus::Running,
            task_count: 12,
            last_active: "2m ago".to_string(),
        },
        AgentInfo {
            id: "agent-002".to_string(),
            name: "Backend Engineer".to_string(),
            status: AgentStatus::Idle,
            task_count: 3,
            last_active: "15m ago".to_string(),
        },
        AgentInfo {
            id: "agent-003".to_string(),
            name: "Code Reviewer".to_string(),
            status: AgentStatus::Blocked,
            task_count: 7,
            last_active: "1h ago".to_string(),
        },
    ]
}

/// Root application component
#[component]
pub fn App() -> impl IntoView {
    // Initialize global contexts
    let _theme_ctx = provide_theme();
    let _i18n_ctx = provide_i18n(Locale::PtBr);
    let _stream_ctx = provide_stream(StreamConfig::default());

    let (state, set_state) = signal(DashboardState::Loading);
    let (active_tab, set_active_tab) = signal(Tab::Agents);

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
                </div>
            </header>

            <nav class="cockpit-tabs">
                <TabButton tab=Tab::Agents active_tab=active_tab set_active_tab=set_active_tab />
                <TabButton tab=Tab::Tasks active_tab=active_tab set_active_tab=set_active_tab />
                <TabButton tab=Tab::Memory active_tab=active_tab set_active_tab=set_active_tab />
                <TabButton tab=Tab::Skills active_tab=active_tab set_active_tab=set_active_tab />
                <TabButton tab=Tab::Logs active_tab=active_tab set_active_tab=set_active_tab />
                <TabButton tab=Tab::Soar active_tab=active_tab set_active_tab=set_active_tab />
            </nav>

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
                            <div class="error-banner">
                                <p>{format!("Connection lost: {}", err_msg)}</p>
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
        Tab::Agents => {
            let agents = sample_agents();
            view! {
                <div class="tab-panel tab-panel-agents">
                    <LazyRoster agents=agents max_visible=20 />
                </div>
            }.into_any()
        }
        Tab::Tasks => view! {
            <div class="tab-panel tab-panel-tasks">
                <div class="placeholder-panel">
                    <p>"Task DAG visualization — coming in Sprint 6.2.2"</p>
                </div>
            </div>
        }.into_any(),
        Tab::Memory => view! {
            <div class="tab-panel tab-panel-memory">
                <div class="placeholder-panel">
                    <p>"Memory timeline — coming in Sprint 6.2.3"</p>
                </div>
            </div>
        }.into_any(),
        Tab::Skills => view! {
            <div class="tab-panel tab-panel-skills">
                <BenchmarkPanel />
            </div>
        }.into_any(),
        Tab::Logs => view! {
            <div class="tab-panel tab-panel-logs">
                <LiveEventFeed />
            </div>
        }.into_any(),
        Tab::Soar => view! {
            <div class="tab-panel tab-panel-soar">
                <SoarDashboard />
            </div>
        }.into_any(),
    }
}

async fn fetch_health() -> Result<HealthResponse, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init("/api/v1/health", &opts)
        .map_err(|e| format!("{:?}", e))?;

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