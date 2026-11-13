//! # EmployeeRole — 8 roles padronizados para AI Employees (T8.5.1)
//!
//! Define enum com 8 papéis de negócio padronizados, cada um mapeado para
//! um personality preset existente e um conjunto de rotinas padrão.
//!
//! ## Roles
//! 1. **Developer** — Engenharia de software, code review, debugging
//! 2. **Researcher** — Pesquisa, análise de dados, síntese de informações
//! 3. **DevOps** — Infraestrutura, CI/CD, monitoring, deploy
//! 4. **SecurityAuditor** — Auditoria de segurança, threat modeling, compliance
//! 5. **Designer** — UX/UI, design system, acessibilidade
//! 6. **ProjectManager** — Planejamento, tracking, coordenação de equipe
//! 7. **SupportAgent** — Suporte ao usuário, onboarding, troubleshooting
//! 8. **ContentCreator** — Documentação, blog posts, relatórios técnicos
//!
//! ## Integração
//! Cada role mapeia para um slug de PersonalityProfile existente no registry
//! e define um conjunto de rotinas executáveis pelo RoutineScheduler.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Papel de negócio padronizado para AI Employees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EmployeeRole {
 /// Engenheiro de software: code review, debugging, implementação.
 #[default]
 Developer,
 /// Pesquisador: análise de dados, literature review, síntese.
 Researcher,
 /// Especialista DevOps: infraestrutura, CI/CD, deploy, monitoring.
 DevOps,
 /// Auditor de segurança: pentest, compliance, threat modeling.
 SecurityAuditor,
 /// Designer UX/UI: design system, prototipagem, acessibilidade.
 Designer,
 /// Gerente de projeto: planejamento, tracking, risk management.
 ProjectManager,
 /// Agente de suporte: troubleshooting, onboarding, FAQ.
 SupportAgent,
 /// Criador de conteúdo: documentação, blog, relatórios técnicos.
 ContentCreator,
}


impl fmt::Display for EmployeeRole {
 fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
 match self {
 Self::Developer => write!(f, "developer"),
 Self::Researcher => write!(f, "researcher"),
 Self::DevOps => write!(f, "devops"),
 Self::SecurityAuditor => write!(f, "security_auditor"),
 Self::Designer => write!(f, "designer"),
 Self::ProjectManager => write!(f, "project_manager"),
 Self::SupportAgent => write!(f, "support_agent"),
 Self::ContentCreator => write!(f, "content_creator"),
 }
 }
}

impl EmployeeRole {
 /// Parseia role de string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
 match s.to_lowercase().as_str() {
 "developer" | "dev" | "engineer" => Some(Self::Developer),
 "researcher" | "research" | "analyst" => Some(Self::Researcher),
 "devops" | "infra" | "sre" => Some(Self::DevOps),
 "security_auditor" | "security" | "sec" | "auditor" => Some(Self::SecurityAuditor),
 "designer" | "ux" | "ui" => Some(Self::Designer),
 "project_manager" | "pm" | "manager" => Some(Self::ProjectManager),
 "support_agent" | "support" | "helpdesk" => Some(Self::SupportAgent),
 "content_creator" | "writer" | "docs" => Some(Self::ContentCreator),
 _ => None,
 }
 }

 /// Retorna o slug do PersonalityProfile associado a este role.
 pub fn personality_slug(&self) -> &'static str {
 match self {
 Self::Developer => "senior-engineer",
 Self::Researcher => "research-analyst",
 Self::DevOps => "devops-specialist",
 Self::SecurityAuditor => "security-auditor",
 Self::Designer => "creative-designer",
 Self::ProjectManager => "project-manager",
 Self::SupportAgent => "support-agent",
 Self::ContentCreator => "content-creator",
 }
 }

 /// Retorna descrição legível do role.
 pub fn description(&self) -> &'static str {
 match self {
 Self::Developer => "Software engineering: code review, debugging, implementation, testing",
 Self::Researcher => "Research & analysis: data synthesis, literature review, report generation",
 Self::DevOps => "Infrastructure & operations: CI/CD, deployment, monitoring, reliability",
 Self::SecurityAuditor => "Security auditing: vulnerability assessment, compliance, threat modeling",
 Self::Designer => "UX/UI design: prototyping, design systems, accessibility, user research",
 Self::ProjectManager => "Project management: planning, tracking, risk management, coordination",
 Self::SupportAgent => "User support: troubleshooting, onboarding, FAQ, escalation handling",
 Self::ContentCreator => "Content creation: documentation, blog posts, technical reports, tutorials",
 }
 }

 /// Retorna lista de rotinas padrão associadas a este role.
 pub fn default_routines(&self) -> Vec<&'static str> {
 match self {
 Self::Developer => vec![
 "code-review",
 "run-tests",
 "fix-lint-warnings",
 "update-dependencies",
 "write-unit-tests",
 ],
 Self::Researcher => vec![
 "literature-search",
 "data-analysis",
 "synthesis-report",
 "competitive-analysis",
 "trend-monitoring",
 ],
 Self::DevOps => vec![
 "deploy-staging",
 "monitor-alerts",
 "backup-verification",
 "infra-cost-report",
 "ci-pipeline-health",
 ],
 Self::SecurityAuditor => vec![
 "vulnerability-scan",
 "dependency-audit",
 "compliance-check",
 "secret-leak-detection",
 "access-review",
 ],
 Self::Designer => vec![
 "design-review",
 "accessibility-audit",
 "component-update",
 "user-feedback-analysis",
 "style-guide-compliance",
 ],
 Self::ProjectManager => vec![
 "sprint-planning",
 "progress-report",
 "risk-assessment",
 "stakeholder-update",
 "retrospective-summary",
 ],
 Self::SupportAgent => vec![
 "ticket-triage",
 "faq-update",
 "onboarding-guide",
 "escalation-handoff",
 "satisfaction-survey",
 ],
 Self::ContentCreator => vec![
 "doc-update",
 "blog-draft",
 "changelog-generation",
 "tutorial-creation",
 "readme-improvement",
 ],
 }
 }

 /// Retorna todos os roles como slice ordenado.
 pub fn all() -> &'static [EmployeeRole] {
 &[
 EmployeeRole::Developer,
 EmployeeRole::Researcher,
 EmployeeRole::DevOps,
 EmployeeRole::SecurityAuditor,
 EmployeeRole::Designer,
 EmployeeRole::ProjectManager,
 EmployeeRole::SupportAgent,
 EmployeeRole::ContentCreator,
 ]
 }

 /// Retorna o número total de roles definidos.
 pub fn count() -> usize {
 Self::all().len()
 }
}

