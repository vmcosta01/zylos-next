//! # SkillContract — Contrato formal de skill (T8.4.1)
//!
//! Define a estrutura formal `(P, O, A, V, F)` inspirada em SkillOps:
//! - **P**reconditions: condições que devem ser verdadeiras antes da execução
//! - **O**utputs: schema dos artefatos produzidos
//! - **A**ssertions: invariantes verificáveis pós-execução
//! - **V**alidators: funções de validação aplicadas ao output
//! - **F**itness: métrica de qualidade/relevância da skill
//!
//! ## Validação
//! Contracts são validados em load-time via `validate_contract()`.
//! Skills sem contract válido são rejeitadas pelo IndexRegistry.
//!
//! ## Serialização
//! Contracts são serializados via serde e embutidos no `index.json`
//! ou em arquivos `SKILL.md` frontmatter.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Erros de validação de contrato.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ContractError {
    #[error("Precondition inválida: {0}")]
    InvalidPrecondition(String),
    #[error("Output schema inválido: {0}")]
    InvalidOutputSchema(String),
    #[error("Assertion inválida: {0}")]
    InvalidAssertion(String),
    #[error("Validator não encontrado: {0}")]
    ValidatorNotFound(String),
    #[error("Fitness metric inválida: {0}")]
    InvalidFitnessMetric(String),
    #[error("Campo obrigatório ausente: {0}")]
    MissingField(String),
    #[error("Erro de serialização: {0}")]
    SerializationError(String),
}

/// Predicado de precondição.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Predicate {
    /// Nome do predicado (ex: "file_exists", "env_var_set", "tool_available").
    pub name: String,
    /// Argumentos do predicado.
    pub args: Vec<String>,
    /// Mensagem de erro se o predicado falhar.
    pub error_message: Option<String>,
}

impl Predicate {
    pub fn new(name: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            name: name.into(),
            args,
            error_message: None,
        }
    }

    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.error_message = Some(msg.into());
        self
    }
}

/// Schema de output produzido pela skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputSchema {
    /// Nome do output.
    pub name: String,
    /// Tipo JSON Schema do output.
    pub json_type: String,
    /// Descrição do output.
    pub description: Option<String>,
    /// Se o output é obrigatório.
    pub required: bool,
}

impl OutputSchema {
    pub fn new(name: impl Into<String>, json_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            json_type: json_type.into(),
            description: None,
            required: true,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// Assertion verificável pós-execução.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assertion {
    /// ID único da assertion dentro do contrato.
    pub id: String,
    /// Expressão ou descrição da assertion.
    pub expression: String,
    /// Severidade se a assertion falhar.
    pub severity: AssertionSeverity,
    /// Mensagem descritiva.
    pub message: Option<String>,
}

/// Severidade de uma assertion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssertionSeverity {
    /// Falha bloqueia a skill (hard failure).
    Error,
    /// Falha gera warning mas não bloqueia.
    Warning,
    /// Informativo apenas.
    Info,
}

impl Assertion {
    pub fn new(id: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            expression: expression.into(),
            severity: AssertionSeverity::Error,
            message: None,
        }
    }

    pub fn with_severity(mut self, severity: AssertionSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = Some(msg.into());
        self
    }
}

/// Referência a uma função de validação.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidatorRef {
    /// Nome/caminho da função de validação.
    pub function_path: String,
    /// Parâmetros adicionais para o validator.
    pub params: HashMap<String, serde_json::Value>,
    /// Se o validator é obrigatório (falha = skill rejeitada).
    pub required: bool,
}

