use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event as SseEvent, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::agui::{AgUiEvent, ApprovalDecision};
use crate::approval::{ApprovalError, ApprovalStore};
use crate::event_bus::EventBus;

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct FilterParams {
    /// Comma-separated event type names to include (e.g. "heartbeat,text_start").
    /// If absent or empty, all events are forwarded.
    pub filter: Option<String>,
}

/// Request body for POST /approval/resolve.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ResolveApprovalRequest {
    pub approval_id: String,
    pub decision: ApprovalDecision,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default = "default_resolved_by")]
    pub resolved_by: String,
}

fn default_resolved_by() -> String {
    "anonymous".to_string()
}

/// Response body for POST /approval/resolve.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ResolveApprovalResponse {
    pub approval_id: String,
    pub status: String,
    pub message: String,
}

/// Build the Axum router with SSE, WebSocket, and approval endpoints.
pub fn create_router(bus: EventBus, approval_store: Arc<ApprovalStore>) -> Router {
    Router::new()
        .route("/events", get(sse_handler))
        .route("/ws", axum::routing::get(crate::ws::ws_handler))
        .route("/approval/resolve", post(resolve_approval_handler))
        .with_state((bus, approval_store))
}

/// POST /approval/resolve — approve or deny an approval request.
async fn resolve_approval_handler(
    State((_bus, approval_store)): State<(EventBus, Arc<ApprovalStore>)>,
    Json(body): Json<ResolveApprovalRequest>,
) -> Result<Json<ResolveApprovalResponse>, (StatusCode, String)> {
    let result = match body.decision {
        ApprovalDecision::Approved => approval_store.approve(&body.approval_id, &body.resolved_by),
        ApprovalDecision::Denied => {
            approval_store.decline(&body.approval_id, &body.resolved_by, body.reason.clone())
        }
        ApprovalDecision::Expired | ApprovalDecision::Withdrawn => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("decision '{:?}' cannot be set via resolve endpoint", body.decision),
            ));
        }
    };

    match result {
        Ok(request) => {
            let status = match &request.status {
                crate::approval::ApprovalStatus::Approved { .. } => "approved",
                crate::approval::ApprovalStatus::Declined { .. } => "declined",
                _ => "unknown",
            };
            Ok(Json(ResolveApprovalResponse {
                approval_id: body.approval_id,
                status: status.to_string(),
                message: format!("Approval {} successfully", status),
            }))
        }
        Err(ApprovalError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            format!("approval '{}' not found", body.approval_id),
        )),
        Err(ApprovalError::AlreadyDecided) => Err((
            StatusCode::CONFLICT,
            format!("approval '{}' already decided", body.approval_id),
        )),
        Err(ApprovalError::Expired) => Err((
            StatusCode::GONE,
            format!("approval '{}' has expired", body.approval_id),
        )),
    }
}

pub fn parse_event_types(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn matches_filter(event: &AgUiEvent, types: &[String]) -> bool {
    if types.is_empty() {
        return true;
    }
    let name = match event {
        AgUiEvent::Heartbeat { .. } => "heartbeat",
        AgUiEvent::TextMessageStart { .. } => "text_message_start",
        AgUiEvent::TextMessageContent { .. } => "text_message_content",
        AgUiEvent::TextMessageEnd { .. } => "text_message_end",
        AgUiEvent::ToolCallStart { .. } => "tool_call_start",
        AgUiEvent::ToolCallResult { .. } => "tool_call_result",
        AgUiEvent::StateSnapshot { .. } => "state_snapshot",
        AgUiEvent::Error { .. } => "error",
        AgUiEvent::RunStarted { .. } => "run_started",
        AgUiEvent::RunFinished { .. } => "run_finished",
        AgUiEvent::ToolCallEnd { .. } => "tool_call_end",
        AgUiEvent::CustomEvent { .. } => "custom_event",
        AgUiEvent::ApprovalRequest { .. } => "approval_request",
        AgUiEvent::ApprovalResponse { .. } => "approval_response",
    };
    types.iter().any(|t| t.as_str() == name)
}

type SseStream = Pin<Box<dyn Stream<Item = Result<SseEvent, Infallible>> + Send>>;

async fn sse_handler(
    axum::extract::State((bus, _approval_store)): axum::extract::State<(EventBus, Arc<ApprovalStore>)>,
    Query(params): Query<FilterParams>,
) -> Sse<SseStream> {
    let allowed_types = params
        .filter
        .as_deref()
        .map(parse_event_types)
        .unwrap_or_default();

    let stream: SseStream = if allowed_types.is_empty() {
        Box::pin(bus.subscribe_legacy().filter_map(|item| async move {
            match item {
                Ok(envelope) => {
                    let json = serde_json::to_string(&envelope).ok()?;
                    Some(Ok::<_, Infallible>(SseEvent::default().data(json)))
                }
                Err(_) => None,
            }
        }))
    } else {
        let types = allowed_types.clone();
        Box::pin(
            bus.subscribe_filtered_legacy(move |envelope| {
                // Extract AgUiEvent name from Custom payload for legacy filter compat
                if let crate::event_types::ZylosEvent::Custom(ref c) = envelope.payload {
                    if c.name == "AgUiEvent" {
                        if let Ok(agui) = serde_json::from_value::<AgUiEvent>(c.payload.clone()) {
                            return matches_filter(&agui, &types);
                        }
                    }
                }
                false
            })
            .filter_map(|item| async move {
                match item {
                    Ok(envelope) => {
                        let json = serde_json::to_string(&envelope).ok()?;
                        Some(Ok::<_, Infallible>(SseEvent::default().data(json)))
                    }
                    Err(_) => None,
                }
            }),
        )
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_sse_endpoint_returns_200() {
        let bus = EventBus::new(16, None);
        let approval_store = std::sync::Arc::new(crate::approval::ApprovalStore::new(
            crate::approval::ApprovalConfig::default(),
        ));
        let app = create_router(bus, approval_store);

        let request = Request::builder()
            .uri("/events")
            .header("accept", "text/event-stream")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );
    }
}
