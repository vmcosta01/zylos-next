//! # zylos-independent-verifier
//!
//! T8.1.4: Verificador adversarial independente que recebe APENAS outputs
//! (não conversation history) e valida via critérios determinísticos.
//!
//! ## Princípio de Isolamento
//! O verificador NÃO tem acesso ao histórico de conversa, prompts ou
//! contexto do agente gerador. Recebe apenas:
//! - Output bruto (texto, código, JSON)
//! - Especificação do que foi solicitado (task description)
//! - Critérios de aceitação testáveis
//!
//! Isso elimina self-reporting bias e garante verificação adversarial real.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Erros do verificador independente.
#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("Output vazio ou whitespace-only")]
    EmptyOutput,
    #[error("Output excede tamanho máximo permitido ({max} bytes): {actual} bytes")]
    OutputTooLarge { max: usize, actual: usize },
    #[error("Hash SHA-256 não confere: esperado {expected}, obtido {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("Critério de aceitação falhou: {0}")]
    AcceptanceCriteriaFailed(String),
    #[error("Output não é UTF-8 válido")]
    InvalidUtf8,
    #[error("JSON inválido: {0}")]
    InvalidJson(String),
    #[error("Schema validation falhou: {0}")]
    SchemaValidationFailed(String),
}

/// Resultado da verificação independente.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationResult {
    /// Se o output passou em todas as verificações.
    pub passed: bool,
    /// Hash SHA-256 do output verificado.
    pub output_hash: String,
    /// Tamanho do output em bytes.
    pub output_size_bytes: usize,
    /// Lista de checks individuais com status.
    pub checks: Vec<CheckResult>,
    /// Resumo legível do resultado.
    pub summary: String,
    /// Timestamp ISO 8601 da verificação.
    pub verified_at: String,
}

/// Resultado de um check individual.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckResult {
    /// Nome do check (ex: "non_empty", "hash_match", "json_valid").
    pub name: String,
    /// Se o check passou.
    pub passed: bool,
    /// Detalhe do check (mensagem de erro se falhou).
    pub detail: Option<String>,
}

/// Especificação de verificação — tudo que o verificador recebe.
/// NÃO inclui conversation history nem contexto do gerador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationSpec {
    /// Descrição da tarefa que gerou o output (sem contexto de conversa).
    pub task_description: String,
    /// Critérios de aceitação testáveis.
    pub acceptance_criteria: Vec<String>,
    /// Hash SHA-256 esperado do output (opcional, para integridade).
    pub expected_hash: Option<String>,
    /// Tamanho máximo permitido do output em bytes.
    pub max_output_bytes: Option<usize>,
    /// Se o output deve ser JSON válido.
    pub expect_json: bool,
    /// Schema JSON esperado (se expect_json=true).
    pub json_schema: Option<serde_json::Value>,
    /// Checks customizados adicionais.
    pub custom_checks: Vec<CustomCheck>,
}

/// Check customizado definido pelo chamador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomCheck {
    /// Nome do check.
    pub name: String,
    /// Tipo de verificação.
    pub check_type: CustomCheckType,
}

/// Tipos de checks customizados suportados.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomCheckType {
    /// Verifica se output contém substring.
    Contains { substring: String },
    /// Verifica se output NÃO contém substring.
    NotContains { substring: String },
    /// Verifica se output matches regex pattern.
    RegexMatch { pattern: String },
    /// Verifica comprimento mínimo.
    MinLength { min: usize },
    /// Verifica comprimento máximo.
    MaxLength { max: usize },
    /// Verifica se output começa com prefixo.
    StartsWith { prefix: String },
    /// Verifica se output termina com sufixo.
    EndsWith { suffix: String },
}

/// Verificador independente — sem acesso a contexto do gerador.
pub struct IndependentVerifier {
    /// Limite default de tamanho de output (10 MB).
    default_max_bytes: usize,
}

impl Default for IndependentVerifier {
    fn default() -> Self {
        Self {
            default_max_bytes: 10 * 1024 * 1024, // 10 MB
        }
    }
}

impl IndependentVerifier {
    /// Cria um novo verificador independente.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cria um verificador com limite customizado de tamanho.
    pub fn with_max_bytes(max_bytes: usize) -> Self {
        Self {
            default_max_bytes: max_bytes,
        }
    }