/// Configuração completa de um AI Employee.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmployeeConfig {
 /// Role do employee.
 pub role: EmployeeRole,
 /// Nome personalizado (override do display_name do personality).
 pub custom_name: Option<String>,
 /// Rotinas habilitadas (subconjunto das default_routines do role).
 pub enabled_routines: Vec<String>,
 /// Schedule de execução automática (cron expression ou intervalo).
 pub schedule: Option<String>,
 /// Se true, employee está ativo e pode executar rotinas.
 pub active: bool,
 /// Metadata adicional.
 pub metadata: serde_json::Value,
}

impl EmployeeConfig {
 /// Cria configuração default para um role.
 pub fn new(role: EmployeeRole) -> Self {
 Self {
 role,
 custom_name: None,
 enabled_routines: role
 .default_routines()
 .iter()
 .map(|s| s.to_string())
 .collect(),
 schedule: None,
 active: true,
 metadata: serde_json::Value::Null,
 }
 }

 /// Builder: define nome customizado.
 pub fn with_name(mut self, name: impl Into<String>) -> Self {
 self.custom_name = Some(name.into());
 self
 }

 /// Builder: define schedule.
 pub fn with_schedule(mut self, schedule: impl Into<String>) -> Self {
 self.schedule = Some(schedule.into());
 self
 }

 /// Builder: desativa employee.
 pub fn inactive(mut self) -> Self {
 self.active = false;
 self
 }

