//! # GuardrailEnforcer — Enforcement de guardrails por employee role (T8.5.5)
//!
//! Sistema de regras de segurança operacional que previnem ações proibidas
//! por AI Employees. Cada regra define uma condição e uma ação de bloqueio.
//!
//! ## Arquitetura
//! - Regras declarativas em YAML/JSON ou registradas programaticamente
//! - Avaliação sequencial: primeira regra que match = decisão final
//! - Suporte a wildcards em rotinas, roles e recursos
//! - Logging estruturado de todas as decisões (permit/block)
//!
//! ## Exemplos de Guardrails
//! - "Nenhum employee pode executar deploy-production sem aprovação"
//! - "SupportAgent não pode acessar filesystem:/etc"
//! - "ContentCreator não pode executar comandos shell"
//! - "Todos os employees devem respeitar rate limits"

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::roles::EmployeeRole;

/// Ação tomada pelo guardrail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailAction {
    /// Ação permitida.
    Allow,
    /// Ação bloqueada com razão.
    Block { reason: String },
    /// Ação requer aprovação humana antes de prosseguir.
    RequireApproval { message: String },
}

impl std::fmt::Display for GuardrailAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allow => write!(f, "allow"),
            Self::Block { reason } => write!(f, "block: {}", reason),
            Self::RequireApproval { message } => write!(f, "require_approval: {}", message),
        }
    }
}

/// Uma regra de guardrail individual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailRule {
    /// Nome único da regra.
    pub name: String,
    /// Descrição legível da regra.
    pub description: String,
    /// Roles aos quais esta regra se aplica. Vazio = todos os roles.
    pub applicable_roles: Vec<EmployeeRole>,
    /// Padrão de rotina ao qual esta regra se aplica (suporta wildcard `*`).
    pub routine_pattern: String,
    /// Padrão de recurso ao qual esta regra se aplica (ex: "filesystem:/etc/*").
    pub resource_pattern: Option<String>,
    /// Ação a tomar quando a regra match.
    pub action: GuardrailAction,
    /// Prioridade da regra (maior = avaliada primeiro). Default: 0.
    pub priority: i32,
    /// Se true, a regra está ativa.
    pub enabled: bool,
}

impl GuardrailRule {
    /// Cria uma nova regra de bloqueio.
    pub fn block(
        name: impl Into<String>,
        routine_pattern: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            applicable_roles: vec![],
            routine_pattern: routine_pattern.into(),
            resource_pattern: None,
            action: GuardrailAction::Block {
                reason: reason.into(),
            },
            priority: 0,
            enabled: true,
        }
    }

    /// Cria uma nova regra de permissão explícita.
    pub fn allow(name: impl Into<String>, routine_pattern: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            applicable_roles: vec![],
            routine_pattern: routine_pattern.into(),
            resource_pattern: None,
            action: GuardrailAction::Allow,
            priority: 0,
            enabled: true,
        }
    }

    /// Cria uma nova regra de aprovação obrigatória.
    pub fn require_approval(
        name: impl Into<String>,
        routine_pattern: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            applicable_roles: vec![],
            routine_pattern: routine_pattern.into(),
            resource_pattern: None,
            action: GuardrailAction::RequireApproval {
                message: message.into(),
            },
            priority: 0,
            enabled: true,
        }
    }

    /// Builder: define roles aplicáveis.
    pub fn for_roles(mut self, roles: Vec<EmployeeRole>) -> Self {
        self.applicable_roles = roles;
        self
    }

    /// Builder: define padrão de recurso.
    pub fn for_resource(mut self, pattern: impl Into<String>) -> Self {
        self.resource_pattern = Some(pattern.into());
        self
    }

    /// Builder: define descrição.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Builder: define prioridade.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: desativa a regra.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Verifica se esta regra match uma dada combinação de role + rotina + recurso.
    pub fn matches(&self, role: &EmployeeRole, routine_name: &str, resource: Option<&str>) -> bool {
        if !self.enabled {
            return false;
        }

        // Verificar role (vazio = todos)
        if !self.applicable_roles.is_empty() && !self.applicable_roles.contains(role) {
            return false;
        }

        // Verificar padrão de rotina
        if !pattern_matches(&self.routine_pattern, routine_name) {
            return false;
        }

        // Verificar padrão de recurso (se especificado)
        if let Some(ref res_pattern) = self.resource_pattern {
            match resource {
                Some(res) => {
                    if !pattern_matches(res_pattern, res) {
                        return false;
                    }
                }
                None => return false, // Regra requer recurso mas nenhum foi fornecido
            }
        }

        true
    }
}

