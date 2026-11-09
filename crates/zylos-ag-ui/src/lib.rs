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

/// Top-level AG-UI event envelope.
///
/// Each variant corresponds to a typed SSE event in the AG-UI protocol.
/// Events are serialized with a `"type"` tag matching the AG-UI spec
/// (e.g. `"TEXT_MESSAGE_START"`, `"RUN_ERROR"`).
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

/// Signals the beginning of a streamed text message from an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageStart {
    /// Unique identifier for this message.
    pub message_id: String,
    /// Role of the message author (e.g. `"assistant"`, `"user"`).
    pub role: String,
    /// Optional parent message ID for threading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_message_id: Option<String>,
}

/// Incremental text content chunk within a streamed message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageContent {
    /// Message ID this delta belongs to.
    pub message_id: String,
    /// Text fragment to append.
    pub delta: String,
}

/// Signals the end of a streamed text message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextMessageEnd {
    /// Message ID that has finished streaming.
    pub message_id: String,
}

/// Signals the beginning of a tool invocation by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallStart {
    /// Unique identifier for this tool call.
    pub tool_call_id: String,
    /// Name of the tool being invoked.
    pub tool_call_name: String,
    /// Message ID that triggered this tool call.
    pub parent_message_id: String,
}

/// Streaming JSON arguments for an in-progress tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallArgs {
    /// Tool call ID these arguments belong to.
    pub tool_call_id: String,
    /// JSON fragment to append to the accumulated arguments.
    pub delta: String,
}

/// Signals the completion of a tool invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallEnd {
    /// Tool call ID that has completed.
    pub tool_call_id: String,
    /// Optional result payload returned by the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

/// Full state snapshot sent to synchronize client-side state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Complete state value.
    pub snapshot: serde_json::Value,
}

/// Incremental state update expressed as JSON Patch operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDelta {
    /// Ordered list of JSON Patch operations to apply.
    pub delta: Vec<JsonPatchOp>,
}

/// A single JSON Patch operation (RFC 6902 subset).
///
/// Supports `add`, `remove`, and `replace` operations used by
/// [`StateDelta`] to incrementally update client-side state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum JsonPatchOp {
    /// Add a value at the given path.
    #[serde(rename = "add")]
    Add { path: String, value: serde_json::Value },
    /// Remove the value at the given path.
    #[serde(rename = "remove")]
    Remove { path: String },
    /// Replace the value at the given path.
    #[serde(rename = "replace")]
    Replace { path: String, value: serde_json::Value },
}

/// Signals the start of an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStarted {
    /// Unique identifier for this run.
    pub run_id: String,
    /// Optional thread/conversation ID this run belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    /// Optional parent run ID for nested/sub-agent runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    /// Optional input payload that initiated the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
}

/// Signals successful completion of an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFinished {
    /// Run ID that has completed.
    pub run_id: String,
    /// Optional result payload produced by the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Signals that an agent run terminated with an error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunError {
    /// Run ID that failed.
    pub run_id: String,
    /// Machine-readable error code.
    pub error_code: String,
    /// Human-readable error description.
    pub message: String,
}

/// Signals the start of a discrete step within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepStarted {
    /// Unique identifier for this step.
    pub step_id: String,
    /// Human-readable step name.
    pub step_name: String,
    /// Run ID this step belongs to.
    pub run_id: String,
}

/// Signals the completion of a discrete step within a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepFinished {
    /// Step ID that has completed.
    pub step_id: String,
    /// Run ID this step belongs to.
    pub run_id: String,
    /// Optional result payload produced by the step.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Vendor-specific extension event outside the core AG-UI spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEvent {
    /// Event type name (vendor-defined).
    pub name: String,
    /// Arbitrary event payload.
    pub value: serde_json::Value,
}

// ---------------------------------------------------------------------------
// SSE Encoder
// ---------------------------------------------------------------------------

/// Encodes an [`AgUiEvent`] into an axum SSE [`SseEvent`](axum::response::sse::Event).
///
/// The resulting SSE event uses the AG-UI type string (e.g. `"TEXT_MESSAGE_START"`)
/// as the SSE event name and the JSON-serialized event as the data payload.
///
/// # Errors
///
/// Returns [`serde_json::Error`] if the event fails to serialize.
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

/// Internal event type emitted by the zylos orchestrator.
///
/// Converted into [`AgUiEvent`] via [`From`] for SSE streaming.
/// This type is orchestrator-internal; external consumers should use
/// [`AgUiEvent`] directly.
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

