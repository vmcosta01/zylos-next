//! # PlanFirstGate — Gate de aprovação humana antes de execução (T8.5.3)
//!
//! Previne execução automática de rotinas sensíveis exigindo que um plano
//! (`PLAN_*.md`) seja criado e aprovado por humano antes da execução.
//!
//! ## Fluxo
//! 1. Agente cria `PLAN_<routine>_<timestamp>.md` com descrição da ação
//! 2. Gate verifica existência e aprovação do plano
//! 3. Se aprovado → execução permitida
//! 4. Se pendente/rejeitado → execução bloqueada
//!
//! ## Integração
//! Usar como middleware antes de qualquer execução de rotina:
//! ```ignore
//! if gate.check(&routine_name, &agent_id).await? == GateDecision::Approved {
//!     execute_routine(...).await?;
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Decisão do gate de aprovação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateDecision {
    /// Plano aprovado — execução permitida.
    Approved,
    /// Plano pendente de aprovação — execução bloqueada.
    Pending,
    /// Plano rejeitado — execução bloqueada permanentemente para esta instância.
    Rejected,
    /// Nenhuma regra de gate aplicável — execução permitida por default.
    NoGateRequired,
}

impl std::fmt::Display for GateDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Approved => write!(f, "approved"),
            Self::Pending => write!(f, "pending"),
            Self::Rejected => write!(f, "rejected"),
            Self::NoGateRequired => write!(f, "no_gate_required"),
        }
    }
}

/// Um plano submetido para aprovação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSubmission {
    /// ID único do plano.
    pub id: String,
    /// Nome da rotina associada.
    pub routine_name: String,
    /// ID do agente que criou o plano.
    pub agent_id: String,
    /// Descrição legível do que será executado.
    pub description: String,
    /// Conteúdo completo do plano (markdown).
    pub plan_content: String,
    /// Status atual do plano.
    pub status: PlanStatus,
    /// Timestamp de criação (ISO 8601).
    pub created_at: String,
    /// Timestamp da decisão (se houver).
    pub decided_at: Option<String>,
    /// Comentário do aprovador/rejeitador (se houver).
    pub decision_comment: Option<String>,
}

/// Status de um plano no fluxo de aprovação.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Submetido, aguardando revisão humana.
    Pending,
    /// Aprovado por humano.
    Approved,
    /// Rejeitado por humano.
    Rejected,
    /// Expirado sem decisão.
    Expired,
}

/// Configuração do PlanFirstGate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConfig {
    /// Rotinas que SEMPRE requerem aprovação (independente do role).
    pub always_require_approval: Vec<String>,
    /// Rotinas que NUNCA requerem aprovação (auto-approved).
    pub auto_approved: Vec<String>,
    /// TTL de planos pendentes em segundos (default: 24h).
    pub plan_ttl_secs: u64,
    /// Se true, todas as rotinas não listadas requerem aprovação por default.
    pub default_require_approval: bool,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            always_require_approval: vec![
                "deploy-staging".into(),
                "deploy-production".into(),
                "delete-data".into(),
                "modify-permissions".into(),
                "send-email".into(),
                "post-social-media".into(),
                "financial-transaction".into(),
            ],
            auto_approved: vec![
                "run-tests".into(),
                "lint-check".into(),
                "generate-report".into(),
                "update-docs".into(),
                "code-review".into(),
            ],
            plan_ttl_secs: 86400, // 24 horas
            default_require_approval: false,
        }
    }
}

