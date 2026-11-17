//! JWT token generation and validation service.

use crate::claims::{Claims, TenantId};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur during token operations.
#[derive(Debug, Error)]
pub enum TokenError {
    #[error("JWT encoding failed: {0}")]
    EncodingFailed(#[from] jsonwebtoken::errors::Error),
    #[error("token is expired")]
    Expired,
    #[error("invalid token: {0}")]
    Invalid(String),
    #[error("missing secret key configuration")]
    MissingSecret,
}

/// Configuration for token signing and validation.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// HMAC secret used for HS256 signing.
    pub secret: String,
    /// Default token TTL in seconds (default: 3600 = 1 hour).
    pub default_ttl_secs: i64,
    /// Algorithm (default: HS256).
    pub algorithm: Algorithm,
}

impl Default for TokenConfig {
    fn default() -> Self {
        Self {
            secret: std::env::var("ZYLOS_AUTH_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-me-in-production".into()),
            default_ttl_secs: 3600,
            algorithm: Algorithm::HS256,
        }
    }
}

/// Request payload for creating a new token.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTokenRequest {
    pub user_id: String,
    pub tenant_id: TenantId,
    pub roles: Vec<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// Response containing a signed JWT token.
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Service responsible for issuing and validating JWT tokens.
#[derive(Clone)]
pub struct TokenService {
    config: TokenConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
}

impl TokenService {
    /// Create a new TokenService with the given configuration.
    pub fn new(config: TokenConfig) -> Result<Self, TokenError> {
        if config.secret.is_empty() {
            return Err(TokenError::MissingSecret);
        }
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
        Ok(Self {
            config,
            encoding_key,
            decoding_key,
        })
    }

    /// Issue a new JWT token for the given request.
    pub fn create_token(&self, req: &CreateTokenRequest) -> Result<TokenResponse, TokenError> {
        let mut claims = Claims::new(
            req.user_id.clone(),
            req.tenant_id.clone(),
            req.roles.clone(),
            self.config.default_ttl_secs,
        );
        claims.name = req.name.clone();

        let header = Header::new(self.config.algorithm);
        let token = encode(&header, &claims, &self.encoding_key)?;

        Ok(TokenResponse {
            access_token: token,
            token_type: "Bearer".into(),
            expires_in: self.config.default_ttl_secs,
        })
    }

    /// Validate a JWT token string and return the decoded claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, TokenError> {
        let mut validation = Validation::new(self.config.algorithm);
        validation.validate_exp = true;

        let data = decode::<Claims>(token, &self.decoding_key, &validation)?;

        if data.claims.is_expired() {
            return Err(TokenError::Expired);
        }

        Ok(data.claims)
    }

    /// Get the configured default TTL.
    pub fn default_ttl(&self) -> i64 {
        self.config.default_ttl_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_service() -> TokenService {
        let config = TokenConfig {
            secret: "test-secret-for-unit-tests".into(),
            default_ttl_secs: 3600,
            algorithm: Algorithm::HS256,
        };
        TokenService::new(config).expect("failed to create token service")
    }

    #[test]
    fn test_create_and_validate_token() {
        let svc = test_service();
        let req = CreateTokenRequest {
            user_id: "user-123".into(),
            tenant_id: "tenant-abc".into(),
            roles: vec!["editor".into()],
            name: Some("Alice".into()),
        };

        let response = svc.create_token(&req).expect("create failed");
        assert_eq!(response.token_type, "Bearer");
        assert!(response.expires_in > 0);

        let claims = svc
            .validate_token(&response.access_token)
            .expect("validate failed");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.tenant_id, "tenant-abc");
        assert!(claims.has_role("editor"));
        assert_eq!(claims.name, Some("Alice".into()));
    }

    #[test]
    fn test_invalid_token_rejected() {
        let svc = test_service();
        let result = svc.validate_token("not.a.valid.token");
        assert!(result.is_err());
    }

    #[test]
    fn test_wrong_secret_rejected() {
        let svc1 = test_service();
        let svc2 = TokenService::new(TokenConfig {
            secret: "different-secret".into(),
            ..TokenConfig::default()
        })
        .unwrap();

        let req = CreateTokenRequest {
            user_id: "u".into(),
            tenant_id: "t".into(),
            roles: vec![],
            name: None,
        };
        let token = svc1.create_token(&req).unwrap();
        assert!(svc2.validate_token(&token.access_token).is_err());
    }
}
