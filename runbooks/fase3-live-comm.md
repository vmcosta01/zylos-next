# Runbook: Fase 3 — LIVE_COMM_SYSTEM (AG-UI Streaming + Event Bus + Mobile MCP)

**Sistema:** LIVE_COMM_SYSTEM — Fase 3
**Crates:** `zylos-live-comm`, `zylos-mobile`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ COMPLETE — 20/20 testes, clippy limpo, todos módulos implementados (EventBus, AgUiEvent, SSE routes, CockpitApi, HermesBridge, Mobile MCP)
**Pré-requisito:** Fase 2 (MEMORY_SYSTEM) APPROVED ✅

---

## 🎯 Visão Geral (Todos os Níveis)

A Fase 3 implementa a camada de comunicação em tempo real do Zylos-Next: streaming AG-UI via SSE/WebSocket, event bus interno via Tokio broadcast channels, e servidor MCP para integração mobile. Este sistema é a ponte entre os 6 sistemas core soberanos e os clientes externos (Cockpit Leptos, apps mobile, Hermes plugins). Sem esta fase, nenhum agente pode comunicar resultados em tempo real.

### Componentes a Implementar
| Módulo | Crate | Responsabilidade |
|--------|-------|-----------------|
| `AgUiStream` | zylos-live-comm | Streaming SSE/WS de eventos AG-UI para cockpit |
| `EventBus` | zylos-live-comm | Broadcast interno via Tokio channels (fan-out) |
| `MobileMcpServer` | zylos-mobile | Servidor MCP stdio para apps mobile |
| `CockpitApi` | zylos-live-comm | Endpoints REST para Leptos WASM frontend |
| `HermesBridge` | zylos-live-comm | Adapter REST↔Envelope para plugins Hermes |

### Métricas de Sucesso
| Métrica | Target | Como Medir |
|---------|--------|-----------|
| AG-UI frame latency | <10ms p99 | Benchmark com 100 events/s |
| EventBus fan-out (50 subscribers) | <5ms | Benchmark broadcast |
| MCP tool call round-trip | <100ms | Integration test |
| WebSocket reconnection | <500ms | Chaos test (kill/restart) |
| Memory per stream | <2MB | Profiling com 1000 concurrent streams |

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Definir contrato de eventos AG-UI compatível com spec oficial
- Garantir que EventBus não perde mensagens sob carga (backpressure strategy)
- Validar isolamento de streams (um cliente lento não bloqueia outros)
- Definir schema MCP para mobile (tool discovery + execution)
- Auditar segurança do HermesBridge (validação de envelope antes de routing)

### Procedimentos

#### S3.1: Contrato AG-UI
```rust
use serde::{Serialize, Deserialize};

/// Evento AG-UI conforme spec oficial
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum AgUiEvent {
    TextMessageStart { message_id: String },
    TextMessageContent { message_id: String, delta: String },
    TextMessageEnd { message_id: String },
    ToolCallStart { tool_call_id: String, tool_name: String },
    ToolCallResult { tool_call_id: String, result: serde_json::Value },
    StateSnapshot { state: serde_json::Value },
    Error { code: String, message: String },
}
```
**Critério Senior:** Schema compatível com AG-UI spec v0.1. Serde round-trip testado com fixtures oficiais.

#### S3.2: Backpressure no EventBus
```rust
use tokio::sync::broadcast;

// Canal com buffer limitado — receivers lentos recebem RecvError::Lagged
let (tx, _rx) = broadcast::channel::<AgUiEvent>(1024);

// Strategy: lagged receivers recebem snapshot de estado + replay dos últimos N eventos
// Nunca bloqueia o sender (agentes não travam por causa de cliente lento)
```
**Critério Senior:** Sender nunca bloqueia. Lagged receivers recuperam via snapshot + replay. Buffer size configurável via env var.

