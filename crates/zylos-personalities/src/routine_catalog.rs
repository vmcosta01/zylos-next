//! # RoutineCatalog — Catálogo indexado de rotinas por employee role (T8.5.2)
//!
//! Fornece um catálogo estruturado de rotinas executáveis, indexadas por
//! EmployeeRole, com metadata de execução e validação de schema.
//!
//! ## Estrutura
//! Cada rotina é definida como entrada em `index.json` ou registrada
//! programaticamente via `RoutineCatalog::register()`.
//!
//! ## Validação
//! Rotinas são validadas em load-time: nome único, role válido,
//! handler path não-vazio, timeout > 0.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::roles::EmployeeRole;

/// Erros do RoutineCatalog.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogError {
    #[error("Rotina já registrada: {0}")]
    DuplicateRoutine(String),
    #[error("Rotina não encontrada: {0}")]
    NotFound(String),
    #[error("Validação falhou: {0}")]
    ValidationError(String),
    #[error("Erro de serialização: {0}")]
    SerializationError(String),
}

/// Definição de uma rotina executável.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutineDefinition {
    /// Nome único da rotina (ex: "code-review", "vulnerability-scan").
    pub name: String,
    /// Descrição legível da rotina.
    pub description: String,
    /// Role(s) que podem executar esta rotina.
    pub allowed_roles: Vec<EmployeeRole>,
    /// Path do handler (função, script ou endpoint).
    pub handler_path: String,
    /// Timeout máximo de execução em segundos.
    pub timeout_secs: u64,
    /// Se a rotina requer aprovação humana antes de executar.
    pub requires_approval: bool,
    /// Parâmetros esperados pela rotina.
    pub parameters: Vec<RoutineParameter>,
    /// Tags para filtering e grouping.
    pub tags: Vec<String>,
    /// Versão da rotina.
    pub version: String,
}

/// Parâmetro esperado por uma rotina.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutineParameter {
    /// Nome do parâmetro.
    pub name: String,
    /// Tipo JSON Schema do parâmetro.
    pub param_type: String,
    /// Se o parâmetro é obrigatório.
    pub required: bool,
    /// Valor default (se opcional).
    pub default_value: Option<serde_json::Value>,
    /// Descrição do parâmetro.
    pub description: Option<String>,
}

impl RoutineDefinition {
    /// Cria uma nova definição de rotina.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        handler_path: impl Into<String>,
        allowed_roles: Vec<EmployeeRole>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            allowed_roles,
            handler_path: handler_path.into(),
            timeout_secs: 300, // 5 min default
            requires_approval: false,
            parameters: vec![],
            tags: vec![],
            version: "1.0".into(),
        }
    }

    /// Builder: define timeout customizado.
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Builder: marca como requiring approval.
    pub fn requires_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }

    /// Builder: adiciona parâmetro.
    pub fn with_parameter(mut self, param: RoutineParameter) -> Self {
        self.parameters.push(param);
        self
    }

    /// Builder: adiciona tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Builder: define versão.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Valida a consistência interna da definição.
    pub fn validate(&self) -> Result<(), CatalogError> {
        if self.name.is_empty() {
            return Err(CatalogError::ValidationError(
                "Routine name cannot be empty".into(),
            ));
        }
        if self.handler_path.is_empty() {
            return Err(CatalogError::ValidationError(format!(
                "Routine '{}' has empty handler_path",
                self.name
            )));
        }
        if self.timeout_secs == 0 {
            return Err(CatalogError::ValidationError(format!(
                "Routine '{}' has timeout_secs=0",
                self.name
            )));
        }
        if self.allowed_roles.is_empty() {
            return Err(CatalogError::ValidationError(format!(
                "Routine '{}' has no allowed_roles",
                self.name
            )));
        }
        Ok(())
    }

    /// Verifica se um role pode executar esta rotina.
    pub fn allows_role(&self, role: &EmployeeRole) -> bool {
        self.allowed_roles.contains(role)
    }
}

/// Catálogo de rotinas com indexação por nome e role.
pub struct RoutineCatalog {
    /// Rotinas indexadas por nome.
    routines: HashMap<String, RoutineDefinition>,
    /// Index secundário: role → lista de nomes de rotinas.
    role_index: HashMap<EmployeeRole, Vec<String>>,
}

