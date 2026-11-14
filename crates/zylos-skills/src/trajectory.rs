//! # TrajectoryCompiler — Conversão de trajetórias em skill contracts (T8.4.4)
//!
//! Converte execuções documentadas em HANDOFF_*.md e playbooks/*.yaml
//! em candidate SKILL.md com contratos formais preenchidos automaticamente.
//!
//! ## Pipeline
//! 1. Parseia arquivo HANDOFF ou playbook YAML
//! 2. Extrai inputs, outputs, assertions e passos
//! 3. Gera SkillContract auto-preenchido
//! 4. Integra com SkillMiner para deduplication
//!
//! ## Formatos Suportados
//! - HANDOFF_*.md: Markdown estruturado com seções padrão
//! - playbooks/*.yaml: YAML com steps/inputs/outputs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::contract::{Assertion, AssertionSeverity, OutputSchema, Predicate, SkillContract};

/// Fonte de uma trajetória compilada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TrajectorySource {
    /// Arquivo HANDOFF markdown.
    Handoff { path: String },
    /// Arquivo playbook YAML.
    Playbook { path: String },
    /// Log de execução bruto.
    ExecutionLog { source: String },
}

/// Resultado da compilação de uma trajetória.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledTrajectory {
    /// Nome sugerido para a skill.
    pub suggested_name: String,
    /// Descrição extraída da trajetória.
    pub description: String,
    /// Contrato formal gerado.
    pub contract: SkillContract,
    /// Fonte original da trajetória.
    pub source: TrajectorySource,
    /// Passos extraídos da trajetória.
    pub steps: Vec<TrajectoryStep>,
    /// Metadata adicional.
    pub metadata: HashMap<String, String>,
}

/// Um passo individual em uma trajetória.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrajectoryStep {
    /// Número do passo (1-indexed).
    pub step_number: usize,
    /// Descrição do passo.
    pub description: String,
    /// Comando ou ação executada.
    pub action: Option<String>,
    /// Input esperado para este passo.
    pub input: Option<String>,
    /// Output produzido por este passo.
    pub output: Option<String>,
}

/// Parser para arquivos HANDOFF markdown.
pub struct HandoffParser;

impl HandoffParser {
    /// Extrai campos estruturados de um arquivo HANDOFF markdown.
    pub fn parse(content: &str) -> Result<CompiledTrajectory, TrajectoryError> {
        let mut name = String::new();
        let mut description = String::new();
        let mut steps = Vec::new();
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut current_section = String::new();
        let mut step_counter = 0;

        for line in content.lines() {
            let trimmed = line.trim();

            // Detectar seções por headers markdown
            if trimmed.starts_with("# ") {
                name = trimmed.trim_start_matches("# ").trim().to_string();
                continue;
            }
            if trimmed.starts_with("## ") {
                current_section = trimmed.trim_start_matches("## ").trim().to_lowercase();
                continue;
            }

            // Extrair conteúdo baseado na seção atual
            match current_section.as_str() {
                "descrição" | "description" | "resumo" | "summary" => {
                    if !trimmed.is_empty() && !description.contains(trimmed) {
                        if !description.is_empty() {
                            description.push(' ');
                        }
                        description.push_str(trimmed);
                    }
                }
                "passos" | "steps" | "procedimento" | "procedure" => {
                    if !trimmed.is_empty() {
                        step_counter += 1;
                        steps.push(TrajectoryStep {
                            step_number: step_counter,
                            description: trimmed.to_string(),
                            action: None,
                            input: None,
                            output: None,
                        });
                    }
                }
                "inputs" | "entradas" | "pré-requisitos" | "prerequisites" => {
                    if !trimmed.is_empty() {
                        inputs.push(trimmed.to_string());
                    }
                }
                "outputs" | "saídas" | "resultados" | "results" if !trimmed.is_empty() => {
                    outputs.push(trimmed.to_string());
                }
                _ => {}
            }
        }

        if name.is_empty() {
            return Err(TrajectoryError::MissingField("name/title".into()));
        }

        // Construir contrato a partir dos campos extraídos
        let mut contract = SkillContract::new();

        // Adicionar precondições a partir de inputs
        for input in &inputs {
            contract.preconditions.push(Predicate::new(
                format!("input_available:{}", sanitize_id(input)),
                vec![input.clone()],
            ));
        }

        // Adicionar outputs
        for (i, output) in outputs.iter().enumerate() {
            contract.outputs.push(
                OutputSchema::new(format!("output_{}", i + 1), "object")
                    .with_description(output.clone()),
            );
        }

        // Adicionar assertions a partir de passos
        for step in &steps {
            if !step.description.is_empty() {
                contract.assertions.push(
                    Assertion::new(
                        format!("step_{}_completed", step.step_number),
                        format!("step_completed({})", step.step_number),
                    )
                    .with_severity(AssertionSeverity::Warning)
                    .with_message(step.description.clone()),
                );
            }
        }

        Ok(CompiledTrajectory {
            suggested_name: sanitize_id(&name),
            description,
            contract,
            source: TrajectorySource::Handoff {
                path: String::new(),
            },
            steps,
            metadata: HashMap::new(),
        })
    }
}

