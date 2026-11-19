//! Zylos Cockpit — AgentCard Component (Fase 7)
//!
//! Displays agent status with all 6 mandatory states per DESIGN_SYSTEM.md §6:
//! Loading (skeleton), Empty (placeholder), Error (retry), Success (normal),
//! Validation (warning badge), Disabled (grayed out).
//! Includes hover micro-interactions, click navigation, and activity sparkline.

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

/// Card rendering state — implements the 6 mandatory states from DESIGN_SYSTEM.md
#[derive(Debug, Clone, PartialEq)]
pub enum CardState {
    /// Skeleton loading placeholder
    Loading,
    /// No data / empty dataset
    Empty,
    /// Error with retry capability
    Error(String),
    /// Normal display with agent data
    Success,
    /// Warning/validation state
    Validation(String),
    /// Disabled/grayed out
    Disabled(String),
}

/// Agent metadata for card display
#[derive(Debug, Clone, PartialEq)]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub status: AgentStatus,
    pub task_count: usize,
    pub last_active: String,
    /// Activity data points for sparkline (0-100 scale, last 24h in 1h buckets)
    pub activity: Vec<u8>,
}

/// Generate SVG polyline points from activity data
fn sparkline_points(activity: &[u8], width: f32, height: f32) -> String {
    if activity.is_empty() {
        return String::new();
    }
    let len = activity.len() as f32;
    let step_x = width / len.max(1.0);
    activity
        .iter()
        .enumerate()
        .map(|(i, &v)| {
            let x = i as f32 * step_x;
            let y = height - (v as f32 / 100.0) * height;
            format!("{:.1},{:.1}", x, y)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// AgentCard component — renders a single agent with micro-interactions
#[component]
pub fn AgentCard(
    agent: AgentInfo,
    /// Card rendering state (default: Success)
    #[prop(default = CardState::Success)]
    state: CardState,
) -> impl IntoView {
    match state {
        CardState::Loading => view! {
            <div class="agent-card agent-card-loading">
                <div class="skeleton-line skeleton-title"></div>
                <div class="skeleton-line skeleton-badge"></div>
                <div class="skeleton-line skeleton-metric"></div>
            </div>
        }
        .into_any(),

        CardState::Empty => view! {
            <div class="agent-card agent-card-empty">
                <span class="empty-icon">"🤖"</span>
                <p>"Nenhum agente"</p>
            </div>
        }
        .into_any(),

        CardState::Error(msg) => view! {
            <div class="agent-card agent-card-error" role="alert">
                <span class="error-icon">"⚠"</span>
                <p class="error-message">{msg}</p>
                <button class="cta-retry agent-card-retry">"Tentar novamente"</button>
            </div>
        }
        .into_any(),

        CardState::Validation(warning) => {
            let status_class = agent.status.css_class().to_string();
            let status_label = agent.status.label().to_string();
            let name = agent.name.clone();
            let task_count = agent.task_count.to_string();
            let last_active = agent.last_active.clone();
            let sparkline = sparkline_points(&agent.activity, 60.0, 20.0);
            let agent_id = agent.id.clone();

            view! {
                <div
                    class="agent-card agent-card-validation"
                    tabindex="0"
                    role="button"
                    aria-label=format!("Agente {} — aviso: {}", name, warning)
                    on:click=move |_| navigate_to_agent(&agent_id)
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" || ev.key() == " " {
                            navigate_to_agent(&agent_id);
                        }
                    }
                >
                    <div class="validation-badge" title=warning.clone()>"⚠"</div>
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
                    {if !sparkline.is_empty() {
                        view! {
                            <svg class="agent-sparkline" width="60" height="20" viewBox="0 0 60 20">
                                <polyline points=sparkline fill="none" stroke="var(--color-status-warning)" stroke-width="1.5" />
                            </svg>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }}
                </div>
            }
            .into_any()
        }

        CardState::Disabled(reason) => view! {
            <div class="agent-card agent-card-disabled" aria-disabled="true" title=reason>
                <div class="agent-card-header">
                    <span class="agent-name">{agent.name.clone()}</span>
                    <span class="agent-status status-archived">"Disabled"</span>
                </div>
                <p class="disabled-reason">{reason}</p>
            </div>
        }
        .into_any(),

        CardState::Success => {
            let status_class = agent.status.css_class().to_string();
            let status_label = agent.status.label().to_string();
            let name = agent.name.clone();
            let task_count = agent.task_count.to_string();
            let last_active = agent.last_active.clone();
            let sparkline = sparkline_points(&agent.activity, 60.0, 20.0);
            let agent_id = agent.id.clone();

            view! {
                <div
                    class="agent-card agent-card-success"
                    tabindex="0"
                    role="button"
                    aria-label=format!("Agente {} — {}", name, status_label)
                    on:click=move |_| navigate_to_agent(&agent_id)
                    on:keydown=move |ev| {
                        if ev.key() == "Enter" || ev.key() == " " {
                            navigate_to_agent(&agent_id);
                        }
                    }
                >
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
                    {if !sparkline.is_empty() {
                        view! {
                            <svg class="agent-sparkline" width="60" height="20" viewBox="0 0 60 20">
                                <polyline points=sparkline fill="none" stroke="var(--color-accent-primary)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
                            </svg>
                        }.into_any()
                    } else {
                        view! { <span></span> }.into_any()
                    }}
                </div>
            }
            .into_any()
        }
    }
}

/// Navigate to agent detail page
fn navigate_to_agent(agent_id: &str) {
    if let Some(window) = web_sys::window() {
        let url = format!("/agents/{}", agent_id);
        let _ = window.location().set_href(&url);
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

    #[test]
    fn test_sparkline_points_empty() {
        assert_eq!(sparkline_points(&[], 60.0, 20.0), "");
    }

    #[test]
    fn test_sparkline_points_single() {
        let pts = sparkline_points(&[50], 60.0, 20.0);
        assert!(pts.contains(","));
    }

    #[test]
    fn test_sparkline_points_multiple() {
        let pts = sparkline_points(&[0, 50, 100], 60.0, 20.0);
        let parts: Vec<&str> = pts.split(' ').collect();
        assert_eq!(parts.len(), 3);
        // First point at bottom (y=20), last at top (y=0)
        assert!(parts[0].ends_with(",20.0"));
        assert!(parts[2].ends_with(",0.0"));
    }

    #[test]
    fn test_card_state_variants() {
        assert_eq!(CardState::Loading, CardState::Loading);
        assert_ne!(CardState::Loading, CardState::Empty);
        assert_eq!(
            CardState::Error("fail".to_string()),
            CardState::Error("fail".to_string())
        );
    }
}
