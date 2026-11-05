# zylos-bot-system

Runtime de orquestração de agentes com DAG executor, lifecycle management, P2P messaging e CerebeloExecutor com recovery ladder. Inclui WorkflowDSL YAML para definição declarativa de workflows.

## Status vs Master Plan (Fase 2)

| Task | Descrição | Status |
|------|-----------|--------|
| 2.1.1 | DAG Runtime (petgraph + tokio) | ✅ Implementado |
| 2.1.2 | Kanban integration | ❌ Ausente |
| 2.1.3 | WorkflowDSL | ✅ Implementado |
| 2.2.1 | CerebeloExecutor | ✅ Implementado |
| 2.2.2 | Lifecycle gating | ✅ Implementado |
| 2.3 | P2P federation | ⚠️ Parcial — P2PMessenger local implementado, networking real ausente |

## Features Implementadas

- **DagExecutor**: petgraph toposort + cycle detection, conditional edges (CEL-like evaluator), semaphore backpressure, checkpoint/restore SQLite, retry/backoff, artifact passing, cancellation via CancellationToken
- **LifecycleManager**: DashMap + JoinSet agent registry, spawn/kill/shutdown_all com timeout, state broadcasting
- **P2PMessenger**: topic-based pub/sub in-process, dead letter queue FIFO, TTL expiration, publish_with_ack
- **CerebeloExecutor**: multi-backend (Local/Docker/Wasm stubs), ExecutionAuthority task_local, VerifySpec command execution, HeartbeatMonitor (60s/180s), IdempotencyTracker, RecoveryLadder (Retry→Decompose→Delegate→EscalateHuman), CerebeloWriteGate integration
- **WorkflowDSL**: YAML parsing → DagDefinition, validation (unique IDs, edge refs, DFS cycle detection, conditions), compile_spec roundtrip
- **ArtifactProvenance**: node_id/run_id/timestamp/verification_status/parent_artifacts tracking

## API Pública

```rust
// DAG execution
pub struct DagExecutor { ... }
impl DagExecutor {
    pub fn build_graph(def: &DagDefinition) -> Result<Self>;
    pub async fn execute(&self, ctx: NodeContext) -> DagProgress;
    pub fn to_dot(&self) -> String;
}

// Lifecycle
pub struct LifecycleManager { ... }
impl LifecycleManager {
    pub async fn spawn(&self, id, future) -> AgentHandle;
    pub async fn kill_agent(&self, id) -> Result<()>;
    pub async fn shutdown_all(&self, timeout_ms: u64);
}

// P2P
pub struct P2PMessenger { ... }
impl P2PMessenger {
    pub fn subscribe(&self, topic) -> Receiver<Message>;
    pub fn publish(&self, topic, payload);
    pub async fn publish_with_ack(&self, topic, payload, timeout) -> Result<()>;
}

// Cerebelo
pub struct CerebeloExecutor { ... }
impl CerebeloExecutor {
    pub async fn execute(&self, dag: DagDefinition) -> Result<DagProgress>;
}

// Workflowpub fn parse_workflow(yaml: &str) -> Result<DagDefinition>;
pub fn validate_workflow(spec: &WorkflowSpec) -> Result<(), Vec<ValidationError>>;
```

## Gaps Críticos

1. **Kanban integration (Task 2.1.2)**: completamente ausente — sem boardstate connector ou kanban sync
2. **P2P networking real**: P2PMessenger é in-process apenas; sem TCP/QUIC/WebRTC transport layer para federação entre nós
3. **Docker/Wasm backends**: stubs retornando OutcomeUncertainty — implementação real pendente
4. **chrono_now_stub()**: placeholder em p2p.rs indicando que timestamp real não está wired

## Exemplo de Uso

```rust
use zylos_bot_system::{parse_workflow, DagExecutor};

let yaml = r#"
name: deploy-pipeline
version: "1.0"
nodes:
  - id: build
    agent: builder
    task: cargo build --release
  - id: test
    agent: tester
    task: cargo test
edges:
  - from: build
    to: test
"#;
let dag = parse_workflow(yaml)?;
let executor = DagExecutor::build_graph(&dag)?;
let progress = executor.execute(NodeContext::default()).await;
```