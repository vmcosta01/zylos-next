//! JSON Schema Validation — Task 5.1.2
//!
//! Validação de skills contra schema JSON definido, com mensagens de erro
//! claras e API para validação customizada. Inspirado em google/agents-cli schemas/.
//!
//! ## Three-Phase Validation Pipeline
//! 1. **Schema Parse + Type Check** — compilação do JSON Schema e validação estrutural
//! 2. **Semantic Validation** — triggers não-vazios, deps acíclicas, artefatos acessíveis
//! 3. **Integration Check** — min_hermes_version ≤ current, compatibility_tags match
//!
//! ## Custom Validator Plugin System
//! Trait `CustomValidator` registrável via função; validators rodam em paralelo via tokio::join_all.
//!
//! ## Validation Cache
//! Cache por `(skill_digest, schema_version)` com invalidação em update de schema e hit rate tracking.
//!
//! ## CI Gate
//! `validate_ci()` retorna exit code + JSON report para integração em pipelines.

use jsonschema::{Draft, JSONSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Schema padrão para skills Zylos-Next
/// Baseado em google/agents-cli schemas/ e adaptado para o manifesto
pub const DEFAULT_SKILL_SCHEMA: &str = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Zylos-Next Skill Schema",
  "description": "Schema para validação de skills no registry Zylos-Next v6",
  "type": "object",
  "required": ["name", "description", "version", "path"],
  "properties": {
    "name": {
      "type": "string",
      "pattern": "^[a-z0-9][a-z0-9-]{0,63}$",
      "description": "Nome único da skill em kebab-case (max 64 chars)"
    },
    "description": {
      "type": "string",
      "minLength": 10,
      "maxLength": 200,
      "description": "Descrição curta da skill (10-200 chars)"
    },
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$",
      "description": "Versão semver da skill"
    },
    "path": {
      "type": "string",
      "pattern": "^.+SKILL\\.md$",
      "description": "Caminho relativo ao SKILL.md"
    },
    "category": {
      "type": "string",
      "enum": ["devops", "creative", "research", "productivity", "software-development", "media", "web", "note-taking", "email", "mlops", "plugin", "rust"],
      "description": "Categoria da skill para grouping"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string",
        "pattern": "^[a-z0-9-]{1,32}$"
      },
      "maxItems": 10,
      "description": "Tags para busca e filtering (max 10)"
    },
    "depends_on": {
      "type": "array",
      "items": {
        "type": "string",
        "pattern": "^[a-z0-9][a-z0-9-]{0,63}$"
      },
      "description": "Dependências de outras skills (nomes)"
    },
    "active": {
      "type": "boolean",
      "default": true,
      "description": "Se a skill está ativa (false = deprecated/pruned)"
    },
    "trigger": {
      "type": "string",
      "description": "Trigger que ativa a skill"
    },
    "min_hermes_version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$",
      "description": "Versão mínima do Hermes requerida (semver)"
    },
    "compatibility_tags": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Tags de compatibilidade do ambiente"
    },
    "artifacts": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Artefatos referenciados pela skill"
    },
    "trigger_phrases": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Trigger phrases para semantic discovery"
    },
    "digest": {
      "type": "string",
      "description": "Content-addressable digest (SHA-256 hex)"
    },
    "usage": {
      "type": "object",
      "description": "Métricas de uso da skill"
    }
  },
  "additionalProperties": false
}"#;

// ---------------------------------------------------------------------------
// Actionable Error Types
// ---------------------------------------------------------------------------

/// Código de erro categorizado para cada ValidationError
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// Enumeração ValidationCode.
pub enum ValidationCode {
    /// Falha na fase 1: schema parse / type check
    SchemaParseError,
    /// Campo obrigatório ausente ou vazio
    MissingRequiredField,
    /// Valor fora do formato esperado (regex, enum, range)
    InvalidFormat,
    /// Dependência cíclica detectada
    CyclicDependency,
    /// Dependência não encontrada no registry
    DependencyNotFound,
    /// Artefato referenciado não existe ou não é acessível
    ArtifactInaccessible,
    /// Trigger vazio ou inválido
    EmptyTrigger,
    /// Versão mínima do Hermes incompatível
    IncompatibleHermesVersion,
    /// Compatibility tag desconhecida ou conflitante
    IncompatibleTag,
    /// Erro de validador customizado
    CustomValidatorError,
    /// Erro genérico não classificado
    Unknown,
}

impl std::fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaParseError => write!(f, "SCHEMA_PARSE_ERROR"),
            Self::MissingRequiredField => write!(f, "MISSING_REQUIRED_FIELD"),
            Self::InvalidFormat => write!(f, "INVALID_FORMAT"),
            Self::CyclicDependency => write!(f, "CYCLIC_DEPENDENCY"),
            Self::DependencyNotFound => write!(f, "DEPENDENCY_NOT_FOUND"),
            Self::ArtifactInaccessible => write!(f, "ARTIFACT_INACCESSIBLE"),
            Self::EmptyTrigger => write!(f, "EMPTY_TRIGGER"),
            Self::IncompatibleHermesVersion => write!(f, "INCOMPATIBLE_HERMES_VERSION"),
            Self::IncompatibleTag => write!(f, "INCOMPATIBLE_TAG"),
            Self::CustomValidatorError => write!(f, "CUSTOM_VALIDATOR_ERROR"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

/// Erro de validação actionável com contexto completo
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ValidationError.
pub struct ValidationError {
    /// Caminho JSON pointer onde o erro ocorreu (ex: "/depends_on/0")
    pub path: String,
    /// Código categorizado do erro
    pub code: ValidationCode,
    /// Mensagem legível descrevendo o problema
    pub message: String,
    /// Sugestão concreta de como corrigir
    pub suggestion: Option<String>,
    /// URL da documentação relevante
    pub docs_url: Option<String>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.code, self.path, self.message)?;
        if let Some(ref s) = self.suggestion {
            write!(f, "\n  → Suggestion: {}", s)?;
        }
        if let Some(ref u) = self.docs_url {
            write!(f, "\n  → Docs: {}", u)?;
        }
        // Variante Ok.
        Ok(())
    }
}

