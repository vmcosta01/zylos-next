use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

/// AG-UI event types per spec v0.1
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, utoipa::ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum AgUiEvent {
    /// Start of a text message stream
    TextMessageStart {
        message_id: String,
        role: String,
    },
    /// Delta content for an ongoing text message
    TextMessageContent {
        message_id: String,
        delta: String,
    },
    /// End of a text message stream
    TextMessageEnd {
        message_id: String,
    },
    /// Start of a tool call
    ToolCallStart {
        tool_call_id: String,
        tool_name: String,
        arguments: serde_json::Value,
    },
    /// Result of a tool call
    ToolCallResult {
        tool_call_id: String,
        result: serde_json::Value,
        is_error: bool,
    },
    /// Full state snapshot for lagged receiver recovery
    StateSnapshot {
        state: serde_json::Value,
        sequence: u64,
    },
    /// Error event
    Error {
        code: String,
        message: String,
    },
    /// Heartbeat to keep connection alive
    Heartbeat {
        timestamp: DateTime<Utc>,
    },
    /// Signals the start of an agent run
    RunStarted {
        run_id: String,
        thread_id: String,
        parent_run_id: Option<String>,
        input: Option<serde_json::Value>,
    },
    /// Signals the end of an agent run with final status
    RunFinished {
        run_id: String,
        thread_id: String,
        status: RunStatus,
        output: Option<serde_json::Value>,
    },
    /// Signals the end of a tool call stream (complement to ToolCallStart/Result)
    ToolCallEnd {
        tool_call_id: String,
    },
    /// Application-defined custom event for extensibility
    CustomEvent {
        name: String,
        value: serde_json::Value,
    },
    /// HITL approval request from agent to human
    ApprovalRequest {
        approval_id: String,
        summary: String,
        tool_call_id: String,
        args_hash: String,
        ttl_secs: u64,
        created_at: DateTime<Utc>,
    },
    /// Human response to an approval request
    ApprovalResponse {
        approval_id: String,
        decision: ApprovalDecision,
        reason: Option<String>,
        responded_at: DateTime<Utc>,
    },
}

/// Decision for an approval request.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approved,
    Denied,
    Expired,
    Withdrawn,
}

/// Status of a completed or failed run.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

impl AgUiEvent {
    pub fn text_start(message_id: &str, role: &str) -> Self {
        Self::TextMessageStart {
            message_id: message_id.to_string(),
            role: role.to_string(),
        }
    }

    pub fn text_delta(message_id: &str, delta: &str) -> Self {
        Self::TextMessageContent {
            message_id: message_id.to_string(),
            delta: delta.to_string(),
        }
    }

    pub fn text_end(message_id: &str) -> Self {
        Self::TextMessageEnd {
            message_id: message_id.to_string(),
        }
    }

    pub fn heartbeat() -> Self {
        Self::Heartbeat {
            timestamp: Utc::now(),
        }
    }

    pub fn error(code: &str, message: &str) -> Self {
        Self::Error {
            code: code.to_string(),
            message: message.to_string(),
        }
    }

    pub fn state_snapshot(state: serde_json::Value) -> Self {
        Self::StateSnapshot {
            state,
            sequence: 0,
        }
    }

    pub fn tool_call_start(tool_call_id: &str, tool_name: &str, arguments: serde_json::Value) -> Self {
        Self::ToolCallStart {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            arguments,
        }
    }

    pub fn tool_call_result(tool_call_id: &str, result: serde_json::Value, is_error: bool) -> Self {
        Self::ToolCallResult {
            tool_call_id: tool_call_id.to_string(),
            result,
            is_error,
        }
    }

    pub fn run_started(run_id: &str, thread_id: &str, parent_run_id: Option<String>, input: Option<serde_json::Value>) -> Self {
        Self::RunStarted {
            run_id: run_id.to_string(),
            thread_id: thread_id.to_string(),
            parent_run_id,
            input,
        }
    }

    pub fn run_finished(run_id: &str, thread_id: &str, status: RunStatus, output: Option<serde_json::Value>) -> Self {
        Self::RunFinished {
            run_id: run_id.to_string(),
            thread_id: thread_id.to_string(),
            status,
            output,
        }
    }

    pub fn tool_call_end(tool_call_id: &str) -> Self {
        Self::ToolCallEnd {
            tool_call_id: tool_call_id.to_string(),
        }
    }

    pub fn custom_event(name: &str, value: serde_json::Value) -> Self {
        Self::CustomEvent {
            name: name.to_string(),
            value,
        }
    }

    pub fn approval_request(
        approval_id: &str,
        summary: &str,
        tool_call_id: &str,
        args_hash: &str,
        ttl_secs: u64,
    ) -> Self {
        Self::ApprovalRequest {
            approval_id: approval_id.to_string(),
            summary: summary.to_string(),
            tool_call_id: tool_call_id.to_string(),
            args_hash: args_hash.to_string(),
            ttl_secs,
            created_at: Utc::now(),
        }
    }

    pub fn approval_response(
        approval_id: &str,
        decision: ApprovalDecision,
        reason: Option<String>,
    ) -> Self {
        Self::ApprovalResponse {
            approval_id: approval_id.to_string(),
            decision,
            reason,
            responded_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agui_event_serde_roundtrip() {
        let event = AgUiEvent::text_start("msg-001", "assistant");
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_agui_event_delta_roundtrip() {
        let event = AgUiEvent::text_delta("msg-001", "Hello, world!");
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_agui_event_tool_call_roundtrip() {
        let event = AgUiEvent::ToolCallStart {
            tool_call_id: "tc-001".to_string(),
            tool_name: "search".to_string(),
            arguments: serde_json::json!({"query": "rust async"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_agui_event_state_snapshot_roundtrip() {
        let event = AgUiEvent::StateSnapshot {
            state: serde_json::json!({"agents": 5, "active": 3}),
            sequence: 42,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_agui_event_heartbeat() {
        let event = AgUiEvent::heartbeat();
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("Heartbeat"));
    }

    #[test]
    fn test_run_started_roundtrip() {
        let event = AgUiEvent::run_started(
            "run-001",
            "thread-001",
            Some("parent-run-001".into()),
            Some(serde_json::json!({"prompt": "hello"})),
        );
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_run_finished_completed_roundtrip() {
        let event = AgUiEvent::run_finished(
            "run-001",
            "thread-001",
            RunStatus::Completed,
            Some(serde_json::json!({"result": "ok"})),
        );
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_run_finished_failed_roundtrip() {
        let event = AgUiEvent::run_finished("run-002", "thread-002", RunStatus::Failed, None);
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"failed\""));
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_tool_call_end_roundtrip() {
        let event = AgUiEvent::tool_call_end("tc-001");
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_custom_event_roundtrip() {
        let event = AgUiEvent::custom_event("metrics.update", serde_json::json!({"cpu": 45.2, "mem_mb": 128}));
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, deserialized);
    }

    #[test]
    fn test_run_status_variants() {
        for status in [RunStatus::Completed, RunStatus::Failed, RunStatus::Cancelled, RunStatus::TimedOut] {
            let event = AgUiEvent::run_finished("r", "t", status.clone(), None);
            let json = serde_json::to_string(&event).unwrap();
            let deserialized: AgUiEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, deserialized);
        }
    }
}
