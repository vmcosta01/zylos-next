//! Role definitions for RBAC within tenants.

use serde::{Deserialize, Serialize};

/// Built-in roles supported by zylos-auth.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full administrative access within the tenant.
    Admin,
    /// Can create, edit and delete skills and documents.
    Editor,
    /// Read-only access to resources.
    Viewer,
    /// Service account with limited API access.
    ServiceAccount,
    /// Custom role defined at runtime.
    Custom(String),
}

impl Role {
    /// Parse a role string into a Role enum.
    pub fn parse(s: &str) -> Self {
        match s {
            "admin" => Role::Admin,
            "editor" => Role::Editor,
            "viewer" => Role::Viewer,
            "service_account" => Role::ServiceAccount,
            other => Role::Custom(other.to_string()),
        }
    }

    /// Convert role to its string representation.
    pub fn as_str(&self) -> &str {
        match self {
            Role::Admin => "admin",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
            Role::ServiceAccount => "service_account",
            Role::Custom(s) => s.as_str(),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_roundtrip() {
        assert_eq!(Role::parse("admin"), Role::Admin);
        assert_eq!(Role::parse("editor"), Role::Editor);
        assert_eq!(Role::parse("custom_role"), Role::Custom("custom_role".into()));
        assert_eq!(Role::Admin.as_str(), "admin");
    }
}