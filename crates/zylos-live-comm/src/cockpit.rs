//! Cockpit API — REST endpoints for Leptos WASM frontend
//!
//! Provides agent state queries, stream management, and system health
//! endpoints consumed by the Cockpit dashboard.

use crate::event_bus::EventBus;
use axum::{routing::get, Json, Router};
use serde::Serialize;

/// System health response for cockpit dashboard
#[derive(Serialize, serde::Deserialize)]
/// Estrutura HealthResponse.
pub struct HealthResponse {
    /// Campo status.
    pub status: String,
    /// Campo active streams.
    pub active_streams: usize,
    /// Campo version.
    pub version: String,
    /// Nome do serviço (monitoração externa: Caddy/nginx/docker healthcheck).
    #[serde(default)]
    pub service: String,
}

/// Build cockpit API router
pub fn create_cockpit_router(bus: EventBus) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(bus)
}

async fn health_handler(
    axum::extract::State(bus): axum::extract::State<EventBus>,
) -> Json<HealthResponse> {
    // Variante Json.
    Json(HealthResponse {
        status: "healthy".to_string(),
        active_streams: bus.receiver_count(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        service: "zylos-gateway".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_endpoint() {
        let bus = EventBus::new(16, None);
        let app = create_cockpit_router(bus);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);

        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let health: HealthResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(health.status, "healthy");
    }
}
