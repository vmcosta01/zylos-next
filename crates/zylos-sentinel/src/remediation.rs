//! Auto-remediação gated (T11.7.5).
//!
//! Fluxo: `TriggeredAlert` → [`RemediationGate::propose`] → [`PendingRemediation`]
//! → aprovação humana (HITL) → [`RemediationGate::approve`] → executor.
//!
//! Regras de segurança (invariantes testadas):
//! 1. Modo padrão é [`GateMode::DryRun`]: nada é executado, apenas registrado.
//! 2. Ações destrutivas (`restart`, `scale`) **sempre** exigem aprovação humana,
//!    mesmo em [`GateMode::Auto`] e mesmo que a regra YAML diga `gated: false`.
//! 3. Apenas `notify-oncall` (não destrutiva) pode executar sem aprovação, e
//!    somente em `GateMode::Auto` + regra `gated: false` + dentro do limite
//!    `max_auto_executions`.
//! 4. Aprovador não pode ser vazio; propostas expiram após `approval_ttl_secs`.
//! 5. Toda decisão vira um [`RemediationAuditEntry`] e um [`SoarRemediationEvent`]
//!    para o orquestrador SOAR (o gateway converte para `ZylosSecurityEvent`).

use crate::alert_rules::{AlertRulesEngine, AlertSeverity, TriggeredAlert};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ação de remediação suportada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RemediationAction {
    /// Reiniciar serviço.
    RestartService {
        /// Nome do serviço.
        service: String,
    },
    /// Escalar réplicas.
    ScaleReplicas {
        /// Nome do serviço.
        service: String,
        /// Número desejado de réplicas.
        replicas: u32,
    },
    /// Notificar on-call.
    NotifyOncall {
        /// Canal (ex.: `pagerduty`, `slack:#oncall`).
        channel: String,
    },
}

/// Limite superior de réplicas aceitas em uma proposta (proteção contra typo).
pub const MAX_REPLICAS: u32 = 20;

impl RemediationAction {
    /// Faz parse do campo `remediation.action` do YAML.
    ///
    /// Formatos aceitos:
    /// - `restart-<service>` ou `restart:<service>`
    /// - `scale:<service>:<replicas>`
    /// - `notify-oncall` ou `notify-oncall:<channel>`
    /// - `none` → `Ok(None)`
    ///
    /// # Errors
    /// Formato desconhecido ou réplicas inválidas.
    pub fn parse(action: &str) -> Result<Option<Self>, String> {
        let a = action.trim();
        if a.is_empty() || a.eq_ignore_ascii_case("none") {
            return Ok(None);
        }
        if let Some(rest) = a.strip_prefix("notify-oncall") {
            let channel = rest.trim_start_matches(':').trim();
            return Ok(Some(Self::NotifyOncall {
                channel: if channel.is_empty() {
                    "oncall".to_string()
                } else {
                    channel.to_string()
                },
            }));
        }
        if let Some(rest) = a.strip_prefix("scale:") {
            let mut parts = rest.split(':');
            let service = parts.next().unwrap_or("").trim();
            let replicas: u32 = parts
                .next()
                .and_then(|r| r.trim().parse().ok())
                .ok_or_else(|| format!("réplicas inválidas em '{a}'"))?;
            if service.is_empty() || replicas == 0 || replicas > MAX_REPLICAS {
                return Err(format!(
                    "scale inválido em '{a}' (serviço vazio ou réplicas fora de 1..={MAX_REPLICAS})"
                ));
            }
            return Ok(Some(Self::ScaleReplicas {
                service: service.to_string(),
                replicas,
            }));
        }
        let restart = a
            .strip_prefix("restart:")
            .or_else(|| a.strip_prefix("restart-"));
        if let Some(service) = restart {
            let service = service.trim();
            if service.is_empty() {
                return Err(format!("serviço vazio em '{a}'"));
            }
            return Ok(Some(Self::RestartService {
                service: service.to_string(),
            }));
        }
        Err(format!("ação de remediação desconhecida: '{a}'"))
    }

