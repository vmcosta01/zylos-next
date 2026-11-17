//! # GoalProgram — Programas orientados a objetivos para orquestração autônoma
//!
//! T8.1.2: Implementa a struct `GoalProgram` com campos formais para
//! goal/target/evaluation/strategy, persistência via serde e integração
//! com o P2PMessenger para dispatch assíncrono.
//!
//! ## Origem
//! Inspirado em githubnext/autoloop: programas definidos como
//! Goal + Target + Evaluation + Evolution Strategy.
//!
//! ## Persistência
//! Serializável via serde para storage em SQLite ou rules.json declarativo.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identificador único de um GoalProgram.
pub type ProgramId = String;

/// Especificação do alvo que o programa pode modificar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetSpec {
    /// Padrões glob de arquivos que o agente pode modificar.
    pub files: Vec<String>,
    /// Diretórios raiz permitidos (whitelist).
    pub allowed_dirs: Vec<String>,
    /// Arquivos/diretórios explicitamente excluídos.
    pub excluded: Vec<String>,
}

impl Default for TargetSpec {
    fn default() -> Self {
        Self {
            files: vec!["**/*.rs".into()],
            allowed_dirs: vec!["src/".into(), "crates/".into()],
            excluded: vec!["target/".into(), "*.lock".into()],
        }
    }
}

/// Função de avaliação que produz uma métrica numérica.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalSpec {
    /// Comando shell a executar (ex: "cargo test -- --test-threads=1").
    pub command: String,
    /// Nome da métrica extraída do output JSON.
    pub metric_key: String,
    /// Se true, métrica maior é melhor; se false, menor é melhor.
    pub higher_is_better: bool,
    /// Timeout máximo para execução da avaliação.
    #[serde(default = "default_eval_timeout")]
    pub timeout_secs: u64,
}

fn default_eval_timeout() -> u64 {
    300 // 5 minutos default
}

/// Estratégia de evolução para exploração do espaço de soluções.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvolutionStrategy {
    /// Tamanho da população de variantes mantidas.
    pub population_size: usize,
    /// Probabilidade de mutação vs refinamento (0.0-1.0).
    pub exploration_rate: f64,
    /// Dimensões de feature para tracking de diversidade.
    pub feature_dimensions: Vec<String>,
    /// Limiar de estagnação antes de mudar estratégia (iterações).
    pub plateau_threshold: usize,
}

impl Default for EvolutionStrategy {
    fn default() -> Self {
        Self {
            population_size: 5,
            exploration_rate: 0.3,
            feature_dimensions: vec!["algorithm_type".into(), "complexity".into()],
            plateau_threshold: 10,
        }
    }
}

/// Schedule de execução do programa.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgramSchedule {
    /// Executar a cada intervalo fixo.
    Interval { secs: u64 },
    /// Executar em horário específico (cron-like simplificado).
    Cron { expr: String },
    /// Executar apenas quando acionado manualmente.
    Manual,
}

impl Default for ProgramSchedule {
    fn default() -> Self {
        Self::Interval { secs: 21600 } // 6 horas default
    }
}

/// Estado atual de um GoalProgram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramState {
    /// Programa criado mas nunca executado.
    Created,
    /// Aguardando próximo ciclo de execução.
    Scheduled,
    /// Executando iteração atual.
    Running,
    /// Última iteração completada com sucesso.
    LastSuccess,
    /// Última iteração falhou.
    LastFailed,
    /// Métrica alvo atingida; programa concluído.
    Completed,
    /// Programa pausado pelo usuário.
    Paused,
}