impl Default for RoutineCatalog {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutineCatalog {
    /// Cria um catálogo vazio.
    pub fn new() -> Self {
        Self {
            routines: HashMap::new(),
            role_index: HashMap::new(),
        }
    }

    /// Cria um catálogo populado com rotinas padrão para todos os roles.
    pub fn with_defaults() -> Self {
        let mut catalog = Self::new();
        for role in EmployeeRole::all() {
            for routine_name in role.default_routines() {
                let def = RoutineDefinition::new(
                    routine_name,
                    format!("Default routine '{}' for {:?}", routine_name, role),
                    format!("handlers::{}", routine_name.replace('-', "_")),
                    vec![*role],
                );
                // Ignorar erro de duplicata em defaults (roles podem compartilhar rotinas)
                let _ = catalog.register(def);
            }
        }
        catalog
    }

    /// Registra uma nova rotina no catálogo.
    pub fn register(&mut self, def: RoutineDefinition) -> Result<(), CatalogError> {
        def.validate()?;

        if self.routines.contains_key(&def.name) {
            return Err(CatalogError::DuplicateRoutine(def.name.clone()));
        }

        // Atualizar index por role
        for role in &def.allowed_roles {
            self.role_index
                .entry(*role)
                .or_default()
                .push(def.name.clone());
        }

        self.routines.insert(def.name.clone(), def);
        Ok(())
    }

    /// Busca uma rotina por nome.
    pub fn get(&self, name: &str) -> Option<&RoutineDefinition> {
        self.routines.get(name)
    }

    /// Lista todas as rotinas disponíveis para um role específico.
    pub fn list_for_role(&self, role: &EmployeeRole) -> Vec<&RoutineDefinition> {
        self.role_index
            .get(role)
            .map(|names| names.iter().filter_map(|n| self.routines.get(n)).collect())
            .unwrap_or_default()
    }

    /// Lista todas as rotinas registradas.
    pub fn list_all(&self) -> Vec<&RoutineDefinition> {
        self.routines.values().collect()
    }

    /// Busca rotinas por tag.
    pub fn search_by_tag(&self, tag: &str) -> Vec<&RoutineDefinition> {
        self.routines
            .values()
            .filter(|r| r.tags.iter().any(|t| t == tag))
            .collect()
    }

    /// Retorna o número total de rotinas registradas.
    pub fn count(&self) -> usize {
        self.routines.len()
    }

    /// Retorna estatísticas do catálogo.
    pub fn stats(&self) -> CatalogStats {
        let mut role_counts = HashMap::new();
        for role in EmployeeRole::all() {
            let count = self.list_for_role(role).len();
            if count > 0 {
                role_counts.insert(*role, count);
            }
        }

        let approval_required = self
            .routines
            .values()
            .filter(|r| r.requires_approval)
            .count();

        CatalogStats {
            total_routines: self.count(),
            role_counts,
            approval_required_count: approval_required,
        }
    }

    /// Serializa o catálogo para JSON.
    pub fn to_json(&self) -> Result<String, CatalogError> {
        let defs: Vec<&RoutineDefinition> = self.routines.values().collect();
        serde_json::to_string_pretty(&defs)
            .map_err(|e| CatalogError::SerializationError(e.to_string()))
    }

    /// Carrega catálogo de JSON.
    pub fn from_json(json: &str) -> Result<Self, CatalogError> {
        let defs: Vec<RoutineDefinition> = serde_json::from_str(json)
            .map_err(|e| CatalogError::SerializationError(e.to_string()))?;

        let mut catalog = Self::new();
        for def in defs {
            catalog.register(def)?;
        }
        Ok(catalog)
    }
}

/// Estatísticas do catálogo de rotinas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CatalogStats {
    pub total_routines: usize,
    pub role_counts: HashMap<EmployeeRole, usize>,
    pub approval_required_count: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut catalog = RoutineCatalog::new();
        let def = RoutineDefinition::new(
            "test-routine",
            "A test routine",
            "handlers::test",
            vec![EmployeeRole::Developer],
        );
        catalog.register(def).unwrap();

