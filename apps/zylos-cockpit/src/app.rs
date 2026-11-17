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
    provide_theme, AgentAvatarsGrid, AgentAwarenessPanel, AgentInfo, AgentStatus, ChannelPanel,
    ChatPanel, EmojiReactions, EmployeeGovernancePanel, EvidenceLedgerPanel, FederationDashboard,
    FileManager, FleetAuditorPanel, GoalsPanel, HitlApprovalPanel, LazyRoster, MemoryTimeline,
    PersonalitiesPanel, PhoneRelayPanel, SecurityPanel, SentinelPanel, SkillEditorPanel,
    SkillEvolutionPanel, SkinThemesPanel, SoarDashboard, SubagentStatusCards, SwarmDagPanel,
    SwarmTopologyPanel, TaskTriagePanel, TasksDag, ThemeToggle, ThreadManager, VoicePanel,
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

/// Active tab enum for dashboard navigation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Agents,
    Tasks,
    Memory,
    Goals,
    Personalities,
    SwarmDag,
    Sentinel,
    EvidenceLedger,
    HitlApproval,
    Skills,
    Logs,
    Soar,
    Security,
    Awareness,
    SkillEvolution,
    EmployeeGovernance,
    TaskTriage,
    Threads,
    Subagents,
    Files,
    Voice,
    Reactions,
    Avatars,
    Skins,
    Fleet,
    Federation,
    Chat,
    Channels,
    PhoneRelay,
    SwarmTopology,
    SkillEditor,
}

impl Tab {
    pub fn css_class(&self) -> &'static str {
        match self {
            Tab::Agents => "tab-agents",
            Tab::Tasks => "tab-tasks",
            Tab::Memory => "tab-memory",
            Tab::Goals => "tab-goals",
            Tab::Personalities => "tab-personalities",
            Tab::SwarmDag => "tab-swarm-dag",
            Tab::Sentinel => "tab-sentinel",
            Tab::EvidenceLedger => "tab-evidence-ledger",
            Tab::HitlApproval => "tab-hitl",
            Tab::Skills => "tab-skills",
            Tab::Logs => "tab-logs",
            Tab::Soar => "tab-soar",
            Tab::Security => "tab-security",
            Tab::Awareness => "tab-awareness",
            Tab::SkillEvolution => "tab-skill-evolution",
            Tab::EmployeeGovernance => "tab-employee-governance",
            Tab::TaskTriage => "tab-task-triage",
            Tab::Threads => "tab-threads",
            Tab::Subagents => "tab-subagents",
            Tab::Files => "tab-files",
            Tab::Voice => "tab-voice",
            Tab::Reactions => "tab-reactions",
            Tab::Avatars => "tab-avatars",
            Tab::Skins => "tab-skins",
            Tab::Fleet => "tab-fleet",
            Tab::Federation => "tab-federation",
            Tab::Chat => "tab-chat",
            Tab::Channels => "tab-channels",
            Tab::PhoneRelay => "tab-phone-relay",
            Tab::SwarmTopology => "tab-swarm-topology",
            Tab::SkillEditor => "tab-skill-editor",
        }
    }

    pub fn i18n_key(&self) -> &'static str {
        match self {
            Tab::Agents => "tab.agents",
            Tab::Tasks => "tab.tasks",
            Tab::Memory => "tab.memory",
            Tab::Goals => "tab.goals",
            Tab::Personalities => "tab.personalities",
            Tab::SwarmDag => "tab.swarm_dag",
            Tab::Sentinel => "tab.sentinel",
            Tab::EvidenceLedger => "tab.evidence_ledger",
            Tab::HitlApproval => "tab.hitl_approval",
            Tab::Skills => "tab.skills",
            Tab::Logs => "tab.logs",
            Tab::Soar => "tab.soar",
            Tab::Security => "tab.security",
            Tab::Awareness => "tab.awareness",
            Tab::SkillEvolution => "tab.skill_evolution",
            Tab::EmployeeGovernance => "tab.employee_governance",
            Tab::TaskTriage => "tab.task_triage",
            Tab::Threads => "tab.threads",
            Tab::Subagents => "tab.subagents",
            Tab::Files => "tab.files",
            Tab::Voice => "tab.voice",
            Tab::Reactions => "tab.reactions",
            Tab::Avatars => "tab.avatars",
            Tab::Skins => "tab.skins",
            Tab::Fleet => "tab.fleet",
            Tab::Federation => "tab.federation",
            Tab::Chat => "tab.chat",
            Tab::Channels => "tab.channels",
            Tab::PhoneRelay => "tab.phone_relay",
            Tab::SwarmTopology => "tab.swarm_topology",
            Tab::SkillEditor => "tab.skill_editor",
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
    let stream_ctx = provide_stream(StreamConfig::default());
    stream_ctx.connect(); // abre o SSE em /api/v1/events

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
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        <TabButton tab=Tab::SwarmTopology active_tab=active_tab set_active_tab=set_active_tab />
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                <TabButton tab=Tab::SkillEditor active_tab=active_tab set_active_tab=set_active_tab />
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
            }
            .into_any()
        }
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
    }
}

async fn fetch_health() -> Result<HealthResponse, String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Request, RequestInit, Response};

    let opts = RequestInit::new();
    opts.set_method("GET");

    let request = Request::new_with_str_and_init("http://localhost:3000/api/v1/health", &opts)
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
