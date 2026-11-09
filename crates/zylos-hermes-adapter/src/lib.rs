//! E.3 — Integração Hermes Real: adapter para 65 perfis Hermes.
//!
//! Implementa o bridge entre zylos-next e o ecossistema Hermes,
//! expondo skills, memory, HITL e orchestration via MCP HTTP.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A mapping between a Hermes profile and a zylos agent.
///
/// Each Hermes profile corresponds to exactly one zylos agent within a tenant,
/// carrying its own capability set, model configuration, and governance flags.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesProfileMapping {
    /// Unique identifier of the Hermes profile (e.g. `"researcher-a"`).
    pub hermes_profile_id: String,
    /// Corresponding zylos agent identifier (e.g. `"agent-researcher-a"`).
    pub zylos_agent_id: String,
    /// Logical tenant that groups related profiles (e.g. `"research"`, `"development"`).
    pub tenant: String,
    /// List of capabilities enabled for this profile (e.g. `"mcp"`, `"skills"`, `"memory"`).
    pub capabilities: Vec<String>,
    /// LLM provider name (e.g. `"anthropic"`, `"openai"`).
    pub model_provider: String,
    /// Specific model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model_name: String,
    /// Maximum number of tasks this profile can execute concurrently.
    pub max_concurrent_tasks: u32,
    /// Whether human-in-the-loop approval is required before task execution.
    pub approval_required: bool,
    /// Whether equivalence hashing is enabled for output deduplication.
    pub equivalence_hash_enabled: bool,
}

/// Bridge adapter between zylos-next and the Hermes agent runtime.
///
/// Manages profile mappings, MCP endpoint configuration, and authentication
/// for exposing zylos capabilities (skills, memory, HITL) to Hermes agents.
pub struct HermesAdapter {
    /// Registered profile mappings keyed by Hermes profile ID.
    profiles: HashMap<String, HermesProfileMapping>,
    /// Base URL of the MCP HTTP server.
    mcp_endpoint: String,
    /// Bearer token used to authenticate MCP requests.
    auth_token: String,
}

impl HermesAdapter {
    /// Creates a new adapter instance with the given MCP endpoint and auth token.
    ///
    /// The adapter starts with no registered profiles; call [`register_profile`]
    /// or [`load_default_profiles`] before exposing skills.
    pub fn new(mcp_endpoint: String, auth_token: String) -> Self {
        Self {
            profiles: HashMap::new(),
            mcp_endpoint,
            auth_token,
        }
    }

    /// Registers a single Hermes profile mapping in the adapter.
    ///
    /// If a profile with the same `hermes_profile_id` already exists, it is replaced.
    pub fn register_profile(&mut self, mapping: HermesProfileMapping) {
        self.profiles.insert(mapping.hermes_profile_id.clone(), mapping);
    }