/// Parser para arquivos playbook YAML.
pub struct PlaybookParser;

/// Estrutura simplificada de um playbook YAML.
#[derive(Debug, Deserialize)]
struct PlaybookYaml {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    outputs: Vec<String>,
    #[serde(default)]
    steps: Vec<PlaybookStepYaml>,
}

#[derive(Debug, Deserialize)]
struct PlaybookStepYaml {
    description: Option<String>,
    action: Option<String>,
    input: Option<String>,
    output: Option<String>,
}

impl PlaybookParser {
    /// Extrai campos estruturados de um arquivo playbook YAML.
    pub fn parse(yaml_content: &str) -> Result<CompiledTrajectory, TrajectoryError> {
        let playbook: PlaybookYaml = serde_yaml::from_str(yaml_content)
            .map_err(|e| TrajectoryError::ParseError(format!("YAML parse error: {}", e)))?;

        let name = playbook
            .name
            .unwrap_or_else(|| "unnamed-playbook".to_string());
        let description = playbook.description.unwrap_or_default();

        let steps: Vec<TrajectoryStep> = playbook
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| TrajectoryStep {
                step_number: i + 1,
                description: s.description.clone().unwrap_or_default(),
                action: s.action.clone(),
                input: s.input.clone(),
                output: s.output.clone(),
            })
            .collect();

        // Construir contrato
        let mut contract = SkillContract::new();

        for input in &playbook.inputs {
            contract.preconditions.push(Predicate::new(
                format!("input_available:{}", sanitize_id(input)),
                vec![input.clone()],
            ));
        }

        for (i, output) in playbook.outputs.iter().enumerate() {
            contract.outputs.push(
                OutputSchema::new(format!("output_{}", i + 1), "object")
                    .with_description(output.clone()),
            );
        }

        for step in &steps {
            if !step.description.is_empty() {
                contract.assertions.push(
                    Assertion::new(
                        format!("step_{}_completed", step.step_number),
                        format!("step_completed({})", step.step_number),
                    )
                    .with_severity(AssertionSeverity::Warning)
                    .with_message(step.description.clone()),
                );
            }
        }

        Ok(CompiledTrajectory {
            suggested_name: sanitize_id(&name),
            description,
            contract,
            source: TrajectorySource::Playbook {
                path: String::new(),
            },
            steps,
            metadata: HashMap::new(),
        })
    }
}

/// Erros do TrajectoryCompiler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TrajectoryError {
    #[error("Campo obrigatório ausente: {0}")]
    MissingField(String),
    #[error("Erro de parsing: {0}")]
    ParseError(String),
    #[error("Formato não suportado: {0}")]
    UnsupportedFormat(String),
}

