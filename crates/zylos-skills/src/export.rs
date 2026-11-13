//! Multi-Adapter Export — Task 5.3.1 (movido para Fase 6)
//!
//! Exporta skills do registry Zylos para formatos compatíveis com
//! outros ecossistemas de agentes: .cursor/rules, .clinerules, .claude-plugin.
//! Inspirado em obra/superpowers multi-adapter export.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Erros do sistema de export
#[derive(Error, Debug)]
/// Enumeração ExportError.
pub enum ExportError {
    #[error("skill '{0}' não encontrada no registry")]
    // Variante SkillNotFound.
    SkillNotFound(String),

    #[error("formato de export '{0}' não suportado")]
    // Variante UnsupportedFormat.
    UnsupportedFormat(String),

    #[error("falha de I/O: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("falha de serialização JSON: {0}")]
    // Variante JsonError.
    JsonError(#[from] serde_json::Error),

    #[error("diretório de saída não existe: {0}")]
    // Variante OutputDirMissing.
    OutputDirMissing(String),
}

/// Alias de tipo ExportResult.
pub type ExportResult<T> = Result<T, ExportError>;

/// Formatos de export suportados
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Enumeração ExportFormat.
pub enum ExportFormat {
    /// .cursor/rules — formato Cursor IDE
    CursorRules,
    /// .clinerules — formato Cline/Codex
    ClineRules,
    /// .claude-plugin — formato Claude Code plugin
    ClaudePlugin,
}

impl ExportFormat {
    /// Função extension.
    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::CursorRules => "mdc",
            ExportFormat::ClineRules => "json",
            ExportFormat::ClaudePlugin => "json",
        }
    }

    /// Função dir name.
    pub fn dir_name(&self) -> &'static str {
        match self {
            ExportFormat::CursorRules => ".cursor/rules",
            ExportFormat::ClineRules => ".clinerules",
            ExportFormat::ClaudePlugin => ".claude-plugin",
        }
    }
}

/// Skill metadata mínima necessária para export
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExportableSkill.
pub struct ExportableSkill {
    /// Campo id.
    pub id: String,
    /// Campo name.
    pub name: String,
    /// Campo description.
    pub description: String,
    /// Campo version.
    pub version: String,
    /// Campo triggers.
    pub triggers: Vec<String>,
    /// Campo instructions.
    pub instructions: String,
    #[serde(default)]
    /// Campo tools.
    pub tools: Vec<String>,
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Campo metadata.
    pub metadata: HashMap<String, String>,
}

/// Resultado de uma operação de export
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExportReport.
pub struct ExportReport {
    /// Campo format.
    pub format: ExportFormat,
    /// Campo skills exported.
    pub skills_exported: usize,
    /// Campo output path.
    pub output_path: PathBuf,
    /// Campo errors.
    pub errors: Vec<String>,
}

/// Exporter principal — orquestra a conversão e escrita
pub struct SkillExporter {
    skills: Vec<ExportableSkill>,
}

impl SkillExporter {
    /// Função new.
    pub fn new(skills: Vec<ExportableSkill>) -> Self {
        // Variante Self.
        Self { skills }
    }

    /// Exportar todas as skills para o formato especificado
    pub fn export(&self, format: &ExportFormat, output_dir: &Path) -> ExportResult<ExportReport> {
        if !output_dir.exists() {
            return Err(ExportError::OutputDirMissing(
                output_dir.display().to_string(),
            ));
        }

        let mut errors = Vec::new();
        let mut exported = 0usize;

        match format {
            ExportFormat::CursorRules => {
                for skill in &self.skills {
                    match self.export_cursor_rule(skill, output_dir) {
                        // Variante Ok.
                        Ok(()) => exported += 1,
                        // Variante Err.
                        Err(e) => errors.push(format!("{}: {}", skill.id, e)),
                    }
                }
            }
            ExportFormat::ClineRules => {
                match self.export_cline_rules(output_dir) {
                    // Variante Ok.
                    Ok(()) => exported = self.skills.len(),
                    // Variante Err.
                    Err(e) => errors.push(e.to_string()),
                }
            }
            ExportFormat::ClaudePlugin => {
                match self.export_claude_plugin(output_dir) {
                    // Variante Ok.
                    Ok(()) => exported = self.skills.len(),
                    // Variante Err.
                    Err(e) => errors.push(e.to_string()),
                }
            }
        }

        // Variante Ok.
        Ok(ExportReport {
            format: format.clone(),
            skills_exported: exported,
            output_path: output_dir.to_path_buf(),
            errors,
        })
    }

    // ── Cursor Rules (.cursor/rules/*.mdc) ──────────────────────────

