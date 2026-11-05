# Architecture — Zylos-Next v6.0

## Visão Geral da Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                    zylos-gateway (Axum Server)                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐              │
│  │  REST API   │ │  WebSocket  │ │   AG-UI     │              │
│  │   (JSON)    │ │  (SSE/WS)   │ │ (Streaming) │              │
│  └──────┬──────┘ └──────┬──────┘ └──────┬──────┘              │
│         └───────────────┼───────────────┘                      │
│                         ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │              Event Bus (Tokio Broadcast)                 │  │
│  └─────────────────────┬───────────────────────────────────┘  │
│                        │                                       │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│  │   BOT   │ │ MEMORY  │ │ VERIF.  │ │ SANDBOX │ │ SKILLS │ │
│  │ SYSTEM  │ │ SYSTEM  │ │ SYSTEM  │ │ SYSTEM  │ │SYSTEM  │ │
│  └────┬────┘ └────┬────┘ └────┬────┘ └────┬────┘ └───┬────┘ │
│       │           │           │           │          │       │
│  ┌────▼────┐ ┌────▼────┐ ┌────▼────┐ ┌────▼────┐ ┌───▼───┐ │
│  │SQLite   │ │SQLite   │ │SQLite   │ │WASMtime │ │index  │ │
│  │WAL+     │ │WAL+     │ │WAL+     │ │+Docker  │ │.json  │ │
│  │petgraph │ │VectorSt.│ │Ground   │ │+Browser │ │+Schema│ │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └───────┘ │
└─────────────────────────────────────────────────────────────────┘
         ▲              ▲              ▲
         │              │              │
   ┌─────┴─────┐ ┌─────┴─────┐ ┌─────┴─────┐
   │zylos-cli  │ │zylos-     │ │Leptos     │
   │(Admin)    │ │daemon     │ │WASM       │
   └───────────┘ │(Shared)   │ │(Cockpit)  │
                 └───────────┘ └───────────┘
```

## 6 Sistemas Core Soberanos

### 1. BOT_SYSTEM (zylos-bot-system)
**Responsabilidade:** Lifecycle de agentes, spawning, DAG execution, P2P messaging type-safe, shadow-git, recovery-ladder.

**Componentes:**
- `LifecycleManager`: Spawn/kill/suspend agents via Tokio JoinSet + CancellationToken
- `DAGExecutor`: Executa workflows como DAGs com rollback automático (petgraph)
- `P2PMessenger`: Mensagens type-safe entre agentes via Tokio channels
- `ShadowGit`: Versionamento de estado de workflow para rollback
- `RecoveryLadder`: Escalabilidade de recovery (local → shadow → checkpoint)

**Fluxo de dados:**
```
Request → DAGExecutor → [Agent1 → Agent2 → Agent3] → Result
                ↓
          ShadowGit (commit state)
                ↓
          RecoveryLadder (on failure)
```

### 2. MEMORY_SYSTEM (zylos-memory-system)
**Responsabilidade:** Context Graph SQLite WAL, VectorStore trait, brief-aware rerank, 4-scope isolation.

**Componentes:**
- `ContextGraph`: petgraph 0.6 sobre SQLite WAL (padrão ctxgraph F1=0.800)
- `VectorStore` trait: Interface agnóstica (Rig-inspired) com adapters para Qdrant/LanceDB
- `BriefAwareRerank`: Reranking baseado em brief do agente
- `ScopeIsolation`: 4 escopos (global, team, agent, task)

**Fluxo de dados:**
```
Agent Query → ScopeIsolation → BriefAwareRerank → ContextGraph
                                    ↓
                              VectorStore (embeddings)
