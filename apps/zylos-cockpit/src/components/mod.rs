//! Zylos Cockpit — Components Module

pub mod agent_awareness_panel;
pub mod agent_card;
pub mod agents_panel;
pub mod chat_panel;
pub mod employee_governance_panel;
pub mod evidence_ledger_panel;
pub mod goals_panel;
pub mod hitl_approval_panel;
pub mod inbox_panel;
pub mod inline_computer;
pub use agents_panel::AgentsPanel;
pub mod lazy_roster;
pub mod memory_timeline;
pub mod personalities_panel;
pub mod security_panel;
pub mod sentinel_panel;
pub mod skill_evolution_panel;
pub mod soar_dashboard;
pub mod swarm_dag_panel;
pub mod task_triage_panel;
pub use task_triage_panel::TaskTriagePanel;
pub mod thread_manager;
pub use thread_manager::ThreadManager;
pub mod subagent_status_cards;
pub use subagent_status_cards::SubagentStatusCards;
pub mod file_manager;
pub use file_manager::FileManager;
pub mod voice_panel;
pub use voice_panel::VoicePanel;
pub mod emoji_reactions;
pub use emoji_reactions::EmojiReactions;
pub mod agent_avatars;
pub mod analytics_charts;
pub mod analytics_panel;
pub use agent_avatars::AgentAvatarsGrid;
pub use analytics_panel::AnalyticsPanel;
pub mod skin_themes;
pub use skin_themes::SkinThemesPanel;
pub mod fleet_auditor;
pub use fleet_auditor::FleetAuditorPanel;
pub mod federation_dashboard;
pub use federation_dashboard::FederationDashboard;
pub mod swarm_topology_panel;
pub use swarm_topology_panel::SwarmTopologyPanel;
pub mod skill_editor_panel;
pub use skill_editor_panel::SkillEditorPanel;
pub mod channel_panel;
pub mod tasks_dag;
pub use channel_panel::ChannelPanel;
pub mod phone_relay_panel;
pub use phone_relay_panel::PhoneRelayPanel;
pub mod vnc_canvas;
pub use vnc_canvas::VncStream;
pub mod integrations_panel;
pub use integrations_panel::IntegrationsPanel;
pub mod auth_login;
pub use auth_login::AuthHeaderButton;
pub mod collab_panel;
pub mod collapsible_sidebar;
pub mod command_palette;
pub mod theme_toggle;
pub mod translucent_header;
pub use collab_panel::CollabPanel;
pub use collapsible_sidebar::{CollapsibleSidebar, SidebarGroup};
pub use command_palette::CommandPalette;
pub use translucent_header::{GatewayStatus, TranslucentHeader};

pub use agent_awareness_panel::AgentAwarenessPanel;
#[allow(unused_imports)]
pub use agent_card::{AgentCard, AgentInfo, AgentStatus};
pub use chat_panel::ChatPanel;
pub use employee_governance_panel::EmployeeGovernancePanel;
pub use evidence_ledger_panel::EvidenceLedgerPanel;
pub use goals_panel::GoalsPanel;
pub use hitl_approval_panel::{
    approve_all_pending, pending_approvals_count, HitlApprovalPanel, InlineApprovals,
};
pub use inbox_panel::{ActivityPanel, InboxPanel};
pub use inline_computer::InlineComputers;
pub use lazy_roster::LazyRoster;
pub use memory_timeline::MemoryTimeline;
pub use personalities_panel::PersonalitiesPanel;
pub use security_panel::SecurityPanel;
pub use sentinel_panel::SentinelPanel;
pub use skill_evolution_panel::SkillEvolutionPanel;
pub use soar_dashboard::SoarDashboard;
pub use swarm_dag_panel::SwarmDagPanel;
pub use tasks_dag::TasksDag;
pub use theme_toggle::{provide_theme, ThemeToggle};
#[allow(unused_imports)]
pub use theme_toggle::{use_theme, Theme, ThemeContext};
pub mod toast_container;
pub use toast_container::{
    provide_toast_store, toast_error, toast_info, toast_success, toast_warning, use_toast_store,
    ToastContainer, ToastStore, ToastVariant,
};
pub mod empty_state;
pub use empty_state::EmptyState;
pub mod error_boundary;
pub use error_boundary::ErrorBoundary;