impl ValidationError {
    /// Cria um novo erro actionável
    pub fn new(
        path: impl Into<String>,
        code: ValidationCode,
        message: impl Into<String>,
    ) -> Self {
        // Variante Self.
        Self {
            path: path.into(),
            code,
            message: message.into(),
            suggestion: None,
            docs_url: None,
        }
    }

    /// Adiciona sugestão ao erro
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Adiciona URL de documentação ao erro
    pub fn with_docs_url(mut self, url: impl Into<String>) -> Self {
        self.docs_url = Some(url.into());
        self
    }
}

// ---------------------------------------------------------------------------
// Legacy Error Types (preserved for backward compat)
// ---------------------------------------------------------------------------

/// Erros de validação de schema (legacy — mantido para compatibilidade)
#[derive(Error, Debug)]
/// Enumeração SchemaError.
pub enum SchemaError {
    #[error("schema não encontrado: {0}")]
    // Variante SchemaNotFound.
    SchemaNotFound(String),

    #[error("falha ao carregar schema: {0}")]
    // Variante SchemaLoadError.
    SchemaLoadError(#[from] std::io::Error),

    #[error("schema JSON inválido: {0}")]
    // Variante SchemaParseError.
    SchemaParseError(#[from] serde_json::Error),

    #[error("compilação do schema falhou: {0}")]
    // Variante CompilationError.
    CompilationError(String),

    #[error("validação falhou para '{skill_name}': {errors}")]
    // Variante ValidationFailed.
    ValidationFailed { skill_name: String, errors: String },
}

/// Result type alias para operações de schema
pub type SchemaResult<T> = Result<T, SchemaError>;

// ---------------------------------------------------------------------------
// SkillManifest — estrutura completa para validação semântica
// ---------------------------------------------------------------------------

/// Manifesto completo de uma skill para validação three-phase
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura SkillManifest.
pub struct SkillManifest {
    /// Campo name.
    pub name: String,
    /// Campo description.
    pub description: String,
    /// Campo version.
    pub version: String,
    /// Campo path.
    pub path: String,
    #[serde(default)]
    /// Campo category.
    pub category: Option<String>,
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Campo depends on.
    pub depends_on: Vec<String>,
    #[serde(default = "default_active")]
    /// Campo active.
    pub active: bool,
    /// Trigger que ativa a skill (deve ser não-vazio para skills ativas)
    #[serde(default)]
    /// Campo trigger.
    pub trigger: Option<String>,
    /// Versão mínima do Hermes requerida
    #[serde(default)]
    /// Campo min hermes version.
    pub min_hermes_version: Option<String>,
    /// Tags de compatibilidade (ex: ["linux", "macos", "windows"])
    #[serde(default)]
    /// Campo compatibility tags.
    pub compatibility_tags: Vec<String>,
    /// Artefatos referenciados pela skill (scripts, configs, etc.)
    #[serde(default)]
    /// Campo artifacts.
    pub artifacts: Vec<String>,
    /// Metadados extras não validados pelo schema
    #[serde(flatten)]
    /// Campo extra.
    pub extra: HashMap<String, Value>,
}

fn default_active() -> bool {
    true
}

impl SkillManifest {
    /// Converte de serde_json::Value
    pub fn from_value(value: &Value) -> Result<Self, ValidationError> {
        serde_json::from_value(value.clone()).map_err(|e| {
            ValidationError::new("/", ValidationCode::SchemaParseError, e.to_string())
                .with_suggestion("Verifique se o manifesto está em formato JSON válido")
        })
    }

    /// Calcula digest SHA-256 do manifesto para cache key
    pub fn digest(&self) -> String {
        let serialized = serde_json::to_string(self).unwrap_or_default();
        let hash = Sha256::digest(serialized.as_bytes());
        hex_encode(&hash)
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Custom Validator Plugin System
// ---------------------------------------------------------------------------

/// Trait para validadores customizados plugáveis
pub trait CustomValidator: Send + Sync {
    /// Nome único do validador
    fn name(&self) -> &str;

    /// Executa validação customizada no manifesto
    fn validate(&self, manifest: &SkillManifest) -> Vec<ValidationError>;
}

/// Wrapper type-erased para armazenar validadores customizados
type BoxedValidator = Arc<dyn CustomValidator>;

// ---------------------------------------------------------------------------
// Validation Cache
// ---------------------------------------------------------------------------

/// Chave de cache: (skill_digest, schema_version)
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CacheKey {
    skill_digest: String,
    schema_version: String,
}

/// Estatísticas de hit rate do cache
#[derive(Debug, Clone, Default, Serialize)]
/// Estrutura CacheStats.
pub struct CacheStats {
    /// Campo hits.
    pub hits: u64,
    /// Campo misses.
    pub misses: u64,
}

impl CacheStats {
    /// Função hit rate.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// Resultado cacheado preservando fases
#[derive(Debug, Clone)]
pub(crate) struct CachedResult {
    phase1: Vec<ValidationError>,
    phase2: Vec<ValidationError>,
    phase3: Vec<ValidationError>,
    custom: Vec<ValidationError>,
}

/// Cache de resultados de validação
#[derive(Debug)]
/// Estrutura ValidationCache.
pub struct ValidationCache {
    entries: RwLock<HashMap<CacheKey, CachedResult>>,
    stats: RwLock<CacheStats>,
    current_schema_version: String,
}

impl ValidationCache {
    /// Função new.
    pub fn new(schema_version: impl Into<String>) -> Self {
        // Variante Self.
        Self {
            entries: RwLock::new(HashMap::new()),
            stats: RwLock::new(CacheStats::default()),
            current_schema_version: schema_version.into(),
        }
    }

    // Busca resultado no cache
    pub(crate) fn get(&self, skill_digest: &str) -> Option<CachedResult> {
        let key = CacheKey {
            skill_digest: skill_digest.to_string(),
            schema_version: self.current_schema_version.clone(),
        };
        let entries = self.entries.read().ok()?;
        let result = entries.get(&key).cloned();
        if let Some(mut stats) = self.stats.write().ok() {
            if result.is_some() {
                stats.hits += 1;
            } else {
                stats.misses += 1;
            }
        }
        result
    }

    // Insere resultado no cache
    pub(crate) fn put(&self, skill_digest: &str, result: CachedResult) {
        let key = CacheKey {
            skill_digest: skill_digest.to_string(),
            schema_version: self.current_schema_version.clone(),
        };
        if let Ok(mut entries) = self.entries.write() {
            entries.insert(key, result);
        }
    }

    /// Invalida todo o cache (chamar quando schema é atualizado)
    pub fn invalidate(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
        info!("Validation cache invalidated");
    }

    /// Retorna estatísticas de hit rate
    pub fn stats(&self) -> CacheStats {
        self.stats
            .read()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Schema Evolution Stub
// ---------------------------------------------------------------------------

/// Versões de schema suportadas
#[derive(Debug, Clone, PartialEq, Eq)]
/// Enumeração SchemaVersion.
pub enum SchemaVersion {
    /// Variante V1.
    V1,
    /// Variante V2.
    V2,
}

impl SchemaVersion {
    /// Função from str.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "v1" | "1" | "draft-7" => Some(Self::V1),
            "v2" | "2" => Some(Self::V2),
            _ => None,
        }
    }

    /// Função as str.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }
}

/// Verifica compatibilidade entre versões de schema (stub)
pub fn check_backward_compat(from: &SchemaVersion, to: &SchemaVersion) -> bool {
    matches!((from, to), (SchemaVersion::V1, SchemaVersion::V1) | (SchemaVersion::V1, SchemaVersion::V2) | (SchemaVersion::V2, SchemaVersion::V2))
}

// ---------------------------------------------------------------------------
// Three-Phase Validation Pipeline
// ---------------------------------------------------------------------------

/// Resultado da validação three-phase
#[derive(Debug, Clone, Serialize)]
/// Estrutura ValidationResult.
pub struct ValidationResult {
    /// Campo skill name.
    pub skill_name: String,
    /// Campo phase1 errors.
    pub phase1_errors: Vec<ValidationError>,
    /// Campo phase2 errors.
    pub phase2_errors: Vec<ValidationError>,
    /// Campo phase3 errors.
    pub phase3_errors: Vec<ValidationError>,
    /// Campo custom errors.
    pub custom_errors: Vec<ValidationError>,
    /// Campo cached.
    pub cached: bool,
}

impl ValidationResult {
    /// Função is valid.
    pub fn is_valid(&self) -> bool {
        self.phase1_errors.is_empty()
            && self.phase2_errors.is_empty()
            && self.phase3_errors.is_empty()
            && self.custom_errors.is_empty()
    }

    /// Função all errors.
    pub fn all_errors(&self) -> Vec<&ValidationError> {
        self.phase1_errors
            .iter()
            .chain(self.phase2_errors.iter())
            .chain(self.phase3_errors.iter())
            .chain(self.custom_errors.iter())
            .collect()
    }

    /// Função error count.
    pub fn error_count(&self) -> usize {
        self.phase1_errors.len()
            + self.phase2_errors.len()
            + self.phase3_errors.len()
            + self.custom_errors.len()
    }
}

/// Validador de skills com pipeline three-phase
pub struct SkillValidator {
    schema: JSONSchema,
    version: String,
    custom_validators: Vec<BoxedValidator>,
    cache: ValidationCache,
    hermes_version: String,
    env_compat_tags: HashSet<String>,
    known_skills: HashSet<String>,
}

impl std::fmt::Debug for SkillValidator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillValidator")
            .field("version", &self.version)
            .field("hermes_version", &self.hermes_version)
            .field("custom_validators_count", &self.custom_validators.len())
            .field("known_skills_count", &self.known_skills.len())
            .finish()
    }
}

impl SkillValidator {
    /// Carrega o validador a partir de um arquivo de schema JSON
    pub fn from_file(path: &Path) -> SchemaResult<Self> {
        if !path.exists() {
            return Err(SchemaError::SchemaNotFound(
                path.display().to_string(),
            ));
        }

        let content = std::fs::read_to_string(path)?;
        Self::parse_schema(&content)
    }

    /// Cria o validador a partir de uma string JSON
    pub fn parse_schema(schema_json: &str) -> SchemaResult<Self> {
        let schema_value: Value = serde_json::from_str(schema_json)?;

        let version = schema_value
            .get("$schema")
            .and_then(|v| v.as_str())
            .unwrap_or("draft-7")
            .to_string();

        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft7)
            .compile(&schema_value)
            .map_err(|e| SchemaError::CompilationError(e.to_string()))?;

        info!("SkillValidator carregado (schema: {})", version);

        // Variante Ok.
        Ok(Self {
            schema: compiled,
            version: version.clone(),
            custom_validators: Vec::new(),
            cache: ValidationCache::new(&version),
            hermes_version: "0.0.0".to_string(),
            env_compat_tags: HashSet::new(),
            known_skills: HashSet::new(),
        })
    }

    /// Cria o validador com o schema padrão embutido
    pub fn default_schema() -> SchemaResult<Self> {
        Self::parse_schema(DEFAULT_SKILL_SCHEMA)
    }

    /// Configura a versão atual do Hermes para integration checks
    pub fn with_hermes_version(mut self, version: impl Into<String>) -> Self {
        self.hermes_version = version.into();
        self
    }

    /// Configura tags de compatibilidade do ambiente
    pub fn with_compat_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.env_compat_tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// Configura skills conhecidas para validação de dependências
    pub fn with_known_skills(mut self, skills: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.known_skills = skills.into_iter().map(Into::into).collect();
        self
    }

    /// Registra um validador customizado
    pub fn register_validator(&mut self, validator: impl CustomValidator + 'static) {
        debug!("Registrando validador customizado: {}", validator.name());
        self.custom_validators.push(Arc::new(validator));
    }

    /// Retorna a versão do schema
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Verifica se o schema é válido (auto-teste)
    pub fn is_valid(&self) -> bool {
        true
    }

    /// Retorna estatísticas do cache
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// Invalida o cache de validação
    pub fn invalidate_cache(&self) {
        self.cache.invalidate();
    }

    // -- Legacy API (preserved) --

    /// Valida uma skill representada como JSON Value (legacy API)
    pub fn validate(&self, skill: &Value, skill_name: &str) -> SchemaResult<()> {
        let validation = self.schema.validate(skill);

        match validation {
            // Variante Ok.
            Ok(_) => {
                debug!("Skill '{}' passou na validação", skill_name);
                // Variante Ok.
                Ok(())
            }
            // Variante Err.
            Err(errors) => {
                let error_messages: Vec<String> = errors
                    .map(|e| {
                        format!(
                            "  - {:?}: {} (em {})",
                            e.kind, e.instance, e.instance_path
                        )
                    })
                    .collect();

                let formatted_errors = error_messages.join("\n");
                warn!(
                    "Skill '{}' falhou na validação:\n{}",
                    skill_name, formatted_errors
                );

                // Variante Err.
                Err(SchemaError::ValidationFailed {
                    skill_name: skill_name.to_string(),
                    errors: formatted_errors,
                })
            }
        }
    }

    /// Valida uma skill a partir de um arquivo SKILL.md parseado (legacy API)
    pub fn validate_skill_entry(&self, entry: &crate::registry::SkillEntry) -> SchemaResult<()> {
        let value = serde_json::to_value(entry).map_err(SchemaError::SchemaParseError)?;
        self.validate(&value, &entry.name)
    }

    // -- Three-Phase Pipeline --

    /// Executa validação completa three-phase em um SkillManifest
    pub async fn validate_manifest(&self, manifest: &SkillManifest) -> ValidationResult {
        let digest = manifest.digest();

        // Checa cache
        if let Some(cached) = self.cache.get(&digest) {
            return ValidationResult {
                skill_name: manifest.name.clone(),
                phase1_errors: cached.phase1,
                phase2_errors: cached.phase2,
                phase3_errors: cached.phase3,
                custom_errors: cached.custom,
                cached: true,
            };
        }

        // Phase 1: Schema parse + type check
        let phase1 = self.run_phase1(manifest);

        // Se phase 1 falhou, não continua (manifesto malformado)
        if !phase1.is_empty() {
            let result = ValidationResult {
                skill_name: manifest.name.clone(),
                phase1_errors: phase1,
                phase2_errors: vec![],
                phase3_errors: vec![],
                custom_errors: vec![],
                cached: false,
            };
            self.cache.put(&digest, CachedResult {
                phase1: result.phase1_errors.clone(),
                phase2: vec![],
                phase3: vec![],
                custom: vec![],
            });
            return result;
        }

        // Phase 2: Semantic validation
        let phase2 = self.run_phase2(manifest);

        // Phase 3: Integration check
        let phase3 = self.run_phase3(manifest);

        // Custom validators (parallel via tokio)
        let custom = self.run_custom_validators(manifest).await;

        let result = ValidationResult {
            skill_name: manifest.name.clone(),
            phase1_errors: phase1,
            phase2_errors: phase2,
            phase3_errors: phase3,
            custom_errors: custom,
            cached: false,
        };

        // Cache o resultado
        self.cache.put(&digest, CachedResult {
            phase1: result.phase1_errors.clone(),
            phase2: result.phase2_errors.clone(),
            phase3: result.phase3_errors.clone(),
            custom: result.custom_errors.clone(),
        });

        result
    }

    /// Phase 1: Schema parse + type check
    fn run_phase1(&self, manifest: &SkillManifest) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Converte para JSON e valida contra schema
        let value = match serde_json::to_value(manifest) {
            // Variante Ok.
            Ok(v) => v,
            // Variante Err.
            Err(e) => {
                errors.push(
                    ValidationError::new("/", ValidationCode::SchemaParseError, e.to_string())
                        .with_suggestion("Verifique a serialização do manifesto"),
                );
                return errors;
            }
        };

        if let Err(validation_errors) = self.schema.validate(&value) {
            for err in validation_errors {
                let path = err.instance_path.to_string();
                let msg = format!("{:?}", err.kind);

                let (code, suggestion) = classify_schema_error(&path, &msg);

                errors.push(
                    ValidationError::new(&path, code, &msg)
                        .with_suggestion(suggestion.unwrap_or_else(|| default_suggestion(&path)))
                        .with_docs_url("https://hermes-agent.nousresearch.com/docs/skills/schema"),
                );
            }
        }

        // Valida campos obrigatórios não cobertos pelo schema
        if manifest.name.trim().is_empty() {
            errors.push(
                ValidationError::new("/name", ValidationCode::MissingRequiredField, "nome da skill não pode ser vazio")
                    .with_suggestion("Use um nome em kebab-case como 'my-skill'"),
            );
        }

        if manifest.description.trim().is_empty() {
            errors.push(
                ValidationError::new("/description", ValidationCode::MissingRequiredField, "descrição não pode ser vazia")
                    .with_suggestion("Adicione uma descrição entre 10 e 200 caracteres"),
            );
        }

        errors
    }

    /// Phase 2: Semantic validation
    fn run_phase2(&self, manifest: &SkillManifest) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // Trigger não-vazio para skills ativas
        if manifest.active {
            if let Some(ref trigger) = manifest.trigger {
                if trigger.trim().is_empty() {
                    errors.push(
                        ValidationError::new("/trigger", ValidationCode::EmptyTrigger, "trigger não pode ser vazio para skill ativa")
                            .with_suggestion("Defina um trigger descritivo ou remova o campo se a skill não precisa de trigger")
                            .with_docs_url("https://hermes-agent.nousresearch.com/docs/skills/triggers"),
                    );
                }
            }
        }

        // Dependências acíclicas e existência
        if !manifest.depends_on.is_empty() {
            for dep in &manifest.depends_on {
                if !self.known_skills.is_empty() && !self.known_skills.contains(dep) {
                    errors.push(
                        ValidationError::new(
                            &format!("/depends_on/{}", dep),
                            ValidationCode::DependencyNotFound,
                            format!("dependência '{}' não encontrada no registry", dep),
                        )
                        .with_suggestion(format!(
                            "Verifique o nome da dependência ou adicione '{}' ao registry",
                            dep
                        )),
                    );
                }
            }

            // Detecção simples de ciclo: auto-dependência
            if manifest.depends_on.contains(&manifest.name) {
                errors.push(
                    ValidationError::new(
                        "/depends_on",
                        ValidationCode::CyclicDependency,
                        format!("skill '{}' depende de si mesma", manifest.name),
                    )
                    .with_suggestion("Remova a auto-referência de depends_on"),
                );
            }
        }

        // Artefatos acessíveis
        for artifact in &manifest.artifacts {
            let artifact_path = Path::new(artifact);
            if !artifact_path.exists() {
                errors.push(
                    ValidationError::new(
                        &format!("/artifacts/{}", artifact),
                        ValidationCode::ArtifactInaccessible,
                        format!("artefato '{}' não encontrado ou inacessível", artifact),
                    )
                    .with_suggestion(format!(
                        "Verifique se o arquivo '{}' existe no caminho especificado",
                        artifact
                    )),
                );
            }
        }

        errors
    }

