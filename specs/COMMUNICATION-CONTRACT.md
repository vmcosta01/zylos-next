# Communication Contract — Zylos-Next v6.0 (Task 0.1.5)

## Context
Defines the inter-system communication contract for the 6 sovereign core crates. This document specifies the transport layer, message envelope, event taxonomy, and integration points with the existing Hermes ecosystem. All contracts are type-safe via Rust traits and validated at runtime via JSON Schema (see `specs/DESIGN.md`).

## Transport Layer

| Channel | Technology | Use Case | Latency Target |
|---------|-----------|----------|----------------|
| In-process | Tokio `mpsc` / `broadcast` | Crate-to-crate within same binary | <1µs |
| IPC | Named pipes / Unix sockets | Daemon ↔ Gateway, sidecar OCR | <5ms |
| HTTP REST | Axum 0.7 + Tower | External API, Hermes plugin bridge | <50ms |
| WebSocket | Axum WS upgrade | Cockpit streaming, AG-UI events | <10ms |
| SSE | Axum `Sse` response | One-way event fan-out to clients | <10ms |
| MCP stdio | Model Context Protocol | Mobile MCP server, tool discovery | <100ms |

## Message Envelope

All cross-system messages use a unified envelope for tracing, routing, and versioning:

```rust
/// Unified message envelope for all inter-system communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Envelope<T: Serialize> {
    /// Unique message identifier (UUIDv7 for time-sortability)
    pub id: String,
    /// Correlation ID for request/response tracing across systems
    pub correlation_id: String,
    /// Source system identifier (e.g., "bot-system", "memory-system")
    pub source: SystemId,
    /// Target system or broadcast topic
    pub target: Target,
    /// Semantic version of the payload schema
    pub version: semver::Version,
    /// UTC timestamp of creation
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Typed payload
    pub payload: T,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "value")]
pub enum Target {
    /// Direct message to a specific system
    System(SystemId),
    /// Broadcast to all subscribers of a topic
    Topic(String),
    /// Reply to a specific message's correlation_id
    ReplyTo(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum SystemId {
    BotSystem,
    MemorySystem,
    LiveComm,
    Verification,
    Sandbox,
    Skills,
    Gateway,
    Mobile,
}
```

## Event Taxonomy

Events are published via Tokio `broadcast` channels within the gateway process. Each system publishes to its own namespace; consumers subscribe selectively.

### BOT_SYSTEM Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `agent.spawned` | `AgentHandle` | LifecycleManager creates agent | LiveComm, Verification |
| `agent.state_changed` | `{id, old_state, new_state}` | Agent transitions state | LiveComm, Memory |
| `dag.started` | `DagId` | DAGExecutor begins workflow | LiveComm, Verification |
| `dag.completed` | `{id, result_summary}` | DAG finishes successfully | Memory, Skills, LiveComm |
| `dag.failed` | `{id, error, failed_node}` | DAG fails after retries | Verification, LiveComm |
| `message.sent` | `{from, to, topic}` | P2PMessenger delivers | LiveComm (audit trail) |

### MEMORY_SYSTEM Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `context.updated` | `{scope, node_count}` | ContextGraph mutation | BotSystem, Verification |
| `vector.indexed` | `{store_id, count}` | VectorStore insert batch | Skills |
| `rerank.completed` | `{query_hash, results_count, latency_ms}` | BriefAwareRerank finishes | BotSystem |
| `contradiction.detected` | `{nodes: [id], resolution}` | Contradiction detector fires | Verification, LiveComm |

### VERIFICATION Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `proof.validated` | `Proof` | Guardrail passes claim | BotSystem, Memory |
| `proof.refuted` | `{proof_id, reason}` | Guardrail or Panel rejects | BotSystem, LiveComm |
| `panel.verdict` | `PanelResult` | AdversarialPanel concludes | BotSystem, Memory |