#### S3.3: Isolamento de Streams
```rust
// Cada stream tem seu próprio JoinHandle + CancellationToken
// Timeout de inatividade (30s) fecha stream automaticamente
// Max concurrent streams por usuário: 10 (configurável)
```
**Critério Senior:** Um stream travado não afeta outros. Resource cleanup garantido via Drop + CancellationToken.

#### S3.4: MCP Server Design
```rust
// MCP stdio server com tool discovery dinâmica
// Tools expostas: list_agents, get_agent_state, send_message, query_memory
// Cada tool valida capability token antes de executar
// Schema JSON gerado automaticamente via schemars
```
**Critério Senior:** Tool discovery <100ms. Capability enforcement em todas as tools. Schema válido contra MCP spec.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| Tokio broadcast (não mpsc) | Fan-out nativo para múltiplos subscribers | mpsc (apenas 1 receiver) |
| SSE como padrão (WS como upgrade) | Compatibilidade com proxies/firewalls, auto-reconnect nativo | WS-only (complexidade de reconnect manual) |
| AG-UI spec v0.1 | Padrão aberto, compatível com ecossistema | Protocolo customizado (lock-in) |
| MCP stdio (não HTTP) | Simplicidade para mobile, zero-config networking | MCP HTTP (overhead desnecessário para local) |
| Axum extractors para validação | Type-safe request parsing, error handling unificado | Manual JSON parsing (error-prone) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Scaffold dos crates `zylos-live-comm` e `zylos-mobile`
- Implementar módulos conforme contratos definidos pelo Senior
- Escrever testes unitários e de integração
- Configurar benchmarks de performance
- Documentar APIs públicas com rustdoc

### Procedimentos

#### P3.1: Scaffold dos Crates
```bash
cd C:/Users/Paulo/zylos-next

# Criar crate live-comm
cargo new --lib crates/zylos-live-comm
# Adicionar ao workspace Cargo.toml [workspace.members]

# Criar crate mobile
cargo new --lib crates/zylos-mobile
# Adicionar ao workspace Cargo.toml [workspace.members]
```

#### P3.2: Dependências (Cargo.toml)
```toml
[dependencies]
axum = { version = "0.7", features = ["ws", "json"] }
tokio = { version = "1.40", features = ["full"] }
tokio-stream = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
tracing = "0.1"
uuid = { version = "1", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }

[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }
tokio-test = "0.4"
```

#### P3.3: Implementar EventBus
```rust
// crates/zylos-live-comm/src/event_bus.rs
use tokio::sync::broadcast;
use crate::AgUiEvent;

pub struct EventBus {
    tx: broadcast::Sender<AgUiEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgUiEvent> {
        self.tx.subscribe()
    }

    pub fn publish(&self, event: AgUiEvent) -> Result<usize, broadcast::SendError<AgUiEvent>> {
        self.tx.send(event)
    }
}
```

#### P3.4: Implementar AG-UI SSE Endpoint
```rust
// crates/zylos-live-comm/src/routes/agui.rs
use axum::response::sse::{Event, Sse};
use tokio_stream::wrappers::BroadcastStream;
use std::convert::Infallible;

pub async fn agui_stream(
    Extension(bus): Extension<EventBus>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => Some(Ok(Event::default().json_data(event).unwrap())),
            Err(BroadcastStreamRecvError::Lagged(n)) => {
                tracing::warn!("Client lagged by {} events", n);
                None // Skip lagged, client will get snapshot on next state update
            }
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
```

#### P3.5: Testes de Integração
```bash
cargo test -p zylos-live-comm --lib
cargo test -p zylos-mobile --lib
```
**Esperado:** Todos os testes passando. Cobertura mínima: EventBus publish/subscribe/lagged, SSE stream lifecycle, MCP tool discovery.

#### P3.6: Benchmarks
```bash
cargo bench -p zylos-live-comm -- event_bus_broadcast_50_subscribers
cargo bench -p zylos-live-comm -- agui_frame_latency_100_events_per_sec
```
**Critério Pleno:** Broadcast <5ms (50 subs), frame latency <10ms p99.

