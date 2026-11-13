//! Eval Flywheel LLM-as-Judge — Task 5.2 (Expanded)
//!
//! Sistema de avaliação de skills com LLM como juiz, métricas de eficácia,
//! relatórios automáticos e API para consulta. Inspirado em google/agents-cli eval run.
//!
//! ## Expansões (Task 5.2)
//! 1. Benchmark harness integration (latency p50/p95/p99)
//! 2. Regression detection vs baseline (>10% threshold)
//! 3. Multi-model evaluation & aggregation
//! 4. SQLite backend for persistent eval history
//! 5. Auto-calibration of thresholds from historical pass rates
//! 6. Report export: Markdown + JSON + CSV with trend data
//! 7. Goal-Contract acceptance criteria integration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use thiserror::Error;
use tracing::{debug, info, warn};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Erros do sistema de eval
#[derive(Error, Debug)]
/// Enumeração EvalError.
pub enum EvalError {
    #[error("configuração de eval não encontrada: {0}")]
    // Variante ConfigNotFound.
    ConfigNotFound(String),

    #[error("falha ao ler configuração: {0}")]
    // Variante IoError.
    IoError(#[from] std::io::Error),

    #[error("JSON inválido: {0}")]
    // Variante ParseError.
    ParseError(#[from] serde_json::Error),

    #[error("skill '{0}' não encontrada para avaliação")]
    // Variante SkillNotFound.
    SkillNotFound(String),

    #[error("provedor LLM '{0}' não suportado")]
    // Variante UnsupportedProvider.
    UnsupportedProvider(String),

    #[error("avaliação falhou: {0}")]
    // Variante EvaluationFailed.
    EvaluationFailed(String),

    #[error("timeout após {0}s")]
    // Variante Timeout.
    Timeout(u64),

    #[error("SQLite error: {0}")]
    // Variante SqliteError.
    SqliteError(#[from] rusqlite::Error),

    #[error("regression detected: {skill} score dropped {delta:.1}% (baseline {baseline:.3} -> current {current:.3})")]
    /// Variante RegressionDetected.
    RegressionDetected {
        skill: String,
        baseline: f64,
        current: f64,
        delta: f64,
    },

    #[error("no baseline found for comparison")]
    /// Variante NoBaseline.
    NoBaseline,
}

/// Result type alias para operações de eval
pub type EvalResult<T> = Result<T, EvalError>;

// ---------------------------------------------------------------------------
// Core enums & structs (preserved from original)
// ---------------------------------------------------------------------------

/// Provedores LLM suportados para julgamento
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
/// Enumeração LlmProvider.
pub enum LlmProvider {
    /// Variante Gemini.
    Gemini,
    /// Variante Claude.
    Claude,
    /// Variante OpenAI.
    OpenAI,
    /// Variante Local.
    Local,
}

impl std::fmt::Display for LlmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LlmProvider::Gemini => write!(f, "gemini"),
            LlmProvider::Claude => write!(f, "claude"),
            LlmProvider::OpenAI => write!(f, "openai"),
            LlmProvider::Local => write!(f, "local"),
        }
    }
}

impl std::str::FromStr for LlmProvider {
    type Err = EvalError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gemini" => Ok(LlmProvider::Gemini),
            "claude" => Ok(LlmProvider::Claude),
            "openai" => Ok(LlmProvider::OpenAI),
            "local" => Ok(LlmProvider::Local),
            _ => Err(EvalError::UnsupportedProvider(s.to_string())),
        }
    }
}

/// Critério de avaliação individual
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalCriterion.
pub struct EvalCriterion {
    /// Campo name.
    pub name: String,
    /// Campo description.
    pub description: String,
    /// Campo weight.
    pub weight: f64,
    /// Campo judge prompt.
    pub judge_prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Campo goal contract criterion.
    pub goal_contract_criterion: Option<GoalContractCriterionRef>,
}

/// Reference to a Goal-Contract acceptance criterion (expansion #7)
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura GoalContractCriterionRef.
pub struct GoalContractCriterionRef {
    /// Campo contract id.
    pub contract_id: String,
    /// Campo criterion index.
    pub criterion_index: usize,
    #[serde(default)]
    /// Campo label.
    pub label: String,
}

/// Configuração de um caso de teste
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalTestCase.
pub struct EvalTestCase {
    /// Campo id.
    pub id: String,
    /// Campo skill name.
    pub skill_name: String,
    /// Campo input.
    pub input: String,
    #[serde(default)]
    /// Campo expected output.
    pub expected_output: Option<String>,
    #[serde(default)]
    /// Campo context.
    pub context: HashMap<String, String>,
    #[serde(default)]
    /// Campo criteria override.
    pub criteria_override: Vec<EvalCriterion>,
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,
}

/// Resultado de uma avaliação individual por critério
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura CriterionScore.
pub struct CriterionScore {
    /// Campo criterion.
    pub criterion: String,
    /// Campo score.
    pub score: f64,
    /// Campo reasoning.
    pub reasoning: String,
    /// Campo duration ms.
    pub duration_ms: f64,
}

/// Resultado completo de uma avaliação
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalReport.
pub struct EvalReport {
    #[serde(default = "generate_run_id")]
    /// Campo run id.
    pub run_id: String,
    /// Campo test case id.
    pub test_case_id: String,
    /// Campo skill name.
    pub skill_name: String,
    /// Campo judge provider.
    pub judge_provider: LlmProvider,
    /// Campo criteria scores.
    pub criteria_scores: Vec<CriterionScore>,
    /// Campo overall score.
    pub overall_score: f64,
    /// Campo verdict.
    pub verdict: String,
    /// Campo evaluated at.
    pub evaluated_at: String,
    /// Campo total duration ms.
    pub total_duration_ms: f64,
    #[serde(default)]
    /// Campo notes.
    pub notes: Vec<String>,
    #[serde(default)]
    /// Campo tags.
    pub tags: Vec<String>,
    #[serde(default)]
    /// Campo goal contract refs.
    pub goal_contract_refs: Vec<GoalContractCriterionRef>,
}

fn generate_run_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Configuração global do eval flywheel
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura EvalConfig.
pub struct EvalConfig {
    /// Campo version.
    pub version: String,
    /// Campo default provider.
    pub default_provider: LlmProvider,
    #[serde(default = "default_timeout")]
    /// Campo timeout seconds.
    pub timeout_seconds: u64,
    #[serde(default = "default_pass_threshold")]
    /// Campo pass threshold.
    pub pass_threshold: f64,
    #[serde(default = "default_review_threshold")]
    /// Campo review threshold.
    pub review_threshold: f64,
    /// Campo criteria.
    pub criteria: Vec<EvalCriterion>,
    /// Campo test cases.
    pub test_cases: Vec<EvalTestCase>,
    #[serde(default = "default_regression_threshold")]
    /// Campo regression threshold pct.
    pub regression_threshold_pct: f64,
    #[serde(default)]
    /// Campo auto calibrate.
    pub auto_calibrate: bool,
    #[serde(default = "default_calibration_min_runs")]
    /// Campo calibration min runs.
    pub calibration_min_runs: usize,
}