    /// Loads the 65 built-in default Hermes profile mappings.
    ///
    /// Each profile is created with sensible defaults: Anthropic Claude Sonnet model,
    /// 5 max concurrent tasks, equivalence hashing enabled, and approval required
    /// only for `"approval"` and `"deployment"` tenants.
    pub fn load_default_profiles(&mut self) {
        let default_profiles = vec![
            ("default", "agent-default", "system"),
            ("researcher-a", "agent-researcher-a", "research"),
            ("researcher-b", "agent-researcher-b", "research"),
            ("coder-alpha", "agent-coder-alpha", "development"),
            ("coder-beta", "agent-coder-beta", "development"),
            ("reviewer", "agent-reviewer", "quality"),
            ("writer", "agent-writer", "documentation"),
            ("analyst", "agent-analyst", "analysis"),
            ("devops", "agent-devops", "infrastructure"),
            ("security", "agent-security", "security"),
            ("data-eng", "agent-data-eng", "data"),
            ("ml-ops", "agent-ml-ops", "ml"),
            ("qa-auto", "agent-qa-auto", "testing"),
            ("pm-assistant", "agent-pm-assistant", "management"),
            ("designer", "agent-designer", "ux"),
            ("translator-pt", "agent-translator-pt", "localization"),
            ("translator-en", "agent-translator-en", "localization"),
            ("support-l1", "agent-support-l1", "support"),
            ("support-l2", "agent-support-l2", "support"),
            ("incident-responder", "agent-incident-responder", "operations"),
            ("soar-dispatcher", "agent-soar-dispatcher", "orchestration"),
            ("memory-consolidator", "agent-memory-consolidator", "memory"),
            ("sentinel-scanner", "agent-sentinel-scanner", "monitoring"),
            ("backup-operator", "agent-backup-operator", "operations"),
            ("deploy-canary", "agent-deploy-canary", "deployment"),
            ("audit-adversarial", "agent-audit-adversarial", "compliance"),
            ("audit-isolated", "agent-audit-isolated", "compliance"),
            ("triage-stage1", "agent-triage-stage1", "triage"),
            ("triage-stage2", "agent-triage-stage2", "triage"),
            ("hitl-approver", "agent-hitl-approver", "approval"),
            ("equivalence-hasher", "agent-equivalence-hasher", "optimization"),
            ("proof-verifier", "agent-proof-verifier", "verification"),
            ("panel-judge", "agent-panel-judge", "adversarial"),
            ("sandbox-wasm", "agent-sandbox-wasm", "isolation"),
            ("sandbox-docker", "agent-sandbox-docker", "isolation"),
            ("skill-evaluator", "agent-skill-evaluator", "skills"),
            ("cockpit-streamer", "agent-cockpit-streamer", "ui"),
            ("roster-manager", "agent-roster-manager", "ui"),
            ("dag-orchestrator", "agent-dag-orchestrator", "orchestration"),
            ("checkpoint-saver", "agent-checkpoint-saver", "persistence"),
            ("lease-manager", "agent-lease-manager", "durability"),
            ("maintenance-loop", "agent-maintenance-loop", "recovery"),
            ("event-router", "agent-event-router", "routing"),
            ("outcome-classifier", "agent-outcome-classifier", "classification"),
            ("secret-masker", "agent-secret-masker", "security"),
            ("pii-redactor", "agent-pii-redactor", "privacy"),
            ("otel-exporter", "agent-otel-exporter", "observability"),
            ("benchmark-runner", "agent-benchmark-runner", "performance"),
            ("doc-generator", "agent-doc-generator", "documentation"),
            ("api-validator", "agent-api-validator", "testing"),
            ("schema-migrator", "agent-schema-migrator", "database"),
            ("config-hotloader", "agent-config-hotloader", "configuration"),
            ("health-checker", "agent-health-checker", "monitoring"),
            ("rate-limiter", "agent-rate-limiter", "governance"),
            ("circuit-breaker", "agent-circuit-breaker", "resilience"),
            ("retry-handler", "agent-retry-handler", "resilience"),
            ("backoff-jitter", "agent-backoff-jitter", "resilience"),
            ("dedup-detector", "agent-dedup-detector", "optimization"),
            ("cache-invalidator", "agent-cache-invalidator", "performance"),
            ("queue-processor", "agent-queue-processor", "async"),
            ("webhook-sender", "agent-webhook-sender", "integration"),
            ("slack-notifier", "agent-slack-notifier", "notification"),
            ("discord-bot", "agent-discord-bot", "notification"),
            ("email-sender", "agent-email-sender", "notification"),
            ("sms-gateway", "agent-sms-gateway", "notification"),
        ];

        for (hermes_id, zylos_id, tenant) in default_profiles {
            self.register_profile(HermesProfileMapping {
                hermes_profile_id: hermes_id.to_string(),
                zylos_agent_id: zylos_id.to_string(),
                tenant: tenant.to_string(),
                capabilities: vec!["mcp".to_string(), "skills".to_string(), "memory".to_string()],
                model_provider: "anthropic".to_string(),
                model_name: "claude-sonnet-4-20250514".to_string(),
                max_concurrent_tasks: 5,
                approval_required: tenant == "approval" || tenant == "deployment",
                equivalence_hash_enabled: true,
            });
        }
    }

    /// Builds an MCP server configuration exposing zylos skills, memory, and HITL tools.
    ///
    /// Returns a [`McpServerConfig`] with four pre-defined tools:
    /// `zylos_skill_search`, `zylos_skill_execute`, `zylos_memory_query`, and `zylos_hitl_request`.
    ///
    /// # Errors
    ///
    /// Returns [`HermesAdapterError::NoProfilesRegistered`] if no profiles have been loaded.
    pub async fn expose_skills_via_mcp(&self) -> Result<McpServerConfig, HermesAdapterError> {
        if self.profiles.is_empty() {
            return Err(HermesAdapterError::NoProfilesRegistered);
        }

        Ok(McpServerConfig {
            endpoint: format!("{}/mcp/v1/skills", self.mcp_endpoint),
            auth_header: format!("Bearer {}", self.auth_token),
            tools: vec![
                McpTool {
                    name: "zylos_skill_search".to_string(),
                    description: "Search zylos skill registry by keyword or tag".to_string(),
                    input_schema: r#"{"type":"object","properties":{"query":{"type":"string"},"limit":{"type":"integer"}}}"#.to_string(),
                },
                McpTool {
                    name: "zylos_skill_execute".to_string(),
                    description: "Execute a registered zylos skill with given parameters".to_string(),
                    input_schema: r#"{"type":"object","properties":{"skill_id":{"type":"string"},"params":{"type":"object"}},"required":["skill_id"]}"#.to_string(),
                },
                McpTool {
                    name: "zylos_memory_query".to_string(),
                    description: "Query consolidated memory across all agents".to_string(),
                    input_schema: r#"{"type":"object","properties":{"scope":{"type":"string"},"key":{"type":"string"}}}"#.to_string(),
                },
                McpTool {
                    name: "zylos_hitl_request".to_string(),
                    description: "Submit a human-in-the-loop approval request".to_string(),
                    input_schema: r#"{"type":"object","properties":{"task_id":{"type":"string"},"summary":{"type":"string"},"risk_level":{"type":"string"}},"required":["task_id","summary"]}"#.to_string(),
                },
            ],
        })
    }