    /// Ação altera estado de infraestrutura (exige HITL sempre).
    pub fn is_destructive(&self) -> bool {
        !matches!(self, Self::NotifyOncall { .. })
    }

    /// Rótulo curto para logs/UI.
    pub fn label(&self) -> String {
        match self {
            Self::RestartService { service } => format!("restart:{service}"),
            Self::ScaleReplicas { service, replicas } => format!("scale:{service}:{replicas}"),
            Self::NotifyOncall { channel } => format!("notify-oncall:{channel}"),
        }
    }
}

/// Modo do gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateMode {
    /// Padrão: registra a proposta e o que *seria* feito; nunca executa.
    #[default]
    DryRun,
    /// Toda ação exige aprovação humana.
    RequireApproval,
    /// Notificações não-gated executam sozinhas; destrutivas ainda exigem aprovação.
    Auto,
}

impl GateMode {
    /// Lê `SENTINEL_REMEDIATION_MODE` (`dry_run` | `require_approval` | `auto`).
    /// Valor ausente ou inválido → `DryRun` (fail-safe).
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
            Some("require_approval") => Self::RequireApproval,
            Some("auto") => Self::Auto,
            _ => Self::DryRun,
        }
    }
}

/// Estado de uma proposta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemediationStatus {
    /// Aguardando aprovação humana.
    PendingApproval,
    /// Registrada em dry-run (não executada).
    DryRun,
    /// Executada com sucesso.
    Executed,
    /// Executor falhou.
    Failed,
    /// Rejeitada por humano.
    Rejected,
    /// Expirou sem decisão.
    Expired,
}

/// Proposta de remediação.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingRemediation {
    /// Identificador (`rem-<n>`).
    pub id: String,
    /// Regra que disparou.
    pub rule_name: String,
    /// Ação.
    pub action: RemediationAction,
    /// Severidade do alerta.
    pub severity: AlertSeverity,
    /// Valor que disparou.
    pub current_value: f64,
    /// Estado.
    pub status: RemediationStatus,
    /// Criada em (epoch s).
    pub created_at_secs: u64,
    /// Quem decidiu (se houver).
    pub decided_by: Option<String>,
    /// Mensagem do executor / motivo.
    pub detail: Option<String>,
}

/// Executor real das ações (implementado pelo gateway / infra).
pub trait RemediationExecutor: Send + Sync {
    /// Executa a ação. Retorna detalhe ou erro.
    ///
    /// # Errors
    /// Falha na execução.
    fn execute(&self, action: &RemediationAction) -> Result<String, String>;
}

/// Executor que só registra (usado em dry-run e testes).
#[derive(Debug, Default)]
pub struct RecordingExecutor {
    calls: std::sync::Mutex<Vec<RemediationAction>>,
}

impl RecordingExecutor {
    /// Ações recebidas até agora.
    pub fn calls(&self) -> Vec<RemediationAction> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }
}

impl RemediationExecutor for RecordingExecutor {
    fn execute(&self, action: &RemediationAction) -> Result<String, String> {
        if let Ok(mut c) = self.calls.lock() {
            c.push(action.clone());
        }
        Ok(format!("recorded {}", action.label()))
    }
}

/// Entrada de auditoria do gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemediationAuditEntry {
    /// ID da proposta.
    pub remediation_id: String,
    /// Evento (`proposed`, `approved`, `rejected`, `executed`, `failed`, `dry_run`, `expired`).
    pub event: String,
    /// Ator (`sentinel` ou aprovador).
    pub actor: String,
    /// Epoch s.
    pub at_secs: u64,
}

