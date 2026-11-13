# zylos-personalities

Personality-driven agent identities with division taxonomy for zylos-gateway. Structured system for defining agent personalities organized by functional divisions (Engineering, Research, Operations, Creative, Security, Management, Support, Custom). Each personality includes behavioral traits, communication style, expertise domains, and collaboration preferences.

## Quick Start

```rust
use zylos_personalities::{PersonalityRegistry, PersonalityProfile, Division, PersonalityQuery};

// Open registry (SQLite WAL, auto-migrations)
let registry = PersonalityRegistry::open("personalities.db").await?;

// Seed built-in personalities (idempotent)
let seeded = registry.seed_defaults().await?; // returns count inserted

// Create custom personality
let mut profile = PersonalityProfile::new("rust-expert", "Rust Expert", Division::Engineering);
profile.description = "Deep Rust systems programming specialist".into();
profile.system_prompt_fragment = "You are a Rust expert...".into();
registry.create(&profile).await?;

// Lookup by slug
let p = registry.get_by_slug("senior-engineer").await?;

// Query with filters
let results = registry.query(&PersonalityQuery {
    division: Some(Division::Engineering),
    active_only: true,
    limit: Some(10),
    ..Default::default()
}).await?;

// Stats overview
let stats = registry.stats().await?;
```

## Types

| Type | Description |
|------|-------------|
| `Division` | Enum: `Engineering`, `Research`, `Operations`, `Creative`, `Security`, `Management`, `Support`, `Custom`. Supports aliases (`eng`, `ops`, `sec`, `mgmt`) via `FromStr`. |
| `CommunicationStyle` | Tone, verbosity, emoji preference, language, proactive flag, format |
| `BehavioralTrait` | Named trait with description and weight 0.0–1.0 |
| `ExpertiseDomain` | Domain name, proficiency level, optional years of experience |
| `CollaborationPreference` | Team size, role, defer-to-experts flag, conflict resolution style |
| `PersonalityProfile` | Complete identity: slug, division, prompt fragment, traits, expertise, tags, versioning |
| `PersonalityQuery` | Filter struct: division, tag, active_only, search substring, limit, offset |
| `PersonalityStats` | Aggregate: total/active profiles, division counts, tag counts |
| `PersonalityRegistry` | SQLite WAL-backed CRUD + query + seed_defaults + stats |

## Error Variants (`PersonalityError`)

| Variant | Cause |
|---------|-------|
| `Database(sqlx::Error)` | SQLite failure |
| `Serialization(serde_json::Error)` | JSON encode/decode of nested profile fields |
| `YamlParse(serde_yaml::Error)` | YAML loading (future file-based import) |
| `NotFound(String)` | Slug or ID lookup miss |
| `DuplicateSlug(String)` | Insert with existing slug |
| `Io(std::io::Error)` | Filesystem error |
| `Internal(String)` | Catch-all logic error |

## Fase 8: AI Employee Governance (2026-11-11)

### EmployeeRole Enum (T8.5.1)

8 roles padronizados mapeados para personality presets e routine sets:

| Role | Descrição |
|------|-----------|
| Developer | Engenharia de software |
| Designer | UX/UI e design visual |
| DevOps | Infraestrutura e CI/CD |
| QaEngineer | Testes e qualidade |
| ProductManager | Gestão de produto |
| SupportAgent | Suporte ao cliente |
| ContentCreator | Criação de conteúdo |
| SecurityAuditor | Auditoria de segurança |

### RoutineCatalog (T8.5.2)

Catálogo indexado de rotinas por role com validação na carga e operações CRUD.

### PlanFirstGate (T8.5.3)

Gate de aprovação humana obrigatória antes da execução de rotinas. Usa UUID lexicográfico como tiebreaker quando timestamps são idênticos (resolução de segundo do `chrono_now_stub`).

### EmployeeRateLimiter (T8.5.4)

Token bucket algorithm por employee/role com limites configuráveis e stats reporting.

### GuardrailEnforcer (T8.5.5)

Sistema de regras declarativas de segurança operacional:

- Regras com prioridade ordenada (maior = avaliada primeiro)
- Wildcards em rotinas (`deploy-*`) e recursos (`filesystem:/etc/*`)
- Filtro por role específico ou todos os roles
- 3 ações: `Allow`, `Block { reason }`, `RequireApproval { message }`
- Decision log com ring buffer para auditoria
- Versões sync e async para uso dentro/fora de runtime Tokio

```rust
use zylos_personalities::guardrails::{GuardrailEnforcer, GuardrailRule};

let enforcer = GuardrailEnforcer::with_defaults();
let decision = enforcer.evaluate("emp-1", &EmployeeRole::DevOps, "deploy-production", None).await;
// → RequireApproval { message: "Production deployment requires human sign-off" }
```

### Hermes Integration

Endpoints dedicados via bridge Hermes (`hermes_employee_api`, `hermes_security_api`):
- `/api/v1/hermes/employee/*` — roles, routines, plan-gate, rate-limiter
- `/api/v1/hermes/security/guardrails/*` — stats, rules CRUD

Cross-reference: [HANDOFF_FASE8.md](../../docs/handoffs/HANDOFF_FASE8.md) §2.7, [POP-Guardrail-Rule-Update](../../playbooks/pop-guardrail-rule-update.md), [SOAR Guardrail Enforcement](../../playbooks/soar-guardrail-enforcement.md)

## Gaps

- Tags stored as JSON string in SQLite column; substring matching (`LIKE %tag%`) can produce false positives on partial matches.
- `seed_defaults` uses hardcoded list; no external YAML/JSON file loading despite `YamlParse` error variant existing.
- No soft-delete — `delete` permanently removes the row.
- `update` bumps version unconditionally even if content is unchanged.
- `query` search is naive substring match across three columns; no ranking or fuzzy matching.
- `fingerprint()` hashes only slug + display_name + division + prompt + traits; changes to expertise_domains, collaboration, or tags do not affect it.
- No export/import API for personality profiles.
- `row_to_profile` uses `unwrap_or_default()` on JSON parse failures — silent data loss on corrupted rows.
- No concurrency control beyond SQLite WAL; simultaneous updates to the same profile may overwrite.

## License

Part of the zylos-next workspace. See the root `LICENSE` file for details.
