# Zylos-Next

Plataforma de orquestração multi-agente nativa em Rust para o ecossistema Hermes. Substitui a camada de orquestração TypeScript do Hermes v1 com segurança de memória, verificação em 5 níveis, aprovação HITL com hash de equivalência, triagem em dois estágios, monitoramento AIOps via Sentinel e isolamento por sandbox WASM/Docker — tudo com ~30MB de RAM por agente em idle.

## Quick Start (< 5 minutos)

### Pré-requisitos

- Rust 1.82+ (`rustup update stable`)
- Docker 24+ (para sandbox isolation)
- 4GB RAM mínimo

### Clone e Build

```bash
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next
cargo build --release -p zylos-gateway
```

### Configuração Mínima

```bash
cat > config.toml << 'EOF'
[server]
listen_addr = "0.0.0.0:3000"
shutdown_timeout_secs = 5

[soar]
audit_key = "change-me-in-production"
event_bus_capacity = 256

[skills]
index_path = "./skills"
EOF
```

### Executar

```bash
./target/release/zylos-gateway --config config.toml
```

### Verificar Saúde

```bash
# Liveness
curl http://localhost:3000/gateway/health

# Readiness (subsystems inicializados)
curl http://localhost:3000/gateway/ready

# Kubernetes liveness probe
curl http://localhost:3000/gateway/live
```

Resposta esperada: `{"status":"ok","subsystems":{"memory":"ready","sandbox":"ready","skills":"ready"}}`

### Streaming de Eventos (SSE)

```bash
curl -N -H "Accept: text/event-stream" \
     -H "Authorization: Bearer <jwt>" \
     http://localhost:3000/api/v1/events
```

Eventos são enviados no formato AG-UI Protocol com campos `decided_by`, `equivalence_hash` e `initiator`.

### Submeter Tarefa via API

```bash
curl -X POST http://localhost:3000/api/v1/tasks \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <jwt>" \
  -d '{
    "agent_id": "code-reviewer",
    "payload": {"action": "review_pr", "repo": "org/repo", "pr_number": 42},
    "priority": "normal"
  }'
```

### Aprovar Ação Pendente

```bash
# Listar aprovações pendentes
curl http://localhost:3000/api/v1/approvals/pending \
  -H "Authorization: Bearer <jwt>"

# Aprovar por equivalence hash
curl -X POST http://localhost:3000/api/v1/approvals/resolve \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <jwt>" \
  -d '{"equivalence_hash": "sha256:abc123...", "decision": "approved"}'
```

## Arquitetura

Zylos-Next é organizado em camadas dentro de um workspace Cargo com 23 crates:

| Camada | Crates | Função |
|--------|--------|--------|
| **Core** | `zylos-core` | Tipos fundamentais, traits, erros compartilhados |
| **Comunicação** | `zylos-live-comm` | AG-UI, EventBus, SSE, approval, triage, sentinel, cockpit |
| **Memória** | `zylos-memory-system` | Consolidação, contradições, decay temporal, Ground levels |
| **Verificação** | `zylos-proof`, `zylos-resolver`, `zylos-panel`, `zylos-router`, `zylos-cerebelo`, `zylos-learn`, `zylos-verification` | Provas em 5 níveis, painéis adversariais, roteamento |
| **Sandbox** | `zylos-sandbox`, `zylos-docker`, `zylos-browser` | WASM (wasmtime), Docker (bollard), browser anti-detect |
| **Integração** | `zylos-soar`, `zylos-hitl`, `zylos-triage`, `zylos-sentinel`, `zylos-skills`, `zylos-task-durability` | Playbooks, HITL, triagem 2-stage, AIOps, registry |
| **Apps** | `zylos-gateway`, `zylos-cockpit`, `zylos-cli`, `zylos-daemon` | Servidor HTTP/SSE, dashboard Leptos, CLI, daemon |
| **Extras** | `zylos-bot-system`, `zylos-mobile`, `zylos-vlm-ocr`, `zylos-search-mcp` | Bots, mobile, OCR/VLM, busca paralela |

Para diagramas C4 completos, veja [ARCHITECTURE.md](ARCHITECTURE.md).

## Crate Map

### Core & Types
- **zylos-core**: Tipos compartilhados (`Ground`, `SafetyTier`, `Decision`), traits, erros. Base para todos os outros crates.

### Comunicação & Eventos
- **zylos-live-comm**: Hub central. AG-UI events, EventBus pub/sub, auth HMAC, cache HTTP, approval cards com CAS claim, task durability com SQL leases, outcome tri-state, triage 2-stage, equivalence approval, sentinel bridge, cockpit SSE, SOAR bridge, security, rate limiting, OpenAPI, skills API, hermes bridge.

