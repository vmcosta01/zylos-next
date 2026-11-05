# Fase 5: Skills Registry — Runbook Zylos-Next

## Visão Geral

O SKILLS_REGISTRY (`zylos-skills`) implementa discovery de skills via `index.json` (<100ms), validação com JSON Schema compilado, eval flywheel com LLM-as-judge, export pipeline para formatos externos (.cursor/.claude), glossário 1st-class, debt tracking, scheduler para rotinas automáticas, e Skills API REST.

**Módulos principais** (de `crates/zylos-skills/src/lib.rs`):
- `registry` — SkillRegistry com SkillEntry, SkillsIndex, SearchResult, RegistryMetrics, UsageStats, PruningPolicy
- `schema` — SkillValidator com DEFAULT_SKILL_SCHEMA, SchemaError, SchemaResult
- `eval` — EvalFlywheel com EvalConfig, EvalCriterion, EvalTestCase, BenchmarkResult, EvalReport, CalibrationSuggestion, MultiModelResult, RegressionReport, TrendDataPoint, LlmProvider
- `export` — SkillExporter com ExportFormat, ExportReport, ExportableSkill
- `glossary` — GlossaryIndex com GlossaryTerm, GlossaryData
- `debt` — DebtParser com DebtItem, DebtReport, DebtCategory, DebtSeverity
- `scheduler` — RoutineScheduler com Routine, TriggerType, SchedulerState, SchedulerStats, ExecutionLog

**Integrações:**
- `zylos-live-comm/src/skills_api.rs` — REST endpoints GET /skills, GET /skills/:id, POST /eval/run
- `scripts/migrate_skills.py` — Migração de SKILL.md Hermes → index.json

**Critério de Saída:** 107+ testes passando; descobre e valida skills em <100ms; exporta para .cursor/.claude; 173+ skills migradas; dupla auditoria APPROVED.

## Pré-requisitos

- Fases 0-4 completas
- Dependências: `serde`, `serde_json`, `jsonschema 0.18`, `tokio`, `chrono`, `uuid`
- `skills/index.json` inicializado (gerado via `migrate_skills.py`)
- `skills/schemas/skill.schema.json` definido
- LLM provider configurado para eval flywheel (opcional)
- Python 3 disponível para script de migração

### Setup

```bash
# Build e testes
cargo build -p zylos-skills
cargo test -p zylos-skills --lib          # Esperado: 86 passed
cargo test -p zylos-live-comm --lib skills_api::tests  # Esperado: 6 passed

# Clippy
cargo clippy -p zylos-skills -p zylos-live-comm -- -D warnings

# Migrar skills Hermes → index.json
python3 scripts/migrate_skills.py

# Validar index.json
cargo test -p zylos-skills --lib registry::tests::test_load_from_directory
```

## Para Dev Senior

### Decisões Arquiteturais

1. **index.json Discovery <100ms**: `SkillsIndex` carrega e indexa `index.json` em memória na startup. `SkillRegistry.search()` faz lookup O(1) por nome ou O(n) filtrado por tags/categoria. Index é reloadable via hot-reload sem restart. Load target <50ms para 500 skills.

2. **JSON Schema Compilado (Draft-7)**: `SkillValidator` compila `DEFAULT_SKILL_SCHEMA` uma vez e reusa. Validação em <1ms por skill. Schema define campos obrigatórios: `name`, `description` (≤200 chars), `version`, `path`, `tags` (≤10 itens). Frontmatter YAML em SKILL.md é parseado e validado.

3. **Eval Flywheel LLM-as-Judge**: `EvalFlywheel` roda test cases contra múltiplos `LlmProvider`. Gera `EvalReport` com `CriterionScore`, `BenchmarkResult`, `RegressionReport`. `CalibrationSuggestion` sugere ajustes quando scores caem. `TrendDataPoint` trackea evolução temporal. `MultiModelResult` compara providers.