/// Evento para o orquestrador SOAR (agnóstico de crate para evitar dependência
/// circular/pesada; o gateway converte para `zylos_soar::ZylosSecurityEvent`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SoarRemediationEvent {
    /// Sempre `zylos-sentinel`.
    pub source: String,
    /// `critical` | `high` | `medium` | `low` (snake_case do zylos-soar).
    pub severity: String,
    /// Sempre `anomaly`.
    pub category: String,
    /// Título.
    pub title: String,
    /// Descrição.
    pub description: String,
    /// Ativos afetados.
    pub affected_assets: Vec<String>,
    /// Payload bruto (proposta serializada).
    pub raw_payload: serde_json::Value,
}

impl SoarRemediationEvent {
    /// Constrói evento a partir de uma proposta.
    pub fn from_pending(p: &PendingRemediation) -> Self {
        let severity = match p.severity {
            AlertSeverity::Sev1 => "critical",
            AlertSeverity::Sev2 => "high",
            AlertSeverity::Sev3 => "medium",
            AlertSeverity::Sev4 => "low",
        };
        let asset = match &p.action {
            RemediationAction::RestartService { service }
            | RemediationAction::ScaleReplicas { service, .. } => service.clone(),
            RemediationAction::NotifyOncall { channel } => channel.clone(),
        };
        Self {
            source: "zylos-sentinel".to_string(),
            severity: severity.to_string(),
            category: "anomaly".to_string(),
            title: format!("Sentinel: {} → {}", p.rule_name, p.action.label()),
            description: format!(
                "Regra '{}' disparou (valor {:.2}); remediação {} em estado {:?}",
                p.rule_name,
                p.current_value,
                p.action.label(),
                p.status
            ),
            affected_assets: vec![asset],
            raw_payload: serde_json::to_value(p).unwrap_or(serde_json::Value::Null),
        }
    }
}

/// Configuração do gate.
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Modo.
    pub mode: GateMode,
    /// TTL da proposta pendente (s).
    pub approval_ttl_secs: u64,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            mode: GateMode::DryRun,
            approval_ttl_secs: 15 * 60,
        }
    }
}

/// Gate HITL de remediação.
pub struct RemediationGate {
    config: GateConfig,
    executor: Box<dyn RemediationExecutor>,
    proposals: Vec<PendingRemediation>,
    audit: Vec<RemediationAuditEntry>,
    soar_outbox: Vec<SoarRemediationEvent>,
    auto_counts: HashMap<String, u32>,
    next_id: u64,
}

impl std::fmt::Debug for RemediationGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemediationGate")
            .field("mode", &self.config.mode)
            .field("proposals", &self.proposals.len())
            .finish()
    }
}

impl RemediationGate {
    /// Cria gate com executor.
    pub fn new(config: GateConfig, executor: Box<dyn RemediationExecutor>) -> Self {
        Self {
            config,
            executor,
            proposals: Vec::new(),
            audit: Vec::new(),
            soar_outbox: Vec::new(),
            auto_counts: HashMap::new(),
            next_id: 1,
        }
    }

    /// Modo atual.
    pub fn mode(&self) -> GateMode {
        self.config.mode
    }

    fn log(&mut self, id: &str, event: &str, actor: &str, now: u64) {
        self.audit.push(RemediationAuditEntry {
            remediation_id: id.to_string(),
            event: event.to_string(),
            actor: actor.to_string(),
            at_secs: now,
        });
    }

    fn emit(&mut self, idx: usize) {
        let ev = SoarRemediationEvent::from_pending(&self.proposals[idx]);
        self.soar_outbox.push(ev);
    }

