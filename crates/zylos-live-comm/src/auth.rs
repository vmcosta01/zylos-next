//! JWT Authentication middleware for Axum (Task 6.1.1)
//!
//! Validates Bearer tokens using HS256 by default.
//! Extracts claims into request extensions for downstream handlers.

use axum::{
    extract::Request,
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
/// Estrutura Claims.
pub struct Claims {
    /// Campo sub.
    pub sub: String, // User ID
    /// Campo role.
    pub role: String, // RBAC role
    /// Campo exp.
    pub exp: usize, // Expiration timestamp
    /// Campo iat.
    pub iat: usize, // Issued at timestamp
}

/// Configuration for JWT authentication
#[derive(Clone)]
/// Estrutura JwtConfig.
pub struct JwtConfig {
    /// Campo secret.
    pub secret: String,
    /// Campo issuer.
    pub issuer: String,
    /// Campo expiration secs.
    pub expiration_secs: u64,
}

impl Default for JwtConfig {
    fn default() -> Self {
        // Variante Self.
        Self {
            secret: std::env::var("ZYLOS_JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".into()),
            issuer: "zylos-gateway".into(),
            expiration_secs: 3600, // 1 hour default
        }
    }
}

impl JwtConfig {
    /// Create a new JWT token for the given user
    pub fn create_token(
        &self,
        user_id: &str,
        role: &str,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            role: role.to_string(),
            exp: now + self.expiration_secs as usize,
            iat: now,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )
    }

    /// Validate a JWT token and return claims
    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let mut validation = Validation::default();
        validation.set_issuer(&[&self.issuer]);

        decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &validation,
        )
        .map(|data| data.claims)
    }
}

/// Axum middleware that validates JWT Bearer tokens
pub async fn jwt_auth_middleware(mut request: Request, next: Next) -> Response {
    let config = request
        .extensions()
        .get::<JwtConfig>()
        .cloned()
        .unwrap_or_default();

    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !auth_header.starts_with("Bearer ") {
        return error_response(
            StatusCode::UNAUTHORIZED,
            "missing or invalid authorization header",
        );
    }

    let token = &auth_header[7..];

    match config.validate_token(token) {
        // Variante Ok.
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        // Variante Err.
        Err(_) => error_response(StatusCode::UNAUTHORIZED, "invalid or expired token"),
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    let body = serde_json::json!({
        "error": message,
        "status": status.as_u16()
    });
    (status, axum::Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_validate_token() {
        let config = JwtConfig {
            secret: "test-secret".into(),
            issuer: "test-issuer".into(),
            expiration_secs: 3600,
        };

        let token = config.create_token("user-1", "admin").unwrap();
        let claims = config.validate_token(&token).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.role, "admin");
    }

    #[test]
    fn test_reject_invalid_token() {
        let config = JwtConfig {
            secret: "test-secret".into(),
            issuer: "test-issuer".into(),
            expiration_secs: 3600,
        };

        let result = config.validate_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_wrong_secret() {
        let config1 = JwtConfig {
            secret: "secret-1".into(),
            issuer: "test".into(),
            expiration_secs: 3600,
        };
        let config2 = JwtConfig {
            secret: "secret-2".into(),
            issuer: "test".into(),
            expiration_secs: 3600,
        };

        let token = config1.create_token("user-1", "admin").unwrap();
        let result = config2.validate_token(&token);
        assert!(result.is_err());
    }
}
