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

## Fase 8: Skill Contracts & Evolution (2026-11-11)

### SkillContract Formal (T8.4.1)

Estrutura formal `(P,O,A,V,F)` para contratos de skills:
- **P** (Preconditions): predicates avaliáveis antes da execução
- **O** (Output Schema): JSON Schema draft-07 para validação estrutural
- **A** (Assertions): condições pós-execução verificáveis
- **V** (Validators): referências a funções de validação registradas
- **F** (Fitness Metrics): métricas de qualidade com baseline

### SkillEvolutionGraph (T8.4.2)

Grafo direcionado (`petgraph::DiGraph`) modelando relações entre skills:
- Edge types: `Extends`, `Replaces`, `Repairs`, `DependsOn`
- Operações: merge, retire, add_validator
- Detecção de ciclos e consistência topológica

### SkillMiner from Logs (T8.4.3)

Extração automática de candidatos a skill a partir de gateway logs:
- Regex-based pattern extraction de tool call sequences
- Candidate generation com scoring de confiança
- EvalFlywheel gate: score < 0.3 descartado automaticamente

### TrajectoryCompiler (T8.4.4)

Conversão HANDOFF markdown + playbook YAML → SkillContract automático:
- Parser extrai preconditions de "Pré-requisitos"
- Output schema inferido de "Deliverables"
- Assertions geradas de "Acceptance Criteria"

### SkillHierarchy 3-Tier (T8.4.5)

Classificação hierárquica de skills em 3 tiers:
- **Planning**: orquestra outras skills (ex: `incident-response`)
- **Functional**: domínio específico (ex: `web-search`)
- **Atomic**: operação unitária (ex: `http-get`)

Filtragem por tier no `IndexRegistry` para queries segmentadas.

### Hermes Integration

Endpoints dedicados via bridge Hermes (`hermes_skill_evolution_api`):
- `/api/v1/hermes/skill-evolution/contracts` — listar/validar
- `/api/v1/hermes/skill-evolution/graph` — grafo de evolução
- `/api/v1/hermes/skill-evolution/miner/run` — mineração de logs
- `/api/v1/hermes/skill-evolution/trajectory/compile` — compilação HANDOFF→Contract
- `/api/v1/hermes/skill-evolution/hierarchy` — listagem por tier

Cross-reference: [HANDOFF_FASE8.md](../../docs/handoffs/HANDOFF_FASE8.md) §2.6, [POP-Skill-Contract-Validation](../../playbooks/pop-skill-contract-validation.md), [Playbook Skill Evolution Lifecycle](../../playbooks/skill-evolution-lifecycle.md)

## Related Crates

- Depends on: `zylos-core`