/// Sanitiza uma string para uso como identificador.
fn sanitize_id(input: &str) -> String {
    input
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_handoff_basic() {
        let content = r#"# Deploy Backend Service

## Descrição
Deploy automatizado do serviço backend para staging.

## Inputs
- Docker image tag
- Environment variables file

## Passos
1. Validar imagem Docker
2. Aplicar variáveis de ambiente
3. Executar deploy via kubectl
4. Verificar health check

## Outputs
- Deployment status
- Service URL
"#;

        let result = HandoffParser::parse(content).unwrap();
        assert_eq!(result.suggested_name, "deploy-backend-service");
        assert!(!result.description.is_empty());
        assert_eq!(result.steps.len(), 4);
        assert_eq!(result.contract.preconditions.len(), 2);
        assert_eq!(result.contract.outputs.len(), 2);
        assert_eq!(result.contract.assertions.len(), 4);
    }

    #[test]
    fn test_parse_handoff_missing_title() {
        let content = "## Descrição\nSem título.\n";
        let result = HandoffParser::parse(content);
        assert!(matches!(result, Err(TrajectoryError::MissingField(_))));
    }

    #[test]
    fn test_parse_playbook_yaml() {
        let yaml = r#"
name: Build and Test
description: CI pipeline for Rust workspace
inputs:
  - source_code
  - rust_toolchain
outputs:
  - test_report
  - coverage_report
steps:
  - description: Checkout code
    action: git checkout
  - description: Run tests
    action: cargo test
    output: test_results.xml
  - description: Generate coverage
    action: cargo tarpaulin
    output: coverage.html
"#;

        let result = PlaybookParser::parse(yaml).unwrap();
        assert_eq!(result.suggested_name, "build-and-test");
        assert_eq!(result.description, "CI pipeline for Rust workspace");
        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.contract.preconditions.len(), 2);
        assert_eq!(result.contract.outputs.len(), 2);
        assert_eq!(result.contract.assertions.len(), 3);

        // Verificar que ações foram extraídas
        assert_eq!(result.steps[0].action, Some("git checkout".into()));
        assert_eq!(result.steps[1].action, Some("cargo test".into()));
        assert_eq!(result.steps[1].output, Some("test_results.xml".into()));
    }

    #[test]
    fn test_parse_playbook_invalid_yaml() {
        let result = PlaybookParser::parse("not: valid: yaml: [broken");
        assert!(matches!(result, Err(TrajectoryError::ParseError(_))));
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("Hello World!"), "hello-world");
        assert_eq!(sanitize_id("Deploy v2.0 (Beta)"), "deploy-v2-0--beta");
        assert_eq!(sanitize_id("simple-name"), "simple-name");
        assert_eq!(sanitize_id("UPPER CASE"), "upper-case");
    }

    #[test]
    fn test_trajectory_step_serialization() {
        let step = TrajectoryStep {
            step_number: 3,
            description: "Run integration tests".into(),
            action: Some("cargo test --test integration".into()),
            input: Some("test fixtures".into()),
            output: Some("test-results.json".into()),
        };
        let json = serde_json::to_string(&step).unwrap();
        let restored: TrajectoryStep = serde_json::from_str(&json).unwrap();
        assert_eq!(step, restored);
    }

    #[test]
    fn test_compiled_trajectory_contract_valid() {
        let content = r#"# Test Skill

## Descrição
Teste unitário.

## Inputs
- config_file

## Passos
1. Carregar config
2. Executar teste

## Outputs
- result
"#;

        let trajectory = HandoffParser::parse(content).unwrap();
        // O contrato gerado deve ser válido
        assert!(trajectory.contract.validate().is_ok());
    }

    #[test]
    fn test_handoff_empty_sections() {
        let content = r#"# Minimal Handoff

## Descrição
Apenas descrição.
"#;

        let result = HandoffParser::parse(content).unwrap();
        assert_eq!(result.suggested_name, "minimal-handoff");
        assert!(result.steps.is_empty());
        assert!(result.contract.preconditions.is_empty());
        assert!(result.contract.outputs.is_empty());
    }

    #[test]
    fn test_playbook_minimal() {
        let yaml = "name: Simple\n";
        let result = PlaybookParser::parse(yaml).unwrap();
        assert_eq!(result.suggested_name, "simple");
        assert!(result.steps.is_empty());
    }

    #[test]
    fn test_trajectory_source_serialization() {
        let sources = vec![
            TrajectorySource::Handoff {
                path: "HANDOFF_1.md".into(),
            },
            TrajectorySource::Playbook {
                path: "deploy.yaml".into(),
            },
            TrajectorySource::ExecutionLog {
                source: "gateway.log".into(),
            },
        ];

        for source in sources {
            let json = serde_json::to_string(&source).unwrap();
            let restored: TrajectorySource = serde_json::from_str(&json).unwrap();
            assert_eq!(source, restored);
        }
    }

    #[test]
    fn test_multiple_outputs_indexed() {
        let content = r#"# Multi Output

## Descrição
Teste.

## Outputs
- Primeiro resultado
- Segundo resultado
- Terceiro resultado
"#;

        let result = HandoffParser::parse(content).unwrap();
        assert_eq!(result.contract.outputs.len(), 3);
        assert_eq!(result.contract.outputs[0].name, "output_1");
        assert_eq!(result.contract.outputs[1].name, "output_2");
        assert_eq!(result.contract.outputs[2].name, "output_3");
    }
}