### Checklist de Validação
- [ ] `cargo check --workspace` verde
- [ ] `cargo clippy -D warnings` limpo para ambos crates
- [ ] `cargo test -p zylos-live-comm --lib` passa todos os testes
- [ ] `cargo test -p zylos-mobile --lib` passa todos os testes
- [ ] EventBus broadcast <5ms com 50 subscribers (benchmark)
- [ ] AG-UI SSE stream entrega eventos em ordem
- [ ] MCP tool discovery retorna schema válido
- [ ] Lagged receiver recupera via snapshot (integration test)
- [ ] Stream timeout fecha conexão após 30s inatividade

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Rodar comandos de scaffold e build
- Executar testes e reportar resultados
- Seguir exemplos sem modificar lógica
- Reportar erros com output completo

### Procedimentos Passo-a-Passo

#### J3.1: Verificar Workspace
```bash
cd C:/Users/Paulo/zylos-next
cargo check --workspace
```
✅ `Finished` → prossiga
❌ Erro → copie output completo e reporte ao Pleno

#### J3.2: Rodar Testes Live-Comm
```bash
cargo test -p zylos-live-comm --lib 2>&1 | tee test-live-comm.log
```
✅ `test result: ok. X passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro

#### J3.3: Rodar Testes Mobile
```bash
cargo test -p zylos-mobile --lib 2>&1 | tee test-mobile.log
```
✅ `test result: ok. X passed` → sucesso
❌ Falha → anote nome do teste + mensagem de erro

#### J3.4: Verificar Exemplo SSE
Se existir `crates/zylos-live-comm/examples/sse_demo.rs`:
```bash
cargo run -p zylos-live-comm --example sse_demo
```
Abra `http://localhost:3000/events` no navegador. Deve ver eventos SSE chegando.
Se não existir exemplo, reporte ao Pleno como gap de documentação.

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **AG-UI** | Protocolo aberto para streaming de eventos de agentes IA (texto, tool calls, state snapshots) |
| **SSE** | Server-Sent Events — protocolo HTTP unidirecional para streaming, auto-reconnect nativo |
| **EventBus** | Canal de broadcast interno que distribui eventos para múltiplos subscribers simultaneamente |
| **Backpressure** | Mecanismo que previne que produtores rápidos sobrecarreguem consumidores lentos |
| **Lagged Receiver** | Subscriber que perdeu eventos porque o buffer encheu; recupera via snapshot de estado |
| **MCP** | Model Context Protocol — padrão para tool discovery e execução via stdio ou HTTP |
| **CancellationToken** | Token do Tokio para cancelamento cooperativo de tasks assíncronas |

### Quando Escalar
- Build falha com dependência missing → **Pleno** (Cargo.toml incompleto)
- Teste falha intermitentemente → **Pleno** (possível race condition em broadcast)
- SSE stream desconecta sem motivo → **Senior** (keepalive ou timeout mal configurado)
- MCP tool discovery retorna schema inválido → **Senior** (schemars derive incorreto)
- Broadcast latency >5ms → **Senior** (performance regression, investigar alocações)

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` linhas 975-1100 (Fase 3 LIVE_COMM)
- **Pré-requisito:** `runbooks/fase2-memory-system.md` (APPROVED)
- **Próxima Fase:** `runbooks/fase4-sandbox-isolation.md`
- **Contratos:** `specs/COMMUNICATION-CONTRACT.md` (Envelope, Event Taxonomy)
- **Segurança:** `specs/SECURITY-MODEL.md` (Capability tokens para MCP tools)
- **Arquitetura:** `ARCHITECTURE.md` §3 (LIVE_COMMUNICATION)
- **Manifesto:** Conceitos #8 (Real-Time First), #12 (Protocol Compliance), #17 (Continuous Update)