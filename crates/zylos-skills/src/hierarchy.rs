//! # SkillHierarchy — Hierarquia 3-tier de skills (T8.4.5)
//!
//! Classifica skills em três níveis para escalabilidade e discovery eficiente:
//! - **Planning**: Skills de alto nível que orquestram outras skills (ex: "deploy-service")
//! - **Functional**: Skills de médio nível que executam uma função completa (ex: "run-tests")
//! - **Atomic**: Skills de baixo nível que executam uma única operação (ex: "compile-rust")
//!
//! ## Benefícios
//! - Discovery mais rápido: filtrar por tier reduz espaço de busca
//! - Composição clara: Planning → Functional → Atomic
//! - Evolução independente: tiers podem evoluir separadamente
//!
//! ## Integração
//! O campo `tier` é adicionado ao index.json e usado pelo IndexRegistry
//! para filtering hierárquico em queries de busca.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Tier de uma skill na hierarquia 3-tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SkillTier {
    /// Skill de planejamento/orquestração de alto nível.
    /// Compõe múltiplas skills funcionais para atingir um objetivo complexo.
    Planning,
    /// Skill funcional de médio nível.
    /// Executa uma função completa e autônoma.
    #[default]
    Functional,
    /// Skill atômica de baixo nível.
    /// Executa uma única operação bem definida.
    Atomic,
}

impl fmt::Display for SkillTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planning => write!(f, "planning"),
            Self::Functional => write!(f, "functional"),
            Self::Atomic => write!(f, "atomic"),
        }
    }
}

impl SkillTier {
    /// Parseia tier de string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "planning" => Some(Self::Planning),
            "functional" => Some(Self::Functional),
            "atomic" => Some(Self::Atomic),
            _ => None,
        }
    }

    /// Retorna o nível numérico do tier (0=Planning, 1=Functional, 2=Atomic).
    /// Útil para ordenação hierárquica.
    pub fn level(&self) -> u8 {
        match self {
            Self::Planning => 0,
            Self::Functional => 1,
            Self::Atomic => 2,
        }
    }

    /// Verifica se este tier pode compor skills do outro tier.
    /// Planning pode compor Functional e Atomic.
    /// Functional pode compor Atomic.
    /// Atomic não compõe outros tiers.
    pub fn can_compose(&self, other: &SkillTier) -> bool {
        self.level() < other.level()
    }

    /// Retorna todos os tiers como slice ordenado.
    pub fn all() -> &'static [SkillTier] {
        &[
            SkillTier::Planning,
            SkillTier::Functional,
            SkillTier::Atomic,
        ]
    }
}

/// Metadata de hierarquia associada a uma skill.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct SkillHierarchyMetadata {
    /// Tier da skill.
    pub tier: SkillTier,
    /// IDs das skills que esta skill compõe (apenas para Planning/Functional).
    pub composes: Vec<String>,
    /// IDs das skills que compõem esta skill (apenas para Functional/Atomic).
    pub composed_by: Vec<String>,
    /// Tags adicionais para filtering.
    pub tags: Vec<String>,
}

impl SkillHierarchyMetadata {
    /// Cria metadata para uma skill de planning.
    pub fn planning(composes: Vec<String>) -> Self {
        Self {
            tier: SkillTier::Planning,
            composes,
            ..Default::default()
        }
    }

    /// Cria metadata para uma skill funcional.
    pub fn functional(composes: Vec<String>, composed_by: Vec<String>) -> Self {
        Self {
            tier: SkillTier::Functional,
            composes,
            composed_by,
            ..Default::default()
        }
    }

    /// Cria metadata para uma skill atômica.
    pub fn atomic(composed_by: Vec<String>) -> Self {
        Self {
            tier: SkillTier::Atomic,
            composed_by,
            ..Default::default()
        }
    }

    /// Builder: adiciona tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Verifica se a metadata tem uma tag específica.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// Filtro de busca por tier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TierFilter {
    /// Tiers incluídos na busca. Se vazio, inclui todos.
    pub include: Vec<SkillTier>,
    /// Tiers excluídos da busca.
    pub exclude: Vec<SkillTier>,
}

impl TierFilter {
    /// Cria filtro que inclui apenas o tier especificado.
    pub fn only(tier: SkillTier) -> Self {
        Self {
            include: vec![tier],
            exclude: vec![],
        }
    }

    /// Cria filtro que exclui o tier especificado.
    pub fn exclude(tier: SkillTier) -> Self {
        Self {
            include: vec![],
            exclude: vec![tier],
        }
    }

    /// Cria filtro que inclui apenas planning e functional.
    pub fn high_level() -> Self {
        Self {
            include: vec![SkillTier::Planning, SkillTier::Functional],
            exclude: vec![],
        }
    }

    /// Cria filtro que inclui apenas atomic.
    pub fn low_level() -> Self {
        Self {
            include: vec![SkillTier::Atomic],
            exclude: vec![],
        }
    }

