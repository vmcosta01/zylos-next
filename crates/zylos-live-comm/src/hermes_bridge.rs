//! Hermes Bridge — Adapter between Hermes plugins and AG-UI event system
//!
//! Validates incoming envelopes from Hermes plugins and routes them
//! through the EventBus for delivery to cockpit/mobile clients.

use serde::{Deserialize, Serialize};
use crate::agui::AgUiEvent;
use crate::error::LiveCommError;
use crate::event_bus::EventBus;

/// Envelope format expected from Hermes plugins
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HermesEnvelope {
    pub source: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// Bridge that validates and forwards Hermes envelopes to the EventBus
pub struct HermesBridge {
    bus: EventBus,
}

impl HermesBridge {
    pub fn new(bus: EventBus) -> Self {
        Self { bus }
    }

    /// Validate and forward an envelope to the EventBus.
    /// Returns error if envelope is malformed or missing required fields.
    pub fn forward(&self, envelope: &HermesEnvelope) -> Result<usize, LiveCommError> {
        // Validate required fields
        if envelope.source.is_empty() {
            return Err(LiveCommError::InvalidEvent("missing source".into()));
        }
        if envelope.event_type.is_empty() {
            return Err(LiveCommError::InvalidEvent("missing event_type".into()));
        }

        // Map envelope to AgUiEvent based on event_type
        let event = match envelope.event_type.as_str() {
            "text.start" => AgUiEvent::text_start(
                envelope.payload.get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                &envelope.source,
            ),
            "text.delta" => AgUiEvent::text_delta(
                envelope.payload.get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                envelope.payload.get("delta")
                    .and_then(|v| v.as_str())
                    .unwrap_or(""),
            ),
            "text.end" => AgUiEvent::text_end(
                envelope.payload.get("message_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
            ),
            "state.snapshot" => AgUiEvent::state_snapshot(envelope.payload.clone()),
            "error" => AgUiEvent::error(
                envelope.payload.get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN"),
                envelope.payload.get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error"),
            ),
            "tool.start" => AgUiEvent::tool_call_start(
                envelope.payload.get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                envelope.payload.get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                envelope.payload.get("arguments").cloned().unwrap_or(serde_json::json!({})),
            ),
            "tool.result" => AgUiEvent::tool_call_result(
                envelope.payload.get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                envelope.payload.get("result").cloned().unwrap_or(serde_json::json!(null)),
                envelope.payload.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
            ),
            _ => {
                // Unknown event types are forwarded as state snapshots for extensibility
                AgUiEvent::state_snapshot(envelope.payload.clone())
            }
        };

        self.bus.publish_agui(event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_stream::StreamExt;

    #[test]
    fn test_forward_text_delta() {
        let bus = EventBus::new(16, None);
        let mut sub = bus.subscribe_legacy();
        let bridge = HermesBridge::new(bus);

        let envelope = HermesEnvelope {
            source: "agent-1".into(),
            event_type: "text.delta".into(),
            payload: serde_json::json!({"message_id": "m1", "delta": "hello"}),
            timestamp: None,
        };

        let count = bridge.forward(&envelope).unwrap();
        assert_eq!(count, 1);

        let received = futures::executor::block_on(async { sub.next().await.unwrap().unwrap() });
        // Verify it's wrapped in a Custom event
        if let crate::event_types::ZylosEvent::Custom(c) = &received.payload {
            assert_eq!(c.name, "AgUiEvent");
            let agui: AgUiEvent = serde_json::from_value(c.payload.clone()).unwrap();
            assert!(matches!(agui, AgUiEvent::TextMessageContent { delta, .. } if delta == "hello"));
        } else {
            panic!("expected Custom event");
        }
    }

    #[test]
    fn test_reject_missing_source() {
        let bus = EventBus::new(16, None);
        let bridge = HermesBridge::new(bus);

        let envelope = HermesEnvelope {
            source: "".into(),
            event_type: "text.delta".into(),
            payload: serde_json::json!({}),
            timestamp: None,
        };

        assert!(bridge.forward(&envelope).is_err());
    }

    #[test]
    fn test_reject_missing_event_type() {
        let bus = EventBus::new(16, None);
        let bridge = HermesBridge::new(bus);

        let envelope = HermesEnvelope {
            source: "agent-1".into(),
            event_type: "".into(),
            payload: serde_json::json!({}),
            timestamp: None,
        };

        assert!(bridge.forward(&envelope).is_err());
    }

    #[test]
    fn test_unknown_event_type_falls_back_to_snapshot() {
        let bus = EventBus::new(16, None);
        let mut sub = bus.subscribe_legacy();
        let bridge = HermesBridge::new(bus);

        let envelope = HermesEnvelope {
            source: "agent-1".into(),
            event_type: "custom.event".into(),
            payload: serde_json::json!({"key": "value"}),
            timestamp: None,
        };

        bridge.forward(&envelope).unwrap();

        let received = futures::executor::block_on(async { sub.next().await.unwrap().unwrap() });
        if let crate::event_types::ZylosEvent::Custom(c) = &received.payload {
            let agui: AgUiEvent = serde_json::from_value(c.payload.clone()).unwrap();
            assert!(matches!(agui, AgUiEvent::StateSnapshot { .. }));
        } else {
            panic!("expected Custom event");
        }
    }
}