impl std::fmt::Display for ProgramState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Running => write!(f, "running"),
            Self::LastSuccess => write!(f, "last_success"),
            Self::LastFailed => write!(f, "last_failed"),
            Self::Completed => write!(f, "completed"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

/// Um programa orientado a objetivos para orquestração autônoma.
///
/// Define WHAT (goal), WHERE (target), HOW TO MEASURE (evaluation)
/// e HOW TO EVOLVE (strategy) para execução cíclica por agentes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalProgram {
    /// Identificador único do programa.
    pub id: ProgramId,
    /// Nome legível do programa.
    pub name: String,
    /// Descrição do objetivo em linguagem natural.
    pub goal: String,
    /// Especificação dos arquivos/diretórios que podem ser modificados.
    pub target: TargetSpec,
    /// Especificação da função de avaliação.
    pub evaluation: EvalSpec,
    /// Estratégia de evolução para exploração.
    pub strategy: EvolutionStrategy,
    /// Schedule de execução.
    pub schedule: ProgramSchedule,
    /// Estado atual do programa.
    pub state: ProgramState,
    /// Melhor métrica alcançada até agora.
    pub best_metric: Option<f64>,
    /// Número total de iterações executadas.
    pub iteration_count: u64,
    /// Timestamp da última execução (ISO 8601).
    pub last_run_at: Option<String>,
    /// Timestamp de criação (ISO 8601).
    pub created_at: String,
    /// Metadata arbitrária para extensão.
    pub metadata: serde_json::Value,
}