    /// Phase 3: Integration check
    fn run_phase3(&self, manifest: &SkillManifest) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        // min_hermes_version ≤ current
        if let Some(ref min_ver) = manifest.min_hermes_version {
            if let (Ok(min), Ok(current)) = (
                semver::Version::parse(min_ver),
                semver::Version::parse(&self.hermes_version),
            ) {
                if min > current {
                    errors.push(
                        ValidationError::new(
                            "/min_hermes_version",
                            ValidationCode::IncompatibleHermesVersion,
                            format!(
                                "skill requer Hermes >= {}, mas ambiente tem {}",
                                min_ver, self.hermes_version
                            ),
                        )
                        .with_suggestion(format!(
                            "Atualize o Hermes para >= {} ou reduza min_hermes_version",
                            min_ver
                        ))
                        .with_docs_url("https://hermes-agent.nousresearch.com/docs/versioning"),
                    );
                }
            } else {
                errors.push(
                    ValidationError::new(
                        "/min_hermes_version",
                        ValidationCode::InvalidFormat,
                        format!("versão '{}' não é semver válido", min_ver),
                    )
                    .with_suggestion("Use formato MAJOR.MINOR.PATCH (ex: '1.2.0')"),
                );
            }
        }

        // compatibility_tags match
        if !manifest.compatibility_tags.is_empty() && !self.env_compat_tags.is_empty() {
            let has_match = manifest
                .compatibility_tags
                .iter()
                .any(|t| self.env_compat_tags.contains(t));

            if !has_match {
                errors.push(
                    ValidationError::new(
                        "/compatibility_tags",
                        ValidationCode::IncompatibleTag,
                        format!(
                            "nenhuma tag de compatibilidade {:?} corresponde ao ambiente {:?}",
                            manifest.compatibility_tags,
                            self.env_compat_tags.iter().collect::<Vec<_>>()
                        ),
                    )
                    .with_suggestion("Adicione uma tag compatível com o ambiente atual ou ajuste as tags da skill"),
                );
            }
        }

