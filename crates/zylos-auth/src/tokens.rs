//! JWT token generation and validation service.
//!
//! F11.11.2 hardening:
//! - no hardcoded fallback secret; secret must be >= [`MIN_SECRET_LEN`] bytes
//! - `iss`/`aud`/`exp`/`sub` required and validated, `nbf` validated
//! - algorithm pinned (no `alg: none`, no algorithm confusion)
//! - short access-token TTL + opaque refresh tokens with rotation and
//!   reuse detection (whole family revoked on replay)
//! - revocation by `jti` (in-memory denylist, purged on expiry)

use crate::claims::{Claims, TenantId, DEFAULT_AUDIENCE, DEFAULT_ISSUER};
use chrono::Utc;
use dashmap::DashMap;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use thiserror::Error;

/// Minimum HMAC secret length in bytes (RFC 7518 §3.2: >= hash size).
pub const MIN_SECRET_LEN: usize = 32;
/// Maximum access-token TTL accepted by [`TokenService::new`] (24h).
pub const MAX_ACCESS_TTL_SECS: i64 = 24 * 3600;

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
    #[error("secret key too short (min {MIN_SECRET_LEN} bytes)")]
    WeakSecret,
    #[error("token has been revoked")]
    Revoked,
    #[error("invalid token configuration: {0}")]
    InvalidConfig(String),
}

/// Configuration for token signing and validation.
#[derive(Debug, Clone)]
pub struct TokenConfig {
    /// HMAC secret used for HS256 signing.
    pub secret: String,
    /// Access-token TTL in seconds (default: 900 = 15 min).
    pub default_ttl_secs: i64,
    /// Refresh-token TTL in seconds (default: 7 days).
    pub refresh_ttl_secs: i64,
    /// Algorithm (default: HS256). Only HMAC algorithms are accepted.
    pub algorithm: Algorithm,
    /// Expected / emitted issuer.
    pub issuer: String,
    /// Expected / emitted audience.
    pub audience: String,
    /// Clock-skew leeway in seconds (default 30).
    pub leeway_secs: u64,
}