impl GoalProgram {
    /// Cria um novo GoalProgram com defaults sensatos.
    pub fn new(
        name: impl Into<String>,
        goal: impl Into<String>,
        eval_command: impl Into<String>,
        metric_key: impl Into<String>,
        higher_is_better: bool,
    ) -> Self {
        let now = chrono_now_stub();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            goal: goal.into(),
            target: TargetSpec::default(),
            evaluation: EvalSpec {
                command: eval_command.into(),
                metric_key: metric_key.into(),
                higher_is_better,
                timeout_secs: default_eval_timeout(),
            },
            strategy: EvolutionStrategy::default(),
            schedule: ProgramSchedule::default(),
            state: ProgramState::Created,
            best_metric: None,
            iteration_count: 0,
            last_run_at: None,
            created_at: now,
            metadata: serde_json::Value::Null,
        }
    }

    /// Builder: define target spec customizado.
    pub fn with_target(mut self, target: TargetSpec) -> Self {
        self.target = target;
        self
    }

    /// Builder: define estratégia de evolução customizada.
    pub fn with_strategy(mut self, strategy: EvolutionStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Builder: define schedule customizado.
    pub fn with_schedule(mut self, schedule: ProgramSchedule) -> Self {
        self.schedule = schedule;
        self
    }

    /// Builder: anexa metadata arbitrária.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Verifica se uma nova métrica é melhor que a melhor conhecida.
    pub fn is_improvement(&self, new_metric: f64) -> bool {
        match self.best_metric {
            Some(best) => {
                if self.evaluation.higher_is_better {
                    new_metric > best
                } else {
                    new_metric < best
                }
            }
            None => true, // Primeira métrica é sempre melhoria
        }
    }

    /// Atualiza estado após iteração bem-sucedida.
    pub fn record_success(&mut self, metric: f64) {
        self.iteration_count += 1;
        self.last_run_at = Some(chrono_now_stub());
        if self.is_improvement(metric) {
            self.best_metric = Some(metric);
        }
        self.state = ProgramState::LastSuccess;
    }

    /// Atualiza estado após iteração falha.
    pub fn record_failure(&mut self) {
        self.iteration_count += 1;
        self.last_run_at = Some(chrono_now_stub());
        self.state = ProgramState::LastFailed;
    }

    /// Retorna o tópico P2P dedicado a este programa.
    pub fn p2p_topic(&self) -> String {
        format!("goal-program.{}", self.id)
    }

    /// Serializa para JSON (para persistência em SQLite ou rules.json).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Desserializa de JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Stub para timestamp ISO 8601. Em produção, usar chrono::Utc::now().
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
    fn test_goal_program_creation() {
        let prog = GoalProgram::new(
            "Test Coverage",
            "Increase test coverage across workspace",
            "cargo test -- --test-threads=1",
            "coverage_percent",
            true,
        );

        assert_eq!(prog.name, "Test Coverage");
        assert_eq!(prog.state, ProgramState::Created);
        assert_eq!(prog.iteration_count, 0);
        assert!(prog.best_metric.is_none());
        assert!(prog.evaluation.higher_is_better);
    }

    #[test]
    fn test_is_improvement_higher_is_better() {
        let mut prog = GoalProgram::new("T", "G", "cmd", "m", true);
        assert!(prog.is_improvement(50.0)); // First metric always improvement

        prog.best_metric = Some(50.0);
        assert!(prog.is_improvement(60.0));
        assert!(!prog.is_improvement(40.0));
        assert!(!prog.is_improvement(50.0)); // Equal is not improvement
    }

    #[test]
    fn test_is_improvement_lower_is_better() {
        let mut prog = GoalProgram::new("T", "G", "cmd", "m", false);
        prog.best_metric = Some(100.0);

        assert!(prog.is_improvement(90.0));
        assert!(!prog.is_improvement(110.0));
    }

    #[test]
    fn test_record_success() {
        let mut prog = GoalProgram::new("T", "G", "cmd", "m", true);
        prog.record_success(75.0);

        assert_eq!(prog.iteration_count, 1);
        assert_eq!(prog.best_metric, Some(75.0));
        assert_eq!(prog.state, ProgramState::LastSuccess);
        assert!(prog.last_run_at.is_some());

        // Second run with worse metric
        prog.record_success(60.0);
        assert_eq!(prog.iteration_count, 2);
        assert_eq!(prog.best_metric, Some(75.0)); // Best unchanged
    }

    #[test]
    fn test_record_failure() {
        let mut prog = GoalProgram::new("T", "G", "cmd", "m", true);
        prog.record_failure();

        assert_eq!(prog.iteration_count, 1);
        assert!(prog.best_metric.is_none());
        assert_eq!(prog.state, ProgramState::LastFailed);
    }

    #[test]
    fn test_p2p_topic() {
        let prog = GoalProgram::new("T", "G", "cmd", "m", true);
        assert!(prog.p2p_topic().starts_with("goal-program."));
        assert_eq!(prog.p2p_topic(), format!("goal-program.{}", prog.id));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let prog = GoalProgram::new(
            "Coverage",
            "Increase coverage",
            "cargo test",
            "coverage",
            true,
        )
        .with_strategy(EvolutionStrategy {
            population_size: 10,
            exploration_rate: 0.5,
            ..Default::default()
        });

        let json = prog.to_json().unwrap();
        let restored = GoalProgram::from_json(&json).unwrap();

        assert_eq!(restored.name, prog.name);
        assert_eq!(restored.goal, prog.goal);
        assert_eq!(restored.strategy.population_size, 10);
        assert_eq!(restored.evaluation.command, "cargo test");
    }

    #[test]
    fn test_builder_pattern() {
        let prog = GoalProgram::new("T", "G", "cmd", "m", true)
            .with_target(TargetSpec {
                files: vec!["**/*.py".into()],
                allowed_dirs: vec!["scripts/".into()],
                excluded: vec![],
            })
            .with_schedule(ProgramSchedule::Manual)
            .with_metadata(serde_json::json!({"owner": "team-a"}));

        assert_eq!(prog.target.files, vec!["**/*.py"]);
        assert_eq!(prog.schedule, ProgramSchedule::Manual);
        assert_eq!(prog.metadata["owner"], "team-a");
    }

    #[test]
    fn test_program_state_display() {
        assert_eq!(format!("{}", ProgramState::Created), "created");
        assert_eq!(format!("{}", ProgramState::Running), "running");
        assert_eq!(format!("{}", ProgramState::Completed), "completed");
    }

    #[test]
    fn test_default_values() {
        let target = TargetSpec::default();
        assert!(target.files.contains(&"**/*.rs".to_string()));

        let strategy = EvolutionStrategy::default();
        assert_eq!(strategy.population_size, 5);
        assert!((strategy.exploration_rate - 0.3).abs() < f64::EPSILON);

        let schedule = ProgramSchedule::default();
        assert_eq!(schedule, ProgramSchedule::Interval { secs: 21600 });
    }
}
