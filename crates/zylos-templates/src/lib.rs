//! zylos-templates: Shareable agent blueprint templates.
//!
//! Provides a template schema for defining agent roles, a registry for
//! managing built-in and community templates, version conflict detection,
//! and CLI-driven installation from a remote registry.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::info;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("template not found: {0}")]
    NotFound(String),

    #[error("version conflict: installed {installed} vs requested {requested} — breaking change detected")]
    VersionConflict {
        installed: String,
        requested: String,
    },

    #[error("invalid template YAML: {0}")]
    InvalidYaml(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "registry")]
    #[error("registry fetch failed: {0}")]
    RegistryFetch(String),
}

pub type Result<T> = std::result::Result<T, TemplateError>;

// ---------------------------------------------------------------------------
// Template schema
// ---------------------------------------------------------------------------

/// Configuration overrides that can be applied when instantiating a template.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigOverrides {
    #[serde(default)]
    pub hitl_tier: Option<u8>,
    #[serde(default)]
    pub sandbox_mode: Option<String>,
    #[serde(default)]
    pub proof_level: Option<String>,
}

/// A shareable agent blueprint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentTemplate {
    /// Unique identifier (e.g. `code-reviewer`, `@community/data-analyst`).
    pub slug: String,
    /// Semantic version.
    pub version: Version,
    /// Human-readable display name.
    pub display_name: String,
    /// Description of the agent's purpose.
    pub description: String,
    /// Personality profile identifier.
    pub personality: String,
    /// Skills to attach to the agent.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Tools the agent can use.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Optional configuration overrides.
    #[serde(default)]
    pub config_overrides: Option<ConfigOverrides>,
    /// Categorisation tags.
    #[serde(default)]
    pub tags: Vec<String>,
}

impl AgentTemplate {
    /// Parse a template from YAML bytes.
    pub fn from_yaml(yaml: &[u8]) -> Result<Self> {
        serde_yaml::from_slice(yaml).map_err(|e| TemplateError::InvalidYaml(e.to_string()))
    }

    /// Serialise the template to YAML bytes.
    pub fn to_yaml(&self) -> Result<Vec<u8>> {
        serde_yaml::to_string(self)
            .map(|s| s.into_bytes())
            .map_err(|e| TemplateError::InvalidYaml(e.to_string()))
    }

    /// Compute a content hash for integrity verification.
    pub fn content_hash(&self) -> String {
        let yaml = self.to_yaml().unwrap_or_default();
        let hash = Sha256::digest(&yaml);
        hex::encode(hash)
    }

    /// Check whether upgrading from `old_version` to `self.version` is a
    /// breaking change (major version bump).
    pub fn is_breaking_upgrade_from(&self, old_version: &Version) -> bool {
        self.version.major > old_version.major
    }
}

// ---------------------------------------------------------------------------
// Instantiated agent configuration (output of template instantiation)
// ---------------------------------------------------------------------------

/// The fully-resolved agent configuration produced by instantiating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantiatedAgent {
    pub agent_id: String,
    pub slug: String,
    pub version: Version,
    pub display_name: String,
    pub personality: String,
    pub skills: Vec<String>,
    pub tools: Vec<String>,
    pub hitl_tier: u8,
    pub sandbox_mode: String,
    pub proof_level: String,
    pub instantiated_at: DateTime<Utc>,
}

