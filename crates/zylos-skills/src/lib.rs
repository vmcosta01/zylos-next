//! zylos-skills — SKILLS_REGISTRY soberano para Zylos-Next v6
//!
//! Sistema 6 do Master Plan: index.json discovery <100ms,
//! JSON Schema compilado, eval flywheel e export pipeline.

pub mod debt;
pub mod eval;
pub mod export;
pub mod glossary;
pub mod registry;
pub mod scheduler;
pub mod schema;

pub use debt::{DebtCategory, DebtError, DebtItem, DebtParser, DebtReport, DebtResult, DebtSeverity};
pub use export::{ExportError, ExportFormat, ExportReport, ExportResult, ExportableSkill, SkillExporter};
pub use scheduler::{ExecutionLog, Routine, RoutineScheduler, RoutineStatus, SchedulerError, SchedulerResult, SchedulerState, SchedulerStats, TriggerType};
pub use eval::{
    BenchmarkLatency, BenchmarkResult, CalibrationSuggestion, CriterionDelta, CriterionScore,
    EvalConfig, EvalCriterion, EvalError, EvalExportFormat, EvalFlywheel, EvalMetrics,
    EvalReport, EvalResult, EvalStore, EvalTestCase, GoalContractCriterionRef, LlmProvider,
    MultiModelResult, RegressionReport, TrendDataPoint,
};
pub use glossary::{GlossaryError, GlossaryIndex, GlossaryResult, GlossaryTerm, Glossary as GlossaryData};
pub use registry::{PruningPolicy, RegistryError, RegistryMetrics, RegistryResult, SearchResult, SkillEntry, SkillRegistry, SkillsIndex, UsageStats};
pub use schema::{SchemaError, SchemaResult, SkillValidator, DEFAULT_SKILL_SCHEMA};
