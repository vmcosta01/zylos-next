# zylos-skills

Sovereign SKILLS_REGISTRY for Zylos-Next v6 — index.json discovery <100ms, compiled JSON Schema, eval flywheel, and export pipeline.

## Architecture

### Modules

- **`registry`** — `SkillRegistry`, `SkillsIndex`, `SkillEntry`, `SearchResult`, `UsageStats`, `RegistryMetrics`, `PruningPolicy` for skill indexing and discovery
- **`schema`** — `SkillValidator`, `DEFAULT_SKILL_SCHEMA` for JSON Schema-based skill validation
- **`eval`** — `EvalFlywheel`, `EvalConfig`, `EvalReport`, `EvalTestCase`, `EvalCriterion`, `BenchmarkResult`, `RegressionReport`, `MultiModelResult`, `LlmProvider` for evaluation and benchmarking
- **`export`** — `SkillExporter`, `ExportFormat`, `ExportReport`, `ExportableSkill` for skill export pipelines
- **`scheduler`** — `RoutineScheduler`, `Routine`, `TriggerType`, `ExecutionLog`, `SchedulerState`, `SchedulerStats` for scheduled skill execution
- **`debt`** — `DebtParser`, `DebtReport`, `DebtItem`, `DebtCategory`, `DebtSeverity` for technical debt tracking in skills
- **`glossary`** — `Glossary`, `GlossaryIndex`, `GlossaryTerm` for skill terminology management

### Key Exports

```rust
pub use registry::{SkillRegistry, SkillsIndex, SkillEntry, SearchResult, UsageStats, RegistryMetrics, PruningPolicy};
pub use schema::{SkillValidator, DEFAULT_SKILL_SCHEMA};
pub use eval::{EvalFlywheel, EvalConfig, EvalReport, EvalTestCase, BenchmarkResult, LlmProvider, MultiModelResult};
pub use export::{SkillExporter, ExportFormat, ExportReport, ExportableSkill};
pub use scheduler::{RoutineScheduler, Routine, TriggerType, ExecutionLog, SchedulerState, SchedulerStats};
pub use debt::{DebtParser, DebtReport, DebtItem, DebtCategory, DebtSeverity};
pub use glossary::{Glossary as GlossaryData, GlossaryIndex, GlossaryTerm};
```

## Usage

```rust
use zylos_skills::{SkillRegistry, SkillValidator, EvalFlywheel, RoutineScheduler};

// Initialize the skill registry with fast index.json discovery
let registry = SkillRegistry::load("skills/index.json").await?;

// Validate a skill against the default JSON Schema
let validator = SkillValidator::new(DEFAULT_SKILL_SCHEMA);
validator.validate(&skill_json)?;

// Search for skills by query
let results = registry.search("web scraping")?;
for entry in &results.entries {
    println!("{}: {}", entry.name, entry.description);
}
```

## Dependencies

| Crate | Features | Purpose |
|-------|----------|---------|
| `zylos-core` | workspace | Shared types and error handling |
| `serde` | workspace | Serialization |
| `serde_json` | workspace | JSON handling |
| `jsonschema` | workspace | JSON Schema validation |
| `thiserror` | workspace | Error types |
| `tracing` | workspace | Structured logging |
| `chrono` | workspace | Timestamps |
| `rusqlite` | workspace | Persistent storage |
| `uuid` | workspace | Unique identifiers |
| `tokio` | sync, rt, macros | Async runtime |
| `sha2` | workspace | Content hashing |
| `hex` | workspace | Hash encoding |
| `strsim` | workspace | Fuzzy string matching for search |
| `semver` | 1 | Semantic version parsing |

## Testing

```bash
cargo test -p zylos-skills
```

Tests cover registry indexing, schema validation, eval flywheel metrics, export serialization, scheduler triggers, debt parsing, and glossary lookups.

## Related Crates

- Depends on: `zylos-core`