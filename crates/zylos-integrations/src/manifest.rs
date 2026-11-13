//! Integration manifest (spec) loader from YAML files.

use crate::error::{IntegrationError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Describes a single SaaS integration's OAuth2 configuration and available actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationSpec {
    /// Unique slug identifier (e.g., "gmail", "linear", "notion").
    pub slug: String,
    /// Human-readable display name.
    pub display_name: String,
    /// OAuth2 authorization endpoint URL.
    pub auth_url: String,
    /// OAuth2 token exchange endpoint URL.
    pub token_url: String,
    /// OAuth2 client ID.
    pub client_id: String,
    /// OAuth2 client secret.
    pub client_secret: String,
    /// Default redirect URI for OAuth callbacks.
    pub redirect_uri: String,
    /// OAuth2 scopes to request during authorization.
    #[serde(default)]
    pub scopes: Vec<String>,
    /// Available actions this integration can execute.
    #[serde(default)]
    pub actions: Vec<ActionSpec>,
    /// Whether HITL approval is required before executing actions.
    #[serde(default)]
    pub requires_approval: bool,
    /// Rate limit: max requests per minute (0 = unlimited).
    #[serde(default)]
    pub rate_limit_rpm: u64,
}

/// Describes a callable action on an integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionSpec {
    /// Action identifier (e.g., "send_email", "create_issue").
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// HTTP method for the API call.
    pub method: String,
    /// URL template with `{token}` placeholder for bearer auth.
    pub url_template: String,
    /// JSON schema for the request body (as serde_json::Value).
    #[serde(default)]
    pub body_schema: Option<serde_json::Value>,
    /// Whether this specific action requires HITL approval.
    #[serde(default)]
    pub requires_approval: bool,
}

/// Registry of all loaded integration specs, keyed by slug.
#[derive(Debug, Clone)]
pub struct SpecRegistry {
    specs: HashMap<String, IntegrationSpec>,
}

impl SpecRegistry {
    /// Load all `.yaml` / `.yml` spec files from a directory.
    pub fn load_dir(dir: &Path) -> Result<Self> {
        let mut specs = HashMap::new();

        if !dir.exists() {
            return Ok(Self { specs });
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "yaml" && ext != "yml" {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            let spec: IntegrationSpec = serde_yaml::from_str(&content)
                .map_err(IntegrationError::YamlParse)?;

            specs.insert(spec.slug.clone(), spec);
        }

        Ok(Self { specs })
    }

    /// Create a registry from a pre-built map (useful for testing / embedded specs).
    pub fn from_map(specs: HashMap<String, IntegrationSpec>) -> Self {
        Self { specs }
    }

    pub fn get(&self, slug: &str) -> Option<&IntegrationSpec> {
        self.specs.get(slug)
    }

    pub fn list(&self) -> Vec<&IntegrationSpec> {
        self.specs.values().collect()
    }

    pub fn slugs(&self) -> Vec<&str> {
        self.specs.keys().map(|k| k.as_str()).collect()
    }
}

/// Convenience function: load all specs from a directory and return them as a Vec.
pub fn load_specs(dir: &Path) -> Result<Vec<IntegrationSpec>> {
    let registry = SpecRegistry::load_dir(dir)?;
    Ok(registry.list().into_iter().cloned().collect())
}