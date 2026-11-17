# zylos-gateway

[![CI](https://github.com/vmcosta01/zylos-next/actions/workflows/ci.yml/badge.svg?branch=master)](https://github.com/vmcosta01/zylos-next/actions/workflows/ci.yml) [![Deploy Preview](https://github.com/vmcosta01/zylos-next/actions/workflows/deploy-preview.yml/badge.svg)](https://github.com/vmcosta01/zylos-next/actions/workflows/deploy-preview.yml)

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

## Hermes Bridge — Integração com Hermes Agent (Fase 8)

A bridge Hermes conecta o Zylos ao Hermes Agent via HTTP, expondo endpoints dedicados por subsistema. Todos requerem `Authorization: Bearer $ZYLOS_HERMES_TOKEN` (mínimo 16 chars).

### Subsistemas Conectados

| Subsistema | Módulo API | Endpoints | Descrição |
|------------|-----------|-----------|-----------|
| bot | `hermes_api.rs` | `/hermes/events`, `/hermes/bot/dispatch-task`, `/hermes/bot/spawn-crew` | Dispatch de tarefas Kanban→BotSystem, spawn de crews via SwarmOrchestrator |
| kanban | `hermes_api.rs` | `/hermes/kanban/{action}` | Catch-all para ações do Kanban Hermes |
| memory | `hermes_api.rs` | `/hermes/memory/{action}` | Catch-all para operações de memória |
| skills | `hermes_api.rs` | `/hermes/skills/{action}` | Catch-all para registry de skills |
| soar | `hermes_api.rs` | `/hermes/soar/{action}` | Catch-all para orquestração SOAR |
| verification | `hermes_api.rs` | `/hermes/verification/{action}` | Catch-all para verificação |
| sandbox | `hermes_api.rs` | `/hermes/sandbox/{action}` | Catch-all para sandbox WASM/Docker |
| live-comm | `hermes_api.rs` | `/hermes/live-comm/{action}` | Catch-all para comunicação em tempo real |
| **security** | `hermes_security_api.rs` | 9 endpoints | Guardrails, capability tokens, egress filter, output sanitizer |
| **desktop** | `hermes_desktop_api.rs` | 9 endpoints | Workspace persistence, agent awareness, relay server |
| **skill-evolution** | `hermes_skill_evolution_api.rs` | 6 endpoints | Contracts, evolution graph, miner, trajectory, hierarchy |
| **employee** | `hermes_employee_api.rs` | 4 endpoints | Roles, routines, plan-first gate, rate limiter |

### Endpoints Dedicados Fase 8

#### Security (`/api/v1/hermes/security/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/guardrails/stats` | Estatísticas do GuardrailEnforcer |
| POST | `/guardrails/rules` | Criar/atualizar regra de guardrail |
| GET | `/capability-tokens` | Listar tokens ativos |
| POST | `/capability-tokens/issue` | Emitir novo token de capacidade |
| DELETE | `/capability-tokens/revoke` | Revogar token |
| GET | `/egress/rules` | Listar regras de egress de rede |
| POST | `/egress/rules` | Adicionar regra de egress |
| GET | `/sanitizer/rules` | Listar regras de sanitização |
| POST | `/sanitizer/sanitize` | Sanitizar output |

#### Desktop (`/api/v1/hermes/desktop/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/workspace/state` | Estado atual do cockpit |
| POST | `/workspace/save` | Salvar estado explicitamente |
| POST | `/workspace/reset` | Resetar para defaults |
| GET | `/awareness/states` | Estados de todos os agentes |
| GET | `/awareness/attention` | Agentes requerendo atenção |
| POST | `/awareness/update` | Atualizar estado de um agente |
| GET | `/relay/devices` | Listar dispositivos mobile pareados |
| POST | `/relay/pair` | Parear novo dispositivo |
| DELETE | `/relay/tokens/revoke` | Revogar token de dispositivo |

#### Skill Evolution (`/api/v1/hermes/skill-evolution/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/contracts` | Listar contratos ativos |
| POST | `/contracts/validate` | Validar contrato formal (P,O,A,V,F) |
| GET | `/graph` | Grafo de evolução (Extends/Replaces/Repairs/DependsOn) |
| POST | `/miner/run` | Executar mineração de skills a partir de logs |
| POST | `/trajectory/compile` | Compilar HANDOFF/playbook → SkillContract |
| GET | `/hierarchy` | Listar skills por tier (Planning/Functional/Atomic) |

#### Employee Governance (`/api/v1/hermes/employee/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/roles` | Listar 8 roles padronizados |
| GET | `/routines` | Catálogo de rotinas por role |
| POST | `/plan-gate/submit` | Submeter plano para aprovação humana |
| GET | `/rate-limiter/stats` | Estatísticas do rate limiter |

### Configuração

```bash
# Gerar token seguro (mínimo 16 chars)
export ZYLOS_HERMES_TOKEN=$(openssl rand -hex 32)

# Ou via config TOML
# [hermes]
# token = "seu-token-aqui"
```

Sem `ZYLOS_HERMES_TOKEN` configurado, todas as rotas `/hermes/*` retornam `503 Service Unavailable` (fail-closed).

### Eventos no Cockpit

Todos os endpoints Hermes publicam eventos no `EventBus`, visíveis em tempo real no cockpit via SSE (`/api/v1/events`) ou WebSocket (`/api/v1/ws`). O source do evento é sempre `hermes:{subsystem}`.

Cross-reference: [HANDOFF_FASE8.md](../../docs/handoffs/HANDOFF_FASE8.md), [MODULE_DOCUMENTATION_STATUS.md](../../docs/MODULE_DOCUMENTATION_STATUS.md)

### Endpoints Fase 9 (Fases 1–4)

#### Approval Gates (`/api/v1/approvals/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/` | Listar requests pendentes |
| POST | `/` | Criar novo request de approval |
| GET | `/:id` | Detalhes de um request |
| POST | `/:id/decide` | Decidir (allow_once/allow_always/refuse) |
| GET | `/stats` | Estatísticas de approvals |

#### Egress Filter (`/api/v1/egress/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| POST | `/check` | Verificar conteúdo contra regras de egress |
| GET | `/rules` | Listar regras ativas |
| POST | `/rules` | Adicionar nova regra |
| DELETE | `/rules/:id` | Remover regra |

#### Audit Log (`/api/v1/audit/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/entries` | Listar entradas (com filtros) |
| GET | `/stats` | Estatísticas de auditoria |
| POST | `/export` | Exportar log (JSON/CSV) |

#### Morning Brief (`/api/v1/brief/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/latest` | Último brief gerado |
| POST | `/generate` | Gerar novo brief |
| GET | `/config` | Configuração atual |
| PUT | `/config` | Atualizar configuração |

#### Model Config (`/api/v1/models/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/` | Listar configs de modelo |
| GET | `/:bot_id` | Config de um bot específico |
| PUT | `/:bot_id` | Atualizar config de bot |
| GET | `/global` | Config global default |
| PUT | `/global` | Atualizar config global |

#### Computer View (`/api/v1/computer-view/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/sessions` | Listar sessões ativas |
| POST | `/sessions` | Criar nova sessão |
| DELETE | `/sessions/:id` | Encerrar sessão |
| POST | `/sessions/:id/control` | Solicitar controle (lease) |
| DELETE | `/sessions/:id/control` | Liberar controle |

#### Spaces (`/api/v1/spaces/*`)

| Método | Rota | Descrição |
|--------|------|-----------|
| GET | `/pages` | Listar páginas (hierárquicas) |
| GET | `/pages/:id` | Conteúdo de uma página |
| POST | `/pages` | Criar nova página |
| PUT | `/pages/:id` | Atualizar página |
| DELETE | `/pages/:id` | Deletar página |
| GET | `/pages/:id/outline` | Extrair outline (headings) |

Cross-reference: [HANDOFF_FASE9.md](../../docs/handoffs/HANDOFF_FASE9.md), [TRACKRECORD_FASE9.md](../../docs/TRACKRECORD_FASE9.md)

## Runbooks

- [`zylos-gateway.runbook.yaml`](../../docs/soar/runbooks/zylos-gateway.runbook.yaml)
- [`zylos-gateway.md`](../../runbooks/zylos-gateway.md)