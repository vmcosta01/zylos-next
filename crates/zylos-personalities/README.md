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
