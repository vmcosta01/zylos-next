//! Event router: evaluates playbook triggers against incoming events and dispatches incidents.

use crate::error::Result;
use crate::event::ZylosSecurityEvent;
use crate::playbook::{Playbook, PlaybookCursor};
use crate::runnable::IncidentContext;
use serde_json::Value;
use uuid::Uuid;

/// A dispatched incident ready for executor handoff.
#[derive(Debug)]
/// Estrutura DispatchedIncident.
pub struct DispatchedIncident {
    /// Campo incident id.
    pub incident_id: Uuid,
    /// Campo playbook.
    pub playbook: Playbook,
    /// Campo cursor.
    pub cursor: PlaybookCursor,
    /// Campo context.
    pub context: IncidentContext,
}

/// Evaluates playbook triggers against incoming events and creates incident contexts.
pub struct EventRouter {
    playbooks: Vec<Playbook>,
}

impl EventRouter {
    /// Função new.
    pub fn new() -> Self {
        // Variante Self.
        Self {
            playbooks: Vec::new(),
        }
    }

    /// Register a playbook for automatic trigger evaluation.
    pub fn register_playbook(&mut self, playbook: Playbook) {
        self.playbooks.push(playbook);
    }

    /// Return a slice of all registered playbooks.
    pub fn playbooks(&self) -> &[Playbook] {
        &self.playbooks
    }

    /// Remove all registered playbooks.
    pub fn clear(&mut self) {
        self.playbooks.clear();
    }

    /// Evaluate all registered playbooks against an incoming event.
    /// Returns a list of dispatched incidents for playbooks whose triggers matched.
    pub fn route(&self, event: &ZylosSecurityEvent) -> Result<Vec<DispatchedIncident>> {
        let mut dispatched = Vec::new();
        let event_value = serde_json::to_value(event)?;

        for playbook in &self.playbooks {
            if self.evaluate_triggers(playbook, &event_value)? {
                let incident_id = Uuid::new_v4();
                let context = IncidentContext::new(
                    incident_id,
                    event.id,
                    event.severity.to_string(),
                    event.category.to_string(),
                    event.affected_assets.clone(),
                    "event-router",
                );
                let cursor = PlaybookCursor::new(playbook);

                dispatched.push(DispatchedIncident {
                    incident_id,
                    playbook: playbook.clone(),
                    cursor,
                    context,
                });
            }
        }

        // Variante Ok.
        Ok(dispatched)
    }

    /// Evaluates if any trigger in the playbook matches the event (OR logic).
    fn evaluate_triggers(&self, playbook: &Playbook, event_value: &Value) -> Result<bool> {
        if playbook.triggers.is_empty() {
            // Playbooks without explicit triggers are assumed to be manually dispatched.
            return Ok(false);
        }

        for trigger in &playbook.triggers {
            if self.evaluate_expression(&trigger.expression, event_value)? {
                return Ok(true);
            }
        }
        // Variante Ok.
        Ok(false)
    }

    /// Simple expression evaluator for trigger conditions.
    /// Supports basic equality: `event.field == "value"` or `event.field == 123`.
    fn evaluate_expression(&self, expr: &str, event_value: &Value) -> Result<bool> {
        if let Some((path, expected_raw)) = expr.split_once("==") {
            let path = path.trim().strip_prefix("event.").unwrap_or(path.trim());
            let expected_raw = expected_raw.trim();

            let expected_value = parse_literal(expected_raw);

            match get_nested_value(event_value, path) {
                // Variante Some.
                Some(actual) => Ok(actual == &expected_value),
                // Variante None.
                None => Ok(false),
            }
        } else if let Some((path, expected_raw)) = expr.split_once("!=") {
            let path = path.trim().strip_prefix("event.").unwrap_or(path.trim());
            let expected_raw = expected_raw.trim();

            let expected_value = parse_literal(expected_raw);

            match get_nested_value(event_value, path) {
                // Variante Some.
                Some(actual) => Ok(actual != &expected_value),
                // Variante None.
                None => Ok(true), // field missing != value is true
            }
        } else {
            // Fallback: treat as existence check (truthy)
            let path = expr.trim().strip_prefix("event.").unwrap_or(expr.trim());
            // Variante Ok.
            Ok(get_nested_value(event_value, path).is_some())
        }
    }
}