fn default_timeout() -> u64 { 30 }
fn default_pass_threshold() -> f64 { 0.7 }
fn default_review_threshold() -> f64 { 0.4 }
fn default_regression_threshold() -> f64 { 10.0 }
fn default_calibration_min_runs() -> usize { 20 }

// ---------------------------------------------------------------------------
// Expansion #1: Benchmark Harness Integration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Estrutura BenchmarkLatency.
pub struct BenchmarkLatency {
    /// Campo p50 ms.
    pub p50_ms: f64,
    /// Campo p95 ms.
    pub p95_ms: f64,
    /// Campo p99 ms.
    pub p99_ms: f64,
    /// Campo sample count.
    pub sample_count: usize,
}

impl BenchmarkLatency {
    /// Função from samples.
    pub fn from_samples(mut samples: Vec<f64>) -> Self {
        if samples.is_empty() { return Self::default(); }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = samples.len();
        let percentile = |p: f64| -> f64 {
            let idx = ((p / 100.0) * (n as f64 - 1.0)).round() as usize;
            samples[idx.min(n - 1)]
        };
        // Variante Self.
        Self {
            p50_ms: percentile(50.0),
            p95_ms: percentile(95.0),
            p99_ms: percentile(99.0),
            sample_count: n,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura BenchmarkResult.
pub struct BenchmarkResult {
    /// Campo skill name.
    pub skill_name: String,
    /// Campo latency.
    pub latency: BenchmarkLatency,
    /// Campo measured at.
    pub measured_at: String,
}

// ---------------------------------------------------------------------------
// Expansion #2: Regression Detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura RegressionReport.
pub struct RegressionReport {
    /// Campo skill name.
    pub skill_name: String,
    /// Campo baseline score.
    pub baseline_score: f64,
    /// Campo current score.
    pub current_score: f64,
    /// Campo delta pct.
    pub delta_pct: f64,
    /// Campo is regression.
    pub is_regression: bool,
    /// Campo threshold pct.
    pub threshold_pct: f64,
    /// Campo criterion deltas.
    pub criterion_deltas: Vec<CriterionDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura CriterionDelta.
pub struct CriterionDelta {
    /// Campo criterion.
    pub criterion: String,
    /// Campo baseline.
    pub baseline: f64,
    /// Campo current.
    pub current: f64,
    /// Campo delta.
    pub delta: f64,
}

// ---------------------------------------------------------------------------
// Expansion #3: Multi-Model Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura MultiModelResult.
pub struct MultiModelResult {
    /// Campo test case id.
    pub test_case_id: String,
    /// Campo skill name.
    pub skill_name: String,
    /// Campo per model.
    pub per_model: HashMap<String, f64>,
    /// Campo aggregate score.
    pub aggregate_score: f64,
    /// Campo model specific failures.
    pub model_specific_failures: Vec<String>,
}

// ---------------------------------------------------------------------------
// Expansion #5: Auto-Calibration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura CalibrationSuggestion.
pub struct CalibrationSuggestion {
    /// Campo suggested pass threshold.
    pub suggested_pass_threshold: f64,
    /// Campo suggested review threshold.
    pub suggested_review_threshold: f64,
    /// Campo historical pass rate.
    pub historical_pass_rate: f64,
    /// Campo sample size.
    pub sample_size: usize,
    /// Campo rationale.
    pub rationale: String,
}

// ---------------------------------------------------------------------------
// Expansion #6: Export Formats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Enumeração EvalExportFormat.
pub enum EvalExportFormat {
    /// Variante Markdown.
    Markdown,
    /// Variante Json.
    Json,
    /// Variante Csv.
    Csv,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura TrendDataPoint.
pub struct TrendDataPoint {
    /// Campo date.
    pub date: String,
    /// Campo avg score.
    pub avg_score: f64,
    /// Campo pass rate.
    pub pass_rate: f64,
    /// Campo eval count.
    pub eval_count: usize,
}

// ---------------------------------------------------------------------------
// Metrics (preserved + expanded)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
/// Estrutura EvalMetrics.
pub struct EvalMetrics {
    /// Campo total evaluations.
    pub total_evaluations: usize,
    /// Campo pass rate.
    pub pass_rate: f64,
    /// Campo avg score.
    pub avg_score: f64,
    /// Campo top skills.
    pub top_skills: Vec<(String, f64)>,
    /// Campo bottom skills.
    pub bottom_skills: Vec<(String, f64)>,
    /// Campo criterion distribution.
    pub criterion_distribution: HashMap<String, Vec<f64>>,
    #[serde(default)]
    /// Campo benchmark latencies.
    pub benchmark_latencies: HashMap<String, BenchmarkLatency>,
    #[serde(default)]
    /// Campo trend.
    pub trend: Vec<TrendDataPoint>,
}

// ---------------------------------------------------------------------------
// Expansion #4: SQLite Storage Backend
// ---------------------------------------------------------------------------

#[derive(Debug)]
/// Estrutura EvalStore.
pub struct EvalStore {
    conn: Mutex<rusqlite::Connection>,
}

impl EvalStore {
    /// Função open.
    pub fn open(path: &Path) -> EvalResult<Self> {
        let conn = Mutex::new(rusqlite::Connection::open(path)?);
        let store = Self { conn };
        store.init_schema()?;
        // Variante Ok.
        Ok(store)
    }

    /// Função in memory.
    pub fn in_memory() -> EvalResult<Self> {
        let conn = Mutex::new(rusqlite::Connection::open_in_memory()?);
        let store = Self { conn };
        store.init_schema()?;
        // Variante Ok.
        Ok(store)
    }

    fn init_schema(&self) -> EvalResult<()> {
        let __conn = self.conn.lock().unwrap();
        __conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS eval_runs (
                run_id TEXT PRIMARY KEY,
                test_case_id TEXT NOT NULL,
                skill_name TEXT NOT NULL,
                judge_provider TEXT NOT NULL,
                overall_score REAL NOT NULL,
                verdict TEXT NOT NULL,
                evaluated_at TEXT NOT NULL,
                total_duration_ms REAL NOT NULL,
                report_json TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_eval_runs_skill ON eval_runs(skill_name);
            CREATE INDEX IF NOT EXISTS idx_eval_runs_date ON eval_runs(evaluated_at);
            CREATE INDEX IF NOT EXISTS idx_eval_runs_provider ON eval_runs(judge_provider);
            CREATE TABLE IF NOT EXISTS benchmark_data (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                skill_name TEXT NOT NULL,
                latency_json TEXT NOT NULL,
                measured_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_bench_skill ON benchmark_data(skill_name);
            ",
        )?;
        // Variante Ok.
        Ok(())
    }

    /// Função save report.
    pub fn save_report(&self, report: &EvalReport) -> EvalResult<()> {
        let report_json = serde_json::to_string(report)?;
        let tags = report.tags.join(",");
        let __conn = self.conn.lock().unwrap();
        __conn.execute(
            "INSERT OR REPLACE INTO eval_runs
                            (run_id, test_case_id, skill_name, judge_provider, overall_score,
                             verdict, evaluated_at, total_duration_ms, report_json, tags)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                report.run_id, report.test_case_id, report.skill_name,
                report.judge_provider.to_string(), report.overall_score,
                report.verdict, report.evaluated_at, report.total_duration_ms,
                report_json, tags,
            ],
        )?;
        // Variante Ok.
        Ok(())
    }

    /// Função query by skill.
    pub fn query_by_skill(&self, skill_name: &str) -> EvalResult<Vec<EvalReport>> {
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT report_json FROM eval_runs WHERE skill_name = ?1 ORDER BY evaluated_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![skill_name], |row| row.get::<_, String>(0))?;
        let mut reports = Vec::new();
        for row in rows {
            let json = row?;
            reports.push(serde_json::from_str(&json)?);
        }
        // Variante Ok.
        Ok(reports)
    }

    /// Função query by date range.
    pub fn query_by_date_range(&self, from: &str, to: &str) -> EvalResult<Vec<EvalReport>> {
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT report_json FROM eval_runs WHERE evaluated_at >= ?1 AND evaluated_at <= ?2 ORDER BY evaluated_at",
        )?;
        let rows = stmt.query_map(rusqlite::params![from, to], |row| row.get::<_, String>(0))?;
        let mut reports = Vec::new();
        for row in rows {
            let json = row?;
            reports.push(serde_json::from_str(&json)?);
        }
        // Variante Ok.
        Ok(reports)
    }

    /// Função query by tag.
    pub fn query_by_tag(&self, tag: &str) -> EvalResult<Vec<EvalReport>> {
        let pattern = format!("%{}%", tag);
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT report_json FROM eval_runs WHERE tags LIKE ?1 ORDER BY evaluated_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![pattern], |row| row.get::<_, String>(0))?;
        let mut reports = Vec::new();
        for row in rows {
            let json = row?;
            reports.push(serde_json::from_str(&json)?);
        }
        // Variante Ok.
        Ok(reports)
    }

    /// Função query by provider.
    pub fn query_by_provider(&self, provider: &LlmProvider) -> EvalResult<Vec<EvalReport>> {
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT report_json FROM eval_runs WHERE judge_provider = ?1 ORDER BY evaluated_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![provider.to_string()], |row| row.get::<_, String>(0))?;
        let mut reports = Vec::new();
        for row in rows {
            let json = row?;
            reports.push(serde_json::from_str(&json)?);
        }
        // Variante Ok.
        Ok(reports)
    }

    /// Função latest report for skill.
    pub fn latest_report_for_skill(&self, skill_name: &str) -> EvalResult<Option<EvalReport>> {
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT report_json FROM eval_runs WHERE skill_name = ?1 ORDER BY evaluated_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(rusqlite::params![skill_name])?;
        if let Some(row) = rows.next()? {
            let json: String = row.get(0)?;
            // Variante Ok.
            Ok(Some(serde_json::from_str(&json)?))
        } else {
            // Variante Ok.
            Ok(None)
        }
    }

    /// Função count.
    pub fn count(&self) -> EvalResult<usize> {
        let __conn = self.conn.lock().unwrap();
        let c: usize = __conn.query_row("SELECT COUNT(*) FROM eval_runs", [], |row| row.get(0))?;
        // Variante Ok.
        Ok(c)
    }

    /// Função save benchmark.
    pub fn save_benchmark(&self, result: &BenchmarkResult) -> EvalResult<()> {
        let latency_json = serde_json::to_string(&result.latency)?;
        let __conn = self.conn.lock().unwrap();
        __conn.execute(
            "INSERT INTO benchmark_data (skill_name, latency_json, measured_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![result.skill_name, latency_json, result.measured_at],
        )?;
        // Variante Ok.
        Ok(())
    }

    /// Função latest benchmark.
    pub fn latest_benchmark(&self, skill_name: &str) -> EvalResult<Option<BenchmarkResult>> {
        let __conn = self.conn.lock().unwrap();
        let mut stmt = __conn.prepare(
            "SELECT latency_json, measured_at FROM benchmark_data WHERE skill_name = ?1 ORDER BY measured_at DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(rusqlite::params![skill_name])?;
        if let Some(row) = rows.next()? {
            let lj: String = row.get(0)?;
            let ma: String = row.get(1)?;
            // Variante Ok.
            Ok(Some(BenchmarkResult { skill_name: skill_name.to_string(), latency: serde_json::from_str(&lj)?, measured_at: ma }))
        } else {
            // Variante Ok.
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// EvalFlywheel (expanded)
// ---------------------------------------------------------------------------

#[derive(Debug)]
/// Estrutura EvalFlywheel.
pub struct EvalFlywheel {
    config: EvalConfig,
    cache: HashMap<String, EvalReport>,
    metrics: EvalMetrics,
    store: Option<EvalStore>,
}

impl EvalFlywheel {
    /// Função load.
    pub fn load(config_path: &Path) -> EvalResult<Self> {
        if !config_path.exists() {
            return Err(EvalError::ConfigNotFound(config_path.display().to_string()));
        }
        let content = std::fs::read_to_string(config_path)?;
        let config: EvalConfig = serde_json::from_str(&content)?;
        info!("EvalFlywheel carregado: {} critérios, {} casos de teste", config.criteria.len(), config.test_cases.len());
        // Variante Ok.
        Ok(Self { config, cache: HashMap::new(), metrics: EvalMetrics::default(), store: None })
    }

    /// Função with config.
    pub fn with_config(config: EvalConfig) -> Self {
        // Variante Self.
        Self { config, cache: HashMap::new(), metrics: EvalMetrics::default(), store: None }
    }

    /// Função empty.
    pub fn empty() -> Self {
        // Variante Self.
        Self {
            config: EvalConfig {
                version: "1.0.0".to_string(),
                default_provider: LlmProvider::Gemini,
                timeout_seconds: 30,
                pass_threshold: 0.7,
                review_threshold: 0.4,
                criteria: default_criteria(),
                test_cases: vec![],
                regression_threshold_pct: 10.0,
                auto_calibrate: false,
                calibration_min_runs: 20,
            },
            cache: HashMap::new(),
            metrics: EvalMetrics::default(),
            store: None,
        }
    }

    /// Função with store.
    pub fn with_store(mut self, store: EvalStore) -> Self {
        self.store = Some(store);
        self
    }

    /// Função add test case.
    pub fn add_test_case(&mut self, case: EvalTestCase) { self.config.test_cases.push(case); }
    /// Função criteria.
    pub fn criteria(&self) -> &[EvalCriterion] { &self.config.criteria }
    /// Função test cases.
    pub fn test_cases(&self) -> &[EvalTestCase] { &self.config.test_cases }

    /// Função evaluate mock.
    pub fn evaluate_mock(&mut self, test_case_id: &str, scores: Vec<CriterionScore>) -> EvalResult<EvalReport> {
        let test_case = self.config.test_cases.iter().find(|tc| tc.id == test_case_id)
            .ok_or_else(|| EvalError::SkillNotFound(test_case_id.to_string()))?;

        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;
        for cs in &scores {
            if let Some(crit) = self.config.criteria.iter().find(|c| c.name == cs.criterion) {
                weighted_sum += cs.score * crit.weight;
                total_weight += crit.weight;
            }
        }
        let overall_score = if total_weight > 0.0 { weighted_sum / total_weight } else { 0.0 };

        let verdict = if overall_score >= self.config.pass_threshold {
            "pass".to_string()
        } else if overall_score >= self.config.review_threshold {
            "needs_review".to_string()
        } else {
            "fail".to_string()
        };

        let goal_contract_refs: Vec<GoalContractCriterionRef> = test_case.criteria_override.iter()
            .filter_map(|c| c.goal_contract_criterion.clone())
            .chain(self.config.criteria.iter().filter_map(|c| c.goal_contract_criterion.clone()))
            .collect();

        let report = EvalReport {
            run_id: generate_run_id(),
            test_case_id: test_case_id.to_string(),
            skill_name: test_case.skill_name.clone(),
            judge_provider: self.config.default_provider.clone(),
            criteria_scores: scores,
            overall_score,
            verdict: verdict.clone(),
            evaluated_at: chrono::Utc::now().to_rfc3339(),
            total_duration_ms: 0.0,
            notes: vec![],
            tags: test_case.tags.clone(),
            goal_contract_refs,
        };

        if let Some(ref store) = self.store {
                    if let Err(e) = store.save_report(&report) { warn!("Failed to persist eval report: {}", e); }
                            }

        self.cache.insert(test_case_id.to_string(), report.clone());
        self.update_metrics();
        debug!("Eval '{}' -> {:.2} ({})", test_case_id, overall_score, verdict);
        // Variante Ok.
        Ok(report)
    }

    /// Função generate judge prompt.
    pub fn generate_judge_prompt(&self, test_case: &EvalTestCase) -> String {
        let criteria_text: Vec<String> = self.config.criteria.iter().map(|c| {
            let gc_ref = c.goal_contract_criterion.as_ref()
                .map(|r| format!(" [GC:{}#{}]", r.contract_id, r.criterion_index)).unwrap_or_default();
            format!("- **{}**{} (peso: {:.1}): {}", c.name, gc_ref, c.weight, c.description)
        }).collect();

        format!(
            r#"Você é um juiz especializado em avaliar a qualidade de skills de IA.

## Skill sendo avaliada
- Nome: {}
- Input: {}
- Output esperado: {}

## Critérios de Avaliação
{}

## Instruções
Para cada critério, atribua um score de 0.0 a 1.0 e justifique sua decisão.
Responda em formato JSON com a estrutura:
{{
  "scores": [
    {{"criterion": "nome", "score": 0.85, "reasoning": "justificativa"}}
  ]
}}

Seja rigoroso mas justo. Penalize alucinações, incompletude e instruções ambíguas."#,
            test_case.skill_name, test_case.input,
            test_case.expected_output.as_deref().unwrap_or("(não especificado)"),
            criteria_text.join("\n")
        )
    }

    /// Função get report.
    pub fn get_report(&self, test_case_id: &str) -> Option<&EvalReport> { self.cache.get(test_case_id) }
    /// Função list reports.
    pub fn list_reports(&self) -> Vec<&EvalReport> { self.cache.values().collect() }
    /// Função metrics.
    pub fn metrics(&self) -> &EvalMetrics { &self.metrics }

    // Expansion #1
    /// Função record benchmark.
    pub fn record_benchmark(&mut self, skill_name: &str, samples: Vec<f64>) -> BenchmarkResult {
        let latency = BenchmarkLatency::from_samples(samples);
        let result = BenchmarkResult { skill_name: skill_name.to_string(), latency: latency.clone(), measured_at: chrono::Utc::now().to_rfc3339() };
        self.metrics.benchmark_latencies.insert(skill_name.to_string(), latency);
        if let Some(ref store) = self.store {
            if let Err(e) = store.save_benchmark(&result) { warn!("Failed to persist benchmark: {}", e); }
        }
        result
    }

    /// Função get benchmark.
    pub fn get_benchmark(&self, skill_name: &str) -> Option<&BenchmarkLatency> {
        self.metrics.benchmark_latencies.get(skill_name)
    }

    // Expansion #2
    /// Função detect regression.
    pub fn detect_regression(&self, current: &EvalReport, baseline: &EvalReport) -> RegressionReport {
        let delta_pct = if baseline.overall_score > 0.0 {
            ((baseline.overall_score - current.overall_score) / baseline.overall_score) * 100.0
        } else { 0.0 };
        let is_regression = delta_pct > self.config.regression_threshold_pct;
        let mut criterion_deltas = Vec::new();
        for cs in &current.criteria_scores {
            let bs = baseline.criteria_scores.iter().find(|b| b.criterion == cs.criterion).map(|b| b.score).unwrap_or(0.0);
            criterion_deltas.push(CriterionDelta { criterion: cs.criterion.clone(), baseline: bs, current: cs.score, delta: cs.score - bs });
        }
        // Variante RegressionReport.
        RegressionReport { skill_name: current.skill_name.clone(), baseline_score: baseline.overall_score, current_score: current.overall_score, delta_pct, is_regression, threshold_pct: self.config.regression_threshold_pct, criterion_deltas }
    }

    /// Função detect regression vs stored.
    pub fn detect_regression_vs_stored(&self, current: &EvalReport) -> EvalResult<RegressionReport> {
        let baseline = self.store.as_ref().ok_or(EvalError::NoBaseline)?
            .latest_report_for_skill(&current.skill_name)?.ok_or(EvalError::NoBaseline)?;
        // Variante Ok.
        Ok(self.detect_regression(current, &baseline))
    }

    // Expansion #3
    /// Função evaluate multi model.
    pub fn evaluate_multi_model<F>(&mut self, test_case_id: &str, providers: &[LlmProvider], mut mock_scores_fn: F) -> EvalResult<MultiModelResult>
    where F: FnMut(&LlmProvider) -> Vec<CriterionScore> {
        let test_case = self.config.test_cases.iter().find(|tc| tc.id == test_case_id)
            .ok_or_else(|| EvalError::SkillNotFound(test_case_id.to_string()))?;
        let mut per_model: HashMap<String, f64> = HashMap::new();
        let mut model_specific_failures = Vec::new();
        for provider in providers {
            let scores = mock_scores_fn(provider);
            let mut ws = 0.0; let mut tw = 0.0;
            for cs in &scores {
                if let Some(crit) = self.config.criteria.iter().find(|c| c.name == cs.criterion) {
                    ws += cs.score * crit.weight; tw += crit.weight;
                }
            }
            let score = if tw > 0.0 { ws / tw } else { 0.0 };
            per_model.insert(provider.to_string(), score);
            if score < self.config.review_threshold { model_specific_failures.push(provider.to_string()); }
        }
        let aggregate_score = if per_model.is_empty() { 0.0 } else { per_model.values().sum::<f64>() / per_model.len() as f64 };
        // Variante Ok.
        Ok(MultiModelResult { test_case_id: test_case_id.to_string(), skill_name: test_case.skill_name.clone(), per_model, aggregate_score, model_specific_failures })
    }

    // Expansion #5
    /// Função suggest calibration.
    pub fn suggest_calibration(&self) -> EvalResult<CalibrationSuggestion> {
        let reports: Vec<EvalReport> = self.cache.values().cloned().collect();
        let sample_size = reports.len();
        if sample_size < self.config.calibration_min_runs {
            return Ok(CalibrationSuggestion {
                suggested_pass_threshold: self.config.pass_threshold,
                suggested_review_threshold: self.config.review_threshold,
                historical_pass_rate: if sample_size > 0 { reports.iter().filter(|r| r.verdict == "pass").count() as f64 / sample_size as f64 } else { 0.0 },
                sample_size,
                rationale: format!("Insufficient data ({} runs, need {}). Keeping current thresholds.", sample_size, self.config.calibration_min_runs),
            });
        }
        let scores: Vec<f64> = reports.iter().map(|r| r.overall_score).collect();
        let passes = reports.iter().filter(|r| r.verdict == "pass").count();
        let pass_rate = passes as f64 / sample_size as f64;
        let mut sorted = scores.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let fail_pct = 1.0 - 0.70;
        let pass_idx = ((fail_pct) * (sorted.len() as f64 - 1.0)).round() as usize;
        let suggested_pass = sorted[pass_idx.min(sorted.len() - 1)];
        let review_idx = ((0.15) * (sorted.len() as f64 - 1.0)).round() as usize;
        let suggested_review = sorted[review_idx.min(sorted.len() - 1)];
        // Variante Ok.
        Ok(CalibrationSuggestion {
            suggested_pass_threshold: (suggested_pass * 100.0).round() / 100.0,
            suggested_review_threshold: (suggested_review * 100.0).round() / 100.0,
            historical_pass_rate: pass_rate, sample_size,
            rationale: format!("Based on {} runs, targeting 70% pass rate. Current pass rate: {:.1}%.", sample_size, pass_rate * 100.0),
        })
    }

    /// Função auto calibrate.
    pub fn auto_calibrate(&mut self) -> EvalResult<Option<CalibrationSuggestion>> {
        if !self.config.auto_calibrate { return Ok(None); }
        let suggestion = self.suggest_calibration()?;
        if suggestion.sample_size >= self.config.calibration_min_runs {
            self.config.pass_threshold = suggestion.suggested_pass_threshold;
            self.config.review_threshold = suggestion.suggested_review_threshold;
            info!("Auto-calibrated: pass={:.2}, review={:.2} ({})", self.config.pass_threshold, self.config.review_threshold, suggestion.rationale);
            // Variante Ok.
            Ok(Some(suggestion))
        } else { Ok(None) }
    }

    // Expansion #6
    /// Função export reports.
    pub fn export_reports(&self, format: EvalExportFormat) -> EvalResult<String> {
        match format {
            EvalExportFormat::Markdown => Ok(self.generate_markdown_report()),
            EvalExportFormat::Json => self.export_json(),
            EvalExportFormat::Csv => self.export_csv(),
        }
    }

    fn export_json(&self) -> EvalResult<String> {
        #[derive(Serialize)]
        struct FullExport<'a> { metrics: &'a EvalMetrics, reports: Vec<&'a EvalReport>, generated_at: String }
        let export = FullExport { metrics: &self.metrics, reports: self.list_reports(), generated_at: chrono::Utc::now().to_rfc3339() };
        serde_json::to_string_pretty(&export).map_err(EvalError::ParseError)
    }

    fn export_csv(&self) -> EvalResult<String> {
        let mut lines = vec!["run_id,test_case_id,skill_name,judge_provider,overall_score,verdict,evaluated_at,total_duration_ms,tags".to_string()];
        for r in self.cache.values() {
            lines.push(format!("{},{},{},{},{:.4},{},{},{:.1},\"{}\"", r.run_id, r.test_case_id, r.skill_name, r.judge_provider, r.overall_score, r.verdict, r.evaluated_at, r.total_duration_ms, r.tags.join(";")));
        }
        // Variante Ok.
        Ok(lines.join("\n"))
    }

    /// Função compute trend.
    pub fn compute_trend(&mut self) {
        let mut by_date: HashMap<String, Vec<f64>> = HashMap::new();
        let mut pass_by_date: HashMap<String, (usize, usize)> = HashMap::new();
        for report in self.cache.values() {
            let date = report.evaluated_at.chars().take(10).collect::<String>();
            by_date.entry(date.clone()).or_default().push(report.overall_score);
            let entry = pass_by_date.entry(date).or_insert((0, 0));
            entry.1 += 1;
            if report.verdict == "pass" { entry.0 += 1; }
        }
        let mut trend: Vec<TrendDataPoint> = by_date.into_iter().map(|(date, scores)| {
            let avg: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
            let (passes, total) = pass_by_date.get(&date).copied().unwrap_or((0, 0));
            TrendDataPoint { date, avg_score: avg, pass_rate: if total > 0 { passes as f64 / total as f64 } else { 0.0 }, eval_count: total }
        }).collect();
        trend.sort_by(|a, b| a.date.cmp(&b.date));
        self.metrics.trend = trend;
    }

    // Expansion #7
    /// Função link goal contract criterion.
    pub fn link_goal_contract_criterion(&mut self, criterion_name: &str, contract_id: &str, criterion_index: usize, label: &str) {
        for crit in &mut self.config.criteria {
            if crit.name == criterion_name {
                crit.goal_contract_criterion = Some(GoalContractCriterionRef { contract_id: contract_id.to_string(), criterion_index, label: label.to_string() });
                break;
            }
        }
    }

    /// Função goal contract refs.
    pub fn goal_contract_refs(&self) -> Vec<&GoalContractCriterionRef> {
        self.config.criteria.iter().filter_map(|c| c.goal_contract_criterion.as_ref()).collect()
    }

    // Original report generation (preserved)
    /// Função generate markdown report.
    pub fn generate_markdown_report(&self) -> String {
        let mut lines = vec![
            "# Relatório de Avaliação de Skills".to_string(),
            format!("Gerado em: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")),
            String::new(),
            "## Resumo".to_string(),
            format!("- Total de avaliações: {}", self.metrics.total_evaluations),
            format!("- Taxa de aprovação: {:.1}%", self.metrics.pass_rate * 100.0),
            format!("- Score médio: {:.2}", self.metrics.avg_score),
            String::new(),
        ];
        if !self.metrics.top_skills.is_empty() {
            lines.push("## Top Skills".to_string());
            for (name, score) in &self.metrics.top_skills { lines.push(format!("- **{}**: {:.2}", name, score)); }
            lines.push(String::new());
        }
        if !self.metrics.bottom_skills.is_empty() {
            lines.push("## Skills com Baixo Desempenho".to_string());
            for (name, score) in &self.metrics.bottom_skills { lines.push(format!("- **{}**: {:.2}", name, score)); }
            lines.push(String::new());
        }
        if !self.metrics.benchmark_latencies.is_empty() {
            lines.push("## Benchmark Latency".to_string());
            lines.push("| Skill | p50 (ms) | p95 (ms) | p99 (ms) | Samples |".to_string());
            lines.push("|-------|----------|----------|----------|---------|".to_string());
            for (skill, lat) in &self.metrics.benchmark_latencies {
                lines.push(format!("| {} | {:.1} | {:.1} | {:.1} | {} |", skill, lat.p50_ms, lat.p95_ms, lat.p99_ms, lat.sample_count));
            }
            lines.push(String::new());
        }
        if !self.metrics.trend.is_empty() {
            lines.push("## Tendência".to_string());
            lines.push("| Data | Avg Score | Pass Rate | Evals |".to_string());
            lines.push("|------|-----------|-----------|-------|".to_string());
            for tp in &self.metrics.trend {
                lines.push(format!("| {} | {:.2} | {:.1}% | {} |", tp.date, tp.avg_score, tp.pass_rate * 100.0, tp.eval_count));
            }
            lines.push(String::new());
        }
        lines.push("## Detalhes por Caso de Teste".to_string());
        for report in self.cache.values() {
            lines.push(format!("\n### {} ({})\n- Score: {:.2}\n- Veredito: {}\n- Juiz: {}", report.test_case_id, report.skill_name, report.overall_score, report.verdict, report.judge_provider));
        }
        lines.join("\n")
    }

    /// Função metrics to json.
    pub fn metrics_to_json(&self) -> EvalResult<String> {
        serde_json::to_string_pretty(&self.metrics).map_err(EvalError::ParseError)
    }

    fn update_metrics(&mut self) {
        let reports: Vec<&EvalReport> = self.cache.values().collect();
        if reports.is_empty() { return; }
        self.metrics.total_evaluations = reports.len();
        let passes = reports.iter().filter(|r| r.verdict == "pass").count();
        self.metrics.pass_rate = passes as f64 / reports.len() as f64;
        let total_score: f64 = reports.iter().map(|r| r.overall_score).sum();
        self.metrics.avg_score = total_score / reports.len() as f64;
        let mut skill_scores: HashMap<String, Vec<f64>> = HashMap::new();
        for report in &reports { skill_scores.entry(report.skill_name.clone()).or_default().push(report.overall_score); }
        let mut skill_avgs: Vec<(String, f64)> = skill_scores.into_iter().map(|(name, scores)| {
            let avg: f64 = scores.iter().sum::<f64>() / scores.len() as f64; (name, avg)
        }).collect();
        skill_avgs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        self.metrics.top_skills = skill_avgs.iter().take(5).cloned().collect();
        self.metrics.bottom_skills = skill_avgs.iter().rev().take(5).cloned().collect();
        self.metrics.criterion_distribution.clear();
        for report in &reports {
            for cs in &report.criteria_scores {
                self.metrics.criterion_distribution.entry(cs.criterion.clone()).or_default().push(cs.score);
            }
        }
    }
}

fn default_criteria() -> Vec<EvalCriterion> {
    vec![
        // Variante EvalCriterion.
        EvalCriterion { name: "correctness".to_string(), description: "A skill produz output correto e factualmente preciso".to_string(), weight: 0.35, judge_prompt: "O output está correto e livre de erros factuais?".to_string(), goal_contract_criterion: None },
        // Variante EvalCriterion.
        EvalCriterion { name: "completeness".to_string(), description: "A skill cobre todos os aspectos solicitados no input".to_string(), weight: 0.25, judge_prompt: "Todos os aspectos do request foram abordados?".to_string(), goal_contract_criterion: None },
        // Variante EvalCriterion.
        EvalCriterion { name: "clarity".to_string(), description: "As instruções são claras e não ambíguas".to_string(), weight: 0.20, judge_prompt: "As instruções são claras e fáceis de seguir?".to_string(), goal_contract_criterion: None },
        // Variante EvalCriterion.
        EvalCriterion { name: "efficiency".to_string(), description: "A skill é eficiente em tokens e steps".to_string(), weight: 0.20, judge_prompt: "A skill evita verbosidade desnecessária e steps redundantes?".to_string(), goal_contract_criterion: None },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_test_case(id: &str, skill: &str) -> EvalTestCase {
        // Variante EvalTestCase.
        EvalTestCase { id: id.to_string(), skill_name: skill.to_string(), input: "Teste de input para avaliação".to_string(), expected_output: Some("Output esperado".to_string()), context: HashMap::new(), criteria_override: vec![], tags: vec!["test".to_string()] }
    }

    fn sample_scores() -> Vec<CriterionScore> {
        vec![
            // Variante CriterionScore.
            CriterionScore { criterion: "correctness".to_string(), score: 0.9, reasoning: "Output correto".to_string(), duration_ms: 100.0 },
            // Variante CriterionScore.
            CriterionScore { criterion: "completeness".to_string(), score: 0.8, reasoning: "Cobriu tudo".to_string(), duration_ms: 80.0 },
            // Variante CriterionScore.
            CriterionScore { criterion: "clarity".to_string(), score: 0.85, reasoning: "Instruções claras".to_string(), duration_ms: 60.0 },
            // Variante CriterionScore.
            CriterionScore { criterion: "efficiency".to_string(), score: 0.7, reasoning: "Poderia ser mais conciso".to_string(), duration_ms: 50.0 },
        ]
    }

    #[test] fn test_empty_flywheel() { let fw = EvalFlywheel::empty(); assert_eq!(fw.criteria().len(), 4); assert_eq!(fw.test_cases().len(), 0); assert_eq!(fw.metrics().total_evaluations, 0); }
    #[test] fn test_add_test_case() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "my-skill")); assert_eq!(fw.test_cases().len(), 1); }
    #[test] fn test_evaluate_mock() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "test-skill")); let report = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert_eq!(report.skill_name, "test-skill"); assert!(report.overall_score > 0.0); assert_eq!(report.verdict, "pass"); }
    #[test] fn test_evaluate_mock_fail() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-002", "bad-skill")); let bad = vec![ CriterionScore { criterion: "correctness".to_string(), score: 0.2, reasoning: "Incorreto".to_string(), duration_ms: 100.0 }, CriterionScore { criterion: "completeness".to_string(), score: 0.3, reasoning: "Incompleto".to_string(), duration_ms: 80.0 }, CriterionScore { criterion: "clarity".to_string(), score: 0.25, reasoning: "Confuso".to_string(), duration_ms: 60.0 }, CriterionScore { criterion: "efficiency".to_string(), score: 0.1, reasoning: "Muito verboso".to_string(), duration_ms: 50.0 } ]; let report = fw.evaluate_mock("tc-002", bad).unwrap(); assert_eq!(report.verdict, "fail"); }
    #[test] fn test_evaluate_mock_not_found() { let mut fw = EvalFlywheel::empty(); assert!(fw.evaluate_mock("nonexistent", sample_scores()).is_err()); }
    #[test] fn test_generate_judge_prompt() { let fw = EvalFlywheel::empty(); let tc = sample_test_case("tc-001", "my-skill"); let prompt = fw.generate_judge_prompt(&tc); assert!(prompt.contains("my-skill")); assert!(prompt.contains("correctness")); assert!(prompt.contains("JSON")); }
    #[test] fn test_metrics_update() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "skill-a")); fw.add_test_case(sample_test_case("tc-002", "skill-b")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); fw.evaluate_mock("tc-002", sample_scores()).unwrap(); let m = fw.metrics(); assert_eq!(m.total_evaluations, 2); assert!(m.pass_rate > 0.0); assert!(m.avg_score > 0.0); }
    #[test] fn test_get_report() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "skill-x")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert!(fw.get_report("tc-001").is_some()); assert_eq!(fw.get_report("tc-001").unwrap().skill_name, "skill-x"); assert!(fw.get_report("nonexistent").is_none()); }
    #[test] fn test_list_reports() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "a")); fw.add_test_case(sample_test_case("tc-002", "b")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); fw.evaluate_mock("tc-002", sample_scores()).unwrap(); assert_eq!(fw.list_reports().len(), 2); }
    #[test] fn test_markdown_report() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "demo-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let md = fw.generate_markdown_report(); assert!(md.contains("# Relatório")); assert!(md.contains("demo-skill")); assert!(md.contains("Taxa de aprovação")); }
    #[test] fn test_metrics_to_json() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "s")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let json = fw.metrics_to_json().unwrap(); let parsed: EvalMetrics = serde_json::from_str(&json).unwrap(); assert_eq!(parsed.total_evaluations, 1); }
    #[test] fn test_with_config() { let config = EvalConfig { version: "2.0.0".to_string(), default_provider: LlmProvider::Claude, timeout_seconds: 60, pass_threshold: 0.8, review_threshold: 0.5, criteria: default_criteria(), test_cases: vec![sample_test_case("tc-pre", "preloaded")], regression_threshold_pct: 10.0, auto_calibrate: false, calibration_min_runs: 20 }; let fw = EvalFlywheel::with_config(config); assert_eq!(fw.test_cases().len(), 1); }
    #[test] fn test_load_missing_config() { assert!(EvalFlywheel::load(Path::new("/nonexistent/eval.json")).is_err()); }
    #[test] fn test_llm_provider_display() { assert_eq!(LlmProvider::Gemini.to_string(), "gemini"); assert_eq!(LlmProvider::Claude.to_string(), "claude"); assert_eq!(LlmProvider::OpenAI.to_string(), "openai"); assert_eq!(LlmProvider::Local.to_string(), "local"); }

    // New tests for expansions
    #[test] fn test_regression_detection_no_regression() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "skill-a")); let baseline = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let current = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let reg = fw.detect_regression(&current, &baseline); assert!(!reg.is_regression); assert!(reg.delta_pct.abs() < 0.01); }
    #[test] fn test_regression_detection_with_regression() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "skill-a")); let baseline = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let bad = vec![ CriterionScore { criterion: "correctness".to_string(), score: 0.3, reasoning: "Bad".to_string(), duration_ms: 100.0 }, CriterionScore { criterion: "completeness".to_string(), score: 0.2, reasoning: "Bad".to_string(), duration_ms: 80.0 }, CriterionScore { criterion: "clarity".to_string(), score: 0.25, reasoning: "Bad".to_string(), duration_ms: 60.0 }, CriterionScore { criterion: "efficiency".to_string(), score: 0.15, reasoning: "Bad".to_string(), duration_ms: 50.0 } ]; let current = fw.evaluate_mock("tc-001", bad).unwrap(); let reg = fw.detect_regression(&current, &baseline); assert!(reg.is_regression); assert!(reg.delta_pct > 10.0); assert_eq!(reg.criterion_deltas.len(), 4); }
    #[test] fn test_multi_model_aggregation() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "multi-skill")); let providers = vec![LlmProvider::Gemini, LlmProvider::Claude, LlmProvider::OpenAI]; let result = fw.evaluate_multi_model("tc-001", &providers, |_p| sample_scores()).unwrap(); assert_eq!(result.per_model.len(), 3); assert!(result.aggregate_score > 0.0); assert!(result.model_specific_failures.is_empty()); }
    #[test] fn test_multi_model_with_failure() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "multi-skill")); let providers = vec![LlmProvider::Gemini, LlmProvider::Local]; let result = fw.evaluate_multi_model("tc-001", &providers, |p| { if *p == LlmProvider::Local { vec![ CriterionScore { criterion: "correctness".to_string(), score: 0.1, reasoning: "Fail".to_string(), duration_ms: 100.0 }, CriterionScore { criterion: "completeness".to_string(), score: 0.1, reasoning: "Fail".to_string(), duration_ms: 80.0 }, CriterionScore { criterion: "clarity".to_string(), score: 0.1, reasoning: "Fail".to_string(), duration_ms: 60.0 }, CriterionScore { criterion: "efficiency".to_string(), score: 0.1, reasoning: "Fail".to_string(), duration_ms: 50.0 } ] } else { sample_scores() } }).unwrap(); assert_eq!(result.model_specific_failures.len(), 1); assert_eq!(result.model_specific_failures[0], "local"); }
    #[test] fn test_sqlite_storage_roundtrip() { let store = EvalStore::in_memory().unwrap(); let mut fw = EvalFlywheel::empty().with_store(store); fw.add_test_case(sample_test_case("tc-001", "stored-skill")); let report = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let stored = fw.store.as_ref().unwrap().query_by_skill("stored-skill").unwrap(); assert_eq!(stored.len(), 1); assert_eq!(stored[0].skill_name, "stored-skill"); assert_eq!(stored[0].run_id, report.run_id); }
    #[test] fn test_sqlite_query_by_tag() { let store = EvalStore::in_memory().unwrap(); let mut fw = EvalFlywheel::empty().with_store(store); fw.add_test_case(sample_test_case("tc-001", "tagged-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert_eq!(fw.store.as_ref().unwrap().query_by_tag("test").unwrap().len(), 1); assert_eq!(fw.store.as_ref().unwrap().query_by_tag("nonexistent").unwrap().len(), 0); }
    #[test] fn test_sqlite_query_by_provider() { let store = EvalStore::in_memory().unwrap(); let mut fw = EvalFlywheel::empty().with_store(store); fw.add_test_case(sample_test_case("tc-001", "prov-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert_eq!(fw.store.as_ref().unwrap().query_by_provider(&LlmProvider::Gemini).unwrap().len(), 1); assert_eq!(fw.store.as_ref().unwrap().query_by_provider(&LlmProvider::Claude).unwrap().len(), 0); }
    #[test] fn test_benchmark_recording() { let mut fw = EvalFlywheel::empty(); let samples = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0]; let result = fw.record_benchmark("bench-skill", samples); assert_eq!(result.skill_name, "bench-skill"); assert!(result.latency.p50_ms > 0.0); assert!(result.latency.p95_ms >= result.latency.p50_ms); assert!(result.latency.p99_ms >= result.latency.p95_ms); assert_eq!(result.latency.sample_count, 10); assert!(fw.get_benchmark("bench-skill").is_some()); }
    #[test] fn test_auto_calibration_insufficient_data() { let mut fw = EvalFlywheel::empty(); fw.config.auto_calibrate = true; fw.config.calibration_min_runs = 100; fw.add_test_case(sample_test_case("tc-001", "cal-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert!(fw.auto_calibrate().unwrap().is_none()); }
    #[test] fn test_auto_calibration_sufficient_data() { let mut fw = EvalFlywheel::empty(); fw.config.auto_calibrate = true; fw.config.calibration_min_runs = 5; for i in 0..10 { fw.add_test_case(sample_test_case(&format!("tc-{:03}", i), "cal-skill")); } for i in 0..10 { fw.evaluate_mock(&format!("tc-{:03}", i), sample_scores()).unwrap(); } let result = fw.auto_calibrate().unwrap(); assert!(result.is_some()); let s = result.unwrap(); assert!(s.suggested_pass_threshold > 0.0); assert!(s.sample_size >= 5); }
    #[test] fn test_export_csv() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "csv-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let csv = fw.export_reports(EvalExportFormat::Csv).unwrap(); assert!(csv.contains("run_id,test_case_id")); assert!(csv.contains("csv-skill")); }
    #[test] fn test_export_json() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "json-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); let json = fw.export_reports(EvalExportFormat::Json).unwrap(); let parsed: serde_json::Value = serde_json::from_str(&json).unwrap(); assert!(parsed["metrics"].is_object()); assert!(parsed["reports"].is_array()); }
    #[test] fn test_goal_contract_linking() { let mut fw = EvalFlywheel::empty(); fw.link_goal_contract_criterion("correctness", "uuid-123", 0, "Output matches spec"); let refs = fw.goal_contract_refs(); assert_eq!(refs.len(), 1); assert_eq!(refs[0].contract_id, "uuid-123"); assert_eq!(refs[0].criterion_index, 0); assert_eq!(refs[0].label, "Output matches spec"); }
    #[test] fn test_goal_contract_in_report() { let mut fw = EvalFlywheel::empty(); fw.link_goal_contract_criterion("correctness", "gc-001", 2, "API returns 200"); fw.add_test_case(sample_test_case("tc-001", "gc-skill")); let report = fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert_eq!(report.goal_contract_refs.len(), 1); assert_eq!(report.goal_contract_refs[0].contract_id, "gc-001"); }
    #[test] fn test_compute_trend() { let mut fw = EvalFlywheel::empty(); fw.add_test_case(sample_test_case("tc-001", "trend-skill")); fw.add_test_case(sample_test_case("tc-002", "trend-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); fw.evaluate_mock("tc-002", sample_scores()).unwrap(); fw.compute_trend(); assert!(!fw.metrics().trend.is_empty()); }
    #[test] fn test_llm_provider_from_str() { assert_eq!("gemini".parse::<LlmProvider>().unwrap(), LlmProvider::Gemini); assert_eq!("CLAUDE".parse::<LlmProvider>().unwrap(), LlmProvider::Claude); assert!("unknown".parse::<LlmProvider>().is_err()); }
    #[test] fn test_benchmark_latency_empty() { let lat = BenchmarkLatency::from_samples(vec![]); assert_eq!(lat.sample_count, 0); assert_eq!(lat.p50_ms, 0.0); }
    #[test] fn test_sqlite_count() { let store = EvalStore::in_memory().unwrap(); assert_eq!(store.count().unwrap(), 0); let mut fw = EvalFlywheel::empty().with_store(store); fw.add_test_case(sample_test_case("tc-001", "count-skill")); fw.evaluate_mock("tc-001", sample_scores()).unwrap(); assert_eq!(fw.store.as_ref().unwrap().count().unwrap(), 1); }
}