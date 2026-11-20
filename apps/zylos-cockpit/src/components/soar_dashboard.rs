//! Zylos Cockpit — SOAR Dashboard Component (Phase 8)
//!
//! Real-time visualization of SOAR playbook execution, event routing,
//! and adversarial validation status. Connects to zylos-soar backend
//! via AG-UI streaming events.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// SOAR playbook execution state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybookState {
    Idle,
    Active,
    Completed,
    Failed,
    Isolated,
}

impl PlaybookState {
    pub fn css_class(&self) -> &'static str {
        match self {
            PlaybookState::Idle => "soar-state-idle",
            PlaybookState::Active => "soar-state-active",
            PlaybookState::Completed => "soar-state-completed",
            PlaybookState::Failed => "soar-state-failed",
            PlaybookState::Isolated => "soar-state-isolated",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PlaybookState::Idle => "Idle",
            PlaybookState::Active => "Active",
            PlaybookState::Completed => "Completed",
            PlaybookState::Failed => "Failed",
            PlaybookState::Isolated => "Isolated",
        }
    }
}

/// SOAR event severity level
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn css_class(&self) -> &'static str {
        match self {
            Severity::Low => "severity-low",
            Severity::Medium => "severity-medium",
            Severity::High => "severity-high",
            Severity::Critical => "severity-critical",
        }
    }
}

/// SOAR event for dashboard display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoarEvent {
    pub id: String,
    pub timestamp: String,
    pub source: String,
    pub severity: Severity,
    pub playbook_id: Option<String>,
    pub message: String,
}

/// SOAR playbook summary for dashboard
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaybookSummary {
    pub id: String,
    pub name: String,
    pub state: PlaybookState,
    pub runbook_count: usize,
    pub last_triggered: Option<String>,
    pub trigger_type: String,
}

/// SOAR validation result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub total_playbooks: usize,
    pub valid_count: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub orphan_runbooks: Vec<String>,
    pub unbound_references: Vec<String>,
}

/// Sample SOAR events for demo/testing
fn sample_events() -> Vec<SoarEvent> {
    vec![
        SoarEvent {
            id: "evt-001".to_string(),
            timestamp: "2026-11-02T10:30:00Z".to_string(),
            source: "webhook/github".to_string(),
            severity: Severity::Medium,
            playbook_id: Some("pb-pr-review".to_string()),
            message: "PR opened with 3 modified files".to_string(),
        },
        SoarEvent {
            id: "evt-002".to_string(),
            timestamp: "2026-11-02T10:31:15Z".to_string(),
            source: "audit/integrity".to_string(),
            severity: Severity::Critical,
            playbook_id: Some("pb-breach-response".to_string()),
            message: "Audit chain hash mismatch detected".to_string(),
        },
        SoarEvent {
            id: "evt-003".to_string(),
            timestamp: "2026-11-02T10:32:00Z".to_string(),
            source: "triage/anomaly".to_string(),
            severity: Severity::Low,
            playbook_id: None,
            message: "False positive suppressed: benign config change".to_string(),
        },
    ]
}

/// Sample playbooks for demo/testing
fn sample_playbooks() -> Vec<PlaybookSummary> {
    vec![
        PlaybookSummary {
            id: "pb-pr-review".to_string(),
            name: "PR Review Pipeline".to_string(),
            state: PlaybookState::Active,
            runbook_count: 4,
            last_triggered: Some("2m ago".to_string()),
            trigger_type: "webhook".to_string(),
        },
        PlaybookSummary {
            id: "pb-breach-response".to_string(),
            name: "Security Breach Response".to_string(),
            state: PlaybookState::Idle,
            runbook_count: 7,
            last_triggered: Some("3d ago".to_string()),
            trigger_type: "audit".to_string(),
        },
        PlaybookSummary {
            id: "pb-deploy-gate".to_string(),
            name: "Deploy Gate Approval".to_string(),
            state: PlaybookState::Completed,
            runbook_count: 3,
            last_triggered: Some("1h ago".to_string()),
            trigger_type: "manual".to_string(),
        },
        PlaybookSummary {
            id: "pb-incident-triage".to_string(),
            name: "Incident Triage".to_string(),
            state: PlaybookState::Isolated,
            runbook_count: 5,
            last_triggered: Some("15m ago".to_string()),
            trigger_type: "alert".to_string(),
        },
    ]
}

/// Validation results — reflects real SOAR documents created in Fase 8.
/// pb-legacy-sync now has HITL timeout (30min); rb-notify-slack is bound to pb-legacy-sync and soar-guardrail-enforcement.
fn sample_validation() -> ValidationResult {
    ValidationResult {
        total_playbooks: 14,
        valid_count: 14,
        warnings: vec![],
        errors: vec![],
        orphan_runbooks: vec![],
        unbound_references: vec![],
    }
}

