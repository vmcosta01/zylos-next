# zylos-gateway

**Fase:** 6.1.1 + 7.1.1 - API Gateway Consolidada
**Status:** ✅ Implementado (972 LOC, 6 arquivos Rust, 36 testes)

## Descricao

API Gateway unificada que consolida todos os endpoints do sistema Zylos. Rota requisicoes para cockpit, SOAR, skills, orquestracao e subsystems da Fase 2 (AgentShield, adapters, memory-sync, personalities, swarm, evidence-ledger, browser-takeover).

## Arquitetura

```
zylos-gateway/
├── src/
│   ├── main.rs              # Entry point, graceful shutdown, config load
│   ├── lib.rs               # AppState + build_router() consolidado
│   ├── config.rs            # GatewayConfig TOML + env vars (hot-reload)
│   ├── orchestrate_api.rs   # POST /api/v1/orchestrate (DAG dispatch)
│   ├── soar_api.rs          # POST /api/v1/soar/events, GET /playbooks
│   └── phase2_integration.rs # Phase 2 subsystems init + routes
└── tests/
    └── e2e_integration.rs   # Integration tests
```

## Quick Start

```bash
# Configurar via TOML
export ZYLOS_CONFIG=/etc/zylos/gateway.toml

# Ou via env vars
export ZYLOS_LISTEN_ADDR=0.0.0.0:3000
export ZYLOS_SHUTDOWN_TIMEOUT=5

# Run
cargo run --bin zylos-gateway
```

## Endpoints

| Metodo | Rota | Descricao |
|--------|------|-----------|
| GET | `/gateway/health` | Health check |
| GET | `/gateway/ready` | Readiness probe |
| GET | `/gateway/live` | Liveness probe |
| GET | `/api/v1/events` | SSE stream (AG-UI) |
| GET | `/api/v1/ws` | WebSocket connection |
| POST | `/api/v1/approval/resolve` | Resolve HITL approval |
| GET | `/api/v1/skills` | List registered skills |
| POST | `/api/v1/eval/run` | Run skill evaluation |
| POST | `/api/v1/soar/events` | Submit SOAR event |
| GET | `/api/v1/soar/playbooks` | List SOAR playbooks |
| POST | `/api/v1/orchestrate` | Dispatch DAG orchestration |
| GET | `/api/v1/phase2/status` | Phase 2 subsystems status |
| GET | `/api/v1/phase2/shield/stats` | AgentShield stats |

## Configuracao (TOML)

```toml
[server]
listen_addr = "0.0.0.0:3000"
shutdown_timeout_secs = 5
max_connections = 1024

[health]
ready_enabled = true
live_enabled = true

[soar]
audit_key = "zylos-gateway-soar-key"
event_bus_capacity = 256

[skills]
index_path = "."
```


## SOAR Auto-Loading

On startup, the gateway automatically discovers and loads SOAR runbooks and playbooks from the filesystem:

- **Runbooks:** Loaded from `docs/soar/runbooks/*.yaml` and registered into the `RunbookRegistry`.
- **Playbooks:** Loaded from `playbooks/*.yaml` and registered into the `EventRouter`.

This allows CLI and Cockpit clients to trigger complex automation workflows via `/api/v1/soar/events` and `/api/v1/soar/playbooks` without manual registration.

## Graceful Shutdown

Recebe SIGINT/SIGTERM e drena requests ativas por `shutdown_timeout_secs` antes de forcar parada.

## Dependencias Principais

- `axum` 0.7 (HTTP framework)
- `tower` 0.5 + `tower-http` 0.6 (middleware)
- `tokio` 1.40 (async runtime)
- `zylos-live-comm` (EventBus, SSE, cockpit routes)
- `zylos-skills` (SkillRegistry, validator)
- `zylos-soar` (orchestrator, runbook registry)

## Runbooks

- [`zylos-gateway.runbook.yaml`](../../docs/soar/runbooks/zylos-gateway.runbook.yaml)
- [`zylos-gateway.md`](../../runbooks/zylos-gateway.md)