impl AgentTemplate {
    /// Instantiate the template into a fully configured agent in <1s.
    pub fn instantiate(&self, agent_id: &str) -> InstantiatedAgent {
        let overrides = self.config_overrides.as_ref();
        InstantiatedAgent {
            agent_id: agent_id.to_string(),
            slug: self.slug.clone(),
            version: self.version.clone(),
            display_name: self.display_name.clone(),
            personality: self.personality.clone(),
            skills: self.skills.clone(),
            tools: self.tools.clone(),
            hitl_tier: overrides.and_then(|o| o.hitl_tier).unwrap_or(1),
            sandbox_mode: overrides
                .and_then(|o| o.sandbox_mode.clone())
                .unwrap_or_else(|| "none".to_string()),
            proof_level: overrides
                .and_then(|o| o.proof_level.clone())
                .unwrap_or_else(|| "basic".to_string()),
            instantiated_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Template registry
// ---------------------------------------------------------------------------

/// In-memory registry of installed templates.
pub struct TemplateRegistry {
    /// Installed templates keyed by slug.
    installed: HashMap<String, AgentTemplate>,
    /// Directory where community templates are stored on disk.
    install_dir: PathBuf,
}

impl TemplateRegistry {
    /// Create a new registry with the given install directory for community
    /// templates.
    pub fn new(install_dir: impl Into<PathBuf>) -> Self {
        Self {
            installed: HashMap::new(),
            install_dir: install_dir.into(),
        }
    }

    /// Load all built-in templates into the registry.
    pub fn load_builtins(&mut self) {
        for tpl in builtin_templates() {
            self.installed.insert(tpl.slug.clone(), tpl);
        }
        info!(count = self.installed.len(), "Loaded built-in templates");
    }

    /// Register a template (built-in or manually loaded).
    pub fn register(&mut self, template: AgentTemplate) {
        self.installed.insert(template.slug.clone(), template);
    }

    /// Get a template by slug.
    pub fn get(&self, slug: &str) -> Result<&AgentTemplate> {
        self.installed
            .get(slug)
            .ok_or_else(|| TemplateError::NotFound(slug.to_string()))
    }

    /// List all registered template slugs.
    pub fn list(&self) -> Vec<&str> {
        self.installed.keys().map(|s| s.as_str()).collect()
    }

    /// Install a community template from YAML bytes, checking for version
    /// conflicts. Returns the installed template.
    pub fn install_from_bytes(&mut self, yaml: &[u8]) -> Result<&AgentTemplate> {
        let template = AgentTemplate::from_yaml(yaml)?;

        // Check for breaking version conflict
        if let Some(existing) = self.installed.get(&template.slug) {
            if template.is_breaking_upgrade_from(&existing.version) {
                return Err(TemplateError::VersionConflict {
                    installed: existing.version.to_string(),
                    requested: template.version.to_string(),
                });
            }
        }

        // Persist to disk
        std::fs::create_dir_all(&self.install_dir)?;
        let filename = format!("{}.yaml", template.slug.replace('/', "_"));
        let path = self.install_dir.join(filename);
        std::fs::write(&path, yaml)?;

        let slug = template.slug.clone();
        self.installed.insert(slug.clone(), template);
        Ok(self.installed.get(&slug).unwrap())
    }

    /// Preview a template without installing it.
    pub fn preview(yaml: &[u8]) -> Result<AgentTemplate> {
        AgentTemplate::from_yaml(yaml)
    }

    /// Number of registered templates.
    pub fn count(&self) -> usize {
        self.installed.len()
    }
}

// ---------------------------------------------------------------------------
// Built-in templates (15 across engineering, ops, research, creative)
// ---------------------------------------------------------------------------

fn builtin_templates() -> Vec<AgentTemplate> {
    vec![
        // Engineering
        make_template(
            "code-reviewer",
            "1.0.0",
            "Code Reviewer",
            "Reviews PRs for security, performance, and style compliance",
            "senior-backend-engineer",
            &["github-pr-review", "security-scan", "performance-analysis"],
            &["github", "sonarqube", "semgrep"],
            Some(ConfigOverrides {
                hitl_tier: Some(2),
                sandbox_mode: Some("docker".into()),
                proof_level: Some("confirmed".into()),
            }),
            &["engineering", "security", "quality"],
        ),
        make_template(
            "test-writer",
            "1.0.0",
            "Test Writer",
            "Generates unit and integration tests from source code",
            "qa-engineer",
            &["test-generation", "coverage-analysis"],
            &["github", "pytest", "jest"],
            None,
            &["engineering", "testing"],
        ),
        make_template(
            "refactor-agent",
            "1.0.0",
            "Refactor Agent",
            "Identifies and applies safe code refactoring patterns",
            "senior-backend-engineer",
            &["code-smell-detection", "refactoring"],
            &["github", "ast-analyzer"],
            Some(ConfigOverrides {
                hitl_tier: Some(3),
                sandbox_mode: Some("docker".into()),
                proof_level: Some("verified".into()),
            }),
            &["engineering", "maintenance"],
        ),
        make_template(
            "doc-writer",
            "1.0.0",
            "Documentation Writer",
            "Generates and maintains technical documentation",
            "technical-writer",
            &["doc-generation", "api-docs", "changelog"],
            &["github", "mdbook"],
            None,
            &["engineering", "documentation"],
        ),
        // Ops
        make_template(
            "incident-responder",
            "1.0.0",
            "Incident Responder",
            "Triages and resolves production incidents",
            "sre-on-call",
            &["log-analysis", "runbook-execution", "alert-correlation"],
            &["pagerduty", "grafana", "kubernetes"],
            Some(ConfigOverrides {
                hitl_tier: Some(3),
                sandbox_mode: Some("docker".into()),
                proof_level: Some("confirmed".into()),
            }),
            &["ops", "reliability"],
        ),
        make_template(
            "infra-provisioner",
            "1.0.0",
            "Infrastructure Provisioner",
            "Provisions cloud infrastructure from IaC templates",
            "devops-engineer",
            &["terraform-plan", "cost-estimation", "compliance-check"],
            &["terraform", "aws-cli", "gcloud"],
            Some(ConfigOverrides {
                hitl_tier: Some(4),
                sandbox_mode: Some("docker".into()),
                proof_level: Some("verified".into()),
            }),
            &["ops", "infrastructure"],
        ),
        make_template(
            "ci-debugger",
            "1.0.0",
            "CI Debugger",
            "Diagnoses and fixes CI/CD pipeline failures",
            "devops-engineer",
            &["pipeline-analysis", "log-parsing", "retry-strategy"],
            &["github-actions", "gitlab-ci"],
            None,
            &["ops", "ci-cd"],
        ),
        make_template(
            "capacity-planner",
            "1.0.0",
            "Capacity Planner",
            "Forecasts resource needs and recommends scaling actions",
            "sre-analyst",
            &["metrics-analysis", "forecasting", "cost-optimization"],
            &["prometheus", "grafana"],
            None,
            &["ops", "scaling"],
        ),
        // Research
        make_template(
            "literature-reviewer",
            "1.0.0",
            "Literature Reviewer",
            "Searches and synthesises academic papers on a topic",
            "research-analyst",
            &["arxiv-search", "paper-summarization", "citation-graph"],
            &["arxiv", "semantic-scholar"],
            None,
            &["research", "academic"],
        ),
        make_template(
            "market-researcher",
            "1.0.0",
            "Market Researcher",
            "Analyses market trends and competitive landscapes",
            "business-analyst",
            &["web-scraping", "trend-analysis", "report-generation"],
            &["browser", "spreadsheet"],
            Some(ConfigOverrides {
                hitl_tier: Some(2),
                sandbox_mode: Some("docker".into()),
                proof_level: Some("basic".into()),
            }),
            &["research", "business"],
        ),
        make_template(
            "data-analyst",
            "1.0.0",
            "Data Analyst",
            "Explores datasets and produces statistical summaries",
            "data-scientist",
            &["sql-query", "visualization", "statistical-analysis"],
            &["pandas", "matplotlib", "duckdb"],
            None,
            &["research", "data"],
        ),
        make_template(
            "patent-searcher",
            "1.0.0",
            "Patent Searcher",
            "Searches patent databases for prior art",
            "ip-analyst",
            &["patent-search", "claim-analysis", "similarity-scoring"],
            &["uspto", "google-patents"],
            None,
            &["research", "legal"],
        ),
        // Creative
        make_template(
            "content-writer",
            "1.0.0",
            "Content Writer",
            "Drafts blog posts, newsletters, and marketing copy",
            "creative-writer",
            &["copywriting", "seo-optimization", "tone-matching"],
            &["notion", "wordpress"],
            None,
            &["creative", "marketing"],
        ),
        make_template(
            "ui-designer",
            "1.0.0",
            "UI Designer",
            "Creates wireframes and design system components",
            "product-designer",
            &["wireframing", "design-system", "accessibility-audit"],
            &["figma", "storybook"],
            Some(ConfigOverrides {
                hitl_tier: Some(2),
                sandbox_mode: Some("none".into()),
                proof_level: Some("basic".into()),
            }),
            &["creative", "design"],
        ),
        make_template(
            "video-script-writer",
            "1.0.0",
            "Video Script Writer",
            "Writes scripts for tutorials, demos, and presentations",
            "creative-writer",
            &["scriptwriting", "storyboard", "timing-analysis"],
            &["notion"],
            None,
            &["creative", "video"],
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn make_template(
    slug: &str,
    version: &str,
    display_name: &str,
    description: &str,
    personality: &str,
    skills: &[&str],
    tools: &[&str],
    config_overrides: Option<ConfigOverrides>,
    tags: &[&str],
) -> AgentTemplate {
    AgentTemplate {
        slug: slug.to_string(),
        version: Version::parse(version).expect("valid semver"),
        display_name: display_name.to_string(),
        description: description.to_string(),
        personality: personality.to_string(),
        skills: skills.iter().map(|s| s.to_string()).collect(),
        tools: tools.iter().map(|s| s.to_string()).collect(),
        config_overrides,
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_count() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        assert_eq!(reg.count(), 15);
    }

    #[test]
    fn test_get_existing_template() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let tpl = reg.get("code-reviewer").unwrap();
        assert_eq!(tpl.display_name, "Code Reviewer");
        assert_eq!(tpl.personality, "senior-backend-engineer");
        assert!(tpl.skills.contains(&"github-pr-review".to_string()));
    }

    #[test]
    fn test_get_nonexistent_template() {
        let reg = TemplateRegistry::new("/tmp/test-templates");
        assert!(matches!(
            reg.get("nonexistent"),
            Err(TemplateError::NotFound(_))
        ));
    }

    #[test]
    fn test_instantiate_template() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let tpl = reg.get("code-reviewer").unwrap();
        let agent = tpl.instantiate("agent-001");
        assert_eq!(agent.agent_id, "agent-001");
        assert_eq!(agent.slug, "code-reviewer");
        assert_eq!(agent.hitl_tier, 2);
        assert_eq!(agent.sandbox_mode, "docker");
        assert_eq!(agent.proof_level, "confirmed");
    }

    #[test]
    fn test_instantiate_defaults() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let tpl = reg.get("test-writer").unwrap();
        let agent = tpl.instantiate("agent-002");
        assert_eq!(agent.hitl_tier, 1);
        assert_eq!(agent.sandbox_mode, "none");
        assert_eq!(agent.proof_level, "basic");
    }

    #[test]
    fn test_yaml_roundtrip() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let tpl = reg.get("incident-responder").unwrap();
        let yaml = tpl.to_yaml().unwrap();
        let parsed = AgentTemplate::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.slug, tpl.slug);
        assert_eq!(parsed.version, tpl.version);
        assert_eq!(parsed.skills, tpl.skills);
    }

    #[test]
    fn test_install_from_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = TemplateRegistry::new(dir.path());

        let yaml = r#"
slug: custom-agent
version: "1.0.0"
display_name: Custom Agent
description: A custom test agent
personality: general
skills: [test-skill]
tools: [test-tool]
tags: [test]
"#;
        let tpl = reg.install_from_bytes(yaml.as_bytes()).unwrap();
        assert_eq!(tpl.slug, "custom-agent");
        assert_eq!(reg.count(), 1);

        // Verify file was persisted
        let persisted = dir.path().join("custom-agent.yaml");
        assert!(persisted.exists());
    }