### Memória & Aprendizado
- **zylos-memory-system**: Persistência SQLite, consolidação noturna, detecção de contradições, decay temporal, Ground level enforcement.
- **zylos-learn**: Auto-aprendizado entre sessões, análise de logs para padrões recorrentes.

### Verificação & Provas
- **zylos-proof**: Construção de provas em 5 níveis (Assumed → Observed → Confirmed → Verified → Proven).
- **zylos-resolver**: Resolução de claims contra evidências disponíveis.
- **zylos-panel**: Painéis adversariais para validação de outputs críticos.
- **zylos-router**: Roteamento de requests para verificadores apropriados.
- **zylos-cerebelo**: Executor de DAGs longos com recovery estruturado.
- **zylos-verification**: Orquestração do pipeline de verificação completo.

### Sandbox & Isolamento
- **zylos-sandbox**: Runtime WASM via wasmtime + fallback Docker via bollard.
- **zylos-docker**: Integração Docker para containers de sandbox.
- **zylos-browser**: Browser automation com anti-detection.

### Integração & Operações
- **zylos-soar**: Playbooks, incidentes, lifecycle management.
- **zylos-hitl**: Human-in-the-loop com equivalence hashing e 4-tier decisions.
- **zylos-triage**: Classificação 2-stage (rules safe-negative + model fallback).
- **zylos-sentinel**: AIOps com detectores pure-function, PII stripping, rate limiting.
- **zylos-skills**: Registry de 338+ skills com eval harness.
- **zylos-task-durability**: SQL leases, maintenance, circuit breaker.

### Aplicações
- **zylos-gateway**: Servidor Axum HTTP/SSE, health probes, config hot-reload, CORS.
- **zylos-cockpit**: Dashboard Leptos 0.7 WASM com SSR nativo.
- **zylos-cli**: Interface de linha de comando.
- **zylos-daemon**: Background service.

### Extras
- **zylos-bot-system**: Gerenciamento de bots.
- **zylos-mobile**: Suporte mobile.
- **zylos-vlm-ocr**: OCR e visão via VLM.
- **zylos-search-mcp**: Busca paralela via MCP.

## Configuração

Arquivo TOML com override por variáveis de ambiente:

```toml
[server]
listen_addr = "0.0.0.0:3000"
shutdown_timeout_secs = 5

[soar]
audit_key = "change-me"          # ZYLOS_SOAR_AUDIT_KEY
event_bus_capacity = 256          # ZYLOS_EVENT_BUS_CAPACITY

[skills]
index_path = "./skills"           # ZYLOS_SKILLS_PATH

[memory]
consolidation_cron = "0 2 * * *"  # Consolidação diária às 2h
decay_half_life_days = 30

[sandbox]
wasm_enabled = true
docker_enabled = true
browser_anti_detect = false

[triage]
model_endpoint = "https://api.openai.com/v1"
rules_only = false                # true = desabilita modelo, só regras

[sentinel]
incident_rate_limit = 5           # por hora por assinatura
approval_rate_limit = 100         # por hora
```

Variáveis de ambiente têm precedência sobre o arquivo TOML. Prefixo: `ZYLOS_`.

## Documentação

- [Architecture](ARCHITECTURE.md) — Diagramas C4, decisões de design, data flow
- [Deployment](DEPLOYMENT.md) — Docker, Kubernetes, bare metal, production checklist
- [Migration](MIGRATION.md) — Guia Hermes v1 → Zylos-Next passo-a-passo
- [FAQ](FAQ.md) — Top 20 perguntas frequentes
- [Contributing](CONTRIBUTING.md) — Guidelines de desenvolvimento e review
- [API Reference](api/gateway-openapi.yaml) — OpenAPI 3.0 completa

## Runbooks & Playbooks

- **Runbooks**: Procedimentos operacionais em `runbooks/`
- **Playbooks**: Workflows SOAR em `playbooks/`:
  - `security-incident-response.yaml` — Resposta a incidentes de segurança
  - `vendor-dependency-pivot.yaml` — Pivot de dependência de vendor
  - `false-positive-suppression.yaml` — Supressão de falsos positivos

## Licença

MIT OR Apache-2.0. Veja [LICENSE](../LICENSE).

## Suporte

- **Issues**: GitHub Issues
- **Security**: security@nousresearch.com (não abra issue pública)
- **Community**: Discord #zylos-next
- **Docs**: Este diretório