    fn export_cursor_rule(&self, skill: &ExportableSkill, output_dir: &Path) -> ExportResult<()> {
        let rules_dir = output_dir.join(".cursor").join("rules");
        std::fs::create_dir_all(&rules_dir)?;

        let filename = format!("{}.mdc", sanitize_filename(&skill.id));
        let path = rules_dir.join(filename);

        let triggers = skill
            .triggers
            .iter()
            .map(|t| format!("- {}", t))
            .collect::<Vec<_>>()
            .join("\n");

        let tools = if skill.tools.is_empty() {
            String::from("_nenhuma ferramenta específica_")
        } else {
            skill
                .tools
                .iter()
                .map(|t| format!("- `{}`", t))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let content = format!(
            r#"---
description: {description}
globs:
alwaysApply: false
tags: [{tags}]
---

# {name}

**Versão:** {version}
**ID:** {id}

## Descrição

{description}

## Triggers

{triggers}

## Instruções

{instructions}

## Ferramentas

{tools}
"#,
            description = skill.description,
            tags = skill.tags.join(", "),
            name = skill.name,
            version = skill.version,
            id = skill.id,
            triggers = triggers,
            instructions = skill.instructions,
            tools = tools,
        );

        std::fs::write(path, content)?;
        // Variante Ok.
        Ok(())
    }

    // ── Cline Rules (.clinerules/rules.json) ────────────────────────

    fn export_cline_rules(&self, output_dir: &Path) -> ExportResult<()> {
        let rules_dir = output_dir.join(".clinerules");
        std::fs::create_dir_all(&rules_dir)?;

        let rules: Vec<ClineRule> = self
            .skills
            .iter()
            .map(|s| ClineRule {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                triggers: s.triggers.clone(),
                instructions: s.instructions.clone(),
                tools: s.tools.clone(),
                version: s.version.clone(),
            })
            .collect();

        let wrapper = ClineRulesFile {
            version: "1.0".to_string(),
            generated_by: "zylos-skills-exporter".to_string(),
            rules,
        };

        let json = serde_json::to_string_pretty(&wrapper)?;
        std::fs::write(rules_dir.join("rules.json"), json)?;
        // Variante Ok.
        Ok(())
    }

    // ── Claude Plugin (.claude-plugin/manifest.json + skills/) ──────

    fn export_claude_plugin(&self, output_dir: &Path) -> ExportResult<()> {
        let plugin_dir = output_dir.join(".claude-plugin");
        let skills_dir = plugin_dir.join("skills");
        std::fs::create_dir_all(&skills_dir)?;

        // Write individual skill files
        let mut manifest_skills = Vec::new();
        for skill in &self.skills {
            let skill_file = format!("{}.json", sanitize_filename(&skill.id));
            let skill_content = serde_json::to_string_pretty(&ClaudeSkillEntry {
                id: skill.id.clone(),
                name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                triggers: skill.triggers.clone(),
                instructions: skill.instructions.clone(),
                tools: skill.tools.clone(),
                tags: skill.tags.clone(),
            })?;
            std::fs::write(skills_dir.join(&skill_file), skill_content)?;
            manifest_skills.push(ClaudeManifestSkillRef {
                id: skill.id.clone(),
                file: format!("skills/{}", skill_file),
            });
        }

        // Write manifest
        let manifest = ClaudePluginManifest {
            name: "zylos-exported-skills".to_string(),
            version: "0.1.0".to_string(),
            description: "Skills exportadas do Zylos-Next Registry".to_string(),
            author: "zylos-next".to_string(),
            skills: manifest_skills,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(plugin_dir.join("manifest.json"), manifest_json)?;
        // Variante Ok.
        Ok(())
    }
}

// ── Tipos internos para serialização ────────────────────────────────

#[derive(Serialize)]
struct ClineRule {
    id: String,
    name: String,
    description: String,
    triggers: Vec<String>,
    instructions: String,
    tools: Vec<String>,
    version: String,
}

#[derive(Serialize)]
struct ClineRulesFile {
    version: String,
    generated_by: String,
    rules: Vec<ClineRule>,
}

#[derive(Serialize)]
struct ClaudeSkillEntry {
    id: String,
    name: String,
    description: String,
    version: String,
    triggers: Vec<String>,
    instructions: String,
    tools: Vec<String>,
    tags: Vec<String>,
}

#[derive(Serialize)]
struct ClaudeManifestSkillRef {
    id: String,
    file: String,
}

#[derive(Serialize)]
struct ClaudePluginManifest {
    name: String,
    version: String,
    description: String,
    author: String,
    skills: Vec<ClaudeManifestSkillRef>,
}

/// Sanitiza nome de arquivo removendo caracteres inseguros
fn sanitize_filename(id: &str) -> String {
    id.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn sample_skills() -> Vec<ExportableSkill> {
        vec![
            // Variante ExportableSkill.
                        ExportableSkill {
                            id: "code-review-v2".to_string(),
                            name: "Code Review v2".to_string(),
                            description: "Revisão de código com foco em segurança e performance".to_string(),
                version: "2.1.0".to_string(),
                triggers: vec!["review code".to_string(), "check PR".to_string()],
                instructions: "Analise o diff, identifique vulnerabilidades...".to_string(),
                tools: vec!["read_file".to_string(), "search_files".to_string()],
                tags: vec!["review".to_string(), "security".to_string()],
                metadata: HashMap::new(),
            },
            // Variante ExportableSkill.
             ExportableSkill {
             id: "test-generator".to_string(),
             name: "Test Generator".to_string(),
             description: "Gera testes unitários automaticamente".to_string(),
                version: "1.0.0".to_string(),
                triggers: vec!["generate tests".to_string()],
                instructions: "Leia o arquivo fonte e gere testes...".to_string(),
                tools: vec!["read_file".to_string(), "write_file".to_string()],
                tags: vec!["testing".to_string()],
                metadata: HashMap::new(),
            },
        ]
    }

    #[test]
    fn test_export_cursor_rules() {
        let tmp = std::env::temp_dir().join("zylos-test-cursor");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let exporter = SkillExporter::new(sample_skills());
        let report = exporter.export(&ExportFormat::CursorRules, &tmp).unwrap();

        assert_eq!(report.skills_exported, 2);
        assert!(report.errors.is_empty());

        let rules_dir = tmp.join(".cursor").join("rules");
        assert!(rules_dir.join("code-review-v2.mdc").exists());
        assert!(rules_dir.join("test-generator.mdc").exists());

        let content = fs::read_to_string(rules_dir.join("code-review-v2.mdc")).unwrap();
        assert!(content.contains("Code Review v2"));
        assert!(content.contains("review code"));
        assert!(content.contains("read_file"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_cline_rules() {
        let tmp = std::env::temp_dir().join("zylos-test-cline");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let exporter = SkillExporter::new(sample_skills());
        let report = exporter.export(&ExportFormat::ClineRules, &tmp).unwrap();

        assert_eq!(report.skills_exported, 2);
        assert!(report.errors.is_empty());

        let rules_file = tmp.join(".clinerules").join("rules.json");
        assert!(rules_file.exists());

        let content: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&rules_file).unwrap()).unwrap();
        assert_eq!(content["rules"].as_array().unwrap().len(), 2);
        assert_eq!(content["generated_by"], "zylos-skills-exporter");

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_claude_plugin() {
        let tmp = std::env::temp_dir().join("zylos-test-claude");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let exporter = SkillExporter::new(sample_skills());
        let report = exporter.export(&ExportFormat::ClaudePlugin, &tmp).unwrap();

        assert_eq!(report.skills_exported, 2);
        assert!(report.errors.is_empty());

        let plugin_dir = tmp.join(".claude-plugin");
        assert!(plugin_dir.join("manifest.json").exists());
        assert!(plugin_dir.join("skills").join("code-review-v2.json").exists());
        assert!(plugin_dir.join("skills").join("test-generator.json").exists());

        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(plugin_dir.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], "zylos-exported-skills");
        assert_eq!(manifest["skills"].as_array().unwrap().len(), 2);

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_export_missing_output_dir() {
        let exporter = SkillExporter::new(sample_skills());
        let result = exporter.export(
            &ExportFormat::CursorRules,
            Path::new("/nonexistent/path/xyz"),
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExportError::OutputDirMissing(_)
        ));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("my-skill_v2"), "my-skill_v2");
        assert_eq!(sanitize_filename("skill/with:special"), "skill_with_special");
        assert_eq!(sanitize_filename("normal-name"), "normal-name");
    }

    #[test]
    fn test_format_extension_and_dir() {
        assert_eq!(ExportFormat::CursorRules.extension(), "mdc");
        assert_eq!(ExportFormat::CursorRules.dir_name(), ".cursor/rules");
        assert_eq!(ExportFormat::ClineRules.extension(), "json");
        assert_eq!(ExportFormat::ClineRules.dir_name(), ".clinerules");
        assert_eq!(ExportFormat::ClaudePlugin.extension(), "json");
        assert_eq!(ExportFormat::ClaudePlugin.dir_name(), ".claude-plugin");
    }

    #[test]
    fn test_export_empty_skills_list() {
        let tmp = std::env::temp_dir().join("zylos-test-empty");
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let exporter = SkillExporter::new(vec![]);
        let report = exporter.export(&ExportFormat::CursorRules, &tmp).unwrap();
        assert_eq!(report.skills_exported, 0);
        assert!(report.errors.is_empty());

        let report2 = exporter.export(&ExportFormat::ClineRules, &tmp).unwrap();
        assert_eq!(report2.skills_exported, 0);

        let _ = fs::remove_dir_all(&tmp);
    }
}