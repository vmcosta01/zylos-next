# Typed Contract — Zylos-Next v6.0

## Visão Geral
Este documento define os contratos de dados tipados (serde + jsonschema) para comunicação entre os 6 sistemas core. Todos os schemas são versionados e validados em tempo de compilação via `jsonschema` crate.

## Schema Registry

### AgentConfig
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://zylos.dev/schemas/agent-config/v1.json",
  "type": "object",
  "required": ["name", "system_prompt", "sandbox_mode"],
  "properties": {
    "name": {
      "type": "string",
      "pattern": "^[a-z0-9][a-z0-9-]{0,63}$",
      "description": "Identificador único do agente (kebab-case, max 64 chars)"
    },
    "description": {
      "type": "string",
      "maxLength": 500
    },
    "system_prompt": {
      "type": "string",
      "minLength": 10,
      "maxLength": 8192
    },
    "skills": {
      "type": "array",
      "items": { "type": "string" },
      "maxItems": 50
    },
    "memory_scope": {
      "type": "string",
      "enum": ["global", "team", "agent", "task"]
    },
    "sandbox_mode": {
      "type": "string",
      "enum": ["wasm", "docker", "hybrid"]
    },
    "verification_level": {
      "type": "string",
      "enum": ["none", "guardrail", "panel", "full"]
    },
    "max_concurrent_runs": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100
    }
  },
  "additionalProperties": false
}
```

### WorkflowDAG
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://zylos.dev/schemas/workflow-dag/v1.json",
  "type": "object",
  "required": ["id", "nodes", "edges"],
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "name": { "type": "string", "maxLength": 128 },
    "nodes": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "agent_name", "task_spec"],
        "properties": {
          "id": { "type": "string" },
          "agent_name": { "type": "string" },
          "task_spec": { "type": "object" },
          "timeout_ms": { "type": "integer", "minimum": 100 },
          "retry_policy": {
            "type": "object",
            "properties": {
              "max_retries": { "type": "integer", "minimum": 0, "maximum": 10 },
              "backoff_ms": { "type": "integer", "minimum": 100 }
            }
          }
        }
      }
    },
    "edges": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["from", "to"],
        "properties": {
          "from": { "type": "string" },
          "to": { "type": "string" },
          "condition": { "type": "string" }
        }
      }
    }
  },
  "additionalProperties": false
}
```

### VerificationClaim
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://zylos.dev/schemas/verification-claim/v1.json",
  "type": "object",
  "required": ["claim_id", "content", "ground_truth"],
  "properties": {
    "claim_id": { "type": "string", "format": "uuid" },
    "content": { "type": "string" },
    "ground_truth": {
      "type": "string",
      "enum": ["verified", "refuted", "pending", "uncertain"]
    },
    "evidence": {
      "type": "array",
      "items": { "type": "string" }
    },
    "judge_verdicts": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "judge_id": { "type": "string" },
          "verdict": { "type": "string" },
          "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
        }
      }
    }
  }
}
```

### MemoryContextEntry
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://zylos.dev/schemas/memory-context/v1.json",
  "type": "object",
  "required": ["entry_id", "scope", "content_hash"],
  "properties": {
    "entry_id": { "type": "string", "format": "uuid" },
    "scope": {
      "type": "string",
      "enum": ["global", "team", "agent", "task"]
    },
    "owner_id": { "type": "string" },
    "content_hash": { "type": "string", "pattern": "^sha256:[a-f0-9]{64}$" },
    "embedding_vector": {
      "type": "array",
      "items": { "type": "number" },
      "maxItems": 1536
    },
    "metadata": { "type": "object" },
    "created_at": { "type": "string", "format": "date-time" },
    "ttl_seconds": { "type": "integer", "minimum": 0 }
  }
}
```

## Validação em Runtime
Todos os schemas acima são compilados via `jsonschema::JSONSchema::compile()` no startup do `zylos-gateway`. Requests inválidos retornam HTTP 400 com detalhes do erro de validação.

## Versionamento
- Schemas seguem semver: breaking changes incrementam major version
- Backward compatibility mantida por pelo menos 2 versões menores
- Migration adapters automáticos para versões legadas

## Referências
- [GOAL-CONTRACT.md](./GOAL-CONTRACT.md) para contratos de verificação
- [ARCHITECTURE.md](../ARCHITECTURE.md) para fluxo de dados entre sistemas