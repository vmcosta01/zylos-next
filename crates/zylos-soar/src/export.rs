//! Multi-Adapter Export for SOAR playbooks and runbooks.
//!
//! Exports SOAR artifacts to formats consumable by external agent ecosystems:
//! - `.cursor/rules` — Cursor IDE rules
//! - `.clinerules` — Cline/Codex rules
//! - `.claude-plugin` — Claude Code plugin manifest
//!
//! Adapted from `zylos-skills/src/export.rs` pattern, specialized for
//! playbook state machines and runbook execution contracts.

use crate::playbook::Playbook;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Export system errors.
#[derive(Error, Debug)]
/// Enumeração SoarExportError.
pub enum SoarExportError {
    #[error("playbook '{0}' has no states to export")]
    // Variante EmptyPlaybook.
    EmptyPlaybook(String),

    #[error("unsupported export format: '{0}'")]
    // Variante UnsupportedFormat.
    UnsupportedFormat(String),

    #[error("I/O error: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    // Variante JsonError.
    JsonError(#[from] serde_json::Error),

    #[error("YAML serialization error: {0}")]
    // Variante YamlError.
    YamlError(#[from] serde_yaml::Error),

    #[error("output directory does not exist: {0}")]
    // Variante OutputDirMissing.
    OutputDirMissing(String),
}

/// Alias de tipo ExportResult.
pub type ExportResult<T> = Result<T, SoarExportError>;

/// Supported export formats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Enumeração SoarExportFormat.
pub enum SoarExportFormat {
    /// .cursor/rules — Cursor IDE format
    CursorRules,
    /// .clinerules — Cline/Codex format
    ClineRules,
    /// .claude-plugin — Claude Code plugin format
    ClaudePlugin,
}

impl SoarExportFormat {
    /// Função extension.
    pub fn extension(&self) -> &'static str {
        match self {
            SoarExportFormat::CursorRules => "mdc",
            SoarExportFormat::ClineRules => "json",
            SoarExportFormat::ClaudePlugin => "json",
        }
    }

    /// Função dir name.
    pub fn dir_name(&self) -> &'static str {
        match self {
            SoarExportFormat::CursorRules => ".cursor/rules",
            SoarExportFormat::ClineRules => ".clinerules",
            SoarExportFormat::ClaudePlugin => ".claude-plugin",
        }
    }
}

/// Exportable representation of a SOAR playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExportablePlaybook.
pub struct ExportablePlaybook {
    /// Campo name.
    pub name: String,
    /// Campo version.
    pub version: String,
    /// Campo description.
    pub description: String,
    /// Campo triggers.
    pub triggers: Vec<String>,
    /// Campo roles.
    pub roles: HashMap<String, String>,
    /// Campo states.
    pub states: Vec<ExportableState>,
    /// Campo terminal states.
    pub terminal_states: Vec<String>,
}

/// Exportable representation of a playbook state.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExportableState.
pub struct ExportableState {
    /// Campo name.
    pub name: String,
    /// Campo auto.
    pub auto: bool,
    /// Campo approval required.
    pub approval_required: bool,
    /// Campo runbook.
    pub runbook: Option<String>,
    /// Campo timeout minutes.
    pub timeout_minutes: Option<u32>,
    /// Campo escalation.
    pub escalation: Option<String>,
    /// Campo transitions.
    pub transitions: Vec<ExportableTransition>,
    /// Campo actions.
    pub actions: Vec<String>,
    /// Campo terminal.
    pub terminal: bool,
}

/// Exportable transition between states.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExportableTransition.
pub struct ExportableTransition {
    /// Campo condition.
    pub condition: Option<String>,
    /// Campo to.
    pub to: String,
}

/// Result of an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura SoarExportReport.
pub struct SoarExportReport {
    /// Campo format.
    pub format: SoarExportFormat,
    /// Campo playbooks exported.
    pub playbooks_exported: usize,
    /// Campo total states.
    pub total_states: usize,
    /// Campo output path.
    pub output_path: PathBuf,
    /// Campo errors.
    pub errors: Vec<String>,
}