/// Verifica se um padrão (com suporte a wildcard `*`) match uma string.
fn pattern_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return value.ends_with(suffix);
    }
    pattern == value
}

/// Decisão completa do enforcer com metadata de auditoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailDecision {
    /// Ação decidida.
    pub action: GuardrailAction,
    /// Nome da regra que produziu a decisão (None = default allow).
    pub matched_rule: Option<String>,
    /// Timestamp da decisão (ISO 8601).
    pub decided_at: String,
    /// Contexto da avaliação.
    pub context: EvaluationContext,
}

/// Contexto da avaliação de guardrails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub employee_id: String,
    pub role: EmployeeRole,
    pub routine_name: String,
    pub resource: Option<String>,
}

/// Enforcer de guardrails com avaliação sequencial por prioridade.
pub struct GuardrailEnforcer {
    /// Regras ordenadas por prioridade (descendente).
    rules: Arc<RwLock<Vec<GuardrailRule>>>,
    /// Log de decisões para auditoria.
    decision_log: Arc<RwLock<Vec<GuardrailDecision>>>,
    /// Tamanho máximo do log de decisões.
    max_log_size: usize,
}

impl Default for GuardrailEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl GuardrailEnforcer {
    /// Cria um enforcer vazio.
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            decision_log: Arc::new(RwLock::new(Vec::new())),
            max_log_size: 1000,
        }
    }

    /// Cria um enforcer com regras default de segurança.
    pub fn with_defaults() -> Self {
        let enforcer = Self::new();
        let default_rules = vec![
            // Bloquear operações financeiras sem aprovação
            GuardrailRule::require_approval(
                "financial-approval",
                "financial-*",
                "Financial operations require explicit human approval",
            )
            .with_priority(100)
            .with_description("All financial transactions need approval"),
            // Bloquear deploy em produção sem aprovação
            GuardrailRule::require_approval(
                "prod-deploy-approval",
                "deploy-production",
                "Production deployment requires human sign-off",
            )
            .with_priority(90)
            .with_description("No auto-deploy to production"),
            // Bloquear exclusão de dados
            GuardrailRule::require_approval(
                "data-deletion-approval",
                "delete-*",
                "Data deletion requires confirmation",
            )
            .with_priority(80)
            .with_description("Prevent accidental data loss"),
            // SupportAgent não pode acessar filesystem sensível
            GuardrailRule::block(
                "support-no-sensitive-fs",
                "*",
                "Support agents cannot access sensitive filesystem paths",
            )
            .for_roles(vec![EmployeeRole::SupportAgent])
            .for_resource("filesystem:/etc/*")
            .with_priority(70),
            // ContentCreator não pode executar shell commands
            GuardrailRule::block(
                "writer-no-shell",
                "*",
                "Content creators cannot execute shell commands directly",
            )
            .for_roles(vec![EmployeeRole::ContentCreator])
            .for_resource("exec:shell:*")
            .with_priority(70),
            // Rate limit enforcement (sempre ativo)
            GuardrailRule::allow("default-allow", "*")
                .with_priority(-1000)
                .with_description("Default allow when no other rule matches"),
        ];

        // Registrar regras default via versão síncrona (seguro fora de runtime async)
        for rule in default_rules {
            let _ = enforcer.add_rule_sync(rule);
        }
        enforcer
    }

    /// Adiciona uma regra ao enforcer (async-safe).
    pub async fn add_rule(&self, rule: GuardrailRule) -> Result<(), GuardrailError> {
        if rule.name.is_empty() {
            return Err(GuardrailError::InvalidRule(
                "Rule name cannot be empty".into(),
            ));
        }

        let mut rules = self.rules.write().await;

        // Verificar duplicata
        if rules.iter().any(|r| r.name == rule.name) {
            return Err(GuardrailError::DuplicateRule(rule.name));
        }

        rules.push(rule);
        // Ordenar por prioridade descendente
        rules.sort_by_key(|r| std::cmp::Reverse(r.priority));

        Ok(())
    }

    /// Versão síncrona de add_rule para uso fora de runtime Tokio.
    pub fn add_rule_sync(&self, rule: GuardrailRule) -> Result<(), GuardrailError> {
        if rule.name.is_empty() {
            return Err(GuardrailError::InvalidRule(
                "Rule name cannot be empty".into(),
            ));
        }

        let mut rules = self.rules.blocking_write();

        if rules.iter().any(|r| r.name == rule.name) {
            return Err(GuardrailError::DuplicateRule(rule.name));
        }

        rules.push(rule);
        rules.sort_by_key(|r| std::cmp::Reverse(r.priority));

        Ok(())
    }

    /// Remove uma regra por nome.
    pub async fn remove_rule(&self, name: &str) -> bool {
        let mut rules = self.rules.write().await;
        let len_before = rules.len();
        rules.retain(|r| r.name != name);
        rules.len() < len_before
    }

    /// Avalia uma ação contra todas as regras.
    ///
    /// Retorna a decisão da primeira regra que match (por prioridade).
    /// Se nenhuma regra match, retorna Allow por default.
    pub async fn evaluate(
        &self,
        employee_id: &str,
        role: &EmployeeRole,
        routine_name: &str,
        resource: Option<&str>,
    ) -> GuardrailDecision {
        let rules = self.rules.read().await;

        let mut matched_action = GuardrailAction::Allow;
        let mut matched_rule_name: Option<String> = None;

        for rule in rules.iter() {
            if rule.matches(role, routine_name, resource) {
                matched_action = rule.action.clone();
                matched_rule_name = Some(rule.name.clone());
                break; // Primeira regra que match vence (já ordenado por prioridade)
            }
        }

        let decision = GuardrailDecision {
            action: matched_action,
            matched_rule: matched_rule_name,
            decided_at: chrono_now_stub(),
            context: EvaluationContext {
                employee_id: employee_id.to_string(),
                role: *role,
                routine_name: routine_name.to_string(),
                resource: resource.map(|r| r.to_string()),
            },
        };

        // Log da decisão
        self.log_decision(decision.clone()).await;

        decision
    }

    /// Versão síncrona de evaluate para uso em contextos não-async.
    pub fn evaluate_sync(
        &self,
        employee_id: &str,
        role: &EmployeeRole,
        routine_name: &str,
        resource: Option<&str>,
    ) -> GuardrailDecision {
        let rules = self.rules.blocking_read();

        let mut matched_action = GuardrailAction::Allow;
        let mut matched_rule_name: Option<String> = None;

        for rule in rules.iter() {
            if rule.matches(role, routine_name, resource) {
                matched_action = rule.action.clone();
                matched_rule_name = Some(rule.name.clone());
                break;
            }
        }

        GuardrailDecision {
            action: matched_action,
            matched_rule: matched_rule_name,
            decided_at: chrono_now_stub(),
            context: EvaluationContext {
                employee_id: employee_id.to_string(),
                role: *role,
                routine_name: routine_name.to_string(),
                resource: resource.map(|r| r.to_string()),
            },
        }
    }

    /// Lista todas as regras ativas.
    pub async fn list_rules(&self) -> Vec<GuardrailRule> {
        let rules = self.rules.read().await;
        rules.iter().filter(|r| r.enabled).cloned().collect()
    }

    /// Retorna o número de regras registradas (async).
    pub async fn rule_count(&self) -> usize {
        self.rules.read().await.len()
    }

    /// Retorna o número de regras registradas (sync, para uso fora de runtime Tokio).
    pub fn rule_count_sync(&self) -> usize {
        self.rules.blocking_read().len()
    }

    /// Retorna estatísticas do enforcer.
    pub async fn stats(&self) -> EnforcerStats {
        let rules = self.rules.read().await;
        let log = self.decision_log.read().await;

        let active_rules = rules.iter().filter(|r| r.enabled).count();
        let total_decisions = log.len();
        let blocked = log
            .iter()
            .filter(|d| matches!(d.action, GuardrailAction::Block { .. }))
            .count();
        let approved_required = log
            .iter()
            .filter(|d| matches!(d.action, GuardrailAction::RequireApproval { .. }))
            .count();

        EnforcerStats {
            total_rules: rules.len(),
            active_rules,
            total_decisions,
            blocked_count: blocked,
            approval_required_count: approved_required,
        }
    }

    /// Registra decisão no log de auditoria.
    async fn log_decision(&self, decision: GuardrailDecision) {
        let mut log = self.decision_log.write().await;
        if log.len() >= self.max_log_size {
            // Remover entradas mais antigas (FIFO)
            let drain_count = log.len() - self.max_log_size + 1;
            log.drain(..drain_count);
        }
        log.push(decision);
    }

    /// Retorna as últimas N decisões do log.
    pub async fn recent_decisions(&self, limit: usize) -> Vec<GuardrailDecision> {
        let log = self.decision_log.read().await;
        log.iter().rev().take(limit).cloned().collect()
    }
}