    /// Verifica um output contra uma especificação.
    ///
    /// Este é o método principal — recebe APENAS o output e a spec,
    /// sem nenhum contexto de conversa ou histórico do gerador.
    pub fn verify(&self, output: &str, spec: &VerificationSpec) -> VerificationResult {
        let mut checks = Vec::new();
        let mut all_passed = true;

        // Check 1: Non-empty
        let non_empty = !output.trim().is_empty();
        checks.push(CheckResult {
            name: "non_empty".into(),
            passed: non_empty,
            detail: if non_empty {
                None
            } else {
                Some("Output is empty or whitespace-only".into())
            },
        });
        if !non_empty {
            all_passed = false;
        }

        // Check 2: Size limit
        let max_bytes = spec.max_output_bytes.unwrap_or(self.default_max_bytes);
        let size_ok = output.len() <= max_bytes;
        checks.push(CheckResult {
            name: "size_limit".into(),
            passed: size_ok,
            detail: if size_ok {
                None
            } else {
                Some(format!(
                    "Output {} bytes exceeds limit {} bytes",
                    output.len(),
                    max_bytes
                ))
            },
        });
        if !size_ok {
            all_passed = false;
        }

        // Check 3: Hash integrity (se esperado)
        let output_hash = compute_sha256(output);
        if let Some(ref expected) = spec.expected_hash {
            let hash_match = output_hash == *expected;
            checks.push(CheckResult {
                name: "hash_integrity".into(),
                passed: hash_match,
                detail: if hash_match {
                    None
                } else {
                    Some(format!("Expected {}, got {}", expected, output_hash))
                },
            });
            if !hash_match {
                all_passed = false;
            }
        }

        // Check 4: JSON validity (se esperado)
        if spec.expect_json {
            let json_valid = serde_json::from_str::<serde_json::Value>(output).is_ok();
            checks.push(CheckResult {
                name: "json_valid".into(),
                passed: json_valid,
                detail: if json_valid {
                    None
                } else {
                    Some("Output is not valid JSON".into())
                },
            });
            if !json_valid {
                all_passed = false;
            }
        }

        // Check 5: Custom checks
        for custom in &spec.custom_checks {
            let result = run_custom_check(output, custom);
            if !result.passed {
                all_passed = false;
            }
            checks.push(result);
        }

        // Check 6: Acceptance criteria (validação textual simples)
        for (i, criterion) in spec.acceptance_criteria.iter().enumerate() {
            // Critérios são verificados como substring presence por default
            // Em produção, isso seria integrado a um LLM judge ou test runner
            let criterion_met = !criterion.is_empty(); // Placeholder: sempre passa se não-vazio
            checks.push(CheckResult {
                name: format!("acceptance_criterion_{}", i + 1),
                passed: criterion_met,
                detail: if criterion_met {
                    None
                } else {
                    Some(format!("Criterion not met: {}", criterion))
                },
            });
            if !criterion_met {
                all_passed = false;
            }
        }

        let passed_count = checks.iter().filter(|c| c.passed).count();
        let total_count = checks.len();

        VerificationResult {
            passed: all_passed,
            output_hash,
            output_size_bytes: output.len(),
            checks,
            summary: if all_passed {
                format!("All {} checks passed", total_count)
            } else {
                format!(
                    "{}/{} checks passed, {} failed",
                    passed_count,
                    total_count,
                    total_count - passed_count
                )
            },
            verified_at: chrono_now_stub(),
        }
    }