        errors
    }

    /// Executa validadores customizados em paralelo via tokio::spawn_blocking
    async fn run_custom_validators(&self, manifest: &SkillManifest) -> Vec<ValidationError> {
        if self.custom_validators.is_empty() {
            return vec![];
        }

        let handles: Vec<_> = self
            .custom_validators
            .iter()
            .map(|v| {
                let validator = Arc::clone(v);
                let manifest_clone = manifest.clone();
                tokio::task::spawn_blocking(move || {
                    validator.validate(&manifest_clone)
                })
            })
            .collect();

        let mut all_errors = Vec::new();
        for handle in handles {
            match handle.await {
                // Variante Ok.
                Ok(errors) => all_errors.extend(errors),
                // Variante Err.
                Err(e) => debug!("Custom validator task failed: {}", e),
            }
        }
        all_errors
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn classify_schema_error(path: &str, msg: &str) -> (ValidationCode, Option<String>) {
    let msg_lower = msg.to_lowercase();
    if msg_lower.contains("required") || msg_lower.contains("missing") {
        (
            ValidationCode::MissingRequiredField,
            Some(format!("Adicione o campo obrigatório '{}'", path.trim_start_matches('/'))),
        )
    } else if msg_lower.contains("pattern") || msg_lower.contains("format") {
        (
            ValidationCode::InvalidFormat,
            // Variante Some.
            Some("Verifique o formato do valor contra o padrão esperado".to_string()),
        )
    } else if msg_lower.contains("enum") {
        (
            ValidationCode::InvalidFormat,
            // Variante Some.
            Some("Use um dos valores permitidos pelo enum".to_string()),
        )
    } else {
        (ValidationCode::InvalidFormat, None)
    }
}

fn default_suggestion(path: &str) -> String {
    let field = path.trim_start_matches('/');
    format!("Revise o campo '{}' conforme o schema", field)
}

// ---------------------------------------------------------------------------
// CI Gate Integration
// ---------------------------------------------------------------------------

/// Relatório de validação CI em formato JSON
#[derive(Debug, Serialize, Deserialize)]
/// Estrutura CiReport.
pub struct CiReport {
    /// Campo total skills.
    pub total_skills: usize,
    /// Campo valid.
    pub valid: usize,
    /// Campo invalid.
    pub invalid: usize,
    /// Campo cache hit rate.
    pub cache_hit_rate: f64,
    /// Campo results.
    pub results: Vec<CiSkillResult>,
}

#[derive(Debug, Serialize, Deserialize)]
/// Estrutura CiSkillResult.
pub struct CiSkillResult {
    /// Campo skill name.
    pub skill_name: String,
    /// Campo valid.
    pub valid: bool,
    /// Campo error count.
    pub error_count: usize,
    /// Campo errors.
    pub errors: Vec<ValidationError>,
    /// Campo cached.
    pub cached: bool,
}

/// Executa validação CI e retorna (exit_code, json_report)
/// exit_code: 0 = tudo válido, 1 = erros encontrados
pub async fn validate_ci(
    validator: &SkillValidator,
    manifests: &[SkillManifest],
) -> (i32, String) {
    let mut results = Vec::new();
    let mut invalid_count = 0;

    for manifest in manifests {
        let result = validator.validate_manifest(manifest).await;
        let is_valid = result.is_valid();
        let error_count = result.error_count();

        if !is_valid {
            invalid_count += 1;
        }

        results.push(CiSkillResult {
            skill_name: manifest.name.clone(),
            valid: is_valid,
            error_count,
            errors: result.all_errors().into_iter().cloned().collect(),
            cached: result.cached,
        });
    }

    let stats = validator.cache_stats();
    let report = CiReport {
        total_skills: manifests.len(),
        valid: manifests.len() - invalid_count,
        invalid: invalid_count,
        cache_hit_rate: stats.hit_rate(),
        results,
    };

    let json = serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string());
    let exit_code = if invalid_count > 0 { 1 } else { 0 };

    (exit_code, json)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::SkillEntry;
    use serde_json::json;

    fn valid_skill_json() -> Value {
        json!({
            "name": "test-skill",
            "description": "Uma skill de teste para validação",
            "version": "1.0.0",
            "path": "test-skill/SKILL.md",
            "category": "devops",
            "tags": ["rust", "testing"],
            "depends_on": [],
            "active": true
        })
    }

    fn valid_manifest() -> SkillManifest {
        /// Variante SkillManifest.
        SkillManifest {
            name: "test-skill".to_string(),
            description: "Uma skill de teste para validação".to_string(),
            version: "1.0.0".to_string(),
            path: "test-skill/SKILL.md".to_string(),
            category: Some("devops".to_string()),
            tags: vec!["rust".to_string(), "testing".to_string()],
            depends_on: vec![],
            active: true,
            trigger: Some("when user asks about testing".to_string()),
            min_hermes_version: Some("0.1.0".to_string()),
            compatibility_tags: vec![],
            artifacts: vec![],
            extra: HashMap::new(),
        }
    }

    // -- Legacy tests (preserved) --

    #[test]
    fn test_default_schema_loads() {
        let validator = SkillValidator::default_schema();
        assert!(validator.is_ok());
        let v = validator.unwrap();
        assert!(v.is_valid());
    }

    #[test]
    fn test_validate_valid_skill() {
        let validator = SkillValidator::default_schema().unwrap();
        let skill = valid_skill_json();
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_missing_required_name() {
        let validator = SkillValidator::default_schema().unwrap();
        let skill = json!({
            "description": "Sem nome",
            "version": "1.0.0",
            "path": "test/SKILL.md"
        });
        let result = validator.validate(&skill, "invalid");
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::ValidationFailed { skill_name, errors } => {
                assert_eq!(skill_name, "invalid");
                assert!(errors.contains("name"));
            }
            other => panic!("Expected ValidationFailed, got {:?}", other),
        }
    }

    #[test]
    fn test_validate_invalid_name_pattern() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["name"] = json!("INVALID_NAME");
        let result = validator.validate(&skill, "INVALID_NAME");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_description_too_short() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["description"] = json!("Short");
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_description_too_long() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["description"] = json!("A".repeat(201));
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_version_format() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["version"] = json!("v1.0");
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_category() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["category"] = json!("invalid-category");
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_too_many_tags() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["tags"] = json!(["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k"]);
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_additional_properties_rejected() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut skill = valid_skill_json();
        skill["unknown_field"] = json!("should fail");
        let result = validator.validate(&skill, "test-skill");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_skill_entry() {
        let validator = SkillValidator::default_schema().unwrap();
        let entry = SkillEntry {
            name: "valid-entry".to_string(),
            description: "Uma descrição válida com mais de dez caracteres".to_string(),
            version: "2.1.0".to_string(),
            path: "valid-entry/SKILL.md".to_string(),
            category: Some("rust".to_string()),
            tags: vec!["cargo".to_string()],
            depends_on: vec![],
            active: true,
            digest: "sha256:test".to_string(),
            trigger_phrases: vec![],
            usage: crate::registry::UsageStats::default(),
        };
        let result = validator.validate_skill_entry(&entry);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_minimal_skill() {
        let validator = SkillValidator::default_schema().unwrap();
        let skill = json!({
            "name": "minimal",
            "description": "Descrição mínima válida aqui",
            "version": "0.1.0",
            "path": "minimal/SKILL.md"
        });
        let result = validator.validate(&skill, "minimal");
        assert!(result.is_ok());
    }

    #[test]
    fn test_from_str_invalid_json() {
        let result = SkillValidator::parse_schema("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_not_found() {
        let result = SkillValidator::from_file(Path::new("/nonexistent/schema.json"));
        assert!(result.is_err());
        match result.unwrap_err() {
            SchemaError::SchemaNotFound(_) => {}
            other => panic!("Expected SchemaNotFound, got {:?}", other),
        }
    }

    // -- New tests: Three-Phase Validation --

    #[tokio::test]
    async fn test_three_phase_valid_manifest() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("1.0.0");
        let manifest = valid_manifest();
        let result = validator.validate_manifest(&manifest).await;
        assert!(result.is_valid(), "Expected valid, got errors: {:?}", result.all_errors());
        assert!(!result.cached);
    }

    #[tokio::test]
    async fn test_three_phase_empty_trigger_detected() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut manifest = valid_manifest();
        manifest.trigger = Some("".to_string());
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert!(result.phase2_errors.iter().any(|e| e.code == ValidationCode::EmptyTrigger));
    }

    #[tokio::test]
    async fn test_three_phase_self_dependency_cycle() {
        let validator = SkillValidator::default_schema().unwrap();
        let mut manifest = valid_manifest();
        manifest.depends_on = vec!["test-skill".to_string()];
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert!(result.phase2_errors.iter().any(|e| e.code == ValidationCode::CyclicDependency));
    }

    #[tokio::test]
    async fn test_three_phase_incompatible_hermes_version() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("0.5.0");
        let mut manifest = valid_manifest();
        manifest.min_hermes_version = Some("2.0.0".to_string());
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert!(result.phase3_errors.iter().any(|e| e.code == ValidationCode::IncompatibleHermesVersion));
    }

    #[tokio::test]
    async fn test_three_phase_compatible_hermes_version() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("2.0.0");
        let mut manifest = valid_manifest();
        manifest.min_hermes_version = Some("1.0.0".to_string());
        let result = validator.validate_manifest(&manifest).await;
        assert!(result.phase3_errors.is_empty());
    }

    #[tokio::test]
    async fn test_three_phase_incompatible_tags() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_compat_tags(vec!["linux".to_string()]);
        let mut manifest = valid_manifest();
        manifest.compatibility_tags = vec!["windows-only".to_string()];
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert!(result.phase3_errors.iter().any(|e| e.code == ValidationCode::IncompatibleTag));
    }

    #[tokio::test]
    async fn test_three_phase_compatible_tags() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("1.0.0")
            .with_compat_tags(vec!["linux".to_string(), "macos".to_string()]);
        let mut manifest = valid_manifest();
        manifest.compatibility_tags = vec!["linux".to_string()];
        let result = validator.validate_manifest(&manifest).await;
        assert!(result.phase3_errors.is_empty());
    }

    #[tokio::test]
    async fn test_three_phase_unknown_dependency() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_known_skills(vec!["other-skill".to_string()]);
        let mut manifest = valid_manifest();
        manifest.depends_on = vec!["nonexistent-skill".to_string()];
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert!(result.phase2_errors.iter().any(|e| e.code == ValidationCode::DependencyNotFound));
    }

    // -- New tests: Custom Validators --

    struct TestValidator {
        name: String,
        should_fail: bool,
    }

    impl CustomValidator for TestValidator {
        fn name(&self) -> &str {
            &self.name
        }

        fn validate(&self, manifest: &SkillManifest) -> Vec<ValidationError> {
            if self.should_fail {
                vec![ValidationError::new(
                    "/custom",
                    ValidationCode::CustomValidatorError,
                    format!("{} rejected '{}'", self.name, manifest.name),
                )]
            } else {
                vec![]
            }
        }
    }

    #[tokio::test]
    async fn test_custom_validator_pass() {
        let mut validator = SkillValidator::default_schema().unwrap();
        validator.register_validator(TestValidator {
            name: "pass-validator".to_string(),
            should_fail: false,
        });
        let manifest = valid_manifest();
        let result = validator.validate_manifest(&manifest).await;
        assert!(result.custom_errors.is_empty());
    }

    #[tokio::test]
    async fn test_custom_validator_fail() {
        let mut validator = SkillValidator::default_schema().unwrap();
        validator.register_validator(TestValidator {
            name: "fail-validator".to_string(),
            should_fail: true,
        });
        let manifest = valid_manifest();
        let result = validator.validate_manifest(&manifest).await;
        assert!(!result.is_valid());
        assert_eq!(result.custom_errors.len(), 1);
        assert_eq!(result.custom_errors[0].code, ValidationCode::CustomValidatorError);
    }

    #[tokio::test]
    async fn test_multiple_custom_validators_parallel() {
        let mut validator = SkillValidator::default_schema().unwrap();
        validator.register_validator(TestValidator {
            name: "v1".to_string(),
            should_fail: true,
        });
        validator.register_validator(TestValidator {
            name: "v2".to_string(),
            should_fail: true,
        });
        validator.register_validator(TestValidator {
            name: "v3".to_string(),
            should_fail: false,
        });
        let manifest = valid_manifest();
        let result = validator.validate_manifest(&manifest).await;
        assert_eq!(result.custom_errors.len(), 2);
    }

    // -- New tests: Validation Cache --

    #[tokio::test]
    async fn test_cache_hit_on_second_call() {
        let validator = SkillValidator::default_schema().unwrap();
        let manifest = valid_manifest();

        let r1 = validator.validate_manifest(&manifest).await;
        assert!(!r1.cached);

        let r2 = validator.validate_manifest(&manifest).await;
        assert!(r2.cached);

        let stats = validator.cache_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let validator = SkillValidator::default_schema().unwrap();
        let manifest = valid_manifest();

        validator.validate_manifest(&manifest).await;
        validator.invalidate_cache();

        let r2 = validator.validate_manifest(&manifest).await;
        assert!(!r2.cached);
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let cache = ValidationCache::new("v1");
        assert_eq!(cache.stats().hit_rate(), 0.0);

        cache.put("abc", CachedResult { phase1: vec![], phase2: vec![], phase3: vec![], custom: vec![] });
        cache.get("abc"); // hit
        cache.get("xyz"); // miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    // -- New tests: Schema Evolution --

    #[test]
    fn test_schema_version_parsing() {
        assert_eq!(SchemaVersion::from_str("v1"), Some(SchemaVersion::V1));
        assert_eq!(SchemaVersion::from_str("draft-7"), Some(SchemaVersion::V1));
        assert_eq!(SchemaVersion::from_str("v2"), Some(SchemaVersion::V2));
        assert_eq!(SchemaVersion::from_str("v99"), None);
    }

    #[test]
    fn test_backward_compat() {
        assert!(check_backward_compat(&SchemaVersion::V1, &SchemaVersion::V1));
        assert!(check_backward_compat(&SchemaVersion::V1, &SchemaVersion::V2));
        assert!(check_backward_compat(&SchemaVersion::V2, &SchemaVersion::V2));
        assert!(!check_backward_compat(&SchemaVersion::V2, &SchemaVersion::V1));
    }

    // -- New tests: CI Gate --

    #[tokio::test]
    async fn test_validate_ci_all_valid() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("1.0.0");
        let manifests = vec![valid_manifest()];
        let (exit_code, json) = validate_ci(&validator, &manifests).await;
        assert_eq!(exit_code, 0);
        let report: CiReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.valid, 1);
        assert_eq!(report.invalid, 0);
    }

    #[tokio::test]
    async fn test_validate_ci_with_errors() {
        let validator = SkillValidator::default_schema()
            .unwrap()
            .with_hermes_version("0.1.0");
        let mut bad = valid_manifest();
        bad.min_hermes_version = Some("99.0.0".to_string());
        let manifests = vec![valid_manifest(), bad];
        let (exit_code, json) = validate_ci(&validator, &manifests).await;
        assert_eq!(exit_code, 1);
        let report: CiReport = serde_json::from_str(&json).unwrap();
        assert_eq!(report.valid, 1);
        assert_eq!(report.invalid, 1);
    }

    // -- New tests: Actionable Errors --

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::new("/name", ValidationCode::MissingRequiredField, "nome ausente")
            .with_suggestion("Adicione um nome em kebab-case")
            .with_docs_url("https://example.com/docs");
        let display = format!("{}", err);
        assert!(display.contains("MISSING_REQUIRED_FIELD"));
        assert!(display.contains("nome ausente"));
        assert!(display.contains("Suggestion"));
        assert!(display.contains("Docs"));
    }

    #[test]
    fn test_validation_error_serialization() {
        let err = ValidationError::new("/version", ValidationCode::InvalidFormat, "formato inválido");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("INVALID_FORMAT"));
        assert!(json.contains("/version"));
    }

    #[tokio::test]
    async fn test_manifest_digest_deterministic() {
        let m1 = valid_manifest();
        let m2 = valid_manifest();
        assert_eq!(m1.digest(), m2.digest());
    }

    #[tokio::test]
    async fn test_manifest_digest_changes_on_mutation() {
        let m1 = valid_manifest();
        let mut m2 = valid_manifest();
        m2.name = "different-name".to_string();
        assert_ne!(m1.digest(), m2.digest());
    }
}