/// Creates an SSE stream from a channel of [`OrchestratorEvent`]s.
///
/// Converts each incoming orchestrator event to its AG-UI representation
/// and encodes it as an SSE frame. Malformed events are logged and skipped
/// rather than terminating the stream.
///
/// # Example
///
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
    /// Creates a [`TextMessageStart`](AgUiEvent::TextMessageStart) event.
    pub fn text_start(message_id: &str, role: &str) -> Self {
        AgUiEvent::TextMessageStart(TextMessageStart {
            message_id: message_id.to_string(),
            role: role.to_string(),
            parent_message_id: None,
        })
    }

    /// Creates a [`TextMessageContent`](AgUiEvent::TextMessageContent) event.
    pub fn text_delta(message_id: &str, delta: &str) -> Self {
        AgUiEvent::TextMessageContent(TextMessageContent {
            message_id: message_id.to_string(),
            delta: delta.to_string(),
        })
    }

    /// Creates a [`TextMessageEnd`](AgUiEvent::TextMessageEnd) event.
    pub fn text_end(message_id: &str) -> Self {
        AgUiEvent::TextMessageEnd(TextMessageEnd {
            message_id: message_id.to_string(),
        })
    }

    /// Creates a [`RunStarted`](AgUiEvent::RunStarted) event.
    pub fn run_started(run_id: &str, thread_id: Option<&str>) -> Self {
        AgUiEvent::RunStarted(RunStarted {
            run_id: run_id.to_string(),
            thread_id: thread_id.map(|s| s.to_string()),
            parent_run_id: None,
            input: None,
        })
    }

    /// Creates a [`RunFinished`](AgUiEvent::RunFinished) event.
    pub fn run_finished(run_id: &str) -> Self {
        AgUiEvent::RunFinished(RunFinished {
            run_id: run_id.to_string(),
            result: None,
        })
    }

    /// Creates a [`RunError`](AgUiEvent::RunError) event.
    pub fn run_error(run_id: &str, code: &str, message: &str) -> Self {
        AgUiEvent::RunError(RunError {
            run_id: run_id.to_string(),
            error_code: code.to_string(),
            message: message.to_string(),
        })
    }

    /// Creates a [`ToolCallStart`](AgUiEvent::ToolCallStart) event.
    pub fn tool_start(tool_call_id: &str, tool_name: &str, parent_msg_id: &str) -> Self {
        AgUiEvent::ToolCallStart(ToolCallStart {
            tool_call_id: tool_call_id.to_string(),
            tool_call_name: tool_name.to_string(),
            parent_message_id: parent_msg_id.to_string(),
        })
    }

    /// Creates a [`ToolCallArgs`](AgUiEvent::ToolCallArgs) event.
    pub fn tool_args(tool_call_id: &str, delta: &str) -> Self {
        AgUiEvent::ToolCallArgs(ToolCallArgs {
            tool_call_id: tool_call_id.to_string(),
            delta: delta.to_string(),
        })
    }

    /// Creates a [`ToolCallEnd`](AgUiEvent::ToolCallEnd) event.
    pub fn tool_end(tool_call_id: &str, result: Option<&str>) -> Self {
        AgUiEvent::ToolCallEnd(ToolCallEnd {
            tool_call_id: tool_call_id.to_string(),
            result: result.map(|s| s.to_string()),
        })
    }

    /// Creates a [`StateSnapshot`](AgUiEvent::StateSnapshot) event.
    pub fn state_snapshot(state: serde_json::Value) -> Self {
        AgUiEvent::StateSnapshot(StateSnapshot { snapshot: state })
    }

    /// Creates a [`StepStarted`](AgUiEvent::StepStarted) event.
    pub fn step_started(step_id: &str, step_name: &str, run_id: &str) -> Self {
        AgUiEvent::StepStarted(StepStarted {
            step_id: step_id.to_string(),
            step_name: step_name.to_string(),
            run_id: run_id.to_string(),
        })
    }

    /// Creates a [`StepFinished`](AgUiEvent::StepFinished) event.
    pub fn step_finished(step_id: &str, run_id: &str) -> Self {
        AgUiEvent::StepFinished(StepFinished {
            step_id: step_id.to_string(),
            run_id: run_id.to_string(),
            result: None,
        })
    }

    /// Creates a [`Custom`](AgUiEvent::Custom) vendor extension event.
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
        let _sse = encode_event(&evt).unwrap();
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

    #[test]
    fn test_run_lifecycle_constructors() {
        let started = AgUiEvent::run_started("run-1", Some("thread-a"));
        match &started {
            AgUiEvent::RunStarted(rs) => {
                assert_eq!(rs.run_id, "run-1");
                assert_eq!(rs.thread_id.as_deref(), Some("thread-a"));
                assert!(rs.parent_run_id.is_none());
                assert!(rs.input.is_none());
            }
            _ => panic!("expected RunStarted"),
        }

        let finished = AgUiEvent::run_finished("run-1");
        match &finished {
            AgUiEvent::RunFinished(rf) => {
                assert_eq!(rf.run_id, "run-1");
                assert!(rf.result.is_none());
            }
            _ => panic!("expected RunFinished"),
        }

        let error = AgUiEvent::run_error("run-1", "TIMEOUT", "agent timed out");
        match &error {
            AgUiEvent::RunError(re) => {
                assert_eq!(re.run_id, "run-1");
                assert_eq!(re.error_code, "TIMEOUT");
                assert_eq!(re.message, "agent timed out");
            }
            _ => panic!("expected RunError"),
        }
    }

    #[test]
    fn test_step_lifecycle_constructors() {
        let started = AgUiEvent::step_started("step-1", "planning", "run-1");
        match &started {
            AgUiEvent::StepStarted(ss) => {
                assert_eq!(ss.step_id, "step-1");
                assert_eq!(ss.step_name, "planning");
                assert_eq!(ss.run_id, "run-1");
            }
            _ => panic!("expected StepStarted"),
        }

        let finished = AgUiEvent::step_finished("step-1", "run-1");
        match &finished {
            AgUiEvent::StepFinished(sf) => {
                assert_eq!(sf.step_id, "step-1");
                assert_eq!(sf.run_id, "run-1");
                assert!(sf.result.is_none());
            }
            _ => panic!("expected StepFinished"),
        }
    }

    #[test]
    fn test_custom_event_constructor() {
        let evt = AgUiEvent::custom("metrics", serde_json::json!({"latency_ms": 42}));
        match &evt {
            AgUiEvent::Custom(c) => {
                assert_eq!(c.name, "metrics");
                assert_eq!(c.value["latency_ms"], 42);
            }
            _ => panic!("expected Custom"),
        }
        // Roundtrip
        let json = serde_json::to_string(&evt).unwrap();
        let parsed: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&parsed).unwrap(), json);
    }

    #[test]
    fn test_state_snapshot_constructor() {
        let state = serde_json::json!({"agents": [{"id": "a1", "status": "idle"}]});
        let evt = AgUiEvent::state_snapshot(state.clone());
        match &evt {
            AgUiEvent::StateSnapshot(ss) => {
                assert_eq!(ss.snapshot, state);
            }
            _ => panic!("expected StateSnapshot"),
        }
    }

    #[test]
    fn test_text_message_full_lifecycle() {
        let start = AgUiEvent::text_start("msg-1", "assistant");
        let delta1 = AgUiEvent::text_delta("msg-1", "Hello");
        let delta2 = AgUiEvent::text_delta("msg-1", ", world!");
        let end = AgUiEvent::text_end("msg-1");

        for evt in [&start, &delta1, &delta2, &end] {
            let sse = encode_event(evt).unwrap();
            // Verify SSE encoding doesn't panic and produces valid event
            let _ = sse;
        }

        // Verify JSON type tags
        let json = serde_json::to_string(&delta1).unwrap();
        assert!(json.contains("TEXT_MESSAGE_CONTENT"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_encode_all_event_types() {
        let events = vec![
            AgUiEvent::text_start("m1", "user"),
            AgUiEvent::text_delta("m1", "hi"),
            AgUiEvent::text_end("m1"),
            AgUiEvent::tool_start("tc1", "search", "m1"),
            AgUiEvent::tool_args("tc1", "{}"),
            AgUiEvent::tool_end("tc1", None),
            AgUiEvent::state_snapshot(serde_json::json!({})),
            AgUiEvent::StateDelta(StateDelta { delta: vec![] }),
            AgUiEvent::run_started("r1", None),
            AgUiEvent::run_finished("r1"),
            AgUiEvent::run_error("r1", "ERR", "fail"),
            AgUiEvent::step_started("s1", "plan", "r1"),
            AgUiEvent::step_finished("s1", "r1"),
            AgUiEvent::custom("ping", serde_json::json!(null)),
        ];

        let expected_types = [
            "TEXT_MESSAGE_START",
            "TEXT_MESSAGE_CONTENT",
            "TEXT_MESSAGE_END",
            "TOOL_CALL_START",
            "TOOL_CALL_ARGS",
            "TOOL_CALL_END",
            "STATE_SNAPSHOT",
            "STATE_DELTA",
            "RUN_STARTED",
            "RUN_FINISHED",
            "RUN_ERROR",
            "STEP_STARTED",
            "STEP_FINISHED",
            "CUSTOM",
        ];

        for (evt, expected_type) in events.iter().zip(expected_types.iter()) {
            let json = serde_json::to_string(evt).unwrap();
            assert!(
                json.contains(expected_type),
                "Expected {} in JSON: {}",
                expected_type,
                json
            );
            // Verify encode_event succeeds for every variant
            let _sse = encode_event(evt).unwrap();
        }
    }

    #[test]
    fn test_orchestrator_event_all_variants_convert() {
        let orch_events: Vec<OrchestratorEvent> = vec![
            OrchestratorEvent::TextStart { message_id: "m1".into(), role: "assistant".into() },
            OrchestratorEvent::TextDelta { message_id: "m1".into(), delta: "hi".into() },
            OrchestratorEvent::TextEnd { message_id: "m1".into() },
            OrchestratorEvent::ToolStart { tool_call_id: "tc1".into(), tool_name: "x".into(), parent_message_id: "m1".into() },
            OrchestratorEvent::ToolArgs { tool_call_id: "tc1".into(), delta: "{}".into() },
            OrchestratorEvent::ToolEnd { tool_call_id: "tc1".into(), result: None },
            OrchestratorEvent::Snapshot { state: serde_json::json!({}) },
            OrchestratorEvent::Delta { ops: vec![] },
            OrchestratorEvent::Started { run_id: "r1".into(), thread_id: None, input: None },
            OrchestratorEvent::Finished { run_id: "r1".into(), result: None },
            OrchestratorEvent::Error { run_id: "r1".into(), code: "E".into(), message: "err".into() },
            OrchestratorEvent::StepStart { step_id: "s1".into(), step_name: "go".into(), run_id: "r1".into() },
            OrchestratorEvent::StepEnd { step_id: "s1".into(), run_id: "r1".into(), result: None },
            OrchestratorEvent::Custom { name: "x".into(), value: serde_json::json!(1) },
        ];

        for orch in orch_events {
            let ag: AgUiEvent = orch.into();
            // Every conversion must produce a serializable event
            let json = serde_json::to_string(&ag).unwrap();
            assert!(!json.is_empty());
        }
    }

    #[tokio::test]
    async fn test_ag_ui_stream_produces_events() {
        use futures::StreamExt;

        let (tx, rx) = mpsc::channel::<OrchestratorEvent>(8);
        let stream = ag_ui_stream(rx);

        tx.send(OrchestratorEvent::Started {
            run_id: "r1".into(),
            thread_id: None,
            input: None,
        }).await.unwrap();
        tx.send(OrchestratorEvent::TextStart {
            message_id: "m1".into(),
            role: "assistant".into(),
        }).await.unwrap();
        tx.send(OrchestratorEvent::TextEnd {
            message_id: "m1".into(),
        }).await.unwrap();
        drop(tx); // close channel to end stream

        let items: Vec<_> = stream.collect().await;
        assert_eq!(items.len(), 3);
        for item in &items {
            assert!(item.is_ok());
        }
    }

    #[test]
    fn test_json_patch_op_serialization() {
        let add = JsonPatchOp::Add {
            path: "/foo".into(),
            value: serde_json::json!(42),
        };
        let json = serde_json::to_string(&add).unwrap();
        assert!(json.contains("\"op\":\"add\""));
        assert!(json.contains("\"path\":\"/foo\""));
        assert!(json.contains("42"));

        let remove = JsonPatchOp::Remove { path: "/bar".into() };
        let json = serde_json::to_string(&remove).unwrap();
        assert!(json.contains("\"op\":\"remove\""));

        let replace = JsonPatchOp::Replace {
            path: "/baz".into(),
            value: serde_json::json!("new"),
        };
        let json = serde_json::to_string(&replace).unwrap();
        assert!(json.contains("\"op\":\"replace\""));
        assert!(json.contains("\"new\""));
    }

    #[test]
    fn test_tool_end_with_and_without_result() {
        let with_result = AgUiEvent::tool_end("tc-1", Some("output"));
        let json = serde_json::to_string(&with_result).unwrap();
        assert!(json.contains("\"result\":\"output\""));

        let without_result = AgUiEvent::tool_end("tc-2", None);
        let json = serde_json::to_string(&without_result).unwrap();
        assert!(!json.contains("\"result\""));
    }
}