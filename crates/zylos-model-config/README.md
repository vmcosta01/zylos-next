# zylos-model-config

> Multi-Model Selector para Zylos Cockpit v2 — configuração de modelos por bot com tier strategy e reasoning effort.

## Purpose

Permitir que cada bot tenha configuração individual de modelo LLM, com hierarquia de resolução: conversation > bot > global default. Suporta tier strategy (cheapest/balanced/fastest/smartest) e reasoning effort (low/medium/high/auto). Inspirado em AnyModelBot (`model-picker.js`, `lib/frontier.js`).

## Architecture

```
┌─────────────────┐ ┌──────────────────┐ ┌─────────────┐
│ Cockpit Chat │────▶│ ModelConfigStore │────▶│ SQLite DB │
│ /api/v1/models │ │ (resolve hierarchy)│ │ model_configs│
└─────────────────┘ └──────────────────┘ └─────────────┘
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `ModelConfig`, `ResolvedModelConfig`, `TierStrategy`, `ReasoningEffort`, `UpsertModelConfig`, `AvailableModel` |
| `store` | Persistência SQLite com upsert, resolução hierárquica (bot > global > default), migrations |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/models/config?bot_id=X` | Config efetiva do bot (com herança) |
| PUT | `/api/v1/models/config/:bot_id` | Define config específica do bot |
| DELETE | `/api/v1/models/config/:bot_id` | Remove override do bot (volta ao global) |
| GET | `/api/v1/models/list` | Lista modelos disponíveis por provider |

## Schema

### model_configs

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| bot_id | TEXT UNIQUE | Identificador do bot ou "__global__" |
| provider | TEXT | openai, anthropic, ollama, etc. |
| model_id | TEXT | gpt-4o, claude-3.5-sonnet, llama3, etc. |
| tier_strategy | TEXT | cheapest, balanced, fastest, smartest |
| reasoning_effort | TEXT | low, medium, high, auto |
| max_tokens | INTEGER | Limite de tokens de saída (opcional) |
| temperature | REAL | Temperatura de sampling (opcional) |
| is_default | BOOLEAN | Se true, é o default do provider |
| created_at | TEXT | ISO 8601 timestamp |
| updated_at | TEXT | ISO 8601 timestamp |

## Hierarquia de Resolução

```
Conversation override (futuro)
        ↓ (se não existe)
Bot-specific config (model_configs WHERE bot_id = ?)
        ↓ (se não existe)
Global config (model_configs WHERE bot_id = '__global__')
        ↓ (se não existe)
Hardcoded default (openai/gpt-4o-mini/balanced/medium)
```

O campo `source` na `ResolvedModelConfig` indica qual nível foi usado: "bot", "global" ou "default".

## Testing

```bash
cargo test -p zylos-model-config
```

- **14 testes unitários** cobrindo: create/get, upsert update, resolve bot-specific, resolve fallback to global, resolve fallback to default, delete bot config, delete nonexistent, list all, defaults applied on create, tier strategy roundtrip, reasoning effort roundtrip, defaults, invalid from_str, upsert serialization
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON |
| serde_json | workspace | Parsing de request/response |
| sqlx | 0.8 (sqlite) | Persistência async SQLite |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601 |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |

## Related Documents

- **Playbook:** [pb-model-tier-strategy.md](../../playbooks/pb-model-tier-strategy.md)
- **Runbook:** [rb-zylos-model-config.md](../../runbooks/rb-zylos-model-config.md)
- **POP:** [pop-model-fallback.md](../../playbooks/pop-model-fallback.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.3
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)