    /// Verifica se um tier passa pelo filtro.
    pub fn matches(&self, tier: &SkillTier) -> bool {
        // Se há exclusões explícitas, verificar primeiro
        if self.exclude.contains(tier) {
            return false;
        }
        // Se há inclusões explícitas, tier deve estar na lista
        if !self.include.is_empty() {
            return self.include.contains(tier);
        }
        // Sem inclusões nem exclusões → aceita tudo
        true
    }
}

/// Migração de skills existentes para o sistema de tiers.
///
/// Usa heurísticas baseadas em nome/descrição para classificar automaticamente.
pub fn infer_tier_from_name(name: &str) -> SkillTier {
    let lower = name.to_lowercase();

    // Planning keywords
    if lower.contains("orchestrat")
        || lower.contains("pipeline")
        || lower.contains("workflow")
        || lower.contains("deploy")
        || lower.contains("release")
        || lower.contains("plan")
        || lower.contains("strategy")
    {
        return SkillTier::Planning;
    }

    // Atomic keywords
    if lower.contains("parse")
        || lower.contains("format")
        || lower.contains("validate")
        || lower.contains("hash")
        || lower.contains("encode")
        || lower.contains("decode")
        || lower.contains("compress")
        || lower.contains("convert")
        || lower.contains("sanitize")
    {
        return SkillTier::Atomic;
    }

    // Default: Functional
    SkillTier::Functional
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_display() {
        assert_eq!(format!("{}", SkillTier::Planning), "planning");
        assert_eq!(format!("{}", SkillTier::Functional), "functional");
        assert_eq!(format!("{}", SkillTier::Atomic), "atomic");
    }

    #[test]
    fn test_tier_from_str() {
        assert_eq!(SkillTier::parse("planning"), Some(SkillTier::Planning));
        assert_eq!(SkillTier::parse("FUNCTIONAL"), Some(SkillTier::Functional));
        assert_eq!(SkillTier::parse("Atomic"), Some(SkillTier::Atomic));
        assert_eq!(SkillTier::parse("unknown"), None);
        assert_eq!(SkillTier::parse(""), None);
    }

    #[test]
    fn test_tier_level_ordering() {
        assert!(SkillTier::Planning.level() < SkillTier::Functional.level());
        assert!(SkillTier::Functional.level() < SkillTier::Atomic.level());
        assert_eq!(SkillTier::Planning.level(), 0);
        assert_eq!(SkillTier::Functional.level(), 1);
        assert_eq!(SkillTier::Atomic.level(), 2);
    }

    #[test]
    fn test_can_compose() {
        // Planning pode compor Functional e Atomic
        assert!(SkillTier::Planning.can_compose(&SkillTier::Functional));
        assert!(SkillTier::Planning.can_compose(&SkillTier::Atomic));
        // Functional pode compor Atomic
        assert!(SkillTier::Functional.can_compose(&SkillTier::Atomic));
        // Não pode compor mesmo tier ou superior
        assert!(!SkillTier::Planning.can_compose(&SkillTier::Planning));
        assert!(!SkillTier::Functional.can_compose(&SkillTier::Planning));
        assert!(!SkillTier::Functional.can_compose(&SkillTier::Functional));
        assert!(!SkillTier::Atomic.can_compose(&SkillTier::Planning));
        assert!(!SkillTier::Atomic.can_compose(&SkillTier::Functional));
        assert!(!SkillTier::Atomic.can_compose(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_serialization_roundtrip() {
        for tier in SkillTier::all() {
            let json = serde_json::to_string(tier).unwrap();
            let restored: SkillTier = serde_json::from_str(&json).unwrap();
            assert_eq!(*tier, restored);
        }
    }

    #[test]
    fn test_hierarchy_metadata_default() {
        let meta = SkillHierarchyMetadata::default();
        assert_eq!(meta.tier, SkillTier::Functional);
        assert!(meta.composes.is_empty());
        assert!(meta.composed_by.is_empty());
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn test_hierarchy_metadata_planning() {
        let meta = SkillHierarchyMetadata::planning(vec!["build".into(), "test".into()]);
        assert_eq!(meta.tier, SkillTier::Planning);
        assert_eq!(meta.composes.len(), 2);
        assert!(meta.composed_by.is_empty());
    }

    #[test]
    fn test_hierarchy_metadata_functional() {
        let meta = SkillHierarchyMetadata::functional(
            vec!["compile".into()],
            vec!["deploy-pipeline".into()],
        );
        assert_eq!(meta.tier, SkillTier::Functional);
        assert_eq!(meta.composes.len(), 1);
        assert_eq!(meta.composed_by.len(), 1);
    }

    #[test]
    fn test_hierarchy_metadata_atomic() {
        let meta = SkillHierarchyMetadata::atomic(vec!["run-tests".into()]);
        assert_eq!(meta.tier, SkillTier::Atomic);
        assert!(meta.composes.is_empty());
        assert_eq!(meta.composed_by.len(), 1);
    }

    #[test]
    fn test_hierarchy_metadata_with_tags() {
        let meta = SkillHierarchyMetadata::default()
            .with_tag("rust")
            .with_tag("ci-cd")
            .with_tag("backend");

        assert_eq!(meta.tags.len(), 3);
        assert!(meta.has_tag("rust"));
        assert!(meta.has_tag("ci-cd"));
        assert!(!meta.has_tag("frontend"));
    }

    #[test]
    fn test_tier_filter_only() {
        let filter = TierFilter::only(SkillTier::Planning);
        assert!(filter.matches(&SkillTier::Planning));
        assert!(!filter.matches(&SkillTier::Functional));
        assert!(!filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_filter_exclude() {
        let filter = TierFilter::exclude(SkillTier::Atomic);
        assert!(filter.matches(&SkillTier::Planning));
        assert!(filter.matches(&SkillTier::Functional));
        assert!(!filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_filter_high_level() {
        let filter = TierFilter::high_level();
        assert!(filter.matches(&SkillTier::Planning));
        assert!(filter.matches(&SkillTier::Functional));
        assert!(!filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_filter_low_level() {
        let filter = TierFilter::low_level();
        assert!(!filter.matches(&SkillTier::Planning));
        assert!(!filter.matches(&SkillTier::Functional));
        assert!(filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_filter_default_matches_all() {
        let filter = TierFilter::default();
        assert!(filter.matches(&SkillTier::Planning));
        assert!(filter.matches(&SkillTier::Functional));
        assert!(filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_tier_filter_exclude_overrides_include() {
        let filter = TierFilter {
            include: vec![SkillTier::Planning, SkillTier::Functional],
            exclude: vec![SkillTier::Functional],
        };
        assert!(filter.matches(&SkillTier::Planning));
        assert!(!filter.matches(&SkillTier::Functional)); // Excluído apesar de incluído
        assert!(!filter.matches(&SkillTier::Atomic));
    }

    #[test]
    fn test_infer_tier_planning_keywords() {
        assert_eq!(infer_tier_from_name("deploy-service"), SkillTier::Planning);
        assert_eq!(infer_tier_from_name("CI Pipeline"), SkillTier::Planning);
        assert_eq!(
            infer_tier_from_name("orchestrate-agents"),
            SkillTier::Planning
        );
        assert_eq!(
            infer_tier_from_name("Release Workflow"),
            SkillTier::Planning
        );
        assert_eq!(infer_tier_from_name("plan-sprint"), SkillTier::Planning);
    }

    #[test]
    fn test_infer_tier_atomic_keywords() {
        assert_eq!(infer_tier_from_name("parse-json"), SkillTier::Atomic);
        assert_eq!(infer_tier_from_name("validate-schema"), SkillTier::Atomic);
        assert_eq!(infer_tier_from_name("hash-sha256"), SkillTier::Atomic);
        assert_eq!(infer_tier_from_name("encode-base64"), SkillTier::Atomic);
        assert_eq!(infer_tier_from_name("sanitize-output"), SkillTier::Atomic);
        assert_eq!(infer_tier_from_name("format-date"), SkillTier::Atomic);
    }

    #[test]
    fn test_infer_tier_functional_default() {
        assert_eq!(infer_tier_from_name("run-tests"), SkillTier::Functional);
        assert_eq!(infer_tier_from_name("build-project"), SkillTier::Functional);
        assert_eq!(infer_tier_from_name("analyze-code"), SkillTier::Functional);
        assert_eq!(
            infer_tier_from_name("generate-report"),
            SkillTier::Functional
        );
    }

    #[test]
    fn test_hierarchy_metadata_serialization() {
        let meta = SkillHierarchyMetadata::functional(
            vec!["compile".into(), "lint".into()],
            vec!["ci-pipeline".into()],
        )
        .with_tag("rust")
        .with_tag("quality");

        let json = serde_json::to_string(&meta).unwrap();
        let restored: SkillHierarchyMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, restored);
    }

    #[test]
    fn test_tier_filter_serialization() {
        let filter = TierFilter {
            include: vec![SkillTier::Planning, SkillTier::Functional],
            exclude: vec![SkillTier::Atomic],
        };
        let json = serde_json::to_string(&filter).unwrap();
        let restored: TierFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, restored);
    }

    #[test]
    fn test_all_tiers() {
        let all = SkillTier::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], SkillTier::Planning);
        assert_eq!(all[1], SkillTier::Functional);
        assert_eq!(all[2], SkillTier::Atomic);
    }

    #[test]
    fn test_default_tier_is_functional() {
        assert_eq!(SkillTier::default(), SkillTier::Functional);
    }
}