    /// Processa um alerta disparado. Devolve a proposta criada (se a regra tem
    /// remediação válida).
    ///
    /// # Errors
    /// Ação inválida no YAML.
    pub fn propose(
        &mut self,
        engine: &AlertRulesEngine,
        alert: &TriggeredAlert,
        now_secs: u64,
    ) -> Result<Option<PendingRemediation>, String> {
        let Some(rule) = engine.rules().iter().find(|r| r.name == alert.rule_name) else {
            return Ok(None);
        };
        let Some(rem) = &rule.remediation else {
            return Ok(None);
        };
        let Some(action) = RemediationAction::parse(&rem.action)? else {
            return Ok(None);
        };

        let id = format!("rem-{}", self.next_id);
        self.next_id += 1;
        let mut p = PendingRemediation {
            id: id.clone(),
            rule_name: alert.rule_name.clone(),
            action: action.clone(),
            severity: alert.severity,
            current_value: alert.current_value,
            status: RemediationStatus::PendingApproval,
            created_at_secs: now_secs,
            decided_by: None,
            detail: None,
        };
        self.log(&id, "proposed", "sentinel", now_secs);

        match self.config.mode {
            GateMode::DryRun => {
                p.status = RemediationStatus::DryRun;
                p.detail = Some(format!("dry-run: executaria {}", action.label()));
                self.log(&id, "dry_run", "sentinel", now_secs);
            }
            GateMode::RequireApproval => {}
            GateMode::Auto => {
                let count = self.auto_counts.get(&rule.name).copied().unwrap_or(0);
                let within_limit = rem.max_auto_executions.is_none_or(|max| count < max);
                if !action.is_destructive() && !rem.gated && within_limit {
                    *self.auto_counts.entry(rule.name.clone()).or_insert(0) += 1;
                    let r = self.executor.execute(&action);
                    self.apply_result(&mut p, r, "sentinel-auto", now_secs);
                }
            }
        }

        self.proposals.push(p.clone());
        let idx = self.proposals.len() - 1;
        self.emit(idx);
        Ok(Some(p))
    }

    fn apply_result(
        &mut self,
        p: &mut PendingRemediation,
        r: Result<String, String>,
        actor: &str,
        now: u64,
    ) {
        p.decided_by = Some(actor.to_string());
        match r {
            Ok(d) => {
                p.status = RemediationStatus::Executed;
                p.detail = Some(d);
                self.log(&p.id, "executed", actor, now);
            }
            Err(e) => {
                p.status = RemediationStatus::Failed;
                p.detail = Some(e);
                self.log(&p.id, "failed", actor, now);
            }
        }
    }

