//! Scheduler Rotinas — Task 5.2.3
//!
//! Scheduler de rotinas com suporte a cron, trigger e execução manual.
//! Execução sem agente ativo, logs detalhados e API para gerenciamento.
//! Inspirado em Open Duet routines.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Erros do sistema de scheduler
#[derive(Error, Debug)]
/// Enumeração SchedulerError.
pub enum SchedulerError {
    #[error("rotina '{0}' não encontrada")]
    // Variante RoutineNotFound.
    RoutineNotFound(String),

    #[error("expressão cron inválida: {0}")]
    // Variante InvalidCron.
    InvalidCron(String),

    #[error("falha ao persistir estado: {0}")]
    // Variante PersistenceError.
    PersistenceError(String),

    #[error("falha de I/O: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("JSON inválido: {0}")]
    // Variante ParseError.
    ParseError(#[from] serde_json::Error),

    #[error("rotina já existe: {0}")]
    // Variante DuplicateRoutine.
    DuplicateRoutine(String),

    #[error("execução falhou: {0}")]
    // Variante ExecutionFailed.
    ExecutionFailed(String),
}

/// Result type alias para operações do scheduler
pub type SchedulerResult<T> = Result<T, SchedulerError>;

/// Tipo de gatilho da rotina
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
/// Enumeração TriggerType.
pub enum TriggerType {
    // Expressão cron (ex: "0 */6 * * *")
    Cron {
        expression: String,
    },
    // Gatilho por evento (ex: "on_push", "on_merge")
    Event {
        event: String,
    },
    /// Execução manual sob demanda
    Manual,
    // Intervalo fixo em segundos
    Interval {
        seconds: u64,
    },
}

impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerType::Cron { expression } => write!(f, "cron({})", expression),
            TriggerType::Event { event } => write!(f, "event({})", event),
            TriggerType::Manual => write!(f, "manual"),
            TriggerType::Interval { seconds } => write!(f, "interval({}s)", seconds),
        }
    }
}

/// Status atual da rotina
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Enumeração RoutineStatus.
pub enum RoutineStatus {
    /// Variante Active.
    Active,
    /// Variante Paused.
    Paused,
    /// Variante Disabled.
    Disabled,
    /// Variante Failed.
    Failed,
}

impl std::fmt::Display for RoutineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutineStatus::Active => write!(f, "active"),
            RoutineStatus::Paused => write!(f, "paused"),
            RoutineStatus::Disabled => write!(f, "disabled"),
            RoutineStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Definição de uma rotina agendada
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura Routine.
pub struct Routine {
    /// ID único da rotina
    pub id: String,

    /// Nome descritivo
    pub name: String,

    /// Descrição do que a rotina faz
    pub description: String,

    /// Tipo de gatilho
    pub trigger: TriggerType,

    /// Comando ou skill a executar
    pub action: String,

    /// Argumentos para o comando/skill
    #[serde(default)]
    /// Campo args.
    pub args: Vec<String>,

    /// Status atual
    pub status: RoutineStatus,

    /// Timestamp da última execução (ISO 8601)
    pub last_run: Option<String>,

    /// Timestamp da próxima execução prevista (ISO 8601)
    pub next_run: Option<String>,

    /// Contador de execuções consecutivas com falha
    #[serde(default)]
    /// Campo consecutive failures.
    pub consecutive_failures: u32,

    /// Máximo de tentativas antes de desativar
    #[serde(default = "default_max_retries")]
    /// Campo max retries.
    pub max_retries: u32,

    /// Timeout em segundos
    #[serde(default = "default_timeout")]
    /// Campo timeout seconds.
    pub timeout_seconds: u64,

    /// Tags para organização
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,

    /// Criado em (ISO 8601)
    pub created_at: String,

    /// Atualizado em (ISO 8601)
    pub updated_at: String,
}

fn default_max_retries() -> u32 {
    3
}

fn default_timeout() -> u64 {
    300
}

/// Registro de execução de uma rotina
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura ExecutionLog.
pub struct ExecutionLog {
    /// ID da execução
    pub execution_id: String,

    /// ID da rotina executada
    pub routine_id: String,