    /// Validates that at least 65 profiles are registered in the adapter.
    ///
    /// # Errors
    ///
    /// Returns [`HermesAdapterError::InsufficientProfiles`] when fewer than 65
    /// profiles have been registered.
    pub fn validate_profile_count(&self) -> Result<(), HermesAdapterError> {
        if self.profiles.len() < 65 {
            return Err(HermesAdapterError::InsufficientProfiles {
                expected: 65,
                actual: self.profiles.len(),
            });
        }
        Ok(())
    }

    /// Computes aggregate integration metrics across all registered profiles.
    pub fn integration_metrics(&self) -> IntegrationMetrics {
        let approval_required_count = self.profiles.values()
            .filter(|p| p.approval_required)
            .count();
        let equivalence_enabled_count = self.profiles.values()
            .filter(|p| p.equivalence_hash_enabled)
            .count();

        IntegrationMetrics {
            total_profiles: self.profiles.len(),
            approval_required_profiles: approval_required_count,
            equivalence_hash_profiles: equivalence_enabled_count,
            unique_tenants: self.profiles.values()
                .map(|p| p.tenant.clone())
                .collect::<std::collections::HashSet<_>>()
                .len(),
        }
    }
}

/// Configuration for an MCP HTTP server exposing zylos tools.
#[derive(Debug, Serialize)]
pub struct McpServerConfig {
    /// Full URL of the MCP skills endpoint.
    pub endpoint: String,
    /// Authorization header value (e.g. `"Bearer <token>"`).
    pub auth_header: String,
    /// List of MCP tools exposed by this server.
    pub tools: Vec<McpTool>,
}

/// A single tool definition in the MCP protocol.
#[derive(Debug, Serialize)]
pub struct McpTool {
    /// Unique tool name (e.g. `"zylos_skill_search"`).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema string describing the tool's input parameters.
    pub input_schema: String,
}

/// Aggregate metrics about the adapter's registered profiles.
#[derive(Debug, Serialize)]
pub struct IntegrationMetrics {
    /// Total number of registered profiles.
    pub total_profiles: usize,
    /// Number of profiles that require human-in-the-loop approval.
    pub approval_required_profiles: usize,
    /// Number of profiles with equivalence hashing enabled.
    pub equivalence_hash_profiles: usize,
    /// Number of distinct tenants across all profiles.
    pub unique_tenants: usize,
}