### SANDBOX Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `sandbox.created` | `SandboxId` | WasmSandbox instantiated | BotSystem |
| `sandbox.snapshot` | `SnapshotMetadata` | Snapshot saved | BotSystem, Memory |
| `sandbox.terminated` | `{id, exit_code}` | Sandbox destroyed | BotSystem, LiveComm |

### LIVE_COMM Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `stream.started` | `{session_id, agent_ids}` | AG-UI stream opens | External clients |
| `stream.event` | `AgUiEvent` | Individual AG-UI frame | External clients |
| `cockpit.update` | `CockpitState` | Lazy-roster refresh | Leptos frontend |

### SKILLS Events
| Event | Payload Type | Trigger | Consumers |
|-------|-------------|---------|-----------|
| `skill.discovered` | `{name, version, schema_hash}` | index.json reload | BotSystem, Gateway |
| `eval.completed` | `{skill_name, score, verdict}` | Eval flywheel run | Memory, LiveComm |

## Hermes Integration Bridge

The gateway exposes a REST+WS adapter that translates between Hermes plugin protocols and internal Zylos envelopes:

```
Hermes Plugin → HTTP POST /api/v1/hermes/{system}/{action}
                ↓
         Gateway validates JSON Schema (specs/DESIGN.md)
                ↓
         Wraps in Envelope<T> with correlation_id
                ↓
         Routes to target system via Tokio channel
                ↓
         Response wrapped in Envelope<ResponseT>
                ↓
         HTTP 200/4xx/5xx with typed error body
```

### Supported Hermes Integration Points
| Hermes Plugin | Zylos System | Endpoint | Direction |
|--------------|-------------|----------|-----------|
| Kanban | BotSystem | `/api/v1/hermes/bot/dispatch-task` | Hermes → Zylos |
| Boardstate | LiveComm | `/api/v1/hermes/livecomm/widget-update` | Bidirectional |
| Crew | BotSystem | `/api/v1/hermes/bot/spawn-crew` | Hermes → Zylos |
| Operations | Verification | `/api/v1/hermes/verification/audit-request` | Hermes → Zylos |
| Cron | MemorySystem | `/api/v1/hermes/memory/schedule-consolidation` | Hermes → Zylos |
| Curator | Skills | `/api/v1/hermes/skills/register` | Hermes → Zylos |

## Error Contract

All errors follow RFC 9457 Problem Details format:

```json
{
  "type": "https://zylos.dev/errors/validation-failed",
  "title": "Schema Validation Failed",
  "status": 400,
  "detail": "Field 'agent_name' does not match pattern ^[a-z0-9][a-z0-9-]{0,63}$",
  "instance": "/api/v1/hermes/bot/dispatch-task",
  "correlation_id": "0191f8a7-...",
  "errors": [
    {"field": "agent_name", "reason": "pattern_mismatch", "value": "Invalid Name!"}
  ]
}
```

## Acceptance Criteria
- [ ] Envelope type compiles in `zylos-core` with serde + jsonschema validation
- [ ] All 6 systems publish events matching the taxonomy above
- [ ] Hermes bridge endpoint returns valid Problem Details on error
- [ ] Cross-system round-trip latency <50ms (in-process) measured via benchmark
- [ ] JSON Schema for every event payload registered in `specs/DESIGN.md`
- [ ] Correlation IDs propagate end-to-end across ≥3 system hops in integration test

## Out of Scope
- gRPC transport (deferred to Fase 7 if needed)
- Cross-machine distributed messaging (single-node for v1.0)
- Hermes plugin source code modification (adapter-only integration)
- Authentication/authorization protocol (covered in SECURITY.md)

## References
- [DESIGN.md](./DESIGN.md) — Typed data schemas
- [GOAL-CONTRACT.md](./GOAL-CONTRACT.md) — Verification contracts
- [ARCHITECTURE.md](../ARCHITECTURE.md) — System topology and data flow
- [Master Plan §1.6](../IMPLEMENTATION_PLAN.md) — Tech stack decisions