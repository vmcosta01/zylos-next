//! JWT Claims and tenant identification for multi-tenant auth.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Unique identifier for a tenant (organization/workspace).
pub type TenantId = String;

/// Default issuer embedded in tokens (`iss`).
pub const DEFAULT_ISSUER: &str = "zylos-gateway";
/// Default audience embedded in tokens (`aud`).
pub const DEFAULT_AUDIENCE: &str = "zylos-api";

/// JWT claims embedded in every access token.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// Subject — user ID (UUID v4).
    pub sub: String,
    /// Tenant this user belongs to.
    pub tenant_id: TenantId,
    /// Roles assigned to the user within the tenant.
    pub roles: Vec<String>,
    /// Token issued at.
    pub iat: i64,
    /// Token expiration (Unix timestamp).
    pub exp: i64,
    /// Not before (Unix timestamp). F11.11.2.
    #[serde(default)]
    pub nbf: i64,
    /// Unique token id — used for revocation (denylist). F11.11.2.
    #[serde(default)]
    pub jti: String,
    /// Issuer. F11.11.2.
    #[serde(default)]
    pub iss: String,
    /// Audience. F11.11.2.
    #[serde(default)]
    pub aud: String,
    /// Optional human-readable name for display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Claims {
    /// Create new claims with the given user, tenant, roles and TTL.
    ///
    /// A random `jti` is generated; `iss`/`aud` use the crate defaults and
    /// are overwritten by [`crate::TokenService`] with its configured values.
    pub fn new(user_id: String, tenant_id: TenantId, roles: Vec<String>, ttl_secs: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            tenant_id,
            roles,
            iat: now.timestamp(),
            exp: now.timestamp().saturating_add(ttl_secs),
            nbf: now.timestamp(),
            jti: uuid::Uuid::new_v4().to_string(),
            iss: DEFAULT_ISSUER.into(),
            aud: DEFAULT_AUDIENCE.into(),
            name: None,
        }
    }

    /// Check if the token is expired based on current UTC time.
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }

    /// Check if the user has a specific role.
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claims_creation_and_expiry() {
        let claims = Claims::new(
            "user-1".into(),
            "tenant-a".into(),
            vec!["admin".into()],
            3600,
        );
        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.tenant_id, "tenant-a");
        assert!(claims.has_role("admin"));
        assert!(!claims.has_role("viewer"));
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_claims_expired() {
        let claims = Claims::new("u".into(), "t".into(), vec![], -1);
        // exp is in the past because ttl was negative
        assert!(claims.is_expired());
    }

    #[test]
    fn test_claims_have_unique_jti() {
        let a = Claims::new("u".into(), "t".into(), vec![], 60);
        let b = Claims::new("u".into(), "t".into(), vec![], 60);
        assert!(!a.jti.is_empty());
        assert_ne!(a.jti, b.jti);
    }

    #[test]
    fn test_ttl_overflow_saturates() {
        let c = Claims::new("u".into(), "t".into(), vec![], i64::MAX);
        assert_eq!(c.exp, i64::MAX);
    }
}