/// Converts a `Playbook` into an `ExportablePlaybook`.
impl From<&Playbook> for ExportablePlaybook {
    fn from(pb: &Playbook) -> Self {
        let mut roles = HashMap::new();
        if let Some(ref ic) = pb.roles.incident_commander {
            roles.insert("incident_commander".to_string(), ic.clone());
        }
        if let Some(ref ex) = pb.roles.executor {
            roles.insert("executor".to_string(), ex.clone());
        }
        if let Some(ref ap) = pb.roles.approver {
            roles.insert("approver".to_string(), ap.clone());
        }

        let states: Vec<ExportableState> = pb
            .states
            .iter()
            .map(|s| ExportableState {
                name: s.name.clone(),
                auto: s.auto,
                approval_required: s.approval_required,
                runbook: s.runbook.clone(),
                timeout_minutes: s.timeout_minutes,
                escalation: s.escalation.clone(),
                transitions: s
                    .next
                    .iter()
                    .map(|t| ExportableTransition {
                        condition: t.condition.clone(),
                        to: t.to.clone(),
                    })
                    .collect(),
                actions: s.actions.clone(),
                terminal: s.terminal,
            })
            .collect();

        let terminal_states: Vec<String> = states
            .iter()
            .filter(|s| s.terminal)
            .map(|s| s.name.clone())
            .collect();

        let triggers: Vec<String> = pb
            .triggers
            .iter()
            .map(|t| t.expression.clone())
            .collect();

        let description = format!(
            "SOAR Playbook '{}': {} states, {} triggers, terminal states: [{}]",
            pb.name,
            states.len(),
            triggers.len(),
            terminal_states.join(", ")
        );

        // Variante ExportablePlaybook.
        ExportablePlaybook {
            name: pb.name.clone(),
            version: pb.version.clone(),
            description,
            triggers,
            roles,
            states,
            terminal_states,
        }
    }
}

/// Main exporter — orchestrates conversion and file writing.
pub struct SoarExporter {
    playbooks: Vec<ExportablePlaybook>,
}

impl SoarExporter {
    /// Função new.
    pub fn new(playbooks: Vec<ExportablePlaybook>) -> Self {
        // Variante Self.
        Self { playbooks }
    }

    /// Create from raw `Playbook` references.
    pub fn from_playbooks(playbooks: &[Playbook]) -> Self {
        let exportable: Vec<ExportablePlaybook> =
            playbooks.iter().map(ExportablePlaybook::from).collect();
        Self::new(exportable)
    }