impl ValidatorRef {
    pub fn new(function_path: impl Into<String>) -> Self {
        Self {
            function_path: function_path.into(),
            params: HashMap::new(),
            required: true,
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

/// Métrica de fitness/qualidade da skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FitnessMetric {
    /// Nome da métrica (ex: "success_rate", "avg_latency_ms", "user_rating").
    pub name: String,
    /// Valor atual da métrica.
    pub value: f64,
    /// Valor mínimo aceitável (threshold).
    pub min_threshold: Option<f64>,
    /// Valor máximo aceitável (threshold).
    pub max_threshold: Option<f64>,
    /// Timestamp da última atualização (ISO 8601).
    pub updated_at: Option<String>,
}

impl FitnessMetric {
    pub fn new(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            value,
            min_threshold: None,
            max_threshold: None,
            updated_at: None,
        }
    }

    pub fn with_min(mut self, min: f64) -> Self {
        self.min_threshold = Some(min);
        self
    }

    pub fn with_max(mut self, max: f64) -> Self {
        self.max_threshold = Some(max);
        self
    }

    /// Verifica se o valor está dentro dos thresholds.
    pub fn is_within_threshold(&self) -> bool {
        if let Some(min) = self.min_threshold {
            if self.value < min {
                return false;
            }
        }
        if let Some(max) = self.max_threshold {
            if self.value > max {
                return false;
            }
        }
        true
    }
}

/// Contrato formal completo de uma skill: (P, O, A, V, F).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillContract {
    /// Versão do schema do contrato.
    pub version: String,
    /// Precondições (P).
    pub preconditions: Vec<Predicate>,
    /// Outputs (O).
    pub outputs: Vec<OutputSchema>,
    /// Assertions (A).
    pub assertions: Vec<Assertion>,
    /// Validators (V).
    pub validators: Vec<ValidatorRef>,
    /// Fitness metrics (F).
    pub fitness: Vec<FitnessMetric>,
    /// Metadata adicional.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for SkillContract {
    fn default() -> Self {
        Self {
            version: "1.0".into(),
            preconditions: vec![],
            outputs: vec![],
            assertions: vec![],
            validators: vec![],
            fitness: vec![],
            metadata: HashMap::new(),
        }
    }
}

impl SkillContract {
    /// Cria um contrato vazio com versão default.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: adiciona precondição.
    pub fn with_precondition(mut self, predicate: Predicate) -> Self {
        self.preconditions.push(predicate);
        self
    }

    /// Builder: adiciona output.
    pub fn with_output(mut self, output: OutputSchema) -> Self {
        self.outputs.push(output);
        self
    }

    /// Builder: adiciona assertion.
    pub fn with_assertion(mut self, assertion: Assertion) -> Self {
        self.assertions.push(assertion);
        self
    }

    /// Builder: adiciona validator.
    pub fn with_validator(mut self, validator: ValidatorRef) -> Self {
        self.validators.push(validator);
        self
    }

    /// Builder: adiciona fitness metric.
    pub fn with_fitness(mut self, metric: FitnessMetric) -> Self {
        self.fitness.push(metric);
        self
    }