    #[test]
    fn test_version_conflict_detection() {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = TemplateRegistry::new(dir.path());

        let v1 = r#"
slug: my-agent
version: "1.0.0"
display_name: My Agent
description: v1
personality: general
skills: []
tools: []
tags: []
"#;
        reg.install_from_bytes(v1.as_bytes()).unwrap();

        // Minor bump should succeed
        let v1_1 = r#"
slug: my-agent
version: "1.1.0"
display_name: My Agent
description: v1.1
personality: general
skills: []
tools: []
tags: []
"#;
        assert!(reg.install_from_bytes(v1_1.as_bytes()).is_ok());

        // Major bump should fail
        let v2 = r#"
slug: my-agent
version: "2.0.0"
display_name: My Agent v2
description: v2 breaking
personality: general
skills: []
tools: []
tags: []
"#;
        let err = reg.install_from_bytes(v2.as_bytes()).unwrap_err();
        assert!(matches!(err, TemplateError::VersionConflict { .. }));
    }

    #[test]
    fn test_preview_without_install() {
        let yaml = r#"
slug: preview-agent
version: "0.1.0"
display_name: Preview Agent
description: Preview only
personality: general
skills: []
tools: []
tags: []
"#;
        let tpl = TemplateRegistry::preview(yaml.as_bytes()).unwrap();
        assert_eq!(tpl.slug, "preview-agent");
    }