/// Errors produced by [`HermesAdapter`] operations.
#[derive(Debug, thiserror::Error)]
pub enum HermesAdapterError {
    /// Returned when an operation requires at least one profile but none are registered.
    #[error("No profiles registered in adapter")]
    NoProfilesRegistered,
    /// Returned when the profile count is below the expected minimum of 65.
    #[error("Insufficient profiles: expected {expected}, got {actual}")]
    InsufficientProfiles {
        /// Expected minimum number of profiles.
        expected: usize,
        /// Actual number of registered profiles.
        actual: usize,
    },
    /// Returned when the MCP HTTP connection cannot be established.
    #[error("MCP connection failed: {0}")]
    McpConnectionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_default_profiles_65() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        assert_eq!(adapter.profiles.len(), 65, "Should load exactly 65 default profiles");
        assert!(adapter.validate_profile_count().is_ok());
    }

    #[test]
    fn test_integration_metrics() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        let metrics = adapter.integration_metrics();
        assert_eq!(metrics.total_profiles, 65);
        assert!(metrics.approval_required_profiles > 0, "Some profiles should require approval");
        assert!(metrics.unique_tenants > 10, "Should have diverse tenants");
    }

    #[tokio::test]
    async fn test_expose_skills_via_mcp() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        let config = adapter.expose_skills_via_mcp().await.expect("should expose MCP config");
        assert_eq!(config.tools.len(), 4, "Should expose 4 MCP tools");
        assert!(config.endpoint.contains("/mcp/v1/skills"));
    }

    #[test]
    fn test_validate_empty_adapter_fails() {
        let adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );

        assert!(adapter.validate_profile_count().is_err());
    }

    #[test]
    fn test_register_single_profile() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );

        let mapping = HermesProfileMapping {
            hermes_profile_id: "custom-profile".to_string(),
            zylos_agent_id: "agent-custom".to_string(),
            tenant: "custom".to_string(),
            capabilities: vec!["mcp".to_string()],
            model_provider: "openai".to_string(),
            model_name: "gpt-4o".to_string(),
            max_concurrent_tasks: 10,
            approval_required: false,
            equivalence_hash_enabled: false,
        };

        adapter.register_profile(mapping);
        assert_eq!(adapter.profiles.len(), 1);
        assert!(adapter.profiles.contains_key("custom-profile"));
    }

    #[test]
    fn test_register_profile_overwrites_existing() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        let original_count = adapter.profiles.len();
        let mapping = HermesProfileMapping {
            hermes_profile_id: "default".to_string(),
            zylos_agent_id: "agent-default-v2".to_string(),
            tenant: "system".to_string(),
            capabilities: vec!["mcp".to_string(), "skills".to_string()],
            model_provider: "openai".to_string(),
            model_name: "gpt-4o".to_string(),
            max_concurrent_tasks: 20,
            approval_required: true,
            equivalence_hash_enabled: false,
        };

        adapter.register_profile(mapping);
        assert_eq!(adapter.profiles.len(), original_count, "Overwrite should not change count");
        assert_eq!(adapter.profiles["default"].zylos_agent_id, "agent-default-v2");
        assert_eq!(adapter.profiles["default"].max_concurrent_tasks, 20);
    }

    #[tokio::test]
    async fn test_expose_skills_empty_fails() {
        let adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );

        let result = adapter.expose_skills_via_mcp().await;
        assert!(result.is_err());
        match result.unwrap_err() {
            HermesAdapterError::NoProfilesRegistered => {}
            other => panic!("Expected NoProfilesRegistered, got {:?}", other),
        }
    }

    #[test]
    fn test_integration_metrics_empty() {
        let adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );

        let metrics = adapter.integration_metrics();
        assert_eq!(metrics.total_profiles, 0);
        assert_eq!(metrics.approval_required_profiles, 0);
        assert_eq!(metrics.equivalence_hash_profiles, 0);
        assert_eq!(metrics.unique_tenants, 0);
    }

    #[test]
    fn test_default_profiles_approval_flags() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        // approval and deployment tenants require approval
        assert!(adapter.profiles["hitl-approver"].approval_required);
        assert!(adapter.profiles["deploy-canary"].approval_required);
        // other tenants do not
        assert!(!adapter.profiles["default"].approval_required);
        assert!(!adapter.profiles["researcher-a"].approval_required);
    }

    #[test]
    fn test_mcp_config_auth_header_format() {
        let mut adapter = HermesAdapter::new(
            "http://example.com:9090".to_string(),
            "my-secret-token".to_string(),
        );
        adapter.load_default_profiles();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = rt.block_on(adapter.expose_skills_via_mcp()).unwrap();
        assert_eq!(config.auth_header, "Bearer my-secret-token");
        assert_eq!(config.endpoint, "http://example.com:9090/mcp/v1/skills");
    }

    #[test]
    fn test_mcp_tool_names() {
        let mut adapter = HermesAdapter::new(
            "http://localhost:8080".to_string(),
            "test-token".to_string(),
        );
        adapter.load_default_profiles();

        let rt = tokio::runtime::Runtime::new().unwrap();
        let config = rt.block_on(adapter.expose_skills_via_mcp()).unwrap();
        let names: Vec<&str> = config.tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"zylos_skill_search"));
        assert!(names.contains(&"zylos_skill_execute"));
        assert!(names.contains(&"zylos_memory_query"));
        assert!(names.contains(&"zylos_hitl_request"));
    }

    #[test]
    fn test_error_display_messages() {
        let err = HermesAdapterError::NoProfilesRegistered;
        assert_eq!(format!("{}", err), "No profiles registered in adapter");

        let err = HermesAdapterError::InsufficientProfiles { expected: 65, actual: 10 };
        assert_eq!(format!("{}", err), "Insufficient profiles: expected 65, got 10");

        let err = HermesAdapterError::McpConnectionFailed("timeout".to_string());
        assert_eq!(format!("{}", err), "MCP connection failed: timeout");
    }
}