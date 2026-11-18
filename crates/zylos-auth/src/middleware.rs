//! Axum middleware for JWT Bearer token validation and tenant isolation.

use crate::claims::Claims;
use crate::permissions::Permission;
use crate::tokens::{TokenError, TokenService};
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

/// Extension type attached to requests that pass auth validation.
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub claims: Claims,
}

/// Axum middleware that validates Bearer tokens and injects [`AuthContext`] into request extensions.
///
/// Usage:
/// ```ignore
/// use zylos_auth::middleware::auth_middleware;
/// let token_service = TokenService::new(TokenConfig::default()).unwrap();
/// let app = Router::new()
///     .route("/protected", get(handler))
///     .layer(axum::middleware::from_fn_with_state(token_service, auth_middleware));
/// ```
pub async fn auth_middleware(
    axum::extract::State(token_service): axum::extract::State<TokenService>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // RFC 6750 / RFC 9110: auth scheme is case-insensitive.
    let token = match auth_header.split_once(' ') {
        Some((scheme, tok)) if scheme.eq_ignore_ascii_case("bearer") => tok.trim(),
        _ => return Err(StatusCode::UNAUTHORIZED),
    };
    if token.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    match token_service.validate_token(token) {
        Ok(claims) => {
            request.extensions_mut().insert(AuthContext { claims });
            Ok(next.run(request).await)
        }
        Err(TokenError::Revoked) => {
            warn!("revoked JWT presented");
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(TokenError::Expired) => {
            warn!("expired JWT presented");
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            warn!(error = %e, "invalid JWT presented");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Helper to extract AuthContext from request extensions in handlers.
pub fn require_auth(request: &Request) -> Result<&AuthContext, StatusCode> {
    request
        .extensions()
        .get::<AuthContext>()
        .ok_or(StatusCode::UNAUTHORIZED)
}

/// Check if the authenticated user has a specific permission.
pub fn require_permission(request: &Request, perm: &Permission) -> Result<(), StatusCode> {
    let ctx = require_auth(request)?;
    if perm.is_granted_by(&ctx.claims.roles) {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::{CreateTokenRequest, TokenConfig};
    use axum::body::Body;
    use axum::http::header::AUTHORIZATION;
    use axum::routing::get;
    use axum::{middleware, Router};
    use tower::ServiceExt;

    fn test_service() -> TokenService {
        TokenService::new(TokenConfig {
            secret: "middleware-test-secret-0123456789abcdef".into(),
            default_ttl_secs: 3600,
            ..TokenConfig::default()
        })
        .unwrap()
    }

    #[tokio::test]
    async fn test_valid_token_passes() {
        let svc = test_service();
        let token = svc
            .create_token(&CreateTokenRequest {
                user_id: "u1".into(),
                tenant_id: "t1".into(),
                roles: vec!["admin".into()],
                name: None,
            })
            .unwrap();

        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(svc.clone(), auth_middleware));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header(AUTHORIZATION, format!("Bearer {}", token.access_token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_missing_token_rejected() {
        let svc = test_service();
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(svc, auth_middleware));

        let resp = app
            .oneshot(Request::builder().uri("/ok").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_token_rejected() {
        let svc = test_service();
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .layer(middleware::from_fn_with_state(svc, auth_middleware));

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/ok")
                    .header(AUTHORIZATION, "Bearer invalid.token.here")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