4. **Export Pipeline**: `SkillExporter` converte skills internas para `ExportFormat::Cursor`, `ExportFormat::Claude`, `ExportFormat::Markdown`. `ExportableSkill` trait permite custom exporters.

5. **Debt Tracking**: `DebtParser` analisa comentários `shortcut:` em skills para identificar dívida técnica. `DebtItem` com `DebtCategory` e `DebtSeverity`. `DebtReport` agrega findings e gera health score por skill.

6. **Routine Scheduler**: `RoutineScheduler` executa rotinas periódicas (`TriggerType::Cron` ou `TriggerType::Interval`). Cron validation em registration. Auto-disable após N falhas consecutivas. `SchedulerState` persiste estado entre restarts. `ExecutionLog` registra histórico.

7. **Skills API REST**: Endpoints em `zylos-live-comm/src/skills_api.rs`: GET /skills (lista com filtering), GET /skills/:id (detalhe), POST /eval/run (dispara eval flywheel). Kebab-case para IDs.

8. **Migração Hermes**: `scripts/migrate_skills.py` varre SKILL.md files, extrai frontmatter YAML, gera `index.json` entries. Idempotente — pode ser re-executado após adicionar/remover skills.

### Extension Points

- **Novos export formats**: Implemente `ExportableSkill` para novo formato, registre em `ExportFormat`
- **Custom eval criteria**: Adicione `EvalCriterion` variants para métricas específicas de domínio
- **Novos LLM providers**: Implemente `LlmProvider` trait para novo backend
- **Custom debt categories**: Estenda `DebtCategory` para categorias específicas do projeto
- **Novos trigger types**: Adicione ao `TriggerType` enum para eventos customizados
- **Custom search filters**: Estenda `SearchResult` com novos campos de filtering

### Performance Tuning

- index.json load <50ms para 500 skills
- Search <10ms para queries simples, <50ms para filtered searches
- Schema validation <1ms por skill
- Eval flywheel paralleliza test cases across providers
- Use `PruningPolicy` para remover skills não usadas e manter index lean
- Cache schema compilation (não recompile a cada validação)

## Para Dev Pleno

### Searching Skills

```rust
use zylos_skills::{SkillRegistry, SearchResult};

let registry = SkillRegistry::load("skills/index.json").await?;

// Search by name (O(1))
if let Some(entry) = registry.get("web-scraping") {
    println!("Found: {} v{}", entry.name, entry.version);
}

// Search by tag (O(n) filtered)
let results: Vec<SearchResult> = registry.search_by_tag("browser").await?;

// Metrics
let metrics = registry.metrics();
println!("Total: {}, avg search: {:?}", 
    metrics.total_entries, metrics.avg_search_latency);
```

### Validating Skills

```rust
use zylos_skills::{SkillValidator, DEFAULT_SKILL_SCHEMA};

let validator = SkillValidator::new(DEFAULT_SKILL_SCHEMA)?;

match validator.validate(&skill_json) {
    Ok(_) => println!("Valid"),
    Err(e) => eprintln!("Invalid: {}", e),
}
```

### Running Eval Flywheel

```rust
use zylos_skills::{EvalFlywheel, EvalConfig, LlmProvider};

let config = EvalConfig {
    providers: vec![LlmProvider::OpenAi, LlmProvider::Anthropic],
    parallel: true,
    ..Default::default()
};

let flywheel = EvalFlywheel::new(config);
let report = flywheel.run("web-scraping").await?;

println!("Score: {:.2}", report.overall_score);
for suggestion in &report.calibration_suggestions {
    println!("Suggestion: {}", suggestion.description);
}
```

### Exporting Skills

```rust
use zylos_skills::{SkillExporter, ExportFormat};

let exporter = SkillExporter::new();
let report = exporter.export("web-scraping", ExportFormat::Claude, "output/").await?;
println!("Exported {} files to {}", report.files_written, report.output_dir);
```