    /// Builder: anexa metadata.
    pub fn with_metadata_entry(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Valida a consistência interna do contrato.
    pub fn validate(&self) -> Result<(), ContractError> {
        // Validar precondições
        for (i, pred) in self.preconditions.iter().enumerate() {
            if pred.name.is_empty() {
                return Err(ContractError::InvalidPrecondition(format!(
                    "Precondition {} has empty name",
                    i
                )));
            }
        }

        // Validar outputs
        let mut output_names = std::collections::HashSet::new();
        for output in &self.outputs {
            if output.name.is_empty() {
                return Err(ContractError::InvalidOutputSchema(
                    "Output has empty name".into(),
                ));
            }
            if !output_names.insert(&output.name) {
                return Err(ContractError::InvalidOutputSchema(format!(
                    "Duplicate output name: {}",
                    output.name
                )));
            }
        }

        // Validar assertions
        let mut assertion_ids = std::collections::HashSet::new();
        for assertion in &self.assertions {
            if assertion.id.is_empty() {
                return Err(ContractError::InvalidAssertion(
                    "Assertion has empty id".into(),
                ));
            }
            if !assertion_ids.insert(&assertion.id) {
                return Err(ContractError::InvalidAssertion(format!(
                    "Duplicate assertion id: {}",
                    assertion.id
                )));
            }
            if assertion.expression.is_empty() {
                return Err(ContractError::InvalidAssertion(format!(
                    "Assertion '{}' has empty expression",
                    assertion.id
                )));
            }
        }

        // Validar validators
        for validator in &self.validators {
            if validator.function_path.is_empty() {
                return Err(ContractError::ValidatorNotFound(
                    "Validator has empty function_path".into(),
                ));
            }
        }

        // Validar fitness metrics
        for metric in &self.fitness {
            if metric.name.is_empty() {
                return Err(ContractError::InvalidFitnessMetric(
                    "Fitness metric has empty name".into(),
                ));
            }
            if let (Some(min), Some(max)) = (metric.min_threshold, metric.max_threshold) {
                if min > max {
                    return Err(ContractError::InvalidFitnessMetric(format!(
                        "Metric '{}': min_threshold ({}) > max_threshold ({})",
                        metric.name, min, max
                    )));
                }
            }
        }

        Ok(())
    }

    /// Serializa para JSON.
    pub fn to_json(&self) -> Result<String, ContractError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ContractError::SerializationError(e.to_string()))
    }

    /// Desserializa de JSON.
    pub fn from_json(json: &str) -> Result<Self, ContractError> {
        serde_json::from_str(json).map_err(|e| ContractError::SerializationError(e.to_string()))
    }

    /// Retorna o número total de elementos no contrato.
    pub fn element_count(&self) -> usize {
        self.preconditions.len()
            + self.outputs.len()
            + self.assertions.len()
            + self.validators.len()
            + self.fitness.len()
    }

    /// Verifica se todas as fitness metrics estão dentro dos thresholds.
    pub fn all_fitness_ok(&self) -> bool {
        self.fitness.iter().all(|m| m.is_within_threshold())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_contract_validates() {
        let contract = SkillContract::default();
        assert!(contract.validate().is_ok());
        assert_eq!(contract.version, "1.0");
        assert_eq!(contract.element_count(), 0);
    }

    #[test]
    fn test_full_contract_builder() {
        let contract = SkillContract::new()
            .with_precondition(
                Predicate::new("file_exists", vec!["src/main.rs".into()])
                    .with_error("Main file must exist"),
            )
            .with_precondition(Predicate::new("env_var_set", vec!["RUST_LOG".into()]))
            .with_output(OutputSchema::new("report", "object").with_description("Analysis report"))
            .with_output(OutputSchema::new("warnings", "array").optional())
            .with_assertion(
                Assertion::new("non_empty_report", "output.report != null")
                    .with_severity(AssertionSeverity::Error)
                    .with_message("Report must not be null"),
            )
            .with_assertion(
                Assertion::new("max_warnings", "output.warnings.length < 100")
                    .with_severity(AssertionSeverity::Warning),
            )
            .with_validator(ValidatorRef::new("validate_json_schema"))
            .with_validator(
                ValidatorRef::new("check_file_size")
                    .with_param("max_bytes", serde_json::json!(1048576))
                    .optional(),
            )
            .with_fitness(FitnessMetric::new("success_rate", 0.95).with_min(0.8))
            .with_fitness(
                FitnessMetric::new("avg_latency_ms", 150.0)
                    .with_min(0.0)
                    .with_max(500.0),
            )
            .with_metadata_entry("author", serde_json::json!("team-alpha"));

        assert!(contract.validate().is_ok());
        assert_eq!(contract.preconditions.len(), 2);
        assert_eq!(contract.outputs.len(), 2);
        assert_eq!(contract.assertions.len(), 2);
        assert_eq!(contract.validators.len(), 2);
        assert_eq!(contract.fitness.len(), 2);
        assert_eq!(contract.element_count(), 10);
        assert!(contract.all_fitness_ok());
    }

    #[test]
    fn test_invalid_precondition_empty_name() {
        let contract = SkillContract::new().with_precondition(Predicate::new("", vec![]));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidPrecondition(_))
        ));
    }

    #[test]
    fn test_invalid_output_empty_name() {
        let contract = SkillContract::new().with_output(OutputSchema::new("", "string"));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidOutputSchema(_))
        ));
    }

    #[test]
    fn test_duplicate_output_name() {
        let contract = SkillContract::new()
            .with_output(OutputSchema::new("result", "object"))
            .with_output(OutputSchema::new("result", "string"));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidOutputSchema(_))
        ));
    }

    #[test]
    fn test_invalid_assertion_empty_id() {
        let contract = SkillContract::new().with_assertion(Assertion::new("", "x > 0"));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidAssertion(_))
        ));
    }

    #[test]
    fn test_duplicate_assertion_id() {
        let contract = SkillContract::new()
            .with_assertion(Assertion::new("check_1", "x > 0"))
            .with_assertion(Assertion::new("check_1", "y > 0"));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidAssertion(_))
        ));
    }

    #[test]
    fn test_invalid_assertion_empty_expression() {
        let contract = SkillContract::new().with_assertion(Assertion::new("check_1", ""));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidAssertion(_))
        ));
    }

    #[test]
    fn test_invalid_validator_empty_path() {
        let contract = SkillContract::new().with_validator(ValidatorRef::new(""));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::ValidatorNotFound(_))
        ));
    }

    #[test]
    fn test_invalid_fitness_empty_name() {
        let contract = SkillContract::new().with_fitness(FitnessMetric::new("", 0.5));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidFitnessMetric(_))
        ));
    }

    #[test]
    fn test_invalid_fitness_min_gt_max() {
        let contract = SkillContract::new()
            .with_fitness(FitnessMetric::new("score", 0.5).with_min(0.9).with_max(0.1));
        assert!(matches!(
            contract.validate(),
            Err(ContractError::InvalidFitnessMetric(_))
        ));
    }

    #[test]
    fn test_fitness_within_threshold() {
        let metric = FitnessMetric::new("rate", 0.85).with_min(0.5).with_max(1.0);
        assert!(metric.is_within_threshold());

        let below = FitnessMetric::new("rate", 0.3).with_min(0.5);
        assert!(!below.is_within_threshold());

        let above = FitnessMetric::new("latency", 600.0).with_max(500.0);
        assert!(!above.is_within_threshold());

        let no_threshold = FitnessMetric::new("any", 999.0);
        assert!(no_threshold.is_within_threshold());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let contract = SkillContract::new()
            .with_precondition(Predicate::new("rust_installed", vec![]))
            .with_output(OutputSchema::new("binary", "string"))
            .with_assertion(Assertion::new("compiles", "exit_code == 0"))
            .with_validator(ValidatorRef::new("check_binary_size"))
            .with_fitness(FitnessMetric::new("build_time_ms", 2500.0).with_max(5000.0));

        let json = contract.to_json().unwrap();
        let restored = SkillContract::from_json(&json).unwrap();
        assert_eq!(contract, restored);
    }

    #[test]
    fn test_all_fitness_ok() {
        let ok_contract = SkillContract::new()
            .with_fitness(FitnessMetric::new("a", 0.9).with_min(0.5))
            .with_fitness(FitnessMetric::new("b", 100.0).with_max(200.0));
        assert!(ok_contract.all_fitness_ok());

        let bad_contract =
            SkillContract::new().with_fitness(FitnessMetric::new("a", 0.1).with_min(0.5));
        assert!(!bad_contract.all_fitness_ok());
    }

    #[test]
    fn test_optional_output() {
        let output = OutputSchema::new("debug_log", "string").optional();
        assert!(!output.required);
    }

    #[test]
    fn test_predicate_with_error() {
        let pred = Predicate::new("check_x", vec!["arg1".into()]).with_error("X must be present");
        assert_eq!(pred.error_message, Some("X must be present".into()));
    }

    #[test]
    fn test_validator_with_params() {
        let v = ValidatorRef::new("size_check")
            .with_param("max", serde_json::json!(1024))
            .with_param("unit", serde_json::json!("bytes"))
            .optional();
        assert!(!v.required);
        assert_eq!(v.params.len(), 2);
    }

    #[test]
    fn test_assertion_severity_variants() {
        let error = Assertion::new("e", "x").with_severity(AssertionSeverity::Error);
        let warn = Assertion::new("w", "x").with_severity(AssertionSeverity::Warning);
        let info = Assertion::new("i", "x").with_severity(AssertionSeverity::Info);

        assert_eq!(error.severity, AssertionSeverity::Error);
        assert_eq!(warn.severity, AssertionSeverity::Warning);
        assert_eq!(info.severity, AssertionSeverity::Info);
    }

    #[test]
    fn test_contract_error_display() {
        let err = ContractError::InvalidPrecondition("test reason".into());
        assert!(format!("{}", err).contains("test reason"));

        let err2 = ContractError::MissingField("outputs".into());
        assert!(format!("{}", err2).contains("outputs"));
    }
}