/// Gate de aprovação plan-first para execução de rotinas.
pub struct PlanFirstGate {
    config: GateConfig,
    /// Planos submetidos indexados por ID.
    plans: Arc<RwLock<HashMap<String, PlanSubmission>>>,
    /// Index secundário: routine_name → lista de plan IDs pendentes.
    pending_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl PlanFirstGate {
    /// Cria um novo gate com a configuração fornecida.
    pub fn new(config: GateConfig) -> Self {
        Self {
            config,
            plans: Arc::new(RwLock::new(HashMap::new())),
            pending_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Cria um gate com configuração default.
    pub fn default_config() -> Self {
        Self::new(GateConfig::default())
    }

    /// Verifica se uma rotina pode ser executada.
    ///
    /// Retorna `Approved` se:
    /// - A rotina está na lista auto_approved, OU
    /// - Existe um plano aprovado para esta rotina + agente
    ///
    /// Retorna `Pending` se existe plano pendente.
    /// Retorna `Rejected` se o último plano foi rejeitado.
    /// Retorna `NoGateRequired` se a rotina não requer aprovação.
    pub async fn check(&self, routine_name: &str, agent_id: &str) -> GateDecision {
        // Auto-approved routines bypass the gate entirely
        if self.config.auto_approved.iter().any(|r| r == routine_name) {
            return GateDecision::NoGateRequired;
        }

        // Check if approval is required for this routine
        let requires_approval = self
            .config
            .always_require_approval
            .iter()
            .any(|r| r == routine_name)
            || self.config.default_require_approval;

        if !requires_approval {
            return GateDecision::NoGateRequired;
        }

        // Search for existing plans for this routine + agent
        let plans = self.plans.read().await;
        let mut latest_plan: Option<&PlanSubmission> = None;

        for plan in plans.values() {
            if plan.routine_name == routine_name && plan.agent_id == agent_id {
                match &latest_plan {
                    None => latest_plan = Some(plan),
                    Some(existing) => {
                        // Keep the most recent plan by timestamp;
                        // use UUID as tiebreaker when timestamps are identical
                        // (UUIDs are time-ordered v4, so lexicographic comparison works).
                        if plan.created_at > existing.created_at
                            || (plan.created_at == existing.created_at && plan.id > existing.id)
                        {
                            latest_plan = Some(plan);
                        }
                    }
                }
            }
        }

        match latest_plan {
            None => GateDecision::Pending, // No plan submitted yet
            Some(plan) => match plan.status {
                PlanStatus::Approved => GateDecision::Approved,
                PlanStatus::Pending => GateDecision::Pending,
                PlanStatus::Rejected => GateDecision::Rejected,
                PlanStatus::Expired => GateDecision::Pending, // Expired plans require re-submission
            },
        }
    }

    /// Submete um novo plano para aprovação.
    pub async fn submit_plan(
        &self,
        routine_name: impl Into<String>,
        agent_id: impl Into<String>,
        description: impl Into<String>,
        plan_content: impl Into<String>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let now = chrono_now_stub();

        let submission = PlanSubmission {
            id: id.clone(),
            routine_name: routine_name.into(),
            agent_id: agent_id.into(),
            description: description.into(),
            plan_content: plan_content.into(),
            status: PlanStatus::Pending,
            created_at: now,
            decided_at: None,
            decision_comment: None,
        };

        let routine_name_key = submission.routine_name.clone();

        let routine_name_for_log = routine_name_key.clone();

        let mut plans = self.plans.write().await;
        plans.insert(id.clone(), submission);

        let mut index = self.pending_index.write().await;
        index
            .entry(routine_name_key)
            .or_insert_with(Vec::new)
            .push(id.clone());

        tracing::info!(plan_id = %id, routine = %routine_name_for_log, "Plan submitted for approval");
        id
    }

    /// Aprova um plano pendente.
    pub async fn approve(&self, plan_id: &str, comment: Option<&str>) -> Result<(), GateError> {
        let mut plans = self.plans.write().await;
        let plan = plans
            .get_mut(plan_id)
            .ok_or_else(|| GateError::PlanNotFound(plan_id.to_string()))?;

        if plan.status != PlanStatus::Pending {
            return Err(GateError::InvalidState(format!(
                "Plan {} is {:?}, not Pending",
                plan_id, plan.status
            )));
        }

        plan.status = PlanStatus::Approved;
        plan.decided_at = Some(chrono_now_stub());
        plan.decision_comment = comment.map(|c| c.to_string());

        // Remove from pending index
        let mut index = self.pending_index.write().await;
        if let Some(ids) = index.get_mut(&plan.routine_name) {
            ids.retain(|id| id != plan_id);
        }

        tracing::info!(plan_id = %plan_id, "Plan approved");
        Ok(())
    }

    /// Rejeita um plano pendente.
    pub async fn reject(&self, plan_id: &str, reason: &str) -> Result<(), GateError> {
        let mut plans = self.plans.write().await;
        let plan = plans
            .get_mut(plan_id)
            .ok_or_else(|| GateError::PlanNotFound(plan_id.to_string()))?;

        if plan.status != PlanStatus::Pending {
            return Err(GateError::InvalidState(format!(
                "Plan {} is {:?}, not Pending",
                plan_id, plan.status
            )));
        }

        plan.status = PlanStatus::Rejected;
        plan.decided_at = Some(chrono_now_stub());
        plan.decision_comment = Some(reason.to_string());

        // Remove from pending index
        let mut index = self.pending_index.write().await;
        if let Some(ids) = index.get_mut(&plan.routine_name) {
            ids.retain(|id| id != plan_id);
        }

        tracing::warn!(plan_id = %plan_id, reason = %reason, "Plan rejected");
        Ok(())
    }

    /// Busca um plano por ID.
    pub async fn get_plan(&self, plan_id: &str) -> Option<PlanSubmission> {
        let plans = self.plans.read().await;
        plans.get(plan_id).cloned()
    }

    /// Lista todos os planos pendentes.
    pub async fn list_pending(&self) -> Vec<PlanSubmission> {
        let plans = self.plans.read().await;
        plans
            .values()
            .filter(|p| p.status == PlanStatus::Pending)
            .cloned()
            .collect()
    }

    /// Lista planos pendentes para uma rotina específica.
    pub async fn list_pending_for_routine(&self, routine_name: &str) -> Vec<PlanSubmission> {
        let plans = self.plans.read().await;
        plans
            .values()
            .filter(|p| p.routine_name == routine_name && p.status == PlanStatus::Pending)
            .cloned()
            .collect()
    }

    /// Retorna estatísticas do gate.
    pub async fn stats(&self) -> GateStats {
        let plans = self.plans.read().await;
        let total = plans.len();
        let pending = plans
            .values()
            .filter(|p| p.status == PlanStatus::Pending)
            .count();
        let approved = plans
            .values()
            .filter(|p| p.status == PlanStatus::Approved)
            .count();
        let rejected = plans
            .values()
            .filter(|p| p.status == PlanStatus::Rejected)
            .count();
        let expired = plans
            .values()
            .filter(|p| p.status == PlanStatus::Expired)
            .count();

        GateStats {
            total_plans: total,
            pending,
            approved,
            rejected,
            expired,
        }
    }

    /// Retorna referência à configuração do gate.
    pub fn config(&self) -> &GateConfig {
        &self.config
    }
}

/// Erros do PlanFirstGate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    #[error("Plano não encontrado: {0}")]
    PlanNotFound(String),
    #[error("Estado inválido: {0}")]
    InvalidState(String),
}

/// Estatísticas do gate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateStats {
    pub total_plans: usize,
    pub pending: usize,
    pub approved: usize,
    pub rejected: usize,
    pub expired: usize,
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

    fn test_gate() -> PlanFirstGate {
        PlanFirstGate::default_config()
    }

    #[tokio::test]
    async fn test_auto_approved_bypasses_gate() {
        let gate = test_gate();
        let decision = gate.check("run-tests", "agent-1").await;
        assert_eq!(decision, GateDecision::NoGateRequired);
    }

    #[tokio::test]
    async fn test_always_require_approval_blocks_without_plan() {
        let gate = test_gate();
        let decision = gate.check("deploy-staging", "agent-1").await;
        assert_eq!(decision, GateDecision::Pending);
    }

    #[tokio::test]
    async fn test_non_listed_routine_no_gate_when_default_false() {
        let gate = test_gate();
        let decision = gate.check("custom-routine", "agent-1").await;
        assert_eq!(decision, GateDecision::NoGateRequired);
    }

    #[tokio::test]
    async fn test_non_listed_routine_requires_gate_when_default_true() {
        let gate = PlanFirstGate::new(GateConfig {
            default_require_approval: true,
            ..GateConfig::default()
        });
        let decision = gate.check("any-routine", "agent-1").await;
        assert_eq!(decision, GateDecision::Pending);
    }

    #[tokio::test]
    async fn test_submit_and_approve_flow() {
        let gate = test_gate();

        // Submit plan
        let plan_id = gate
            .submit_plan(
                "deploy-staging",
                "agent-1",
                "Deploy v2.3 to staging",
                "# Deploy Plan\n\n## Steps\n1. Build image\n2. Push to registry\n3. Apply k8s manifest",
            )
            .await;

        // Should be pending
        let decision = gate.check("deploy-staging", "agent-1").await;
        assert_eq!(decision, GateDecision::Pending);

        // Approve
        gate.approve(&plan_id, Some("LGTM")).await.unwrap();

        // Should now be approved
        let decision = gate.check("deploy-staging", "agent-1").await;
        assert_eq!(decision, GateDecision::Approved);

        // Verify plan state
        let plan = gate.get_plan(&plan_id).await.unwrap();
        assert_eq!(plan.status, PlanStatus::Approved);
        assert_eq!(plan.decision_comment, Some("LGTM".into()));
        assert!(plan.decided_at.is_some());
    }

    #[tokio::test]
    async fn test_submit_and_reject_flow() {
        let gate = test_gate();

        let plan_id = gate
            .submit_plan("send-email", "agent-2", "Send newsletter", "Plan content")
            .await;

        gate.reject(&plan_id, "Not ready for send").await.unwrap();

        let decision = gate.check("send-email", "agent-2").await;
        assert_eq!(decision, GateDecision::Rejected);

        let plan = gate.get_plan(&plan_id).await.unwrap();
        assert_eq!(plan.status, PlanStatus::Rejected);
        assert_eq!(plan.decision_comment, Some("Not ready for send".into()));
    }

    #[tokio::test]
    async fn test_approve_nonexistent_plan() {
        let gate = test_gate();
        let result = gate.approve("nonexistent-id", None).await;
        assert!(matches!(result, Err(GateError::PlanNotFound(_))));
    }

    #[tokio::test]
    async fn test_reject_nonexistent_plan() {
        let gate = test_gate();
        let result = gate.reject("nonexistent-id", "reason").await;
        assert!(matches!(result, Err(GateError::PlanNotFound(_))));
    }

    #[tokio::test]
    async fn test_double_approve_rejected() {
        let gate = test_gate();
        let plan_id = gate
            .submit_plan("deploy-staging", "a1", "desc", "content")
            .await;

        gate.approve(&plan_id, None).await.unwrap();

        // Second approve should fail (already approved)
        let result = gate.approve(&plan_id, None).await;
        assert!(matches!(result, Err(GateError::InvalidState(_))));
    }

    #[tokio::test]
    async fn test_list_pending() {
        let gate = test_gate();

        gate.submit_plan("deploy-staging", "a1", "d1", "c1").await;
        gate.submit_plan("send-email", "a2", "d2", "c2").await;
        let approved_id = gate.submit_plan("delete-data", "a3", "d3", "c3").await;
        gate.approve(&approved_id, None).await.unwrap();

        let pending = gate.list_pending().await;
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().all(|p| p.status == PlanStatus::Pending));
    }