/// Erros do GuardrailEnforcer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardrailError {
    #[error("Regra inválida: {0}")]
    InvalidRule(String),
    #[error("Regra duplicada: {0}")]
    DuplicateRule(String),
}

/// Estatísticas do enforcer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnforcerStats {
    pub total_rules: usize,
    pub active_rules: usize,
    pub total_decisions: usize,
    pub blocked_count: usize,
    pub approval_required_count: usize,
}

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
    fn test_pattern_matches_exact() {
        assert!(pattern_matches("deploy-staging", "deploy-staging"));
        assert!(!pattern_matches("deploy-staging", "deploy-production"));
    }

    #[test]
    fn test_pattern_matches_wildcard_all() {
        assert!(pattern_matches("*", "anything"));
        assert!(pattern_matches("*", ""));
    }

    #[test]
    fn test_pattern_matches_prefix_wildcard() {
        assert!(pattern_matches("deploy-*", "deploy-staging"));
        assert!(pattern_matches("deploy-*", "deploy-production"));
        assert!(!pattern_matches("deploy-*", "build-project"));
    }

    #[test]
    fn test_pattern_matches_suffix_wildcard() {
        assert!(pattern_matches("*-production", "deploy-production"));
        assert!(!pattern_matches("*-production", "deploy-staging"));
    }

    #[test]
    fn test_guardrail_rule_matches_role_filter() {
        let rule =
            GuardrailRule::block("test", "*", "blocked").for_roles(vec![EmployeeRole::Developer]);

        assert!(rule.matches(&EmployeeRole::Developer, "any-routine", None));
        assert!(!rule.matches(&EmployeeRole::Designer, "any-routine", None));
    }

    #[test]
    fn test_guardrail_rule_matches_all_roles_when_empty() {
        let rule = GuardrailRule::block("test", "*", "blocked");
        assert!(rule.matches(&EmployeeRole::Developer, "any", None));
        assert!(rule.matches(&EmployeeRole::Designer, "any", None));
        assert!(rule.matches(&EmployeeRole::DevOps, "any", None));
    }

    #[test]
    fn test_guardrail_rule_matches_routine_pattern() {
        let rule = GuardrailRule::block("test", "deploy-*", "blocked");
        assert!(rule.matches(&EmployeeRole::DevOps, "deploy-staging", None));
        assert!(!rule.matches(&EmployeeRole::DevOps, "build-project", None));
    }

    #[test]
    fn test_guardrail_rule_matches_resource_pattern() {
        let rule = GuardrailRule::block("test", "*", "blocked").for_resource("filesystem:/etc/*");

        assert!(rule.matches(
            &EmployeeRole::DevOps,
            "read-config",
            Some("filesystem:/etc/passwd")
        ));
        assert!(!rule.matches(
            &EmployeeRole::DevOps,
            "read-config",
            Some("filesystem:/home/user/file")
        ));
        // Sem recurso fornecido → não match
        assert!(!rule.matches(&EmployeeRole::DevOps, "read-config", None));
    }

    #[test]
    fn test_disabled_rule_never_matches() {
        let rule = GuardrailRule::block("test", "*", "blocked").disabled();
        assert!(!rule.matches(&EmployeeRole::Developer, "any", None));
    }

    #[tokio::test]
    async fn test_evaluate_blocks_matching_rule() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block(
                "no-delete",
                "delete-*",
                "Deletion not allowed",
            ))
            .await
            .unwrap();

        let decision = enforcer
            .evaluate("emp-1", &EmployeeRole::Developer, "delete-data", None)
            .await;

        assert!(matches!(decision.action, GuardrailAction::Block { .. }));
        assert_eq!(decision.matched_rule, Some("no-delete".into()));
    }

    #[tokio::test]
    async fn test_evaluate_allows_when_no_match() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("no-delete", "delete-*", "blocked"))
            .await
            .unwrap();

        let decision = enforcer
            .evaluate("emp-1", &EmployeeRole::Developer, "run-tests", None)
            .await;

        assert_eq!(decision.action, GuardrailAction::Allow);
        assert!(decision.matched_rule.is_none());
    }

    #[tokio::test]
    async fn test_evaluate_priority_ordering() {
        let enforcer = GuardrailEnforcer::new();

        // Regra de baixa prioridade: permite tudo
        enforcer
            .add_rule(GuardrailRule::allow("allow-all", "*").with_priority(-1))
            .await
            .unwrap();

        // Regra de alta prioridade: bloqueia deploy
        enforcer
            .add_rule(
                GuardrailRule::block("block-deploy", "deploy-*", "Deploy blocked")
                    .with_priority(10),
            )
            .await
            .unwrap();

        // Deploy deve ser bloqueado (alta prioridade vence)
        let d1 = enforcer
            .evaluate("e1", &EmployeeRole::DevOps, "deploy-staging", None)
            .await;
        assert!(matches!(d1.action, GuardrailAction::Block { .. }));
        assert_eq!(d1.matched_rule, Some("block-deploy".into()));

        // Outras rotinas devem ser permitidas
        let d2 = enforcer
            .evaluate("e1", &EmployeeRole::DevOps, "run-tests", None)
            .await;
        assert_eq!(d2.action, GuardrailAction::Allow);
    }

    #[tokio::test]
    async fn test_evaluate_require_approval() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::require_approval(
                "approve-prod",
                "deploy-production",
                "Prod deploy needs sign-off",
            ))
            .await
            .unwrap();

        let decision = enforcer
            .evaluate("e1", &EmployeeRole::DevOps, "deploy-production", None)
            .await;

        assert!(matches!(
            decision.action,
            GuardrailAction::RequireApproval { .. }
        ));
    }

    #[tokio::test]
    async fn test_duplicate_rule_rejected() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("dup", "*", "first"))
            .await
            .unwrap();

        let result = enforcer
            .add_rule(GuardrailRule::block("dup", "*", "second"))
            .await;
        assert!(matches!(result, Err(GuardrailError::DuplicateRule(_))));
    }

    #[tokio::test]
    async fn test_invalid_rule_empty_name() {
        let enforcer = GuardrailEnforcer::new();
        let result = enforcer
            .add_rule(GuardrailRule::block("", "*", "reason"))
            .await;
        assert!(matches!(result, Err(GuardrailError::InvalidRule(_))));
    }

    #[tokio::test]
    async fn test_remove_rule() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("removable", "*", "reason"))
            .await
            .unwrap();
        assert_eq!(enforcer.rule_count().await, 1);

        assert!(enforcer.remove_rule("removable").await);
        assert_eq!(enforcer.rule_count().await, 0);

        assert!(!enforcer.remove_rule("nonexistent").await);
    }

    #[tokio::test]
    async fn test_list_rules_excludes_disabled() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("active", "*", "reason"))
            .await
            .unwrap();
        enforcer
            .add_rule(GuardrailRule::block("inactive", "*", "reason").disabled())
            .await
            .unwrap();

        let active = enforcer.list_rules().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "active");
    }

    #[tokio::test]
    async fn test_decision_logging() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("log-test", "test-*", "blocked"))
            .await
            .unwrap();

        enforcer
            .evaluate("e1", &EmployeeRole::Developer, "test-routine", None)
            .await;
        enforcer
            .evaluate("e2", &EmployeeRole::Developer, "other-routine", None)
            .await;

        let recent = enforcer.recent_decisions(10).await;
        assert_eq!(recent.len(), 2);
        // Mais recente primeiro
        assert_eq!(recent[0].context.employee_id, "e2");
        assert_eq!(recent[1].context.employee_id, "e1");
    }

    #[tokio::test]
    async fn test_stats() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(GuardrailRule::block("b1", "block-*", "reason"))
            .await
            .unwrap();
        enforcer
            .add_rule(GuardrailRule::require_approval("a1", "approve-*", "msg"))
            .await
            .unwrap();

        enforcer
            .evaluate("e1", &EmployeeRole::Developer, "block-this", None)
            .await;
        enforcer
            .evaluate("e2", &EmployeeRole::Developer, "approve-this", None)
            .await;
        enforcer
            .evaluate("e3", &EmployeeRole::Developer, "normal-op", None)
            .await;

        let stats = enforcer.stats().await;
        assert_eq!(stats.total_rules, 2);
        assert_eq!(stats.active_rules, 2);
        assert_eq!(stats.total_decisions, 3);
        assert_eq!(stats.blocked_count, 1);
        assert_eq!(stats.approval_required_count, 1);
    }

    // with_defaults usa add_rule_sync internamente — testar fora de runtime Tokio.
    #[test]
    fn test_with_defaults_outside_runtime() {
        let enforcer = GuardrailEnforcer::with_defaults();
        assert!(enforcer.rule_count_sync() > 0);

        // Financial operations should require approval
        let d = enforcer.evaluate_sync("e1", &EmployeeRole::Developer, "financial-transfer", None);
        assert!(matches!(d.action, GuardrailAction::RequireApproval { .. }));

        // Production deploy should require approval
        let d = enforcer.evaluate_sync("e1", &EmployeeRole::DevOps, "deploy-production", None);
        assert!(matches!(d.action, GuardrailAction::RequireApproval { .. }));

        // Normal operation should be allowed
        let d = enforcer.evaluate_sync("e1", &EmployeeRole::Developer, "run-tests", None);
        assert_eq!(d.action, GuardrailAction::Allow);
    }

    #[tokio::test]
    async fn test_role_specific_guardrail() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule(
                GuardrailRule::block("designer-no-shell", "*", "Designers cannot use shell")
                    .for_roles(vec![EmployeeRole::Designer])
                    .for_resource("exec:shell:*"),
            )
            .await
            .unwrap();

        // Designer blocked from shell
        let d = enforcer
            .evaluate(
                "d1",
                &EmployeeRole::Designer,
                "run-script",
                Some("exec:shell:bash"),
            )
            .await;
        assert!(matches!(d.action, GuardrailAction::Block { .. }));

        // Developer NOT blocked (different role)
        let d = enforcer
            .evaluate(
                "dev1",
                &EmployeeRole::Developer,
                "run-script",
                Some("exec:shell:bash"),
            )
            .await;
        assert_eq!(d.action, GuardrailAction::Allow);
    }

    // evaluate_sync usa blocking_read internamente — não pode rodar dentro de #[tokio::test].
    // Testado via thread dedicada fora do runtime Tokio.
    #[test]
    fn test_evaluate_sync_outside_runtime() {
        let enforcer = GuardrailEnforcer::new();
        enforcer
            .add_rule_sync(GuardrailRule::block("sync-test", "sync-*", "blocked"))
            .unwrap();

        let decision = enforcer.evaluate_sync("e1", &EmployeeRole::Developer, "sync-routine", None);
        assert!(matches!(decision.action, GuardrailAction::Block { .. }));
    }

    #[test]
    fn test_guardrail_action_display() {
        assert_eq!(format!("{}", GuardrailAction::Allow), "allow");
        assert!(format!(
            "{}",
            GuardrailAction::Block {
                reason: "test".into()
            }
        )
        .contains("test"));
        assert!(format!(
            "{}",
            GuardrailAction::RequireApproval {
                message: "approve me".into()
            }
        )
        .contains("approve me"));
    }

    #[test]
    fn test_guardrail_rule_serialization() {
        let rule = GuardrailRule::block("ser-test", "deploy-*", "blocked")
            .for_roles(vec![EmployeeRole::DevOps])
            .for_resource("k8s:*")
            .with_priority(50)
            .with_description("Test serialization");

        let json = serde_json::to_string(&rule).unwrap();
        let restored: GuardrailRule = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "ser-test");
        assert_eq!(restored.priority, 50);
        assert_eq!(restored.applicable_roles.len(), 1);
    }

    #[test]
    fn test_enforcer_stats_serialization() {
        let stats = EnforcerStats {
            total_rules: 10,
            active_rules: 8,
            total_decisions: 100,
            blocked_count: 15,
            approval_required_count: 5,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let restored: EnforcerStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }

    #[test]
    fn test_evaluation_context_serialization() {
        let ctx = EvaluationContext {
            employee_id: "emp-1".into(),
            role: EmployeeRole::SecurityAuditor,
            routine_name: "vulnerability-scan".into(),
            resource: Some("network:scan:*".into()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let restored: EvaluationContext = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.employee_id, "emp-1");
        assert_eq!(restored.role, EmployeeRole::SecurityAuditor);
    }
}