```

### 3. LIVE_COMMUNICATION (zylos-live-comm + zylos-mobile)
**Responsabilidade:** AG-UI streaming, event bus SSE/WS, Mobile MCP server, cockpit web.

**Componentes:**
- `AGUIStream`: Streaming de eventos via SSE (Axum)
- `EventBus`: Broadcast via Tokio channels
- `MobileMCP`: MCP server para apps mobile
- `CockpitAPI`: Endpoints para Leptos WASM frontend

### 4. VERIFICATION_PROOF (zylos-verification)
**Responsabilidade:** Ground enum, Panel adversarial, Guardrail <1ms, Tree-of-Thoughts.

**Componentes:**
- `Ground` enum: `Verified | Refuted | Pending | Uncertain`
- `AdversarialPanel`: Juiz ≠ gerador (múltiplos LLMs)
- `Guardrail`: Validação <1ms de claims
- `TreeOfThoughts`: Exploração paralela de hipóteses

### 5. SANDBOX_ISOLATION (zylos-sandbox)
**Responsabilidade:** WASM snapshot/resume, Docker per-agent, browser anti-detect, OCR sidecar.

**Componentes:**
- `WasmSandbox`: wasmtime 25.0 com snapshot/resume
- `DockerIsolation`: Docker por agente (network/filesystem)
- `BrowserAntiDetect`: Fingerprint engine para scraping
- `OCRSidecar`: dots.mocr 3B via vLLM

### 6. SKILLS_REGISTRY (zylos-skills)
**Responsabilidade:** index.json discovery <100ms, JSON Schema compilado, eval flywheel.

**Componentes:**
- `IndexRegistry`: Discovery via index.json
- `SchemaValidator`: jsonschema 0.18 compilado
- `EvalFlywheel`: LLM-as-judge para validação
- `ExportPipeline`: Export para .cursor/.claude

## Tech Stack

| Componente | Tecnologia | Justificativa |
|------------|-----------|---------------|
| Language | Rust 1.82+ | jcode: 27.8MB RAM, 14ms TTFD |
| Async Runtime | Tokio 1.40 | JoinSet + CancellationToken nativo |
| Graph Engine | petgraph 0.6 + SQLite WAL | ctxgraph F1=0.800 |
| Vector Store | Rig VectorStore trait | Interface agnóstica |
| Web Framework | Axum 0.7 + Tower | REST+WS nativo |
| WASM Sandbox | wasmtime 25.0 | Snapshot/resume suportado |
| Serialization | serde + jsonschema 0.18 | Goal-Contract typed |
| Frontend | Leptos 0.7 (WASM) | SSR nativo, sinais |
| Protocols | A2A + MCP + AG-UI | Padrões abertos |
| Observability | tracing + Prometheus | OpenTelemetry nativo |

## Fluxo de Dados Completo

```
1. User Request (Cockpit/CLI/API)
   ↓
2. zylos-gateway (Axum) valida autenticação
   ↓
3. BOT_SYSTEM cria DAG de execução
   ↓
4. MEMORY_SYSTEM recupera contexto relevante (4 scopes)
   ↓
5. SANDBOX_ISOLATION prepara ambiente (WASM/Docker)
   ↓
6. Agentes executam em paralelo (JoinSet)
   ↓
7. VERIFICATION_PROOF valida resultados (Guardrail + Panel)
   ↓
8. SKILLS_REGISTRY registra aprendizados
   ↓
9. LIVE_COMMUNICATION stream result para cockpit
   ↓
10. ShadowGit commit estado final
```

## Decisões Arquiteturais (ADRs)

### ADR-001: Rust como linguagem única
- **Contexto:** Necessidade de <30MB RAM por agente e safety guarantees
- **Decisão:** Rust 1.82+ para todos os sistemas core
- **Consequências:** Curva de aprendizado maior, mas performance 5x melhor que Python

### ADR-002: SQLite WAL como storage principal
- **Contexto:** Necessidade de ACID + portabilidade + zero-config
- **Decisão:** SQLite WAL via rusqlite 0.32
- **Consequências:** Limite de writers concorrentes, mas suficiente para 50+ agentes

### ADR-003: WASM para isolamento de tools
- **Contexto:** Tools de terceiros não podem comprometer sistema
- **Decisão:** wasmtime 25.0 com snapshot/resume
- **Consequências:** Overhead de ~10ms, mas isolamento forte garantido

### ADR-004: Leptos para cockpit web
- **Contexto:** Frontend WASM com SSR nativo
- **Decisão:** Leptos 0.7 ao invés de Yew
- **Consequências:** 20% mais rápido, sinais ao invés de hooks

## Escalabilidade
- **Horizontal:** Múltiplas instâncias de zylos-gateway com shared SQLite
- **Vertical:** Daemon compartilhado (jcode-inspired) para 50+ agentes
- **Lazy loading:** Agentes carregados sob demanda (não todos no boot)

## Segurança
- **Camada Zero:** SECURITY.md define regras ANTES do código
- **Sandboxing:** WASM obrigatório para tools, Docker por agente
- **Authorization:** Verificada server-side em toda request
- **Secrets:** Environment variables apenas, nunca hardcoded

## Observabilidade
- **Tracing:** tracing crate com OpenTelemetry export
- **Métricas:** Prometheus endpoint nativo
- **Logs:** JSON estruturado com correlation IDs
- **Cockpit:** Métricas em tempo real via WebSocket

## Referências
- [PRD](./PRD.md)
- [Design System](./DESIGN_SYSTEM.md)
- [Manifesto Conceitos-Desenvolvimento-IA](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