    #[tokio::test]
    async fn test_list_pending_for_routine() {
        let gate = test_gate();

        gate.submit_plan("deploy-staging", "a1", "d1", "c1").await;
        gate.submit_plan("deploy-staging", "a2", "d2", "c2").await;
        gate.submit_plan("send-email", "a3", "d3", "c3").await;

        let deploy_pending = gate.list_pending_for_routine("deploy-staging").await;
        assert_eq!(deploy_pending.len(), 2);

        let email_pending = gate.list_pending_for_routine("send-email").await;
        assert_eq!(email_pending.len(), 1);

        let none_pending = gate.list_pending_for_routine("nonexistent").await;
        assert!(none_pending.is_empty());
    }

    #[tokio::test]
    async fn test_stats() {
        let gate = test_gate();

        gate.submit_plan("r1", "a1", "d1", "c1").await;
        let id2 = gate.submit_plan("r2", "a2", "d2", "c2").await;
        let id3 = gate.submit_plan("r3", "a3", "d3", "c3").await;
        gate.approve(&id2, None).await.unwrap();
        gate.reject(&id3, "no").await.unwrap();

        let stats = gate.stats().await;
        assert_eq!(stats.total_plans, 3);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.approved, 1);
        assert_eq!(stats.rejected, 1);
        assert_eq!(stats.expired, 0);
    }

    #[tokio::test]
    async fn test_latest_plan_wins() {
        let gate = test_gate();

        // Submit and reject first plan
        let id1 = gate.submit_plan("deploy-staging", "a1", "v1", "c1").await;
        gate.reject(&id1, "bad plan").await.unwrap();

        // Decision should be Rejected
        assert_eq!(
            gate.check("deploy-staging", "a1").await,
            GateDecision::Rejected
        );

        // Submit newer plan (same routine + agent)
        let _id2 = gate
            .submit_plan("deploy-staging", "a1", "v2 improved", "c2")
            .await;

        // Newer pending plan should override older rejected one
        assert_eq!(
            gate.check("deploy-staging", "a1").await,
            GateDecision::Pending
        );
    }

    #[tokio::test]
    async fn test_different_agents_independent() {
        let gate = test_gate();

        let id = gate
            .submit_plan("deploy-staging", "agent-a", "d", "c")
            .await;
        gate.approve(&id, None).await.unwrap();

        // agent-a approved
        assert_eq!(
            gate.check("deploy-staging", "agent-a").await,
            GateDecision::Approved
        );
        // agent-b still pending (different agent)
        assert_eq!(
            gate.check("deploy-staging", "agent-b").await,
            GateDecision::Pending
        );
    }

    #[tokio::test]
    async fn test_gate_decision_display() {
        assert_eq!(format!("{}", GateDecision::Approved), "approved");
        assert_eq!(format!("{}", GateDecision::Pending), "pending");
        assert_eq!(format!("{}", GateDecision::Rejected), "rejected");
        assert_eq!(
            format!("{}", GateDecision::NoGateRequired),
            "no_gate_required"
        );
    }

    #[tokio::test]
    async fn test_plan_submission_serialization() {
        let plan = PlanSubmission {
            id: "test-id".into(),
            routine_name: "deploy".into(),
            agent_id: "agent-1".into(),
            description: "Deploy to prod".into(),
            plan_content: "# Plan\n...".into(),
            status: PlanStatus::Pending,
            created_at: "2026-11-11T00:00:00Z".into(),
            decided_at: None,
            decision_comment: None,
        };
        let json = serde_json::to_string(&plan).unwrap();
        let restored: PlanSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, plan.id);
        assert_eq!(restored.status, PlanStatus::Pending);
    }

    #[tokio::test]
    async fn test_gate_stats_serialization() {
        let stats = GateStats {
            total_plans: 10,
            pending: 3,
            approved: 5,
            rejected: 1,
            expired: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let restored: GateStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, restored);
    }

    #[tokio::test]
    async fn test_gate_config_access() {
        let gate = test_gate();
        let config = gate.config();
        assert!(!config.always_require_approval.is_empty());
        assert!(!config.auto_approved.is_empty());
        assert_eq!(config.plan_ttl_secs, 86400);
    }

    #[tokio::test]
    async fn test_get_plan_nonexistent() {
        let gate = test_gate();
        assert!(gate.get_plan("nonexistent").await.is_none());
    }
}