fn parse_literal(raw: &str) -> Value {
    let trimmed = raw.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        Value::String(trimmed[1..trimmed.len() - 1].to_string())
    } else if let Ok(n) = trimmed.parse::<i64>() {
        Value::Number(serde_json::Number::from(n))
    } else if let Ok(b) = trimmed.parse::<bool>() {
        Value::Bool(b)
    } else {
        Value::String(trimmed.to_string())
    }
}

fn get_nested_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for key in path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventCategory, Severity};
    use chrono::Utc;

    fn make_event(category: EventCategory, severity: Severity) -> ZylosSecurityEvent {
        // Variante ZylosSecurityEvent.
        ZylosSecurityEvent {
            id: Uuid::new_v4(),
            received_at: Utc::now(),
            source: "test".to_string(),
            category,
            severity,
            title: "Test Event".to_string(),
            description: "Test description".to_string(),
            affected_assets: vec!["host-01".to_string()],
            raw_payload: serde_json::json!({}),
        }
    }

    const BREACH_PLAYBOOK_YAML: &str = r#"
name: breach-response
version: "1.0"
triggers:
  - expression: "event.category == \"breach\""
states:
  - name: triage
    auto: true
    terminal: true
"#;

    const CRITICAL_PLAYBOOK_YAML: &str = r#"
name: critical-response
version: "1.0"
triggers:
  - expression: "event.severity == \"critical\""
states:
  - name: isolate
    auto: true
    terminal: true
"#;

    #[test]
    fn router_dispatches_on_matching_trigger() {
        let mut router = EventRouter::new();
        router.register_playbook(Playbook::from_yaml(BREACH_PLAYBOOK_YAML).unwrap());

        let event = make_event(EventCategory::Breach, Severity::High);
        let dispatched = router.route(&event).unwrap();

        assert_eq!(dispatched.len(), 1);
        assert_eq!(dispatched[0].playbook.name, "breach-response");
        assert_eq!(dispatched[0].cursor.current_state, "triage");
    }

    #[test]
    fn router_ignores_non_matching_events() {
        let mut router = EventRouter::new();
        router.register_playbook(Playbook::from_yaml(BREACH_PLAYBOOK_YAML).unwrap());

        let event = make_event(EventCategory::Anomaly, Severity::Low);
        let dispatched = router.route(&event).unwrap();

        assert_eq!(dispatched.len(), 0);
    }

    #[test]
    fn router_dispatches_multiple_playbooks() {
        let mut router = EventRouter::new();
        router.register_playbook(Playbook::from_yaml(BREACH_PLAYBOOK_YAML).unwrap());
        router.register_playbook(Playbook::from_yaml(CRITICAL_PLAYBOOK_YAML).unwrap());

        let event = make_event(EventCategory::Breach, Severity::Critical);
        let dispatched = router.route(&event).unwrap();

        assert_eq!(dispatched.len(), 2);
    }

    #[test]
    fn router_ignores_playbooks_without_triggers() {
        let yaml = r#"
name: manual-only
version: "1.0"
states:
  - name: start
    terminal: true
"#;
        let mut router = EventRouter::new();
        router.register_playbook(Playbook::from_yaml(yaml).unwrap());

        let event = make_event(EventCategory::Breach, Severity::Critical);
        let dispatched = router.route(&event).unwrap();

        assert_eq!(dispatched.len(), 0);
    }

    #[test]
    fn evaluate_inequality_expression() {
        let router = EventRouter::new();
        let event = make_event(EventCategory::Anomaly, Severity::Low);
        let event_value = serde_json::to_value(&event).unwrap();

        let result = router
            .evaluate_expression("event.category != \"breach\"", &event_value)
            .unwrap();
        assert!(result);

        let result = router
            .evaluate_expression("event.severity != \"low\"", &event_value)
            .unwrap();
        assert!(!result);
    }
}
