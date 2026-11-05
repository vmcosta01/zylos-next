//! Webhook ingest layer: HTTP endpoints for receiving external security events.

use crate::error::Result;
use crate::event::{EventCategory, Severity, ZylosSecurityEvent};
use crate::router::EventRouter;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::trace::TraceLayer;
use uuid::Uuid;

/// Shared application state for the webhook server.
pub struct AppState {
    pub router: RwLock<EventRouter>,
}

impl AppState {
    pub fn new(router: EventRouter) -> Self {
        Self {
            router: RwLock::new(router),
        }
    }
}

/// Generic webhook payload — accepts any JSON and normalizes to `ZylosSecurityEvent`.
#[derive(Debug, Clone, Deserialize)]
pub struct WebhookPayload {
    /// Optional event ID; auto-generated if missing.
    pub id: Option<Uuid>,
    /// Event source identifier (e.g. "crowdstrike", "sentinel-one").
    #[serde(default = "default_source")]
    pub source: String,
    /// Severity string: "critical", "high", "medium", "low", "info".
    #[serde(default = "default_severity_str")]
    pub severity: String,
    /// Category string: "breach", "anomaly", "vulnerability", "compliance", "false_positive".
    #[serde(default = "default_category_str")]
    pub category: String,
    /// Human-readable title.
    #[serde(default = "default_title")]
    pub title: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Affected asset identifiers.
    #[serde(default)]
    pub affected_assets: Vec<String>,
    /// Raw payload passthrough.
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

fn default_source() -> String {
    "webhook".to_string()
}
fn default_severity_str() -> String {
    "medium".to_string()
}
fn default_category_str() -> String {
    "anomaly".to_string()
}
fn default_title() -> String {
    "Webhook Event".to_string()
}

/// Response returned after successful webhook ingestion.
#[derive(Debug, Serialize)]
pub struct IngestResponse {
    pub event_id: Uuid,
    pub dispatched_playbooks: Vec<String>,
}

/// Normalize a `WebhookPayload` into a `ZylosSecurityEvent`.
pub fn normalize_payload(payload: WebhookPayload) -> Result<ZylosSecurityEvent> {
    let severity = parse_severity(&payload.severity);
    let category = parse_category(&payload.category);

    Ok(ZylosSecurityEvent::new(
        payload.source,
        severity,
        category,
        payload.title,
        payload.description,
        payload.affected_assets,
        payload.extra,
    ))
}

fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "medium" => Severity::Medium,
        "low" => Severity::Low,
        "info" | "informational" => Severity::Info,
        _ => Severity::Medium,
    }
}

fn parse_category(s: &str) -> EventCategory {
    match s.to_lowercase().as_str() {
        "breach" => EventCategory::Breach,
        "anomaly" => EventCategory::Anomaly,
        "vulnerability" | "vuln" => EventCategory::Vulnerability,
        "compliance" => EventCategory::Compliance,
        "false_positive" | "falsepositive" | "fp" => EventCategory::FalsePositive,
        _ => EventCategory::Anomaly,
    }
}

/// POST /webhook/ingest — receive a security event and route through playbooks.
async fn ingest_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    let event = match normalize_payload(payload) {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(IngestResponse {
                    event_id: Uuid::nil(),
                    dispatched_playbooks: vec![format!("normalization error: {}", e)],
                }),
            );
        }
    };

    let event_id = event.id;
    let router = state.router.read().await;
    let dispatched = match router.route(&event) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IngestResponse {
                    event_id,
                    dispatched_playbooks: vec![format!("routing error: {}", e)],
                }),
            );
        }
    };

    let playbook_names: Vec<String> = dispatched
        .iter()
        .map(|d| d.playbook.name.clone())
        .collect();

    // TODO: hand off dispatched incidents to executor in background task
    drop(dispatched);

    (
        StatusCode::ACCEPTED,
        Json(IngestResponse {
            event_id,
            dispatched_playbooks: playbook_names,
        }),
    )
}

/// POST /health — liveness check.
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
}

/// Build the Axum router with all webhook endpoints.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/webhook/ingest", post(ingest_webhook))
        .route("/health", axum::routing::get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_state() -> Arc<AppState> {
        Arc::new(AppState::new(EventRouter::new()))
    }

    #[test]
    fn normalize_valid_payload() {
        let payload = WebhookPayload {
            id: None,
            source: "test-source".to_string(),
            severity: "critical".to_string(),
            category: "breach".to_string(),
            title: "Test Breach".to_string(),
            description: "A test breach event".to_string(),
            affected_assets: vec!["host-01".to_string()],
            extra: serde_json::json!({}),
        };

        let event = normalize_payload(payload).unwrap();
        assert_eq!(event.severity, Severity::Critical);
        assert_eq!(event.category, EventCategory::Breach);
        assert_eq!(event.source, "test-source");
        assert_eq!(event.affected_assets, vec!["host-01"]);
    }

    #[test]
    fn normalize_unknown_severity_defaults_medium() {
        let payload = WebhookPayload {
            id: None,
            source: "x".to_string(),
            severity: "unknown".to_string(),
            category: "anomaly".to_string(),
            title: "T".to_string(),
            description: String::new(),
            affected_assets: vec![],
            extra: serde_json::json!({}),
        };

        let event = normalize_payload(payload).unwrap();
        assert_eq!(event.severity, Severity::Medium);
    }

    #[test]
    fn normalize_unknown_category_defaults_anomaly() {
        let payload = WebhookPayload {
            id: None,
            source: "x".to_string(),
            severity: "low".to_string(),
            category: "something-weird".to_string(),
            title: "T".to_string(),
            description: String::new(),
            affected_assets: vec![],
            extra: serde_json::json!({}),
        };

        let event = normalize_payload(payload).unwrap();
        assert_eq!(event.category, EventCategory::Anomaly);
    }

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let app = build_router(test_state());

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ingest_endpoint_accepts_valid_payload() {
        let app = build_router(test_state());

        let body = serde_json::json!({
            "source": "test",
            "severity": "high",
            "category": "breach",
            "title": "Test Event",
            "affected_assets": ["host-01"]
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhook/ingest")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }
}