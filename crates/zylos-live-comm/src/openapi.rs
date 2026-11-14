//! OpenAPI documentation scaffold for zylos-live-comm (Task 6.1.3)
//!
//! Generates OpenAPI 3.0 spec using utoipa for all live-comm endpoints.
//! Includes SSE, WebSocket, auth, and rate-limit documentation.

use utoipa::OpenApi;

use crate::agui::AgUiEvent;
use crate::auth::Claims;
use crate::routes::FilterParams;

// OpenAPI documentation root for zylos-live-comm
#[derive(OpenApi)]
#[openapi(
    paths(
        sse_events_endpoint,
        ws_stream_endpoint,
    ),
    components(schemas(
        AgUiEvent,
        Claims,
        FilterParams,
        ErrorResponse,
        RateLimitResponse,
    )),
    tags(
        (name = "live-comm", description = "Real-time AG-UI streaming endpoints"),
        (name = "auth", description = "JWT authentication"),
    ),
    info(
        title = "Zylos Live Communication API",
        version = "0.1.0",
        description = "Real-time event streaming via SSE and WebSocket for AG-UI protocol",
        license(name = "MIT"),
    ),
    modifiers(&SecurityAddon),
)]
/// Estrutura ApiDoc.
pub struct ApiDoc;

/// Security scheme addon for JWT Bearer auth
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
        }
    }
}

/// Standard error response
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
/// Estrutura ErrorResponse.
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// HTTP status code
    pub status: u16,
}

/// Rate limit exceeded response
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
/// Estrutura RateLimitResponse.
pub struct RateLimitResponse {
    /// Error message
    pub error: String,
    /// HTTP status code (429)
    pub status: u16,
    /// Seconds until rate limit resets
    pub retry_after_secs: u64,
}

/// SSE events endpoint documentation
#[utoipa::path(
    get,
    path = "/events",
    tag = "live-comm",
    params(FilterParams),
    responses(
        (status = 200, description = "Server-Sent Events stream of AG-UI events", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized - invalid or missing JWT", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = RateLimitResponse),
    ),
    security(("bearer_auth" = []))
)]
/// Função sse events endpoint.
pub fn sse_events_endpoint() {}

/// WebSocket streaming endpoint documentation
#[utoipa::path(
    get,
    path = "/ws",
    tag = "live-comm",
    params(FilterParams),
    responses(
        (status = 101, description = "Switching Protocols - WebSocket upgrade successful"),
        (status = 401, description = "Unauthorized - invalid or missing JWT", body = ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = RateLimitResponse),
    ),
    security(("bearer_auth" = []))
)]
/// Função ws stream endpoint.
pub fn ws_stream_endpoint() {}

/// Generate the OpenAPI JSON specification
pub fn generate_openapi_json() -> String {
    ApiDoc::openapi()
        .to_pretty_json()
        .expect("valid OpenAPI JSON")
}

/// Generate the OpenAPI YAML specification
pub fn generate_openapi_yaml() -> String {
    ApiDoc::openapi().to_yaml().expect("valid OpenAPI YAML")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_openapi_json() {
        let json = generate_openapi_json();
        assert!(
            json.contains("\"title\": \"Zylos Live Communication API\"")
                || json.contains("\"title\":\"Zylos Live Communication API\"")
        );
        assert!(json.contains("/events"));
        assert!(json.contains("/ws"));
        assert!(json.contains("bearer_auth"));
    }

    #[test]
    fn test_generate_openapi_yaml() {
        let yaml = generate_openapi_yaml();
        assert!(yaml.contains("title: Zylos Live Communication API"));
        assert!(yaml.contains("/events"));
        assert!(yaml.contains("/ws"));
    }

    #[test]
    fn test_error_response_schema() {
        let json = generate_openapi_json();
        assert!(json.contains("ErrorResponse"));
        assert!(json.contains("RateLimitResponse"));
    }
}