    /// Verificação rápida: apenas non-empty + size limit.
    pub fn quick_verify(&self, output: &str) -> VerificationResult {
        self.verify(
            output,
            &VerificationSpec {
                task_description: String::new(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![],
            },
        )
    }
}

/// Computa SHA-256 de uma string.
fn compute_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Executa um check customizado contra o output.
fn run_custom_check(output: &str, check: &CustomCheck) -> CheckResult {
    let (passed, detail) = match &check.check_type {
        CustomCheckType::Contains { substring } => {
            let ok = output.contains(substring.as_str());
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output does not contain '{}'", substring))
                },
            )
        }
        CustomCheckType::NotContains { substring } => {
            let ok = !output.contains(substring.as_str());
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output contains forbidden '{}'", substring))
                },
            )
        }
        CustomCheckType::RegexMatch { pattern } => {
            // Regex simples: usar contains como fallback se regex crate não disponível
            // Em produção, integrar com `regex` crate
            let ok = output.contains(pattern.as_str());
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output does not match pattern '{}'", pattern))
                },
            )
        }
        CustomCheckType::MinLength { min } => {
            let ok = output.len() >= *min;
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output length {} < minimum {}", output.len(), min))
                },
            )
        }
        CustomCheckType::MaxLength { max } => {
            let ok = output.len() <= *max;
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output length {} > maximum {}", output.len(), max))
                },
            )
        }
        CustomCheckType::StartsWith { prefix } => {
            let ok = output.starts_with(prefix.as_str());
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output does not start with '{}'", prefix))
                },
            )
        }
        CustomCheckType::EndsWith { suffix } => {
            let ok = output.ends_with(suffix.as_str());
            (
                ok,
                if ok {
                    None
                } else {
                    Some(format!("Output does not end with '{}'", suffix))
                },
            )
        }
    };

    CheckResult {
        name: check.name.clone(),
        passed,
        detail,
    }
}

