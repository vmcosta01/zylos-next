//! zylos-skills — SKILLS_REGISTRY soberano para Zylos-Next v6
//!
//! Sistema 6 do Master Plan: index.json discovery <100ms,
//! JSON Schema compilado, eval flywheel e export pipeline.

/// Módulo debt.
pub mod debt;
/// Módulo eval.
pub mod eval;
/// Módulo export.
pub mod export;
/// Módulo glossary.
pub mod glossary;
/// Módulo registry.
pub mod registry;
/// Módulo scheduler.
pub mod scheduler;
/// Módulo schema.
pub mod schema;
/// Módulo contract — SkillContract formal (P,O,A,V,F) (T8.4.1).
pub mod contract;
/// Módulo evolution — SkillEvolutionGraph com merge/retire/add_validator (T8.4.2).
pub mod evolution;
/// Módulo miner — SkillMiner automático a partir de logs (T8.4.3).
pub mod miner;
/// Módulo trajectory — TrajectoryCompiler para HANDOFF/playbook → SkillContract (T8.4.4).
pub mod trajectory;
/// Módulo hierarchy — SkillHierarchy 3-tier Planning/Functional/Atomic (T8.4.5).
pub mod hierarchy;

pub use contract::{
    Assertion, AssertionSeverity, ContractError, FitnessMetric, OutputSchema, Predicate,
    SkillContract, ValidatorRef,
};
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