impl Default for TokenConfig {
    /// Reads `ZYLOS_AUTH_SECRET`. **No fallback secret**: when the variable is
    /// unset the secret is empty and [`TokenService::new`] fails closed.
    fn default() -> Self {
        Self {
            secret: std::env::var("ZYLOS_AUTH_SECRET").unwrap_or_default(),
            default_ttl_secs: 900,
            refresh_ttl_secs: 7 * 24 * 3600,
            algorithm: Algorithm::HS256,
            issuer: DEFAULT_ISSUER.into(),
            audience: DEFAULT_AUDIENCE.into(),
            leeway_secs: 30,
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
    /// Opaque refresh token (only when issued via [`TokenService::create_token_pair`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Clone)]
struct RefreshEntry {
    sub: String,
    tenant_id: TenantId,
    roles: Vec<String>,
    name: Option<String>,
    exp: i64,
    family: String,
    used: bool,
}

/// Service responsible for issuing and validating JWT tokens.
#[derive(Clone)]
pub struct TokenService {
    config: TokenConfig,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    /// jti -> exp (denylist).
    revoked: Arc<DashMap<String, i64>>,
    /// sha256(refresh token) -> entry.
    refresh: Arc<DashMap<String, RefreshEntry>>,
    /// revoked refresh families.
    revoked_families: Arc<DashMap<String, i64>>,
}

fn hash_refresh(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

fn random_token() -> String {
    // 2 x UUIDv4 = 244 bits from the OS CSPRNG (getrandom).
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

impl TokenService {
    /// Create a new TokenService with the given configuration.
    pub fn new(config: TokenConfig) -> Result<Self, TokenError> {
        if config.secret.is_empty() {
            return Err(TokenError::MissingSecret);
        }
        if config.secret.len() < MIN_SECRET_LEN {
            return Err(TokenError::WeakSecret);
        }
        if !matches!(
            config.algorithm,
            Algorithm::HS256 | Algorithm::HS384 | Algorithm::HS512
        ) {
            return Err(TokenError::InvalidConfig(
                "only HS256/HS384/HS512 are supported with a shared secret".into(),
            ));
        }
        if config.default_ttl_secs <= 0 || config.default_ttl_secs > MAX_ACCESS_TTL_SECS {
            return Err(TokenError::InvalidConfig(format!(
                "access TTL must be in 1..={MAX_ACCESS_TTL_SECS}s"
            )));
        }
        if config.refresh_ttl_secs <= 0 {
            return Err(TokenError::InvalidConfig("refresh TTL must be > 0".into()));
        }
        let encoding_key = EncodingKey::from_secret(config.secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());
        Ok(Self {
            config,
            encoding_key,
            decoding_key,
            revoked: Arc::new(DashMap::new()),
            refresh: Arc::new(DashMap::new()),
            revoked_families: Arc::new(DashMap::new()),
        })
    }

    fn sign(&self, req: &CreateTokenRequest) -> Result<(String, Claims), TokenError> {
        let mut claims = Claims::new(
            req.user_id.clone(),
            req.tenant_id.clone(),
            req.roles.clone(),
            self.config.default_ttl_secs,
        );
        claims.name = req.name.clone();
        claims.iss = self.config.issuer.clone();
        claims.aud = self.config.audience.clone();
        let header = Header::new(self.config.algorithm);
        let token = encode(&header, &claims, &self.encoding_key)?;
        Ok((token, claims))
    }

    /// Issue a new (access-only) JWT token for the given request.
    pub fn create_token(&self, req: &CreateTokenRequest) -> Result<TokenResponse, TokenError> {
        let (token, _) = self.sign(req)?;
        Ok(TokenResponse {
            access_token: token,
            token_type: "Bearer".into(),
            expires_in: self.config.default_ttl_secs,
            refresh_token: None,
        })
    }

    /// Issue an access token plus an opaque, single-use refresh token.
    pub fn create_token_pair(&self, req: &CreateTokenRequest) -> Result<TokenResponse, TokenError> {
        let family = uuid::Uuid::new_v4().to_string();
        self.issue_pair(req, family)
    }

    fn issue_pair(
        &self,
        req: &CreateTokenRequest,
        family: String,
    ) -> Result<TokenResponse, TokenError> {
        let mut resp = self.create_token(req)?;
        let rt = random_token();
        self.refresh.insert(
            hash_refresh(&rt),
            RefreshEntry {
                sub: req.user_id.clone(),
                tenant_id: req.tenant_id.clone(),
                roles: req.roles.clone(),
                name: req.name.clone(),
                exp: Utc::now()
                    .timestamp()
                    .saturating_add(self.config.refresh_ttl_secs),
                family,
                used: false,
            },
        );
        resp.refresh_token = Some(rt);
        Ok(resp)
    }

    /// Exchange a refresh token for a new pair (rotation).
    ///
    /// Presenting an already-used refresh token is treated as theft: the
    /// entire token family is revoked and [`TokenError::Revoked`] returned.
    pub fn refresh(&self, refresh_token: &str) -> Result<TokenResponse, TokenError> {
        let key = hash_refresh(refresh_token);
        let now = Utc::now().timestamp();
        let entry = {
            let mut e = self
                .refresh
                .get_mut(&key)
                .ok_or_else(|| TokenError::Invalid("unknown refresh token".into()))?;
            if self.revoked_families.contains_key(&e.family) {
                return Err(TokenError::Revoked);
            }
            if e.used {
                let fam = e.family.clone();
                let exp = e.exp;
                drop(e);
                self.revoked_families.insert(fam, exp);
                tracing::warn!("refresh token reuse detected — family revoked");
                return Err(TokenError::Revoked);
            }
            if e.exp < now {
                return Err(TokenError::Expired);
            }
            e.used = true;
            e.clone()
        };
        self.issue_pair(
            &CreateTokenRequest {
                user_id: entry.sub,
                tenant_id: entry.tenant_id,
                roles: entry.roles,
                name: entry.name,
            },
            entry.family,
        )
    }

    /// Revoke a refresh token's whole family (logout).
    pub fn revoke_refresh(&self, refresh_token: &str) -> bool {
        let key = hash_refresh(refresh_token);
        if let Some(e) = self.refresh.get(&key) {
            self.revoked_families.insert(e.family.clone(), e.exp);
            true
        } else {
            false
        }
    }

    /// Revoke an access token by `jti` until its expiry.
    pub fn revoke_jti(&self, jti: &str, exp: i64) {
        self.revoked.insert(jti.to_string(), exp);
    }

    /// Validate then revoke the given access token (logout).
    pub fn revoke_token(&self, token: &str) -> Result<(), TokenError> {
        let claims = self.validate_token(token)?;
        self.revoke_jti(&claims.jti, claims.exp);
        Ok(())
    }

    /// Whether a `jti` is in the denylist.
    pub fn is_revoked(&self, jti: &str) -> bool {
        self.revoked.contains_key(jti)
    }

    /// Drop expired denylist / refresh entries. Returns entries removed.
    pub fn purge_expired(&self) -> usize {
        let now = Utc::now().timestamp() - self.config.leeway_secs as i64;
        let before = self.revoked.len() + self.refresh.len() + self.revoked_families.len();
        self.revoked.retain(|_, exp| *exp >= now);
        self.refresh.retain(|_, e| e.exp >= now);
        self.revoked_families.retain(|_, exp| *exp >= now);
        before - (self.revoked.len() + self.refresh.len() + self.revoked_families.len())
    }

    /// Validate a JWT token string and return the decoded claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, TokenError> {
        let mut validation = Validation::new(self.config.algorithm);
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.leeway = self.config.leeway_secs;
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);

        let data = decode::<Claims>(token, &self.decoding_key, &validation).map_err(|e| match e
            .kind()
        {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::Expired,
            _ => TokenError::Invalid(e.to_string()),
        })?;

        if data.claims.is_expired() {
            return Err(TokenError::Expired);
        }
        if data.claims.jti.is_empty() {
            return Err(TokenError::Invalid("missing jti".into()));
        }
        if data.claims.iat > Utc::now().timestamp() + self.config.leeway_secs as i64 {
            return Err(TokenError::Invalid("iat in the future".into()));
        }
        if self.is_revoked(&data.claims.jti) {
            return Err(TokenError::Revoked);
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
    use base64::Engine;

    const SECRET: &str = "test-secret-for-unit-tests-0123456789";

    fn cfg() -> TokenConfig {
        TokenConfig {
            secret: SECRET.into(),
            default_ttl_secs: 3600,
            ..TokenConfig::default()
        }
    }

    fn test_service() -> TokenService {
        TokenService::new(cfg()).expect("failed to create token service")
    }

    fn req() -> CreateTokenRequest {
        CreateTokenRequest {
            user_id: "user-123".into(),
            tenant_id: "tenant-abc".into(),
            roles: vec!["editor".into()],
            name: Some("Alice".into()),
        }
    }

    fn b64(v: &serde_json::Value) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string())
    }

    fn sign_raw(claims: &serde_json::Value, alg: Algorithm, secret: &str) -> String {
        encode(
            &Header::new(alg),
            claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .unwrap()
    }

    fn base_claims() -> serde_json::Value {
        let now = Utc::now().timestamp();
        serde_json::json!({
            "sub": "u", "tenant_id": "t", "roles": ["admin"],
            "iat": now, "nbf": now, "exp": now + 600,
            "jti": "j-1", "iss": DEFAULT_ISSUER, "aud": DEFAULT_AUDIENCE
        })
    }

    #[test]
    fn test_create_and_validate_token() {
        let svc = test_service();
        let response = svc.create_token(&req()).expect("create failed");
        assert_eq!(response.token_type, "Bearer");
        assert!(response.expires_in > 0);

        let claims = svc
            .validate_token(&response.access_token)
            .expect("validate failed");
        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.tenant_id, "tenant-abc");
        assert!(claims.has_role("editor"));
        assert_eq!(claims.name, Some("Alice".into()));
        assert_eq!(claims.iss, DEFAULT_ISSUER);
        assert_eq!(claims.aud, DEFAULT_AUDIENCE);
        assert!(!claims.jti.is_empty());
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
            secret: "different-secret-different-secret-xx".into(),
            ..cfg()
        })
        .unwrap();
        let token = svc1.create_token(&req()).unwrap();
        assert!(svc2.validate_token(&token.access_token).is_err());
    }

    #[test]
    fn test_missing_and_weak_secret_fail_closed() {
        assert!(matches!(
            TokenService::new(TokenConfig {
                secret: String::new(),
                ..cfg()
            }),
            Err(TokenError::MissingSecret)
        ));
        assert!(matches!(
            TokenService::new(TokenConfig {
                secret: "short".into(),
                ..cfg()
            }),
            Err(TokenError::WeakSecret)
        ));
        // the old hardcoded dev fallback must not be accepted implicitly
        assert!(!TokenConfig::default()
            .secret
            .contains("dev-secret-change-me-in-production"));
    }

    #[test]
    fn test_config_rejects_bad_ttl_and_asymmetric_alg() {
        assert!(TokenService::new(TokenConfig {
            default_ttl_secs: 0,
            ..cfg()
        })
        .is_err());
        assert!(TokenService::new(TokenConfig {
            default_ttl_secs: MAX_ACCESS_TTL_SECS + 1,
            ..cfg()
        })
        .is_err());
        assert!(TokenService::new(TokenConfig {
            algorithm: Algorithm::RS256,
            ..cfg()
        })
        .is_err());
    }

    #[test]
    fn test_expired_token_rejected_beyond_leeway() {
        let svc = test_service();
        let mut c = base_claims();
        let now = Utc::now().timestamp();
        c["exp"] = serde_json::json!(now - 120); // > 30s leeway
        let t = sign_raw(&c, Algorithm::HS256, SECRET);
        assert!(matches!(svc.validate_token(&t), Err(TokenError::Expired)));
    }

    #[test]
    fn test_expired_within_leeway_still_rejected_by_strict_check() {
        let svc = test_service();
        let mut c = base_claims();
        c["exp"] = serde_json::json!(Utc::now().timestamp() - 5);
        let t = sign_raw(&c, Algorithm::HS256, SECRET);
        assert!(matches!(svc.validate_token(&t), Err(TokenError::Expired)));
    }

    #[test]
    fn test_alg_none_rejected() {
        let svc = test_service();
        let header = b64(&serde_json::json!({"alg": "none", "typ": "JWT"}));
        let payload = b64(&base_claims());
        for t in [
            format!("{header}.{payload}."),
            format!("{header}.{payload}"),
        ] {
            assert!(svc.validate_token(&t).is_err(), "alg none accepted: {t}");
        }
    }

    #[test]
    fn test_algorithm_confusion_rejected() {
        let svc = test_service();
        // same secret, different HMAC alg -> rejected (alg pinned)
        let t = sign_raw(&base_claims(), Algorithm::HS512, SECRET);
        assert!(svc.validate_token(&t).is_err());
        let t = sign_raw(&base_claims(), Algorithm::HS384, SECRET);
        assert!(svc.validate_token(&t).is_err());
    }

    #[test]
    fn test_tampered_payload_rejected() {
        let svc = test_service();
        let tok = svc.create_token(&req()).unwrap().access_token;
        let parts: Vec<&str> = tok.split('.').collect();
        let mut c = base_claims();
        c["roles"] = serde_json::json!(["admin"]);
        let forged = format!("{}.{}.{}", parts[0], b64(&c), parts[2]);
        assert!(svc.validate_token(&forged).is_err());
    }

    #[test]
    fn test_wrong_issuer_audience_and_missing_claims_rejected() {
        let svc = test_service();
        let mut c = base_claims();
        c["iss"] = serde_json::json!("evil");
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
        let mut c = base_claims();
        c["aud"] = serde_json::json!("other-api");
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
        // legacy token without iss/aud/jti (pre-F11) is rejected
        let mut c = base_claims();
        let o = c.as_object_mut().unwrap();
        o.remove("iss");
        o.remove("aud");
        o.remove("jti");
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
        // missing jti only
        let mut c = base_claims();
        c.as_object_mut().unwrap().remove("jti");
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
    }

    #[test]
    fn test_nbf_and_future_iat_rejected() {
        let svc = test_service();
        let now = Utc::now().timestamp();
        let mut c = base_claims();
        c["nbf"] = serde_json::json!(now + 3600);
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
        let mut c = base_claims();
        c["iat"] = serde_json::json!(now + 3600);
        assert!(svc
            .validate_token(&sign_raw(&c, Algorithm::HS256, SECRET))
            .is_err());
    }

    #[test]
    fn test_revocation_by_jti() {
        let svc = test_service();
        let tok = svc.create_token(&req()).unwrap().access_token;
        assert!(svc.validate_token(&tok).is_ok());
        svc.revoke_token(&tok).unwrap();
        assert!(matches!(svc.validate_token(&tok), Err(TokenError::Revoked)));
        // clones share the denylist
        let clone = svc.clone();
        assert!(matches!(
            clone.validate_token(&tok),
            Err(TokenError::Revoked)
        ));
        // other tokens unaffected
        let other = svc.create_token(&req()).unwrap().access_token;
        assert!(svc.validate_token(&other).is_ok());
    }

    #[test]
    fn test_refresh_rotation_and_reuse_detection() {
        let svc = test_service();
        let p1 = svc.create_token_pair(&req()).unwrap();
        let r1 = p1.refresh_token.clone().unwrap();
        assert!(r1.len() >= 64);
        let p2 = svc.refresh(&r1).unwrap();
        let r2 = p2.refresh_token.clone().unwrap();
        assert_ne!(r1, r2);
        let c = svc.validate_token(&p2.access_token).unwrap();
        assert_eq!(c.sub, "user-123");
        assert_eq!(c.roles, vec!["editor".to_string()]);

        // replay of r1 => family revoked, r2 also dies
        assert!(matches!(svc.refresh(&r1), Err(TokenError::Revoked)));
        assert!(matches!(svc.refresh(&r2), Err(TokenError::Revoked)));
    }

    #[test]
    fn test_refresh_unknown_and_logout() {
        let svc = test_service();
        assert!(svc.refresh("nope").is_err());
        let p = svc.create_token_pair(&req()).unwrap();
        let r = p.refresh_token.unwrap();
        assert!(svc.revoke_refresh(&r));
        assert!(matches!(svc.refresh(&r), Err(TokenError::Revoked)));
    }

    #[test]
    fn test_refresh_expired() {
        let svc = TokenService::new(TokenConfig {
            refresh_ttl_secs: 1,
            ..cfg()
        })
        .unwrap();
        let r = svc
            .create_token_pair(&req())
            .unwrap()
            .refresh_token
            .unwrap();
        // force expiry
        for mut e in svc.refresh.iter_mut() {
            e.exp = Utc::now().timestamp() - 10;
        }
        assert!(matches!(svc.refresh(&r), Err(TokenError::Expired)));
    }

    #[test]
    fn test_purge_expired() {
        let svc = test_service();
        svc.revoke_jti("old", Utc::now().timestamp() - 3600);
        svc.revoke_jti("new", Utc::now().timestamp() + 3600);
        assert_eq!(svc.purge_expired(), 1);
        assert!(svc.is_revoked("new"));
        assert!(!svc.is_revoked("old"));
    }
}