 /// Builder: anexa metadata.
 pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
 self.metadata = metadata;
 self
 }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
 use super::*;

 #[test]
 fn test_all_roles_count() {
 assert_eq!(EmployeeRole::count(), 8);
 assert_eq!(EmployeeRole::all().len(), 8);
 }

 #[test]
 fn test_role_display() {
 assert_eq!(format!("{}", EmployeeRole::Developer), "developer");
 assert_eq!(format!("{}", EmployeeRole::SecurityAuditor), "security_auditor");
 assert_eq!(format!("{}", EmployeeRole::ProjectManager), "project_manager");
 assert_eq!(format!("{}", EmployeeRole::ContentCreator), "content_creator");
 }

 #[test]
 fn test_role_parse() {
         assert_eq!(EmployeeRole::parse("developer"), Some(EmployeeRole::Developer));
         assert_eq!(EmployeeRole::parse("DEV"), Some(EmployeeRole::Developer));
         assert_eq!(EmployeeRole::parse("engineer"), Some(EmployeeRole::Developer));
         assert_eq!(EmployeeRole::parse("researcher"), Some(EmployeeRole::Researcher));
         assert_eq!(EmployeeRole::parse("analyst"), Some(EmployeeRole::Researcher));
         assert_eq!(EmployeeRole::parse("devops"), Some(EmployeeRole::DevOps));
         assert_eq!(EmployeeRole::parse("sre"), Some(EmployeeRole::DevOps));
         assert_eq!(EmployeeRole::parse("security"), Some(EmployeeRole::SecurityAuditor));
         assert_eq!(EmployeeRole::parse("auditor"), Some(EmployeeRole::SecurityAuditor));
         assert_eq!(EmployeeRole::parse("designer"), Some(EmployeeRole::Designer));
         assert_eq!(EmployeeRole::parse("ux"), Some(EmployeeRole::Designer));
         assert_eq!(EmployeeRole::parse("pm"), Some(EmployeeRole::ProjectManager));
         assert_eq!(EmployeeRole::parse("support"), Some(EmployeeRole::SupportAgent));
         assert_eq!(EmployeeRole::parse("writer"), Some(EmployeeRole::ContentCreator));
         assert_eq!(EmployeeRole::parse("docs"), Some(EmployeeRole::ContentCreator));
         assert_eq!(EmployeeRole::parse("nonexistent"), None);
         assert_eq!(EmployeeRole::parse(""), None);
 }

 #[test]
 fn test_personality_slugs_unique() {
 let mut slugs = std::collections::HashSet::new();
 for role in EmployeeRole::all() {
 assert!(
 slugs.insert(role.personality_slug()),
 "Duplicate slug: {}",
 role.personality_slug()
 );
 }
 }

 #[test]
 fn test_personality_slugs_non_empty() {
 for role in EmployeeRole::all() {
 assert!(!role.personality_slug().is_empty());
 }
 }

 #[test]
 fn test_default_routines_non_empty() {
 for role in EmployeeRole::all() {
 let routines = role.default_routines();
 assert!(
 !routines.is_empty(),
 "Role {:?} has no default routines",
 role
 );
 // Todas as rotinas devem ser strings não-vazias
 for routine in routines {
 assert!(!routine.is_empty());
 }
 }
 }

 #[test]
 fn test_developer_routines() {
 let routines = EmployeeRole::Developer.default_routines();
 assert!(routines.contains(&"code-review"));
 assert!(routines.contains(&"run-tests"));
 assert!(routines.len() >= 3);
 }

 #[test]
 fn test_security_auditor_routines() {
 let routines = EmployeeRole::SecurityAuditor.default_routines();
 assert!(routines.contains(&"vulnerability-scan"));
 assert!(routines.contains(&"dependency-audit"));
 assert!(routines.contains(&"secret-leak-detection"));
 }

 #[test]
 fn test_descriptions_non_empty() {
 for role in EmployeeRole::all() {
 assert!(!role.description().is_empty());
 }
 }

 #[test]
 fn test_role_serialization_roundtrip() {
 for role in EmployeeRole::all() {
 let json = serde_json::to_string(role).unwrap();
 let restored: EmployeeRole = serde_json::from_str(&json).unwrap();
 assert_eq!(*role, restored);
 }
 }

 #[test]
 fn test_employee_config_new() {
 let config = EmployeeConfig::new(EmployeeRole::DevOps);
 assert_eq!(config.role, EmployeeRole::DevOps);
 assert!(config.active);
 assert!(config.custom_name.is_none());
 assert!(config.schedule.is_none());
 assert!(!config.enabled_routines.is_empty());
 assert_eq!(
 config.enabled_routines.len(),
 EmployeeRole::DevOps.default_routines().len()
 );
 }

 #[test]
 fn test_employee_config_builder() {
 let config = EmployeeConfig::new(EmployeeRole::Researcher)
 .with_name("Dr. Research")
 .with_schedule("0 9 * * 1-5")
 .with_metadata(serde_json::json!({"department": "R&D"}));

 assert_eq!(config.custom_name, Some("Dr. Research".into()));
 assert_eq!(config.schedule, Some("0 9 * * 1-5".into()));
 assert_eq!(config.metadata["department"], "R&D");
 assert!(config.active);
 }

 #[test]
 fn test_employee_config_inactive() {
 let config = EmployeeConfig::new(EmployeeRole::Designer).inactive();
 assert!(!config.active);
 }

 #[test]
 fn test_employee_config_serialization() {
 let config = EmployeeConfig::new(EmployeeRole::ProjectManager)
 .with_name("PM Alpha")
 .with_schedule("daily");

 let json = serde_json::to_string(&config).unwrap();
 let restored: EmployeeConfig = serde_json::from_str(&json).unwrap();
 assert_eq!(restored.role, EmployeeRole::ProjectManager);
 assert_eq!(restored.custom_name, Some("PM Alpha".into()));
 assert_eq!(restored.schedule, Some("daily".into()));
 }

 #[test]
 fn test_default_role_is_developer() {
 assert_eq!(EmployeeRole::default(), EmployeeRole::Developer);
 }

 #[test]
 fn test_all_roles_have_unique_descriptions() {
 let mut descriptions = std::collections::HashSet::new();
 for role in EmployeeRole::all() {
 assert!(
 descriptions.insert(role.description()),
 "Duplicate description for {:?}",
 role
 );
 }
 }
}