    /// Export all playbooks to the specified format.
    pub fn export(
        &self,
        format: &SoarExportFormat,
        output_dir: &Path,
    ) -> ExportResult<SoarExportReport> {
        if !output_dir.exists() {
            return Err(SoarExportError::OutputDirMissing(
                output_dir.display().to_string(),
            ));
        }

        let mut errors = Vec::new();
        let mut exported = 0usize;
        let mut total_states = 0usize;

        match format {
            SoarExportFormat::CursorRules => {
                for pb in &self.playbooks {
                    match self.export_cursor_rule(pb, output_dir) {
                        // Variante Ok.
                        Ok(()) => {
                            exported += 1;
                            total_states += pb.states.len();
                        }
                        // Variante Err.
                        Err(e) => errors.push(format!("{}: {}", pb.name, e)),
                    }
                }
            }
            SoarExportFormat::ClineRules => {
                match self.export_cline_rules(output_dir) {
                    // Variante Ok.
                    Ok(()) => {
                        exported = self.playbooks.len();
                        total_states = self.playbooks.iter().map(|p| p.states.len()).sum();
                    }
                    // Variante Err.
                    Err(e) => errors.push(e.to_string()),
                }
            }
            SoarExportFormat::ClaudePlugin => {
                match self.export_claude_plugin(output_dir) {
                    // Variante Ok.
                    Ok(()) => {
                        exported = self.playbooks.len();
                        total_states = self.playbooks.iter().map(|p| p.states.len()).sum();
                    }
                    // Variante Err.
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }

        // Variante Ok.
        Ok(SoarExportReport {
            format: format.clone(),
            playbooks_exported: exported,
            total_states,
            output_path: output_dir.to_path_buf(),
            errors,
        })
    }

    // ── Cursor Rules (.cursor/rules/*.mdc) ──────────────────────────

    fn export_cursor_rule(
        &self,
        pb: &ExportablePlaybook,
        output_dir: &Path,
    ) -> ExportResult<()> {
        let rules_dir = output_dir.join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir)?;

        let filename = format!("soar-playbook-{}.mdc", sanitize_filename(&pb.name));
        let path = rules_dir.join(filename);

        let triggers = pb
            .triggers
            .iter()
            .map(|t| format!("- `{}`", t))
            .collect::<Vec<_>>()
            .join("\n");

        let roles = pb
            .roles
            .iter()
            .map(|(k, v)| format!("- **{}**: `{}`", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let states_doc = pb
            .states
            .iter()
            .map(|s| {
                let mut lines = vec![format!("### State: `{}`", s.name)];
                if s.auto {
                    lines.push("- Auto-execution: yes".to_string());
                }
                if s.approval_required {
                    lines.push("- Approval required: yes".to_string());
                }
                if let Some(ref rb) = s.runbook {
                    lines.push(format!("- Runbook: `{}`", rb));
                }
                if let Some(timeout) = s.timeout_minutes {
                    lines.push(format!("- Timeout: {} minutes", timeout));
                }
                if let Some(ref esc) = s.escalation {
                    lines.push(format!("- Escalation: `{}`", esc));
                }
                if s.terminal {
                    lines.push("- **Terminal state**".to_string());
                }
                if !s.transitions.is_empty() {
                    lines.push("- Transitions:".to_string());
                    for t in &s.transitions {
                        match &t.condition {
                            // Variante Some.
                            Some(cond) => lines.push(format!("  - → `{}` (if `{}`)", t.to, cond)),
                            // Variante None.
                            None => lines.push(format!("  - → `{}`", t.to)),
                        }
                    }
                }
                if !s.actions.is_empty() {
                    lines.push(format!("- Actions: {}", s.actions.join(", ")));
                }
                lines.join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let mut tag_list: Vec<&str> = vec!["soar", "playbook", "incident-response"];
        for k in pb.roles.keys() {
            tag_list.push(k.as_str());
        }
        let tags = tag_list.join(", ");

        let content = format!(
            r#"---
description: {description}
globs:
alwaysApply: false
tags: [{tags}]
---

# SOAR Playbook: {name}

**Version:** {version}

## Description

{description}

## Triggers

{triggers}

## Roles

{roles}

## State Machine

{states_doc}

## Terminal States

{terminal}
"#,
            description = pb.description,
            tags = tags,
            name = pb.name,
            version = pb.version,
            triggers = triggers,
            roles = roles,
            states_doc = states_doc,
            terminal = pb.terminal_states.join(", "),
        );

        std::fs::write(path, content)?;
        // Variante Ok.
        Ok(())
    }

    // ── Cline Rules (.clinerules/soar-rules.json) ───────────────────

    fn export_cline_rules(&self, output_dir: &Path) -> ExportResult<()> {
        let rules_dir = output_dir.join(".clinerules");
        std::fs::create_dir_all(&rules_dir)?;

        let rules: Vec<ClineSoarRule> = self
            .playbooks
            .iter()
            .map(|pb| ClineSoarRule {
                id: format!("soar-playbook-{}", pb.name),
                name: pb.name.clone(),
                description: pb.description.clone(),
                triggers: pb.triggers.clone(),
                roles: pb.roles.clone(),
                states: pb.states.clone(),
                terminal_states: pb.terminal_states.clone(),
                version: pb.version.clone(),
            })
            .collect();

        let wrapper = ClineSoarRulesFile {
            version: "1.0".to_string(),
            generated_by: "zylos-soar-exporter".to_string(),
            rules,
        };

        let json = serde_json::to_string_pretty(&wrapper)?;
        std::fs::write(rules_dir.join("soar-rules.json"), json)?;
        // Variante Ok.
        Ok(())
    }

    // ── Claude Plugin (.claude-plugin/manifest.json + playbooks/) ───

    fn export_claude_plugin(&self, output_dir: &Path) -> ExportResult<()> {
        let plugin_dir = output_dir.join(".claude-plugin");
        let playbooks_dir = plugin_dir.join("playbooks");
        std::fs::create_dir_all(&playbooks_dir)?;

        let mut manifest_playbooks = Vec::new();
        for pb in &self.playbooks {
            let pb_file = format!("{}.json", sanitize_filename(&pb.name));
            let pb_content = serde_json::to_string_pretty(&ClaudeSoarPlaybookEntry {
                name: pb.name.clone(),
                version: pb.version.clone(),
                description: pb.description.clone(),
                triggers: pb.triggers.clone(),
                roles: pb.roles.clone(),
                states: pb.states.clone(),
                terminal_states: pb.terminal_states.clone(),
            })?;
            std::fs::write(playbooks_dir.join(&pb_file), pb_content)?;
            manifest_playbooks.push(ClaudeManifestPlaybookRef {
                name: pb.name.clone(),
                file: format!("playbooks/{}", pb_file),
            });
        }

        let manifest = ClaudeSoarPluginManifest {
            name: "zylos-soar-playbooks".to_string(),
            version: "0.1.0".to_string(),
            description: "SOAR playbooks exported from Zylos-Next".to_string(),
            author: "zylos-next".to_string(),
            playbooks: manifest_playbooks,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(plugin_dir.join("manifest.json"), manifest_json)?;
        // Variante Ok.
        Ok(())
    }
}

// ── Internal serialization types ────────────────────────────────────

#[derive(Serialize)]
struct ClineSoarRule {
    id: String,
    name: String,
    description: String,
    triggers: Vec<String>,
    roles: HashMap<String, String>,
    states: Vec<ExportableState>,
    terminal_states: Vec<String>,
    version: String,
}

#[derive(Serialize)]
struct ClineSoarRulesFile {
    version: String,
    generated_by: String,
    rules: Vec<ClineSoarRule>,
}

#[derive(Serialize)]
struct ClaudeSoarPlaybookEntry {
    name: String,
    version: String,
    description: String,
    triggers: Vec<String>,
    roles: HashMap<String, String>,
    states: Vec<ExportableState>,
    terminal_states: Vec<String>,
}

#[derive(Serialize)]
struct ClaudeManifestPlaybookRef {
    name: String,
    file: String,
}

#[derive(Serialize)]
struct ClaudeSoarPluginManifest {
    name: String,
    version: String,
    description: String,
    author: String,
    playbooks: Vec<ClaudeManifestPlaybookRef>,
}

/// Sanitize filename removing unsafe characters.
fn sanitize_filename(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playbook::{Playbook, PlaybookRoles, PlaybookState, PlaybookTrigger, Transition};
    use std::fs;

    fn sample_playbook() -> Playbook {
        // Variante Playbook.
        Playbook {
            name: "security-incident-response".to_string(),
            version: "1.0".to_string(),
            triggers: vec![PlaybookTrigger {
                expression: "event.category == \"Breach\" && event.severity == \"Critical\""
                    .to_string(),
            }],
            roles: PlaybookRoles {
                incident_commander: Some("on-call-security-lead".to_string()),
                executor: Some("zylos-agent-soar".to_string()),
                approver: Some("security-manager".to_string()),
            },
            states: vec![
                // Variante PlaybookState.
                PlaybookState {
                    name: "triage".to_string(),
                    auto: true,
                    approval_required: false,
                    runbook: None,
                    timeout_minutes: None,
                    escalation: None,
                    next: vec![
                        // Variante Transition.
                        Transition {
                            condition: Some("triage.confirmed_breach == true".to_string()),
                            to: "containment".to_string(),
                        },
                        // Variante Transition.
                        Transition {
                            condition: Some("triage.false_positive == true".to_string()),
                            to: "closed".to_string(),
                        },
                    ],
                    actions: vec![],
                    terminal: false,
                },
                // Variante PlaybookState.
                PlaybookState {
                    name: "containment".to_string(),
                    auto: false,
                    approval_required: true,
                    runbook: Some("isolate-compromised-host".to_string()),
                    timeout_minutes: Some(10),
                    escalation: Some("incident-commander".to_string()),
                    next: vec![Transition {
                        condition: None,
                        to: "eradication".to_string(),
                    }],
                    actions: vec![],
                    terminal: false,
                },
                // Variante PlaybookState.
                PlaybookState {
                    name: "eradication".to_string(),
                    auto: true,
                    approval_required: false,
                    runbook: Some("remove-malware-and-restore".to_string()),
                    timeout_minutes: None,
                    escalation: None,
                    next: vec![Transition {
                        condition: None,
                        to: "recovery".to_string(),
                    }],
                    actions: vec![],
                    terminal: false,
                },
                // Variante PlaybookState.
                PlaybookState {
                    name: "recovery".to_string(),
                    auto: true,
                    approval_required: false,
                    runbook: Some("validate-system-integrity".to_string()),
                    timeout_minutes: None,
                    escalation: None,
                    next: vec![Transition {
                        condition: None,
                        to: "post_incident".to_string(),
                    }],
                    actions: vec![],
                    terminal: false,
                },
                // Variante PlaybookState.
                PlaybookState {
                    name: "post_incident".to_string(),
                    auto: true,
                    approval_required: false,
                    runbook: None,
                    timeout_minutes: None,
                    escalation: None,
                    next: vec![],
                    actions: vec![
                        "generate_postmortem".to_string(),
                        "update_metrics".to_string(),
                    ],
                    terminal: true,
                },
                // Variante PlaybookState.
                PlaybookState {
                    name: "closed".to_string(),
                    auto: true,
                    approval_required: false,
                    runbook: None,
                    timeout_minutes: None,
                    escalation: None,
                    next: vec![],
                    actions: vec![],
                    terminal: true,
                },
            ],
        }
    }

    #[test]
    fn test_export_cursor_rules() {
        let tmp = std::env::temp_dir().join("zylos-soar-test-cursor");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let pb = sample_playbook();
        let exporter = SoarExporter::from_playbooks(&[pb]);
        let report = exporter
            .export(&SoarExportFormat::CursorRules, &tmp)
            .unwrap();

        assert_eq!(report.playbooks_exported, 1);
        assert_eq!(report.total_states, 6);
        assert!(report.errors.is_empty());

        let rules_dir = tmp.join(".cursor").join("rules");
        assert!(rules_dir.join("soar-playbook-security-incident-response.mdc").exists());

        let content = fs::read_to_string(rules_dir.join("soar-playbook-security-incident-response.mdc")).unwrap();
        assert!(content.contains("SOAR Playbook: security-incident-response"));
        assert!(content.contains("triage"));
        assert!(content.contains("containment"));
        assert!(content.contains("isolate-compromised-host"));
        assert!(content.contains("Terminal state"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_cline_rules() {
        let tmp = std::env::temp_dir().join("zylos-soar-test-cline");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let pb = sample_playbook();
        let exporter = SoarExporter::from_playbooks(&[pb]);
        let report = exporter
            .export(&SoarExportFormat::ClineRules, &tmp)
            .unwrap();

        assert_eq!(report.playbooks_exported, 1);
        assert_eq!(report.total_states, 6);
        assert!(report.errors.is_empty());

        let rules_file = tmp.join(".clinerules").join("soar-rules.json");
        assert!(rules_file.exists());

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&rules_file).unwrap()).unwrap();
        assert_eq!(content["rules"].as_array().unwrap().len(), 1);
        assert_eq!(content["generated_by"], "zylos-soar-exporter");
        assert_eq!(content["rules"][0]["name"], "security-incident-response");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_claude_plugin() {
        let tmp = std::env::temp_dir().join("zylos-soar-test-claude");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let pb = sample_playbook();
        let exporter = SoarExporter::from_playbooks(&[pb]);
        let report = exporter
            .export(&SoarExportFormat::ClaudePlugin, &tmp)
            .unwrap();

        assert_eq!(report.playbooks_exported, 1);
        assert_eq!(report.total_states, 6);
        assert!(report.errors.is_empty());

        let plugin_dir = tmp.join(".claude-plugin");
        assert!(plugin_dir.join("manifest.json").exists());
        assert!(plugin_dir
            .join("playbooks")
            .join("security-incident-response.json")
            .exists());

        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(plugin_dir.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], "zylos-soar-playbooks");
        assert_eq!(manifest["playbooks"].as_array().unwrap().len(), 1);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_missing_output_dir() {
        let pb = sample_playbook();
        let exporter = SoarExporter::from_playbooks(&[pb]);
        let result = exporter.export(
            &SoarExportFormat::CursorRules,
            Path::new("/nonexistent/path/xyz"),
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SoarExportError::OutputDirMissing(_)
        ));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("my-playbook_v2"), "my-playbook_v2");
        assert_eq!(sanitize_filename("playbook/with:special"), "playbook_with_special");
        assert_eq!(sanitize_filename("normal-name"), "normal-name");
    }

    #[test]
    fn test_format_extension_and_dir() {
        assert_eq!(SoarExportFormat::CursorRules.extension(), "mdc");
        assert_eq!(SoarExportFormat::CursorRules.dir_name(), ".cursor/rules");
        assert_eq!(SoarExportFormat::ClineRules.extension(), "json");
        assert_eq!(SoarExportFormat::ClineRules.dir_name(), ".clinerules");
        assert_eq!(SoarExportFormat::ClaudePlugin.extension(), "json");
        assert_eq!(SoarExportFormat::ClaudePlugin.dir_name(), ".claude-plugin");
    }

    #[test]
    fn test_export_empty_playbooks() {
        let tmp = std::env::temp_dir().join("zylos-soar-test-empty");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let exporter = SoarExporter::new(vec![]);
        let report = exporter
            .export(&SoarExportFormat::CursorRules, &tmp)
            .unwrap();
        assert_eq!(report.playbooks_exported, 0);
        assert_eq!(report.total_states, 0);
        assert!(report.errors.is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }
}