    fn find_pending(&mut self, id: &str, now: u64) -> Result<usize, String> {
        let idx = self
            .proposals
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("remediação '{id}' não encontrada"))?;
        if self.proposals[idx].status != RemediationStatus::PendingApproval {
            return Err(format!(
                "remediação '{id}' não está pendente ({:?})",
                self.proposals[idx].status
            ));
        }
        if now.saturating_sub(self.proposals[idx].created_at_secs) > self.config.approval_ttl_secs {
            self.proposals[idx].status = RemediationStatus::Expired;
            self.log(id, "expired", "sentinel", now);
            self.emit(idx);
            return Err(format!("remediação '{id}' expirou"));
        }
        Ok(idx)
    }

    /// Aprovação humana → executa a ação.
    ///
    /// # Errors
    /// Aprovador vazio, proposta inexistente/não pendente/expirada.
    pub fn approve(
        &mut self,
        id: &str,
        approver: &str,
        now_secs: u64,
    ) -> Result<PendingRemediation, String> {
        let approver = approver.trim();
        if approver.is_empty() {
            return Err("aprovador obrigatório".to_string());
        }
        let idx = self.find_pending(id, now_secs)?;
        self.log(id, "approved", approver, now_secs);
        let mut p = self.proposals[idx].clone();
        let r = self.executor.execute(&p.action);
        self.apply_result(&mut p, r, approver, now_secs);
        self.proposals[idx] = p.clone();
        self.emit(idx);
        Ok(p)
    }

    /// Rejeição humana.
    ///
    /// # Errors
    /// Aprovador vazio, proposta inexistente/não pendente/expirada.
    pub fn reject(
        &mut self,
        id: &str,
        approver: &str,
        reason: &str,
        now_secs: u64,
    ) -> Result<PendingRemediation, String> {
        let approver = approver.trim();
        if approver.is_empty() {
            return Err("aprovador obrigatório".to_string());
        }
        let idx = self.find_pending(id, now_secs)?;
        let p = &mut self.proposals[idx];
        p.status = RemediationStatus::Rejected;
        p.decided_by = Some(approver.to_string());
        p.detail = Some(reason.to_string());
        let out = p.clone();
        self.log(id, "rejected", approver, now_secs);
        self.emit(idx);
        Ok(out)
    }

    /// Todas as propostas.
    pub fn proposals(&self) -> &[PendingRemediation] {
        &self.proposals
    }

    /// Trilha de auditoria.
    pub fn audit(&self) -> &[RemediationAuditEntry] {
        &self.audit
    }

    /// Drena eventos pendentes para o SOAR.
    pub fn drain_soar_events(&mut self) -> Vec<SoarRemediationEvent> {
        std::mem::take(&mut self.soar_outbox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    const RULES: &str = r#"
- name: cpu-high
  metric: cpu_percent
  operator: ">"
  threshold: 80.0
  severity: SEV-2
  remediation:
    action: restart-gateway
    gated: false
- name: errors
  metric: error_rate_percent
  operator: ">"
  threshold: 1.0
  severity: SEV-1
  remediation:
    action: notify-oncall:pagerduty
    gated: false
    max_auto_executions: 1
- name: queue
  metric: queue_depth
  operator: ">"
  threshold: 100.0
  severity: SEV-3
  remediation:
    action: scale:gateway:4
    gated: true
"#;

    struct Shared(Arc<RecordingExecutor>);
    impl RemediationExecutor for Shared {
        fn execute(&self, a: &RemediationAction) -> Result<String, String> {
            self.0.execute(a)
        }
    }

    fn setup(mode: GateMode) -> (AlertRulesEngine, RemediationGate, Arc<RecordingExecutor>) {
        let engine = AlertRulesEngine::load_from_yaml(RULES).unwrap();
        let exec = Arc::new(RecordingExecutor::default());
        let gate = RemediationGate::new(
            GateConfig {
                mode,
                approval_ttl_secs: 60,
            },
            Box::new(Shared(exec.clone())),
        );
        (engine, gate, exec)
    }

    fn fire(engine: &mut AlertRulesEngine, metric: &str, v: f64) -> TriggeredAlert {
        engine.clear_cooldowns();
        let m: HashMap<String, f64> = [(metric.to_string(), v)].into();
        engine.evaluate(&m).pop().expect("alert")
    }

    #[test]
    fn parse_actions() {
        assert_eq!(
            RemediationAction::parse("restart-gateway").unwrap(),
            Some(RemediationAction::RestartService {
                service: "gateway".into()
            })
        );
        assert_eq!(
            RemediationAction::parse("scale:api:3").unwrap(),
            Some(RemediationAction::ScaleReplicas {
                service: "api".into(),
                replicas: 3
            })
        );
        assert_eq!(
            RemediationAction::parse("notify-oncall").unwrap(),
            Some(RemediationAction::NotifyOncall {
                channel: "oncall".into()
            })
        );
        assert_eq!(RemediationAction::parse("none").unwrap(), None);
        assert!(RemediationAction::parse("scale:api:0").is_err());
        assert!(RemediationAction::parse("scale:api:999").is_err());
        assert!(RemediationAction::parse("rm -rf /").is_err());
    }

    #[test]
    fn env_mode_defaults_to_dry_run() {
        assert_eq!(GateMode::from_env_value(None), GateMode::DryRun);
        assert_eq!(GateMode::from_env_value(Some("garbage")), GateMode::DryRun);
        assert_eq!(GateMode::from_env_value(Some("AUTO")), GateMode::Auto);
        assert_eq!(GateMode::default(), GateMode::DryRun);
    }

    #[test]
    fn dry_run_never_executes() {
        let (mut engine, mut gate, exec) = setup(GateMode::DryRun);
        for (m, v) in [
            ("cpu_percent", 95.0),
            ("error_rate_percent", 5.0),
            ("queue_depth", 500.0),
        ] {
            let a = fire(&mut engine, m, v);
            let p = gate.propose(&engine, &a, 0).unwrap().unwrap();
            assert_eq!(p.status, RemediationStatus::DryRun);
        }
        assert!(exec.calls().is_empty());
        // Dry-run não pode ser aprovado depois.
        assert!(gate.approve("rem-1", "alice", 1).is_err());
        assert_eq!(gate.drain_soar_events().len(), 3);
    }

    #[test]
    fn auto_mode_never_auto_executes_destructive() {
        let (mut engine, mut gate, exec) = setup(GateMode::Auto);
        // restart com gated:false ainda exige HITL.
        let a = fire(&mut engine, "cpu_percent", 95.0);
        let p = gate.propose(&engine, &a, 0).unwrap().unwrap();
        assert_eq!(p.status, RemediationStatus::PendingApproval);
        assert!(exec.calls().is_empty());
        // Aprovação executa a ação correta.
        let done = gate.approve(&p.id, "alice", 10).unwrap();
        assert_eq!(done.status, RemediationStatus::Executed);
        assert_eq!(
            exec.calls(),
            vec![RemediationAction::RestartService {
                service: "gateway".into()
            }]
        );
        assert_eq!(done.decided_by.as_deref(), Some("alice"));
    }

    #[test]
    fn auto_mode_notify_respects_max_auto_executions() {
        let (mut engine, mut gate, exec) = setup(GateMode::Auto);
        let a = fire(&mut engine, "error_rate_percent", 3.0);
        let p1 = gate.propose(&engine, &a, 0).unwrap().unwrap();
        assert_eq!(p1.status, RemediationStatus::Executed);
        let a = fire(&mut engine, "error_rate_percent", 3.0);
        let p2 = gate.propose(&engine, &a, 1).unwrap().unwrap();
        assert_eq!(p2.status, RemediationStatus::PendingApproval);
        assert_eq!(exec.calls().len(), 1);
    }

    #[test]
    fn require_approval_scale_reject_and_expire() {
        let (mut engine, mut gate, exec) = setup(GateMode::RequireApproval);
        let a = fire(&mut engine, "queue_depth", 500.0);
        let p = gate.propose(&engine, &a, 0).unwrap().unwrap();
        assert!(gate.approve(&p.id, "  ", 1).is_err(), "aprovador vazio");
        let r = gate.reject(&p.id, "bob", "falso positivo", 2).unwrap();
        assert_eq!(r.status, RemediationStatus::Rejected);
        assert!(gate.approve(&p.id, "bob", 3).is_err(), "não reabre");

        let a = fire(&mut engine, "queue_depth", 500.0);
        let p = gate.propose(&engine, &a, 100).unwrap().unwrap();
        assert!(gate.approve(&p.id, "bob", 100 + 61).is_err());
        assert_eq!(gate.proposals()[1].status, RemediationStatus::Expired);
        assert!(exec.calls().is_empty());
        let events: Vec<_> = gate.audit().iter().map(|e| e.event.as_str()).collect();
        assert_eq!(events, vec!["proposed", "rejected", "proposed", "expired"]);
    }

    #[test]
    fn soar_event_mapping() {
        let (mut engine, mut gate, _) = setup(GateMode::RequireApproval);
        let a = fire(&mut engine, "error_rate_percent", 3.0);
        gate.propose(&engine, &a, 0).unwrap();
        let ev = gate.drain_soar_events().pop().unwrap();
        assert_eq!(ev.source, "zylos-sentinel");
        assert_eq!(ev.severity, "critical");
        assert_eq!(ev.category, "anomaly");
        assert_eq!(ev.affected_assets, vec!["pagerduty".to_string()]);
        assert_eq!(ev.raw_payload["status"], "pending_approval");
        assert!(gate.drain_soar_events().is_empty());
    }
}
