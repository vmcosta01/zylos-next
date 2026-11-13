//! Auto-triage engine: deterministic classification of security events.

use crate::event::{EventCategory, Severity, ZylosSecurityEvent};
use serde::{Deserialize, Serialize};

/// Action determined by the triage engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Enumeração TriageAction.
pub enum TriageAction {
    /// Isolate asset and notify on-call (Critical + Breach).
    IsolateAndNotify,
    /// Enrich context and assign for analysis within SLA.
    EnrichAndAssign,
    /// Create ticket and link to relevant playbook.
    CreateTicket,
    /// Log only; batch-process later (Low / Compliance).
    LogOnly,
    /// Suppress event; adjust rule if false positive detected.
    Suppress,
}

/// Output of the triage decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TriageDecision.
pub struct TriageDecision {
    /// Campo action.
    pub action: TriageAction,
    /// Campo playbook.
    pub playbook: Option<String>,
    /// Campo confidence.
    pub confidence: f64,
    /// Campo reason.
    pub reason: String,
}

/// Deterministic auto-triage engine (no LLM in this layer).
pub struct TriageEngine;

impl TriageEngine {
    /// Função new.
    pub fn new() -> Self {
            // Retorna nova instância do TriageEngine.
            Self
    }

    /// Classify an event into a triage decision.
    pub fn classify(&self, event: &ZylosSecurityEvent) -> TriageDecision {
        match (event.severity, event.category) {
            (Severity::Critical, EventCategory::Breach) => TriageDecision {
                action: TriageAction::IsolateAndNotify,
                playbook: Some("security-incident-response".to_string()),
                confidence: 1.0,
                reason: "Critical breach confirmed — isolate and notify on-call".to_string(),
            },
            (Severity::High, EventCategory::Anomaly) => TriageDecision {
                action: TriageAction::EnrichAndAssign,
                playbook: Some("security-incident-response".to_string()),
                confidence: 0.95,
                reason: "High-severity anomaly — enrich and assign for analysis".to_string(),
            },
            (Severity::Medium, EventCategory::Vulnerability) => TriageDecision {
                action: TriageAction::CreateTicket,
                playbook: Some("vendor-dependency-pivot".to_string()),
                confidence: 0.90,
                reason: "Medium vulnerability — create ticket and link playbook".to_string(),
            },
            (_, EventCategory::FalsePositive) => TriageDecision {
                action: TriageAction::Suppress,
                playbook: None,
                confidence: 1.0,
                reason: "False positive detected — suppress and log learning".to_string(),
            },
            (Severity::Low | Severity::Info, _) => TriageDecision {
                action: TriageAction::LogOnly,
                playbook: None,
                confidence: 1.0,
                reason: "Low/Info severity — log only, batch process".to_string(),
            },
            _ => TriageDecision {
                action: TriageAction::EnrichAndAssign,
                playbook: Some("security-incident-response".to_string()),
                confidence: 0.80,
                reason: format!(
                    "Default triage for {:?}/{:?} — enrich and assign",
                    event.severity, event.category
                ),
            },
        }
    }
}

impl Default for TriageEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_event(severity: Severity, category: EventCategory) -> ZylosSecurityEvent {
        // Variante ZylosSecurityEvent.
        ZylosSecurityEvent {
            id: Uuid::new_v4(),
            source: "test".to_string(),
            severity,
            category,
            title: "Test".to_string(),
            description: "Test event".to_string(),
            affected_assets: vec![],
            raw_payload: serde_json::json!({}),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn critical_breach_triggers_isolation() {
        let engine = TriageEngine::new();
        let event = make_event(Severity::Critical, EventCategory::Breach);
        let decision = engine.classify(&event);
        assert_eq!(decision.action, TriageAction::IsolateAndNotify);
        assert_eq!(
            decision.playbook.as_deref(),
            Some("security-incident-response")
        );
    }

    #[test]
    fn false_positive_suppressed() {
        let engine = TriageEngine::new();
        let event = make_event(Severity::High, EventCategory::FalsePositive);
        let decision = engine.classify(&event);
        assert_eq!(decision.action, TriageAction::Suppress);
        assert!(decision.playbook.is_none());
    }

    #[test]
    fn low_severity_logs_only() {
        let engine = TriageEngine::new();
        let event = make_event(Severity::Low, EventCategory::Compliance);
        let decision = engine.classify(&event);
        assert_eq!(decision.action, TriageAction::LogOnly);
    }
}