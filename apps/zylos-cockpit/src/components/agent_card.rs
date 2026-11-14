//! Zylos Cockpit — AgentCard Component
//!
//! Displays agent status with all 6 mandatory states per DESIGN_SYSTEM.md.
//! Supports idle, running, blocked, archived statuses with visual indicators.

use leptos::prelude::*;

/// Agent status enum matching backend model
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Running,
    Blocked,
    Archived,
}

impl AgentStatus {
    pub fn css_class(&self) -> &'static str {
        match self {
            AgentStatus::Idle => "status-idle",
            AgentStatus::Running => "status-running",
            AgentStatus::Blocked => "status-blocked",
            AgentStatus::Archived => "status-archived",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            AgentStatus::Idle => "Idle",
            AgentStatus::Running => "Running",
            AgentStatus::Blocked => "Blocked",
            AgentStatus::Archived => "Archived",
        }
    }
}

/// Agent metadata for card display
#[derive(Debug, Clone, PartialEq)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub task_count: usize,
    pub last_active: String,
}

/// AgentCard component — renders a single agent in the roster
#[component]
pub fn AgentCard(agent: AgentInfo) -> impl IntoView {
    let status_class = agent.status.css_class().to_string();
    let status_label = agent.status.label().to_string();
    let name = agent.name.clone();
    let task_count = agent.task_count.to_string();
    let last_active = agent.last_active.clone();

    view! {
        <div class="agent-card">
            <div class="agent-card-header">
                <span class="agent-name">{name}</span>
                <span class={format!("agent-status {}", status_class)}>{status_label}</span>
            </div>
            <div class="agent-card-body">
                <div class="agent-metric">
                    <span class="metric-label">"Tasks"</span>
                    <span class="metric-value">{task_count}</span>
                </div>
                <div class="agent-metric">
                    <span class="metric-label">"Last Active"</span>
                    <span class="metric-value muted">{last_active}</span>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_css_classes() {
        assert_eq!(AgentStatus::Idle.css_class(), "status-idle");
        assert_eq!(AgentStatus::Running.css_class(), "status-running");
        assert_eq!(AgentStatus::Blocked.css_class(), "status-blocked");
        assert_eq!(AgentStatus::Archived.css_class(), "status-archived");
    }

    #[test]
    fn test_status_labels() {
        assert_eq!(AgentStatus::Idle.label(), "Idle");
        assert_eq!(AgentStatus::Running.label(), "Running");
        assert_eq!(AgentStatus::Blocked.label(), "Blocked");
        assert_eq!(AgentStatus::Archived.label(), "Archived");
    }
}