### Adding a New Skill

```bash
# 1. Create SKILL.md with valid frontmatter
mkdir -p skills/my-new-skill
cat > skills/my-new-skill/SKILL.md << 'EOF'
---
name: my-new-skill
description: "Does X when user asks about Y"
version: 1.0.0
tags: [productivity, automation]
---
# My New Skill
...
EOF

# 2. Regenerate index.json
python3 scripts/migrate_skills.py

# 3. Validate
cargo test -p zylos-skills --lib registry::tests::test_load_from_directory
```

## Para Dev Junior

### Passo a Passo: Adicionando uma Nova Skill

1. **Crie SKILL.md** com frontmatter YAML válido (name, description ≤200 chars, version, tags ≤10)
2. **Rode migração**: `python3 scripts/migrate_skills.py`
3. **Verifique index.json**: Confirme que nova entry aparece
4. **Valide**: `cargo test -p zylos-skills --lib`
5. **Teste via API**: `GET /skills/my-new-skill` deve retornar detalhes

### Pitfalls Comuns

- **Não pule validação de schema**: Skills inválidas quebram discovery e API
- **Não adicione skills sem trigger/description**: Campos obrigatórios para routing
- **Não ignore eval results**: Regression reports indicam degradação
- **Não modifique index.json manualmente**: Sempre use `migrate_skills.py`
- **Não esqueça versionamento**: Bump version quando mudar comportamento
- **Não use nomes não-kebab-case**: API usa kebab-case para IDs

### Debugging Tips

- `RegistryMetrics` mostra latency percentiles e cache hit rates
- `DebtReport` identifica skills com problemas acumulados
- Eval flywheel logs incluem raw LLM responses para debugging
- API retorna 404 → confirme nome kebab-case e `active: true`
- Use `RUST_LOG=zylos_skills=debug` para ver search/validation traces

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `RegistryError::IndexLoadFailed` | index.json malformado ou missing | Rode `migrate_skills.py`, valide JSON syntax |
| `SchemaError::ValidationFailed` | Skill não conforme ao schema Draft-7 | Verifique campos obrigatórios, description ≤200, tags ≤10 |
| `EvalError::ProviderUnavailable` | LLM provider offline ou auth fail | Verifique API key, tente outro provider |
| `EvalError::UnprocessableEntity` | test_case_id não existe no config | Verifique eval config, liste test cases disponíveis |
| `ExportError::FormatUnsupported` | Formato de export não implementado | Use formato suportado ou implemente `ExportableSkill` |
| `SchedulerError::RoutineFailed` | Action panics ou timeout | Wrap action em catch_unwind, aumente timeout |
| `DebtError::ParseFailed` | Skill content não parseable | Verifique markdown structure e encoding UTF-8 |
| API 404 para skill existente | Nome não kebab-case ou `active: false` | Corrija nome, verifique active flag |
| Search lento (>100ms) | Index muito grande ou não cached | Habilite caching, prune unused skills via PruningPolicy |
| `migrate_skills.py` falha | SKILL.md com encoding inválido ou YAML malformado | Verifique UTF-8, valide frontmatter YAML |

## Referências

- **Source**: `crates/zylos-skills/src/` (7 módulos)
- **Key files**: `lib.rs:1-25`, `registry.rs`, `schema.rs`, `eval.rs`, `export.rs`, `glossary.rs`, `debt.rs`, `scheduler.rs`
- **Skills API**: `crates/zylos-live-comm/src/skills_api.rs`
- **Migration script**: `scripts/migrate_skills.py`
- **Generated index**: `skills/index.json`
- **Audits**: `evals/audits/fase5-audit-adversarial.md`, `evals/audits/fase5-audit-isolated.md`
- **Master Plan**: Linhas 199-202 (Fase 5 SKILLS_REGISTRY overview), linhas 1205-1376 (detalhamento)