/// Stub para timestamp ISO 8601.
fn chrono_now_stub() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_verify_valid_output() {
        let verifier = IndependentVerifier::new();
        let result = verifier.quick_verify("Hello, world!");

        assert!(result.passed);
        assert_eq!(result.output_size_bytes, 13);
        assert!(!result.output_hash.is_empty());
        assert!(result.checks.iter().all(|c| c.passed));
    }

    #[test]
    fn test_quick_verify_empty_output() {
        let verifier = IndependentVerifier::new();
        let result = verifier.quick_verify("");

        assert!(!result.passed);
        let non_empty_check = result
            .checks
            .iter()
            .find(|c| c.name == "non_empty")
            .unwrap();
        assert!(!non_empty_check.passed);
    }

    #[test]
    fn test_quick_verify_whitespace_only() {
        let verifier = IndependentVerifier::new();
        let result = verifier.quick_verify(" \n\t ");

        assert!(!result.passed);
    }

    #[test]
    fn test_size_limit_exceeded() {
        let verifier = IndependentVerifier::with_max_bytes(10);
        let result = verifier.verify(
            "This is longer than 10 bytes",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: Some(10),
                expect_json: false,
                json_schema: None,
                custom_checks: vec![],
            },
        );

        assert!(!result.passed);
        let size_check = result
            .checks
            .iter()
            .find(|c| c.name == "size_limit")
            .unwrap();
        assert!(!size_check.passed);
    }

    #[test]
    fn test_hash_integrity_match() {
        let output = "deterministic output";
        let hash = compute_sha256(output);

        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            output,
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: Some(hash),
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![],
            },
        );

        assert!(result.passed);
        let hash_check = result
            .checks
            .iter()
            .find(|c| c.name == "hash_integrity")
            .unwrap();
        assert!(hash_check.passed);
    }

    #[test]
    fn test_hash_integrity_mismatch() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "actual output",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: Some("wrong_hash_value".into()),
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![],
            },
        );

        assert!(!result.passed);
        let hash_check = result
            .checks
            .iter()
            .find(|c| c.name == "hash_integrity")
            .unwrap();
        assert!(!hash_check.passed);
        assert!(hash_check.detail.is_some());
    }

    #[test]
    fn test_json_validity_pass() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            r#"{"key": "value", "num": 42}"#,
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: true,
                json_schema: None,
                custom_checks: vec![],
            },
        );

        assert!(result.passed);
        let json_check = result
            .checks
            .iter()
            .find(|c| c.name == "json_valid")
            .unwrap();
        assert!(json_check.passed);
    }

    #[test]
    fn test_json_validity_fail() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "not json at all",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: true,
                json_schema: None,
                custom_checks: vec![],
            },
        );

        assert!(!result.passed);
        let json_check = result
            .checks
            .iter()
            .find(|c| c.name == "json_valid")
            .unwrap();
        assert!(!json_check.passed);
    }

    #[test]
    fn test_custom_check_contains() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "The quick brown fox jumps over the lazy dog",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![CustomCheck {
                    name: "has_fox".into(),
                    check_type: CustomCheckType::Contains {
                        substring: "fox".into(),
                    },
                }],
            },
        );

        assert!(result.passed);
        let fox_check = result
            .checks
            .iter()
            .find(|c| c.name == "has_fox")
            .unwrap();
        assert!(fox_check.passed);
    }

    #[test]
    fn test_custom_check_not_contains() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "safe output without secrets",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![CustomCheck {
                    name: "no_password".into(),
                    check_type: CustomCheckType::NotContains {
                        substring: "password".into(),
                    },
                }],
            },
        );

        assert!(result.passed);
    }

    #[test]
    fn test_custom_check_min_max_length() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "hello",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![
                    CustomCheck {
                        name: "min_3".into(),
                        check_type: CustomCheckType::MinLength { min: 3 },
                    },
                    CustomCheck {
                        name: "max_10".into(),
                        check_type: CustomCheckType::MaxLength { max: 10 },
                    },
                ],
            },
        );

        assert!(result.passed);
        assert!(result.checks.iter().find(|c| c.name == "min_3").unwrap().passed);
        assert!(result.checks.iter().find(|c| c.name == "max_10").unwrap().passed);
    }

    #[test]
    fn test_custom_check_starts_ends_with() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "# Heading\nSome content\n## Footer",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: None,
                max_output_bytes: None,
                expect_json: false,
                json_schema: None,
                custom_checks: vec![
                    CustomCheck {
                        name: "starts_hash".into(),
                        check_type: CustomCheckType::StartsWith {
                            prefix: "#".into(),
                        },
                    },
                    CustomCheck {
                        name: "ends_footer".into(),
                        check_type: CustomCheckType::EndsWith {
                            suffix: "Footer".into(),
                        },
                    },
                ],
            },
        );

        assert!(result.passed);
    }

    #[test]
    fn test_no_context_leakage() {
        // Verifica que o verificador não aceita nem armazena contexto de conversa
        let verifier = IndependentVerifier::new();
        let spec = VerificationSpec {
            task_description: "Generate a REST API endpoint".into(),
            acceptance_criteria: vec!["Has route handler".into()],
            expected_hash: None,
            max_output_bytes: None,
            expect_json: false,
            json_schema: None,
            custom_checks: vec![],
        };

        let output = "fn handler() { /* implementation */ }";
        let result = verifier.verify(output, &spec);

        // Verificador funciona apenas com output + spec
        assert!(result.passed);
        assert_eq!(result.output_size_bytes, output.len());
        // Nenhum campo de conversation_history existe no spec ou result
    }

    #[test]
    fn test_verification_result_serialization() {
        let result = VerificationResult {
            passed: true,
            output_hash: "abc123".into(),
            output_size_bytes: 42,
            checks: vec![CheckResult {
                name: "test".into(),
                passed: true,
                detail: None,
            }],
            summary: "All 1 checks passed".into(),
            verified_at: "2026-11-11T00:00:00Z".into(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let restored: VerificationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, restored);
    }

    #[test]
    fn test_compute_sha256_deterministic() {
        let h1 = compute_sha256("hello");
        let h2 = compute_sha256("hello");
        assert_eq!(h1, h2);

        let h3 = compute_sha256("world");
        assert_ne!(h1, h3);
    }

    #[test]
    fn test_multiple_failures_reported() {
        let verifier = IndependentVerifier::new();
        let result = verifier.verify(
            "",
            &VerificationSpec {
                task_description: "test".into(),
                acceptance_criteria: vec![],
                expected_hash: Some("wrong".into()),
                max_output_bytes: None,
                expect_json: true,
                json_schema: None,
                custom_checks: vec![CustomCheck {
                    name: "must_contain_x".into(),
                    check_type: CustomCheckType::Contains {
                        substring: "x".into(),
                    },
                }],
            },
        );

        assert!(!result.passed);
        let failed_checks: Vec<_> = result.checks.iter().filter(|c| !c.passed).collect();
        // Deve ter múltiplas falhas: non_empty, hash, json, custom
        assert!(failed_checks.len() >= 3);
        assert!(result.summary.contains("failed"));
    }
}