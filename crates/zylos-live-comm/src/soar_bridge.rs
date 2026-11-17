//! SOAR Bridge — Converts SOAR event data into AG-UI streaming events.
//!
//! This module bridges SOAR engine outputs into the `AgUiEvent` protocol
//! used by the cockpit dashboard SSE stream. It accepts primitive types
//! rather than `zylos_soar` structs to avoid circular crate dependencies
//! (zylos-soar depends on zylos-live-comm for EventBus).

use crate::agui::AgUiEvent;
use crate::event_bus::EventBus;
use serde_json::json;
use tracing::{debug, warn};

/// Severity mapping from SOAR to dashboard display strings.
fn severity_label(severity: &str) -> &'static str {
    match severity {
        "Critical" | "critical" => "Critical",
        "High" | "high" => "High",
        "Medium" | "medium" => "Medium",
        "Low" | "low" => "Low",
        _ => "Info",
    }
}

/// Publishes a SOAR security event to the AG-UI EventBus as a StateSnapshot.
///
/// Accepts individual fields rather than a `ZylosSecurityEvent` struct
/// to avoid depending on `zylos-soar` (which would create a cycle).
#[allow(clippy::too_many_arguments)]
pub fn publish_soar_event(
    bus: &EventBus,
    event_id: &str,
    source: &str,
    severity: &str,
    category: &str,
    title: &str,
    description: &str,
    affected_assets: &[String],
    received_at_rfc3339: &str,
) {
    let snapshot = json!({
        "type": "soar_event",
        "event_id": event_id,
        "source": source,
        "severity": severity_label(severity),
        "category": category,
        "title": title,
        "description": description,
        "affected_assets": affected_assets,
        "received_at": received_at_rfc3339,
    });

    let agui_event = AgUiEvent::state_snapshot(snapshot);

    match bus.publish_agui(agui_event) {
        Ok(n) => debug!(
            event_id = event_id,
            receivers = n,
            "SOAR event published to AG-UI"
        ),
        Err(e) => {
            warn!(event_id = event_id, error = %e, "Failed to publish SOAR event (no subscribers?)")
        }
    }
}

/// Publishes a playbook execution start event to the AG-UI stream.
pub fn publish_playbook_started(bus: &EventBus, playbook_name: &str, event_id: &str) {
    let snapshot = json!({
        "type": "soar_playbook_started",
        "playbook_name": playbook_name,
        "event_id": event_id,
    });

    let agui_event =
        AgUiEvent::tool_call_start(&format!("soar-pb-{}", event_id), playbook_name, snapshot);

    match bus.publish_agui(agui_event) {
        Ok(n) => debug!(
            playbook = playbook_name,
            receivers = n,
            "Playbook start published"
        ),
        Err(e) => warn!(playbook = playbook_name, error = %e, "Failed to publish playbook start"),
    }
}

/// Publishes a playbook execution completion event to the AG-UI stream.
pub fn publish_playbook_completed(
    bus: &EventBus,
    playbook_name: &str,
    event_id: &str,
    success: bool,
    states_executed: usize,
    error: Option<&str>,
) {
    let result = json!({
        "type": "soar_playbook_completed",
        "playbook_name": playbook_name,
        "event_id": event_id,
        "success": success,
        "states_executed": states_executed,
        "error": error,
    });

    let agui_event =
        AgUiEvent::tool_call_result(&format!("soar-pb-{}", event_id), result, !success);

    match bus.publish_agui(agui_event) {
        Ok(n) => debug!(
            playbook = playbook_name,
            receivers = n,
            "Playbook completion published"
        ),
        Err(e) => {
            warn!(playbook = playbook_name, error = %e, "Failed to publish playbook completion")
        }
    }
}

/// Publishes a validation result snapshot to the AG-UI stream.
pub fn publish_validation_result(
    bus: &EventBus,
    total: usize,
    valid: usize,
    warnings: &[String],
    errors: &[String],
) {
    let snapshot = json!({
        "type": "soar_validation",
        "total_playbooks": total,
        "valid_count": valid,
        "warnings": warnings,
        "errors": errors,
    });

    let agui_event = AgUiEvent::state_snapshot(snapshot);

    match bus.publish_agui(agui_event) {
        Ok(n) => debug!(receivers = n, "Validation result published"),
        Err(e) => warn!(error = %e, "Failed to publish validation result"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_publish_soar_event_produces_state_snapshot() {
        let bus = EventBus::new(16, None);
        let mut stream = bus.subscribe_legacy();

        publish_soar_event(
            &bus,
            "evt-001",
            "test-source",
            "Critical",
            "Breach",
            "Test Breach",
            "Test description",
            &["host-01".to_string()],
            "2026-11-01T00:00:00Z",
        );

        let received = stream.next().await.unwrap().unwrap();
        if let crate::event_types::ZylosEvent::Custom(c) = &received.payload {
            let agui: AgUiEvent = serde_json::from_value(c.payload.clone()).unwrap();
            assert!(matches!(agui, AgUiEvent::StateSnapshot { .. }));
            if let AgUiEvent::StateSnapshot { state, .. } = agui {
                assert_eq!(state["type"], "soar_event");
                assert_eq!(state["severity"], "Critical");
                assert_eq!(state["category"], "Breach");
                assert_eq!(state["title"], "Test Breach");
            }
        } else {
            panic!("expected Custom event");
        }
    }

    #[tokio::test]
    async fn test_publish_playbook_lifecycle() {
        let bus = EventBus::new(16, None);
        let mut stream = bus.subscribe_legacy();

        publish_playbook_started(&bus, "pb-test", "evt-001");
        publish_playbook_completed(&bus, "pb-test", "evt-001", true, 3, None);

        let start_env = stream.next().await.unwrap().unwrap();
        if let crate::event_types::ZylosEvent::Custom(c) = &start_env.payload {
            let agui: AgUiEvent = serde_json::from_value(c.payload.clone()).unwrap();
            assert!(matches!(agui, AgUiEvent::ToolCallStart { .. }));
        } else {
            panic!("expected Custom event for start");
        }

        let end_env = stream.next().await.unwrap().unwrap();
        if let crate::event_types::ZylosEvent::Custom(c) = &end_env.payload {
            let agui: AgUiEvent = serde_json::from_value(c.payload.clone()).unwrap();
            assert!(matches!(
                agui,
                AgUiEvent::ToolCallResult {
                    is_error: false,
                    ..
                }
            ));
        } else {
            panic!("expected Custom event for end");
        }
    }

    #[tokio::test]
    async fn test_severity_label_mapping() {
        assert_eq!(severity_label("Critical"), "Critical");
        assert_eq!(severity_label("critical"), "Critical");
        assert_eq!(severity_label("High"), "High");
        assert_eq!(severity_label("unknown"), "Info");
    }
}