        let retrieved = catalog.get("test-routine");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "A test routine");
    }

    #[test]
    fn test_duplicate_rejected() {
        let mut catalog = RoutineCatalog::new();
        let def1 = RoutineDefinition::new("dup", "First", "h1", vec![EmployeeRole::Developer]);
        let def2 = RoutineDefinition::new("dup", "Second", "h2", vec![EmployeeRole::Researcher]);

        catalog.register(def1).unwrap();
        let err = catalog.register(def2);
        assert!(matches!(err, Err(CatalogError::DuplicateRoutine(_))));
    }

    #[test]
    fn test_validation_empty_name() {
        let def = RoutineDefinition::new("", "desc", "handler", vec![EmployeeRole::Developer]);
        assert!(matches!(
            def.validate(),
            Err(CatalogError::ValidationError(_))
        ));
    }

    #[test]
    fn test_validation_empty_handler() {
        let def = RoutineDefinition::new("test", "desc", "", vec![EmployeeRole::Developer]);
        assert!(matches!(
            def.validate(),
            Err(CatalogError::ValidationError(_))
        ));
    }

    #[test]
    fn test_validation_zero_timeout() {
        let def = RoutineDefinition::new("test", "desc", "handler", vec![EmployeeRole::Developer])
            .with_timeout(0);
        assert!(matches!(
            def.validate(),
            Err(CatalogError::ValidationError(_))
        ));
    }

    #[test]
    fn test_validation_no_roles() {
        let def = RoutineDefinition::new("test", "desc", "handler", vec![]);
        assert!(matches!(
            def.validate(),
            Err(CatalogError::ValidationError(_))
        ));
    }

    #[test]
    fn test_list_for_role() {
        let mut catalog = RoutineCatalog::new();

        catalog
            .register(RoutineDefinition::new(
                "r1",
                "d1",
                "h1",
                vec![EmployeeRole::Developer, EmployeeRole::DevOps],
            ))
            .unwrap();
        catalog
            .register(RoutineDefinition::new(
                "r2",
                "d2",
                "h2",
                vec![EmployeeRole::Developer],
            ))
            .unwrap();
        catalog
            .register(RoutineDefinition::new(
                "r3",
                "d3",
                "h3",
                vec![EmployeeRole::Researcher],
            ))
            .unwrap();

        let dev_routines = catalog.list_for_role(&EmployeeRole::Developer);
        assert_eq!(dev_routines.len(), 2);

        let ops_routines = catalog.list_for_role(&EmployeeRole::DevOps);
        assert_eq!(ops_routines.len(), 1);
        assert_eq!(ops_routines[0].name, "r1");

        let research_routines = catalog.list_for_role(&EmployeeRole::Researcher);
        assert_eq!(research_routines.len(), 1);

        let designer_routines = catalog.list_for_role(&EmployeeRole::Designer);
        assert!(designer_routines.is_empty());
    }

    #[test]
    fn test_with_defaults() {
        let catalog = RoutineCatalog::with_defaults();
        assert!(catalog.count() > 0);

        // Cada role deve ter pelo menos uma rotina
        for role in EmployeeRole::all() {
            let routines = catalog.list_for_role(role);
            assert!(
                !routines.is_empty(),
                "Role {:?} has no default routines",
                role
            );
        }
    }

    #[test]
    fn test_search_by_tag() {
        let mut catalog = RoutineCatalog::new();

        catalog
            .register(
                RoutineDefinition::new("r1", "d1", "h1", vec![EmployeeRole::Developer])
                    .with_tag("security")
                    .with_tag("automated"),
            )
            .unwrap();
        catalog
            .register(
                RoutineDefinition::new("r2", "d2", "h2", vec![EmployeeRole::SecurityAuditor])
                    .with_tag("security")
                    .with_tag("manual"),
            )
            .unwrap();
        catalog
            .register(
                RoutineDefinition::new("r3", "d3", "h3", vec![EmployeeRole::Designer])
                    .with_tag("ux"),
            )
            .unwrap();

        let security = catalog.search_by_tag("security");
        assert_eq!(security.len(), 2);

        let ux = catalog.search_by_tag("ux");
        assert_eq!(ux.len(), 1);

        let nonexistent = catalog.search_by_tag("nonexistent");
        assert!(nonexistent.is_empty());
    }

    #[test]
    fn test_allows_role() {
        let def = RoutineDefinition::new(
            "shared",
            "Shared routine",
            "handler",
            vec![EmployeeRole::Developer, EmployeeRole::DevOps],
        );

        assert!(def.allows_role(&EmployeeRole::Developer));
        assert!(def.allows_role(&EmployeeRole::DevOps));
        assert!(!def.allows_role(&EmployeeRole::Designer));
    }

    #[test]
    fn test_builder_pattern() {
        let def = RoutineDefinition::new(
            "full",
            "Full routine",
            "handler",
            vec![EmployeeRole::Developer],
        )
        .with_timeout(600)
        .requires_approval()
        .with_parameter(RoutineParameter {
            name: "target".into(),
            param_type: "string".into(),
            required: true,
            default_value: None,
            description: Some("Target path".into()),
        })
        .with_tag("ci-cd")
        .with_version("2.0");

        assert_eq!(def.timeout_secs, 600);
        assert!(def.requires_approval);
        assert_eq!(def.parameters.len(), 1);
        assert_eq!(def.tags, vec!["ci-cd"]);
        assert_eq!(def.version, "2.0");
    }

    #[test]
    fn test_stats() {
        let mut catalog = RoutineCatalog::new();

        catalog
            .register(
                RoutineDefinition::new("r1", "d1", "h1", vec![EmployeeRole::Developer])
                    .requires_approval(),
            )
            .unwrap();
        catalog
            .register(RoutineDefinition::new(
                "r2",
                "d2",
                "h2",
                vec![EmployeeRole::Developer],
            ))
            .unwrap();
        catalog
            .register(RoutineDefinition::new(
                "r3",
                "d3",
                "h3",
                vec![EmployeeRole::Researcher],
            ))
            .unwrap();

        let stats = catalog.stats();
        assert_eq!(stats.total_routines, 3);
        assert_eq!(stats.approval_required_count, 1);
        assert_eq!(stats.role_counts.get(&EmployeeRole::Developer), Some(&2));
        assert_eq!(stats.role_counts.get(&EmployeeRole::Researcher), Some(&1));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut catalog = RoutineCatalog::new();
        catalog
            .register(
                RoutineDefinition::new(
                    "ser-test",
                    "Serialization test",
                    "handler",
                    vec![EmployeeRole::DevOps],
                )
                .with_timeout(120)
                .with_tag("infra"),
            )
            .unwrap();

        let json = catalog.to_json().unwrap();
        let restored = RoutineCatalog::from_json(&json).unwrap();

        assert_eq!(restored.count(), 1);
        let def = restored.get("ser-test").unwrap();
        assert_eq!(def.description, "Serialization test");
        assert_eq!(def.timeout_secs, 120);
    }

    #[test]
    fn test_from_json_invalid() {
        let result = RoutineCatalog::from_json("not valid json");
        assert!(matches!(result, Err(CatalogError::SerializationError(_))));
    }

    #[test]
    fn test_list_all() {
        let catalog = RoutineCatalog::with_defaults();
        let all = catalog.list_all();
        assert_eq!(all.len(), catalog.count());
    }

    #[test]
    fn test_routine_parameter_serialization() {
        let param = RoutineParameter {
            name: "branch".into(),
            param_type: "string".into(),
            required: false,
            default_value: Some(serde_json::json!("main")),
            description: Some("Git branch to deploy".into()),
        };
        let json = serde_json::to_string(&param).unwrap();
        let restored: RoutineParameter = serde_json::from_str(&json).unwrap();
        assert_eq!(param, restored);
    }

    #[test]
    fn test_catalog_stats_serialization() {
        let stats = CatalogStats {
            total_routines: 10,
            role_counts: HashMap::from([(EmployeeRole::Developer, 5), (EmployeeRole::DevOps, 3)]),
            approval_required_count: 2,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let restored: CatalogStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }
}