    #[test]
    fn test_content_hash_deterministic() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let tpl = reg.get("data-analyst").unwrap();
        let h1 = tpl.content_hash();
        let h2 = tpl.content_hash();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_list_templates() {
        let mut reg = TemplateRegistry::new("/tmp/test-templates");
        reg.load_builtins();
        let list = reg.list();
        assert_eq!(list.len(), 15);
        assert!(list.contains(&"code-reviewer"));
        assert!(list.contains(&"video-script-writer"));
    }

    #[test]
    fn test_invalid_yaml_rejected() {
        let result = AgentTemplate::from_yaml(b"not: valid: yaml: [[[");
        assert!(matches!(result, Err(TemplateError::InvalidYaml(_))));
    }

    #[test]
    fn test_is_breaking_upgrade() {
        let tpl = AgentTemplate {
            slug: "test".into(),
            version: Version::parse("2.0.0").unwrap(),
            display_name: "Test".into(),
            description: "".into(),
            personality: "general".into(),
            skills: vec![],
            tools: vec![],
            config_overrides: None,
            tags: vec![],
        };
        assert!(tpl.is_breaking_upgrade_from(&Version::parse("1.5.0").unwrap()));
        assert!(!tpl.is_breaking_upgrade_from(&Version::parse("2.0.0").unwrap()));
        assert!(!tpl.is_breaking_upgrade_from(&Version::parse("2.1.0").unwrap()));
    }
}