    /// Timestamp de início (ISO 8601)
    pub started_at: String,

    /// Timestamp de término (ISO 8601)
    pub finished_at: Option<String>,

    /// Duração em milissegundos
    pub duration_ms: Option<f64>,

    /// Se a execução foi bem-sucedida
    pub success: bool,

    /// Output da execução
    pub output: String,

    /// Mensagem de erro (se houve)
    pub error: Option<String>,

    /// Quem disparou a execução ("scheduler", "manual", "event:<name>")
    pub triggered_by: String,
}

/// Estado persistente do scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura SchedulerState.
pub struct SchedulerState {
    /// Versão do formato de estado
    pub version: String,

    /// Rotinas registradas
    pub routines: HashMap<String, Routine>,

    /// Histórico de execuções (últimas N por rotina)
    pub execution_history: HashMap<String, Vec<ExecutionLog>>,

    /// Último save (ISO 8601)
    pub saved_at: String,
}

impl Default for SchedulerState {
    fn default() -> Self {
        // Variante Self.
        Self {
            version: "1.0.0".to_string(),
            routines: HashMap::new(),
            execution_history: HashMap::new(),
            saved_at: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Gerenciador de rotinas agendadas
#[derive(Debug)]
/// Estrutura RoutineScheduler.
pub struct RoutineScheduler {
    state: SchedulerState,
    state_path: Option<String>,
    max_history_per_routine: usize,
}

impl RoutineScheduler {
    /// Cria um scheduler vazio
    pub fn new() -> Self {
        // Variante Self.
        Self {
            state: SchedulerState::default(),
            state_path: None,
            max_history_per_routine: 50,
        }
    }

    /// Carrega o estado de um arquivo JSON
    pub fn load(path: &Path) -> SchedulerResult<Self> {
        let content = std::fs::read_to_string(path)?;
        let state: SchedulerState = serde_json::from_str(&content)?;

        info!(
            "Scheduler carregado: {} rotinas de '{}'",
            state.routines.len(),
            path.display()
        );

        // Variante Ok.
        Ok(Self {
            state,
            state_path: Some(path.display().to_string()),
            max_history_per_routine: 50,
        })
    }

    /// Salva o estado no arquivo configurado
    pub fn save(&mut self) -> SchedulerResult<()> {
        self.state.saved_at = chrono::Utc::now().to_rfc3339();

        if let Some(ref path) = self.state_path {
            let json = serde_json::to_string_pretty(&self.state)?;
            std::fs::write(path, json)?;
            debug!("Estado do scheduler salvo em '{}'", path);
        } else {
            warn!("Nenhum caminho de estado configurado; save ignorado");
        }

        // Variante Ok.
        Ok(())
    }

    /// Define o caminho de persistência
    pub fn set_state_path(&mut self, path: &str) {
        self.state_path = Some(path.to_string());
    }

    /// Registra uma nova rotina
    pub fn add_routine(&mut self, routine: Routine) -> SchedulerResult<()> {
        if self.state.routines.contains_key(&routine.id) {
            return Err(SchedulerError::DuplicateRoutine(routine.id.clone()));
        }

        // Valida expressão cron se aplicável
        if let TriggerType::Cron { ref expression } = routine.trigger {
            validate_cron_expression(expression)?;
        }

        info!(
            "Rotina registrada: '{}' ({})",
            routine.name, routine.trigger
        );
        self.state.routines.insert(routine.id.clone(), routine);
        // Variante Ok.
        Ok(())
    }

    /// Remove uma rotina
    pub fn remove_routine(&mut self, id: &str) -> SchedulerResult<Routine> {
        self.state
            .routines
            .remove(id)
            .ok_or_else(|| SchedulerError::RoutineNotFound(id.to_string()))
    }

    /// Obtém uma rotina por ID
    pub fn get_routine(&self, id: &str) -> Option<&Routine> {
        self.state.routines.get(id)
    }

    /// Lista todas as rotinas
    pub fn list_routines(&self) -> Vec<&Routine> {
        self.state.routines.values().collect()
    }

    /// Lista rotinas filtradas por status
    pub fn list_by_status(&self, status: &RoutineStatus) -> Vec<&Routine> {
        self.state
            .routines
            .values()
            .filter(|r| r.status == *status)
            .collect()
    }

    /// Lista rotinas filtradas por tag
    pub fn list_by_tag(&self, tag: &str) -> Vec<&Routine> {
        self.state
            .routines
            .values()
            .filter(|r| r.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Pausa uma rotina
    pub fn pause_routine(&mut self, id: &str) -> SchedulerResult<()> {
        let routine = self
            .state
            .routines
            .get_mut(id)
            .ok_or_else(|| SchedulerError::RoutineNotFound(id.to_string()))?;

        routine.status = RoutineStatus::Paused;
        routine.updated_at = chrono::Utc::now().to_rfc3339();
        info!("Rotina '{}' pausada", id);
        // Variante Ok.
        Ok(())
    }

    /// Reativa uma rotina
    pub fn resume_routine(&mut self, id: &str) -> SchedulerResult<()> {
        let routine = self
            .state
            .routines
            .get_mut(id)
            .ok_or_else(|| SchedulerError::RoutineNotFound(id.to_string()))?;

        routine.status = RoutineStatus::Active;
        routine.consecutive_failures = 0;
        routine.updated_at = chrono::Utc::now().to_rfc3339();
        info!("Rotina '{}' reativada", id);
        // Variante Ok.
        Ok(())
    }

    /// Registra o resultado de uma execução
    pub fn record_execution(&mut self, log: ExecutionLog) {
        let history = self
            .state
            .execution_history
            .entry(log.routine_id.clone())
            .or_default();

        // Atualiza estado da rotina
        if let Some(routine) = self.state.routines.get_mut(&log.routine_id) {
            routine.last_run = Some(log.started_at.clone());
            routine.updated_at = chrono::Utc::now().to_rfc3339();

            if log.success {
                routine.consecutive_failures = 0;
            } else {
                routine.consecutive_failures += 1;
                if routine.consecutive_failures >= routine.max_retries {
                    routine.status = RoutineStatus::Failed;
                    error!(
                        "Rotina '{}' desativada após {} falhas consecutivas",
                        log.routine_id, routine.consecutive_failures
                    );
                }
            }
        }

        history.push(log);

        // Limita histórico
        while history.len() > self.max_history_per_routine {
            history.remove(0);
        }
    }

    /// Obtém o histórico de execuções de uma rotina
    pub fn get_execution_history(&self, routine_id: &str) -> Option<&Vec<ExecutionLog>> {
        self.state.execution_history.get(routine_id)
    }

    /// Retorna rotinas que devem ser executadas agora (baseado em cron/interval)
    pub fn get_due_routines(&self, now: &str) -> Vec<&Routine> {
        self.state
            .routines
            .values()
            .filter(|r| {
                r.status == RoutineStatus::Active
                    && r.next_run
                        .as_ref()
                        .map(|nr| nr.as_str() <= now)
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Retorna estatísticas resumidas do scheduler
    pub fn stats(&self) -> SchedulerStats {
        let total = self.state.routines.len();
        let active = self
            .state
            .routines
            .values()
            .filter(|r| r.status == RoutineStatus::Active)
            .count();
        let paused = self
            .state
            .routines
            .values()
            .filter(|r| r.status == RoutineStatus::Paused)
            .count();
        let failed = self
            .state
            .routines
            .values()
            .filter(|r| r.status == RoutineStatus::Failed)
            .count();

        let total_executions: usize = self.state.execution_history.values().map(|h| h.len()).sum();

        let successful_executions: usize = self
            .state
            .execution_history
            .values()
            .flat_map(|h| h.iter())
            .filter(|l| l.success)
            .count();

        // Variante SchedulerStats.
        SchedulerStats {
            total_routines: total,
            active_routines: active,
            paused_routines: paused,
            failed_routines: failed,
            total_executions,
            successful_executions,
            success_rate: if total_executions > 0 {
                successful_executions as f64 / total_executions as f64
            } else {
                1.0
            },
        }
    }

    /// Serializa o estado completo para JSON
    pub fn to_json(&self) -> SchedulerResult<String> {
        serde_json::to_string_pretty(&self.state).map_err(SchedulerError::ParseError)
    }

    /// Gera relatório em Markdown
    pub fn report_markdown(&self) -> String {
        let stats = self.stats();
        let mut lines = vec![
            "# Relatório do Scheduler de Rotinas".to_string(),
            format!("Gerado em: {}", chrono::Utc::now().to_rfc3339()),
            String::new(),
            "## Estatísticas Gerais".to_string(),
            format!("- **Total de rotinas:** {}", stats.total_routines),
            format!("- **Ativas:** {}", stats.active_routines),
            format!("- **Pausadas:** {}", stats.paused_routines),
            format!("- **Falhadas:** {}", stats.failed_routines),
            format!("- **Total de execuções:** {}", stats.total_executions),
            format!("- **Taxa de sucesso:** {:.1}%", stats.success_rate * 100.0),
            String::new(),
            "## Rotinas Registradas".to_string(),
        ];

        let mut routines: Vec<_> = self.state.routines.values().collect();
        routines.sort_by(|a, b| a.name.cmp(&b.name));

        for routine in &routines {
            let last = routine.last_run.as_deref().unwrap_or("nunca");
            let next = routine.next_run.as_deref().unwrap_or("N/A");

            lines.push(format!("\n### {} (`{}`)", routine.name, routine.id));
            lines.push(format!("- **Status:** {}", routine.status));
            lines.push(format!("- **Gatilho:** {}", routine.trigger));
            lines.push(format!("- **Ação:** `{}`", routine.action));
            lines.push(format!("- **Última execução:** {}", last));
            lines.push(format!("- **Próxima execução:** {}", next));
            lines.push(format!(
                "- **Falhas consecutivas:** {}/{}",
                routine.consecutive_failures, routine.max_retries
            ));

            if !routine.tags.is_empty() {
                lines.push(format!(
                    "- **Tags:** {}",
                    routine
                        .tags
                        .iter()
                        .map(|t| format!("`{}`", t))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            // Últimas execuções
            if let Some(history) = self.state.execution_history.get(&routine.id) {
                let recent: Vec<_> = history.iter().rev().take(5).collect();
                if !recent.is_empty() {
                    lines.push("\n**Últimas execuções:**".to_string());
                    for log in &recent {
                        let status = if log.success { "✅" } else { "❌" };
                        let dur = log
                            .duration_ms
                            .map(|d| format!("{:.0}ms", d))
                            .unwrap_or_else(|| "?".to_string());
                        lines.push(format!(
                            "- {} {} ({}) — {}",
                            status, log.started_at, dur, log.triggered_by
                        ));
                    }
                }
            }
        }

        lines.join("\n")
    }
}

impl Default for RoutineScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Estatísticas resumidas do scheduler
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura SchedulerStats.
pub struct SchedulerStats {
    /// Campo total routines.
    pub total_routines: usize,
    /// Campo active routines.
    pub active_routines: usize,
    /// Campo paused routines.
    pub paused_routines: usize,
    /// Campo failed routines.
    pub failed_routines: usize,
    /// Campo total executions.
    pub total_executions: usize,
    /// Campo successful executions.
    pub successful_executions: usize,
    /// Campo success rate.
    pub success_rate: f64,
}

/// Validação básica de expressão cron (5 campos)
fn validate_cron_expression(expr: &str) -> SchedulerResult<()> {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(SchedulerError::InvalidCron(format!(
            "esperado 5 campos, encontrado {}: '{}'",
            fields.len(),
            expr
        )));
    }

    // Validação superficial: cada campo deve conter apenas caracteres válidos
    for (i, field) in fields.iter().enumerate() {
        for ch in field.chars() {
            if !ch.is_ascii_digit() && !matches!(ch, '*' | '/' | '-' | ',' | '?') {
                return Err(SchedulerError::InvalidCron(format!(
                    "caractere inválido '{}' no campo {} de '{}'",
                    ch, i, expr
                )));
            }
        }
    }

    // Variante Ok.
    Ok(())
}

/// Cria uma rotina de exemplo para testes
pub fn sample_routine(id: &str, name: &str) -> Routine {
    let now = chrono::Utc::now().to_rfc3339();
    // Variante Routine.
    Routine {
        id: id.to_string(),
        name: name.to_string(),
        description: format!("Rotina de teste: {}", name),
        trigger: TriggerType::Cron {
            expression: "0 */6 * * *".to_string(),
        },
        action: "hermes eval run".to_string(),
        args: vec!["--all".to_string()],
        status: RoutineStatus::Active,
        last_run: None,
        next_run: None,
        consecutive_failures: 0,
        max_retries: 3,
        timeout_seconds: 300,
        tags: vec!["test".to_string()],
        created_at: now.clone(),
        updated_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_routine() {
        let mut sched = RoutineScheduler::new();
        let routine = sample_routine("r-001", "Eval Diário");
        sched.add_routine(routine).unwrap();

        let got = sched.get_routine("r-001").unwrap();
        assert_eq!(got.name, "Eval Diário");
        assert_eq!(got.status, RoutineStatus::Active);
    }

    #[test]
    fn test_duplicate_routine_rejected() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "A")).unwrap();
        let result = sched.add_routine(sample_routine("r-001", "B"));
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_routine() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "X")).unwrap();
        let removed = sched.remove_routine("r-001").unwrap();
        assert_eq!(removed.id, "r-001");
        assert!(sched.get_routine("r-001").is_none());
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut sched = RoutineScheduler::new();
        assert!(sched.remove_routine("nope").is_err());
    }

    #[test]
    fn test_pause_and_resume() {
        let mut sched = RoutineScheduler::new();
        sched
            .add_routine(sample_routine("r-001", "Pausável"))
            .unwrap();

        sched.pause_routine("r-001").unwrap();
        assert_eq!(
            sched.get_routine("r-001").unwrap().status,
            RoutineStatus::Paused
        );

        sched.resume_routine("r-001").unwrap();
        assert_eq!(
            sched.get_routine("r-001").unwrap().status,
            RoutineStatus::Active
        );
    }

    #[test]
    fn test_list_by_status() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "A")).unwrap();
        sched.add_routine(sample_routine("r-002", "B")).unwrap();
        sched.pause_routine("r-002").unwrap();

        let active = sched.list_by_status(&RoutineStatus::Active);
        assert_eq!(active.len(), 1);

        let paused = sched.list_by_status(&RoutineStatus::Paused);
        assert_eq!(paused.len(), 1);
    }

    #[test]
    fn test_list_by_tag() {
        let mut sched = RoutineScheduler::new();
        let mut r1 = sample_routine("r-001", "Tagged");
        r1.tags = vec!["deploy".to_string(), "prod".to_string()];
        let mut r2 = sample_routine("r-002", "Other");
        r2.tags = vec!["dev".to_string()];

        sched.add_routine(r1).unwrap();
        sched.add_routine(r2).unwrap();

        let deploy = sched.list_by_tag("deploy");
        assert_eq!(deploy.len(), 1);
        assert_eq!(deploy[0].id, "r-001");
    }

    #[test]
    fn test_record_execution_success() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "Exec")).unwrap();

        let log = ExecutionLog {
            execution_id: "e-001".to_string(),
            routine_id: "r-001".to_string(),
            started_at: "2026-10-31T10:00:00Z".to_string(),
            finished_at: Some("2026-10-31T10:00:05Z".to_string()),
            duration_ms: Some(5000.0),
            success: true,
            output: "OK".to_string(),
            error: None,
            triggered_by: "scheduler".to_string(),
        };

        sched.record_execution(log);

        let history = sched.get_execution_history("r-001").unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].success);

        let routine = sched.get_routine("r-001").unwrap();
        assert_eq!(routine.consecutive_failures, 0);
        assert!(routine.last_run.is_some());
    }

    #[test]
    fn test_consecutive_failures_auto_disable() {
        let mut sched = RoutineScheduler::new();
        let mut routine = sample_routine("r-001", "Frágil");
        routine.max_retries = 2;
        sched.add_routine(routine).unwrap();

        for i in 0..2 {
            let log = ExecutionLog {
                execution_id: format!("e-{}", i),
                routine_id: "r-001".to_string(),
                started_at: format!("2026-10-31T10:0{}:00Z", i),
                finished_at: None,
                duration_ms: None,
                success: false,
                output: String::new(),
                error: Some("boom".to_string()),
                triggered_by: "scheduler".to_string(),
            };
            sched.record_execution(log);
        }

        let routine = sched.get_routine("r-001").unwrap();
        assert_eq!(routine.status, RoutineStatus::Failed);
        assert_eq!(routine.consecutive_failures, 2);
    }

    #[test]
    fn test_stats_empty() {
        let sched = RoutineScheduler::new();
        let stats = sched.stats();
        assert_eq!(stats.total_routines, 0);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_stats_with_data() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "S")).unwrap();

        let log = ExecutionLog {
            execution_id: "e-001".to_string(),
            routine_id: "r-001".to_string(),
            started_at: "2026-10-31T10:00:00Z".to_string(),
            finished_at: Some("2026-10-31T10:00:01Z".to_string()),
            duration_ms: Some(1000.0),
            success: true,
            output: "done".to_string(),
            error: None,
            triggered_by: "manual".to_string(),
        };
        sched.record_execution(log);

        let stats = sched.stats();
        assert_eq!(stats.total_routines, 1);
        assert_eq!(stats.active_routines, 1);
        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.successful_executions, 1);
        assert_eq!(stats.success_rate, 1.0);
    }

    #[test]
    fn test_validate_cron_valid() {
        assert!(validate_cron_expression("0 */6 * * *").is_ok());
        assert!(validate_cron_expression("30 2 * * 1").is_ok());
        assert!(validate_cron_expression("*/5 * * * *").is_ok());
    }

    #[test]
    fn test_validate_cron_invalid_fields() {
        assert!(validate_cron_expression("* * *").is_err());
        assert!(validate_cron_expression("* * * * * *").is_err());
    }

    #[test]
    fn test_validate_cron_invalid_chars() {
        assert!(validate_cron_expression("abc * * * *").is_err());
    }

    #[test]
    fn test_report_markdown() {
        let mut sched = RoutineScheduler::new();
        sched
            .add_routine(sample_routine("r-001", "Relatório"))
            .unwrap();

        let md = sched.report_markdown();
        assert!(md.contains("# Relatório do Scheduler"));
        assert!(md.contains("Relatório"));
        assert!(md.contains("r-001"));
    }

    #[test]
    fn test_to_json_roundtrip() {
        let mut sched = RoutineScheduler::new();
        sched.add_routine(sample_routine("r-001", "JSON")).unwrap();

        let json = sched.to_json().unwrap();
        let parsed: SchedulerState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.routines.len(), 1);
        assert_eq!(parsed.version, "1.0.0");
    }

    #[test]
    fn test_trigger_display() {
        assert_eq!(
            TriggerType::Cron {
                expression: "0 * * * *".to_string()
            }
            .to_string(),
            "cron(0 * * * *)"
        );
        assert_eq!(TriggerType::Manual.to_string(), "manual");
        assert_eq!(
            TriggerType::Interval { seconds: 3600 }.to_string(),
            "interval(3600s)"
        );
        assert_eq!(
            TriggerType::Event {
                event: "on_push".to_string()
            }
            .to_string(),
            "event(on_push)"
        );
    }

    #[test]
    fn test_history_limit() {
        let mut sched = RoutineScheduler::new();
        sched.max_history_per_routine = 5;
        sched
            .add_routine(sample_routine("r-001", "Limited"))
            .unwrap();

        for i in 0..10 {
            let log = ExecutionLog {
                execution_id: format!("e-{}", i),
                routine_id: "r-001".to_string(),
                started_at: format!("2026-10-31T10:{:02}:00Z", i),
                finished_at: None,
                duration_ms: None,
                success: true,
                output: String::new(),
                error: None,
                triggered_by: "scheduler".to_string(),
            };
            sched.record_execution(log);
        }

        let history = sched.get_execution_history("r-001").unwrap();
        assert_eq!(history.len(), 5);
        // Os mais antigos foram removidos; o primeiro restante é e-5
        assert_eq!(history[0].execution_id, "e-5");
    }
}