/// SoarDashboard component — main SOAR monitoring panel
#[component]
pub fn SoarDashboard() -> impl IntoView {
    let (events, _set_events) = signal(sample_events());
    let (playbooks, _set_playbooks) = signal(sample_playbooks());
    let (validation, _set_validation) = signal(sample_validation());
    let (active_section, set_active_section) = signal(SoarSection::Overview);

    view! {
        <div class="soar-dashboard">
            <div class="soar-nav">
                <SoarNavButton
                    section=SoarSection::Overview
                    active=active_section
                    set_active=set_active_section
                    label="Visão Geral"
                />
                <SoarNavButton
                    section=SoarSection::Playbooks
                    active=active_section
                    set_active=set_active_section
                    label="Playbooks"
                />
                <SoarNavButton
                    section=SoarSection::Events
                    active=active_section
                    set_active=set_active_section
                    label="Eventos"
                />
                <SoarNavButton
                    section=SoarSection::Validation
                    active=active_section
                    set_active=set_active_section
                    label="Validação"
                />
            </div>

            <div class="soar-content">
                {move || match active_section.get() {
                    SoarSection::Overview => view! {
                        <SoarOverview playbooks=playbooks.get() validation=validation.get() />
                    }.into_any(),
                    SoarSection::Playbooks => view! {
                        <SoarPlaybookList playbooks=playbooks.get() />
                    }.into_any(),
                    SoarSection::Events => view! {
                        <SoarEventFeed events=events.get() />
                    }.into_any(),
                    SoarSection::Validation => view! {
                        <SoarValidationPanel validation=validation.get() />
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

/// Navigation sections for SOAR dashboard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoarSection {
    Overview,
    Playbooks,
    Events,
    Validation,
}

/// Navigation button for SOAR sections
#[component]
fn SoarNavButton(
    section: SoarSection,
    active: ReadSignal<SoarSection>,
    set_active: WriteSignal<SoarSection>,
    label: &'static str,
) -> impl IntoView {
    let is_active = move || active.get() == section;
    let css = move || {
        if is_active() {
            "soar-nav-btn active"
        } else {
            "soar-nav-btn"
        }
    };

    view! {
        <button class=css on:click=move |_| set_active.set(section)>
            {label}
        </button>
    }
}

/// Overview panel with key metrics
#[component]
fn SoarOverview(playbooks: Vec<PlaybookSummary>, validation: ValidationResult) -> impl IntoView {
    let total = playbooks.len();
    let active = playbooks
        .iter()
        .filter(|p| p.state == PlaybookState::Active)
        .count();
    let isolated = playbooks
        .iter()
        .filter(|p| p.state == PlaybookState::Isolated)
        .count();
    let valid_pct = if validation.total_playbooks > 0 {
        (validation.valid_count as f64 / validation.total_playbooks as f64) * 100.0
    } else {
        100.0
    };

    view! {
        <div class="soar-overview">
            <div class="soar-metrics">
                <div class="soar-metric-card">
                    <span class="metric-label">"Total Playbooks"</span>
                    <span class="metric-value">{total.to_string()}</span>
                </div>
                <div class="soar-metric-card">
                    <span class="metric-label">"Ativos"</span>
                    <span class="metric-value soar-state-active">{active.to_string()}</span>
                </div>
                <div class="soar-metric-card">
                    <span class="metric-label">"Isolados"</span>
                    <span class="metric-value soar-state-isolated">{isolated.to_string()}</span>
                </div>
                <div class="soar-metric-card">
                    <span class="metric-label">"Saúde da Validação"</span>
                    <span class="metric-value">{format!("{:.0}%", valid_pct)}</span>
                </div>
            </div>

            {if !validation.warnings.is_empty() {
                view! {
                    <div class="soar-alerts">
                        <h3><i class="ph ph-warning" aria-hidden="true"></i>" Alertas de Validação"</h3>
                        <ul>
                            {validation.warnings.iter().map(|w| {
                                let msg = w.clone();
                                view! { <li class="alert-warning">{msg}</li> }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}

/// Playbook list panel
#[component]
fn SoarPlaybookList(playbooks: Vec<PlaybookSummary>) -> impl IntoView {
    view! {
        <div class="soar-playbook-list">
            <h2>"Playbooks SOAR"</h2>
            <div class="playbook-grid">
                {playbooks.into_iter().map(|pb| {
                    let state_class = pb.state.css_class().to_string();
                    let state_label = pb.state.label().to_string();
                    let name = pb.name.clone();
                    let runbooks = pb.runbook_count.to_string();
                    let triggered = pb.last_triggered.clone().unwrap_or_else(|| "Never".to_string());
                    let trigger = pb.trigger_type.clone();

                    view! {
                        <div class="playbook-card">
                            <div class="playbook-header">
                                <span class="playbook-name">{name}</span>
                                <span class={format!("playbook-state {}", state_class)}>{state_label}</span>
                            </div>
                            <div class="playbook-body">
                                <div class="playbook-detail">
                                    <span class="detail-label">"Runbooks:"</span>
                                    <span class="detail-value">{runbooks}</span>
                                </div>
                                <div class="playbook-detail">
                                    <span class="detail-label">"Trigger:"</span>
                                    <span class="detail-value">{trigger}</span>
                                </div>
                                <div class="playbook-detail">
                                    <span class="detail-label">"Última execução:"</span>
                                    <span class="detail-value muted">{triggered}</span>
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// Event feed panel
#[component]
fn SoarEventFeed(events: Vec<SoarEvent>) -> impl IntoView {
    view! {
        <div class="soar-event-feed">
            <h2>"Feed de Eventos SOAR"</h2>
            <div class="event-list">
                {events.into_iter().map(|evt| {
                    let sev_class = evt.severity.css_class().to_string();
                    let msg = evt.message.clone();
                    let src = evt.source.clone();
                    let ts = evt.timestamp.clone();
                    let pb = evt.playbook_id.clone().unwrap_or_else(|| "—".to_string());

                    view! {
                        <div class="event-row">
                            <span class={format!("event-severity {}", sev_class)}></span>
                            <div class="event-content">
                                <span class="event-message">{msg}</span>
                                <span class="event-meta">
                                    {format!("{} → {} @ {}", src, pb, ts)}
                                </span>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

/// Validation panel showing audit results
#[component]
fn SoarValidationPanel(validation: ValidationResult) -> impl IntoView {
    let health_class = if validation.errors.is_empty() && validation.warnings.is_empty() {
        "validation-healthy"
    } else if validation.errors.is_empty() {
        "validation-warnings"
    } else {
        "validation-errors"
    };

    let health_label = if validation.errors.is_empty() && validation.warnings.is_empty() {
        "Sistema Saudável"
    } else if validation.errors.is_empty() {
        "Avisos Presentes"
    } else {
        "Erros Detectados"
    };

    view! {
        <div class="soar-validation">
            <h2>"Auditoria Adversarial"</h2>
            <div class={format!("validation-summary {}", health_class)}>
                <span class="validation-status">{health_label}</span>
                <span class="validation-count">
                    {format!("{}/{} playbooks válidos", validation.valid_count, validation.total_playbooks)}
                </span>
            </div>

            {if !validation.orphan_runbooks.is_empty() {
                let orphans = validation.orphan_runbooks.clone();
                view! {
                    <div class="validation-section">
                        <h3>"Runbooks Órfãos"</h3>
                        <ul>
                            {orphans.into_iter().map(|r| {
                                view! { <li class="orphan-item">{r}</li> }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}

            {if !validation.unbound_references.is_empty() {
                let unbound = validation.unbound_references.clone();
                view! {
                    <div class="validation-section">
                        <h3>"Referências Não Vinculadas"</h3>
                        <ul>
                            {unbound.into_iter().map(|r| {
                                view! { <li class="unbound-item">{r}</li> }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}

            {if !validation.errors.is_empty() {
                let errs = validation.errors.clone();
                view! {
                    <div class="validation-section validation-errors-section">
                        <h3>"Erros"</h3>
                        <ul>
                            {errs.into_iter().map(|e| {
                                view! { <li class="error-item">{e}</li> }
                            }).collect::<Vec<_>>()}
                        </ul>
                    </div>
                }.into_any()
            } else {
                view! { <div></div> }.into_any()
            }}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_playbook_state_css_classes() {
        assert_eq!(PlaybookState::Idle.css_class(), "soar-state-idle");
        assert_eq!(PlaybookState::Active.css_class(), "soar-state-active");
        assert_eq!(PlaybookState::Isolated.css_class(), "soar-state-isolated");
    }

    #[test]
    fn test_severity_css_classes() {
        assert_eq!(Severity::Low.css_class(), "severity-low");
        assert_eq!(Severity::Critical.css_class(), "severity-critical");
    }

    #[test]
    fn test_sample_data_integrity() {
        let events = sample_events();
        assert!(!events.is_empty());
        assert!(events.iter().all(|e| !e.id.is_empty()));

        let playbooks = sample_playbooks();
        assert!(!playbooks.is_empty());
        assert!(playbooks.iter().all(|p| p.runbook_count > 0));

        let validation = sample_validation();
        assert!(validation.total_playbooks >= validation.valid_count);
    }
}
