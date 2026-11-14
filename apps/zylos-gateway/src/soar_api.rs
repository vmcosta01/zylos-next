//! SOAR API Routes — REST endpoints for triggering SOAR event processing.
//!
//! These endpoints allow the cockpit frontend and external systems to submit
//! security events to the SOAR orchestrator. Events are processed asynchronously
//! and results stream back to connected clients via AG-UI SSE.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use zylos_soar::ZylosSecurityEvent;

use crate::AppState;

/// Request body for submitting a security event to the SOAR engine.
#[derive(Debug, Deserialize)]
pub struct SoarEventRequest {
    pub source: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub affected_assets: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Response returned after successful event submission.
#[derive(Debug, Serialize)]
pub struct SoarEventResponse {
    pub event_id: String,
    pub status: &'static str,
    pub message: String,
}

/// POST /api/v1/soar/events — Submit a security event for SOAR processing.
async fn post_soar_event(
    State(state): State<AppState>,
    Json(request): Json<SoarEventRequest>,
) -> impl IntoResponse {
    // Deserialize severity and category from strings via serde (snake_case)
    let severity: zylos_soar::Severity =
        serde_json::from_value(serde_json::Value::String(request.severity.to_lowercase()))
            .unwrap_or(zylos_soar::Severity::Medium);

    let category: zylos_soar::EventCategory =
        serde_json::from_value(serde_json::Value::String(request.category.to_lowercase()))
            .unwrap_or(zylos_soar::EventCategory::Anomaly);

    let event = ZylosSecurityEvent::new(
        &request.source,
        severity,
        category,
        &request.title,
        &request.description,
        request.affected_assets,
        request.metadata,
    );

    let event_id = event.id.to_string();

    info!(event_id = %event_id, source = %request.source, "SOAR event received via API");

    match state.soar_orchestrator.process_event(event).await {
        Ok(result) => {
            let success_count = result
                .execution_results
                .iter()
                .filter(|r| r.success)
                .count();
            let total = result.execution_results.len();

            (
                StatusCode::OK,
                Json(SoarEventResponse {
                    event_id,
                    status: "processed",
                    message: format!("{}/{} playbooks succeeded", success_count, total),
                }),
            )
        }
        Err(e) => {
            error!(event_id = %event_id, error = %e, "SOAR event processing failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SoarEventResponse {
                    event_id,
                    status: "error",
                    message: e.to_string(),
                }),
            )
        }
    }
}

/// GET /api/v1/soar/playbooks — List available SOAR playbooks.
async fn get_soar_playbooks(State(state): State<AppState>) -> impl IntoResponse {
    let playbooks = state.soar_orchestrator.list_playbooks().await;
    (StatusCode::OK, Json(playbooks))
}

/// Build the SOAR API router.
pub fn create_soar_router() -> Router<AppState> {
    Router::new()
        .route("/soar/events", post(post_soar_event))
        .route("/soar/playbooks", get(get_soar_playbooks))
}
