//! # zylos-ag-ui
//!
//! AG-UI (Agent-User Interaction) protocol adapter for zylos-gateway.
//! Bridges internal agent events to the AG-UI streaming format used by
//! frontends like OpenDuet and other AG-UI-compatible clients.
//!
//! ## Protocol Overview
//!
//! AG-UI defines a server-sent event stream with typed events:
//! - `TEXT_MESSAGE_START` / `TEXT_MESSAGE_CONTENT` / `TEXT_MESSAGE_END`
//! - `TOOL_CALL_START` / `TOOL_CALL_ARGS` / `TOOL_CALL_END`
//! - `STATE_SNAPSHOT` / `STATE_DELTA`
//! - `RUN_STARTED` / `RUN_FINISHED` / `RUN_ERROR`
//! - `STEP_STARTED` / `STEP_FINISHED`
//! - `CUSTOM` (vendor extensions)
//!
//! This crate provides:
//! 1. Typed event structs matching the AG-UI spec
//! 2. An SSE encoder that serializes events to `data:` lines
//! 3. An axum handler that bridges zylos orchestrator output to SSE

use axum::response::sse::Event as SseEvent;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// AG-UI Event Types
// ---------------------------------------------------------------------------

/// Top-level AG-UI event envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgUiEvent {
    #[serde(rename = "TEXT_MESSAGE_START")]
    TextMessageStart(TextMessageStart),
    #[serde(rename = "TEXT_MESSAGE_CONTENT")]
    TextMessageContent(TextMessageContent),
    #[serde(rename = "TEXT_MESSAGE_END")]
    TextMessageEnd(TextMessageEnd),
    #[serde(rename = "TOOL_CALL_START")]
    ToolCallStart(ToolCallStart),
    #[serde(rename = "TOOL_CALL_ARGS")]
    ToolCallArgs(ToolCallArgs),
    #[serde(rename = "TOOL_CALL_END")]
    ToolCallEnd(ToolCallEnd),
    #[serde(rename = "STATE_SNAPSHOT")]
    StateSnapshot(StateSnapshot),
    #[serde(rename = "STATE_DELTA")]
    StateDelta(StateDelta),
    #[serde(rename = "RUN_STARTED")]
    RunStarted(RunStarted),
    #[serde(rename = "RUN_FINISHED")]
    RunFinished(RunFinished),
    #[serde(rename = "RUN_ERROR")]
    RunError(RunError),
    #[serde(rename = "STEP_STARTED")]
    StepStarted(StepStarted),
    #[serde(rename = "STEP_FINISHED")]
    StepFinished(StepFinished),
    #[serde(rename = "CUSTOM")]
    Custom(CustomEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageStart {
    pub message_id: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageContent {
    pub message_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageEnd {
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStart {
    pub tool_call_id: String,
    pub tool_call_name: String,
    pub parent_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallArgs {
    pub tool_call_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEnd {
    pub tool_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    pub delta: Vec<JsonPatchOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum JsonPatchOp {
    #[serde(rename = "add")]
    Add { path: String, value: serde_json::Value },
    #[serde(rename = "remove")]
    Remove { path: String },
    #[serde(rename = "replace")]
    Replace { path: String, value: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStarted {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFinished {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunError {
    pub run_id: String,
    pub error_code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStarted {
    pub step_id: String,
    pub step_name: String,
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFinished {
    pub step_id: String,
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEvent {
    pub name: String,
    pub value: serde_json::Value,
}

// ---------------------------------------------------------------------------
// SSE Encoder
// ---------------------------------------------------------------------------

/// Encodes an AgUiEvent into an SSE data line
pub fn encode_event(event: &AgUiEvent) -> Result<SseEvent, serde_json::Error> {
    let json = serde_json::to_string(event)?;
    let event_type = match event {
        AgUiEvent::TextMessageStart(_) => "TEXT_MESSAGE_START",
        AgUiEvent::TextMessageContent(_) => "TEXT_MESSAGE_CONTENT",
        AgUiEvent::TextMessageEnd(_) => "TEXT_MESSAGE_END",
        AgUiEvent::ToolCallStart(_) => "TOOL_CALL_START",
        AgUiEvent::ToolCallArgs(_) => "TOOL_CALL_ARGS",
        AgUiEvent::ToolCallEnd(_) => "TOOL_CALL_END",
        AgUiEvent::StateSnapshot(_) => "STATE_SNAPSHOT",
        AgUiEvent::StateDelta(_) => "STATE_DELTA",
        AgUiEvent::RunStarted(_) => "RUN_STARTED",
        AgUiEvent::RunFinished(_) => "RUN_FINISHED",
        AgUiEvent::RunError(_) => "RUN_ERROR",
        AgUiEvent::StepStarted(_) => "STEP_STARTED",
        AgUiEvent::StepFinished(_) => "STEP_FINISHED",
        AgUiEvent::Custom(_) => "CUSTOM",
    };
    Ok(SseEvent::default().event(event_type).data(json))
}

// ---------------------------------------------------------------------------
// Bridge: zylos orchestrator → AG-UI SSE stream
// ---------------------------------------------------------------------------

/// Internal event type from the zylos orchestrator that gets translated
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Agent started producing text
    TextStart { message_id: String, role: String },
    /// Incremental text chunk
    TextDelta { message_id: String, delta: String },
    /// Agent finished text output
    TextEnd { message_id: String },
    /// Tool invocation begins
    ToolStart {
        tool_call_id: String,
        tool_name: String,
        parent_message_id: String,
    },
    /// Streaming tool arguments (JSON fragment)
    ToolArgs {
        tool_call_id: String,
        delta: String,
    },
    /// Tool invocation complete
    ToolEnd {
        tool_call_id: String,
        result: Option<String>,
    },
    /// Full state snapshot
    Snapshot { state: serde_json::Value },
    /// JSON Patch delta
    Delta { ops: Vec<JsonPatchOp> },
    /// Run lifecycle
    Started {
        run_id: String,
        thread_id: Option<String>,
        input: Option<serde_json::Value>,
    },
    Finished {
        run_id: String,
        result: Option<serde_json::Value>,
    },
    Error {
        run_id: String,
        code: String,
        message: String,
    },
    /// Step lifecycle
    StepStart {
        step_id: String,
        step_name: String,
        run_id: String,
    },
    StepEnd {
        step_id: String,
        run_id: String,
        result: Option<serde_json::Value>,
    },
    /// Vendor-specific custom event
    Custom {
        name: String,
        value: serde_json::Value,
    },
}

impl From<OrchestratorEvent> for AgUiEvent {
    fn from(e: OrchestratorEvent) -> Self {
        match e {
            OrchestratorEvent::TextStart { message_id, role } => {
                AgUiEvent::TextMessageStart(TextMessageStart {
                    message_id,
                    role,
                    parent_message_id: None,
                })
            }
            OrchestratorEvent::TextDelta { message_id, delta } => {
                AgUiEvent::TextMessageContent(TextMessageContent { message_id, delta })
            }
            OrchestratorEvent::TextEnd { message_id } => {
                AgUiEvent::TextMessageEnd(TextMessageEnd { message_id })
            }
            OrchestratorEvent::ToolStart {
                tool_call_id,
                tool_name,
                parent_message_id,
            } => AgUiEvent::ToolCallStart(ToolCallStart {
                tool_call_id,
                tool_call_name: tool_name,
                parent_message_id,
            }),
            OrchestratorEvent::ToolArgs {
                tool_call_id,
                delta,
            } => AgUiEvent::ToolCallArgs(ToolCallArgs {
                tool_call_id,
                delta,
            }),
            OrchestratorEvent::ToolEnd {
                tool_call_id,
                result,
            } => AgUiEvent::ToolCallEnd(ToolCallEnd {
                tool_call_id,
                result,
            }),
            OrchestratorEvent::Snapshot { state } => {
                AgUiEvent::StateSnapshot(StateSnapshot { snapshot: state })
            }
            OrchestratorEvent::Delta { ops } => AgUiEvent::StateDelta(StateDelta { delta: ops }),
            OrchestratorEvent::Started {
                run_id,
                thread_id,
                input,
            } => AgUiEvent::RunStarted(RunStarted {
                run_id,
                thread_id,
                parent_run_id: None,
                input,
            }),
            OrchestratorEvent::Finished { run_id, result } => {
                AgUiEvent::RunFinished(RunFinished { run_id, result })
            }
            OrchestratorEvent::Error {
                run_id,
                code,
                message,
            } => AgUiEvent::RunError(RunError {
                run_id,
                error_code: code,
                message,
            }),
            OrchestratorEvent::StepStart {
                step_id,
                step_name,
                run_id,
            } => AgUiEvent::StepStarted(StepStarted {
                step_id,
                step_name,
                run_id,
            }),
            OrchestratorEvent::StepEnd {
                step_id,
                run_id,
                result,
            } => AgUiEvent::StepFinished(StepFinished {
                step_id,
                run_id,
                result,
            }),
            OrchestratorEvent::Custom { name, value } => {
                AgUiEvent::Custom(CustomEvent { name, value })
            }
        }
    }
}

/// Creates an SSE stream from a channel of orchestrator events.
///
/// Usage in an axum handler:
/// ```ignore
/// let (tx, rx) = mpsc::channel::<OrchestratorEvent>(64);
/// // spawn orchestrator that sends events to tx
/// let stream = ag_ui_stream(rx);
/// Sse::new(stream).keep_alive(Default::default())
/// ```
pub fn ag_ui_stream(
    mut rx: mpsc::Receiver<OrchestratorEvent>,
) -> impl Stream<Item = Result<SseEvent, Infallible>> + Send {
    async_stream::stream! {
        while let Some(evt) = rx.recv().await {
            let ag_evt: AgUiEvent = evt.into();
            match encode_event(&ag_evt) {
                Ok(sse) => yield Ok(sse),
                Err(e) => {
                    tracing::error!("Failed to encode AG-UI event: {}", e);
                    // Skip malformed events rather than killing the stream
                    continue;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience constructors
// ---------------------------------------------------------------------------

impl AgUiEvent {
    pub fn text_start(message_id: &str, role: &str) -> Self {
        AgUiEvent::TextMessageStart(TextMessageStart {
            message_id: message_id.to_string(),
            role: role.to_string(),
            parent_message_id: None,
        })
    }

    pub fn text_delta(message_id: &str, delta: &str) -> Self {
        AgUiEvent::TextMessageContent(TextMessageContent {
            message_id: message_id.to_string(),
            delta: delta.to_string(),
        })
    }

    pub fn text_end(message_id: &str) -> Self {
        AgUiEvent::TextMessageEnd(TextMessageEnd {
            message_id: message_id.to_string(),
        })
    }

    pub fn run_started(run_id: &str, thread_id: Option<&str>) -> Self {
        AgUiEvent::RunStarted(RunStarted {
            run_id: run_id.to_string(),
            thread_id: thread_id.map(|s| s.to_string()),
            parent_run_id: None,
            input: None,
        })
    }

    pub fn run_finished(run_id: &str) -> Self {
        AgUiEvent::RunFinished(RunFinished {
            run_id: run_id.to_string(),
            result: None,
        })
    }

    pub fn run_error(run_id: &str, code: &str, message: &str) -> Self {
        AgUiEvent::RunError(RunError {
            run_id: run_id.to_string(),
            error_code: code.to_string(),
            message: message.to_string(),
        })
    }

    pub fn tool_start(tool_call_id: &str, tool_name: &str, parent_msg_id: &str) -> Self {
        AgUiEvent::ToolCallStart(ToolCallStart {
            tool_call_id: tool_call_id.to_string(),
            tool_call_name: tool_name.to_string(),
            parent_message_id: parent_msg_id.to_string(),
        })
    }

    pub fn tool_args(tool_call_id: &str, delta: &str) -> Self {
        AgUiEvent::ToolCallArgs(ToolCallArgs {
            tool_call_id: tool_call_id.to_string(),
            delta: delta.to_string(),
        })
    }

    pub fn tool_end(tool_call_id: &str, result: Option<&str>) -> Self {
        AgUiEvent::ToolCallEnd(ToolCallEnd {
            tool_call_id: tool_call_id.to_string(),
            result: result.map(|s| s.to_string()),
        })
    }

    pub fn state_snapshot(state: serde_json::Value) -> Self {
        AgUiEvent::StateSnapshot(StateSnapshot { snapshot: state })
    }

    pub fn step_started(step_id: &str, step_name: &str, run_id: &str) -> Self {
        AgUiEvent::StepStarted(StepStarted {
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            run_id: run_id.to_string(),
        })
    }

    pub fn step_finished(step_id: &str, run_id: &str) -> Self {
        AgUiEvent::StepFinished(StepFinished {
            step_id: step_id.to_string(),
            run_id: run_id.to_string(),
            result: None,
        })
    }

    pub fn custom(name: &str, value: serde_json::Value) -> Self {
        AgUiEvent::Custom(CustomEvent {
            name: name.to_string(),
            value,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_text_message() {
        let evt = AgUiEvent::text_start("msg-1", "assistant");
        let sse = encode_event(&evt).unwrap();
        // SseEvent doesn't expose fields directly, but we can verify it serializes
        let json = serde_json::to_string(&evt).unwrap();
        assert!(json.contains("TEXT_MESSAGE_START"));
        assert!(json.contains("msg-1"));
    }

    #[test]
    fn test_roundtrip_tool_call() {
        let start = AgUiEvent::tool_start("tc-1", "web_search", "msg-1");
        let args = AgUiEvent::tool_args("tc-1", r#"{"query":"rust"}"#);
        let end = AgUiEvent::tool_end("tc-1", Some(r#"{"results":[]}"#));

        for evt in [&start, &args, &end] {
            let json = serde_json::to_string(evt).unwrap();
            let parsed: AgUiEvent = serde_json::from_str(&json).unwrap();
            // Verify roundtrip produces same type tag
            let re_json = serde_json::to_string(&parsed).unwrap();
            assert_eq!(json, re_json);
        }
    }

    #[test]
    fn test_orchestrator_event_conversion() {
        let orch = OrchestratorEvent::Started {
            run_id: "run-42".into(),
            thread_id: Some("thread-7".into()),
            input: Some(serde_json::json!({"prompt": "hello"})),
        };
        let ag: AgUiEvent = orch.into();
        match ag {
            AgUiEvent::RunStarted(rs) => {
                assert_eq!(rs.run_id, "run-42");
                assert_eq!(rs.thread_id.as_deref(), Some("thread-7"));
            }
            _ => panic!("expected RunStarted"),
        }
    }

    #[test]
    fn test_state_delta_ops() {
        let delta = AgUiEvent::StateDelta(StateDelta {
            delta: vec![
                JsonPatchOp::Add {
                    path: "/agents/0/status".into(),
                    value: serde_json::json!("running"),
                },
                JsonPatchOp::Remove {
                    path: "/temp".into(),
                },
            ],
        });
        let json = serde_json::to_string(&delta).unwrap();
        assert!(json.contains("STATE_DELTA"));
        assert!(json.contains("/agents/0/status"));
    }
}