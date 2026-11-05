# Fase 2: Bot System — Runbook Zylos-Next

## Visão Geral

O BOT_SYSTEM (`zylos-bot-system`) implementa lifecycle management de agentes, execução DAG de workflows, messaging P2P type-safe, workflow DSL parsing, integração com Cerebelo executor, e bridge para Kanban boardstate.

**Módulos principais** (de `crates/zylos-bot-system/src/lib.rs`):
- `lifecycle` — LifecycleManager com AgentHandle, AgentId, AgentState, spawn/kill/suspend via Tokio JoinSet + CancellationToken
- `dag` — DagExecutor com DagNode, DagEdge, DagId, NodeStatus para execução topológica com rollback
- `p2p` — P2PMessenger com Message, MessageId, Topic para messaging type-safe entre agentes
- `p2p_network` — Networking P2P (feature-gated `p2p-network`)
- `workflow_dsl` — parse_workflow, validate_workflow, WorkflowSpec
- `cerebelo` — Integração com Cerebelo DAG executor
- `kanban_bridge` — Bridge para Kanban boardstate

**Critério de Saída:** Workflows 100+ nós com rollback automático; lifecycle manager suporta spawn/kill/suspend graceful.

## Pré-requisitos

- Fase 0 completa
- Dependências: `tokio 1.40` (features: full), `serde`, `uuid`, `petgraph` (para DAG), `tracing`
- Feature opcional: `p2p-network` para networking distribuído
- `zylos-core` disponível para tipos compartilhados

### Setup

```bash
cargo build -p zylos-bot-system
cargo test -p zylos-bot-system

# Com networking P2P
cargo build -p zylos-bot-system --features p2p-network
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Lifecycle via JoinSet + CancellationToken**: `LifecycleManager` gerencia spawn/kill/suspend de agentes usando `JoinSet` para tracking concorrente e `CancellationToken` para graceful shutdown. Nunca fire-and-forget. `AgentHandle` encapsula o handle + token + state.

2. **DAG Executor Topológico**: `DagExecutor` executa workflows como DAGs com ordenação topológica. `DagNode` representa unidades de trabalho, `DagEdge` dependências. `NodeStatus` trackea estado de cada nó. Rollback automático em falha (futuro).

3. **P2P Messaging Type-Safe**: `P2PMessenger` envia `Message` com `MessageId` e `Topic` entre agentes. Type-safe via generics. Suporta pub/sub por topic.

4. **Workflow DSL**: `parse_workflow()` e `validate_workflow()` permitem definir workflows declarativamente via `WorkflowSpec`. Validação em compile-time quando possível.

5. **Cerebelo Integration**: `cerebelo` module integra com o Cerebelo executor para DAGs longos com recovery estruturado automático.

6. **Kanban Bridge**: `kanban_bridge` conecta bot lifecycle ao Kanban boardstate para visualização de progresso.

### Extension Points

- **Novos AgentState variants**: Adicione ao enum em `lifecycle.rs` para estados customizados
- **Custom DAG node types**: Estenda `DagNode` para payloads específicos de domínio
- **Novos message topics**: Defina topics em `p2p.rs` para categorização
- **Workflow DSL extensions**: Adicione constructs ao `WorkflowSpec` parser
- **Novos lifecycle hooks**: Implemente callbacks em `LifecycleManager` para eventos de state transition

### Performance Tuning

- Lifecycle manager deve suportar 50+ agentes concurrentes sem lag
- DAG execution de 100+ nós com ordering topológica eficiente
- P2P messaging latency <10ms para mensagens locais
- Use `JoinSet::join_next()` para processar resultados à medida que completam
- CancellationToken propagation evita polling desnecessário

## Para Dev Pleno

### Lifecycle Manager

```rust
use zylos_bot_system::{LifecycleManager, AgentHandle, AgentState};
use tokio_util::sync::CancellationToken;

let manager = LifecycleManager::new();
let root_token = CancellationToken::new();

// Spawn agent
let handle: AgentHandle = manager.spawn("agent_001", root_token.child_token(), async move {
    // Agent loop
    loop {
        // work...
    }
}).await?;

// Check state
let state: AgentState = handle.state();

// Graceful shutdown
handle.cancel(); // Signals cancellation
manager.wait_for_shutdown(handle.id()).await?;
```

### DAG Execution

```rust
use zylos_bot_system::{DagExecutor, DagNode, DagEdge, DagId};

let mut executor = DagExecutor::new();

// Define nodes
executor.add_node(DagNode {
    id: DagId::new("fetch_data"),
    payload: serde_json::json!({"url": "https://api.example.com"}),
    ..Default::default()
});

executor.add_node(DagNode {
    id: DagId::new("process_data"),
    payload: serde_json::json!({"transform": "normalize"}),
    ..Default::default()
});

// Define dependency
executor.add_edge(DagEdge {
    from: DagId::new("fetch_data"),
    to: DagId::new("process_data"),
});

// Execute topologically
let results = executor.execute().await?;
```

### P2P Messaging

```rust
use zylos_bot_system::{P2PMessenger, Message, Topic};

let messenger = P2PMessenger::new();

// Subscribe to topic
messenger.subscribe(Topic::new("alerts")).await;

// Send message
let msg = Message {
    id: MessageId::generate(),
    topic: Topic::new("alerts"),
    payload: serde_json::json!({"severity": "high", "message": "disk full"}),
};
messenger.send(msg).await?;

// Receive
if let Some(msg) = messenger.recv(Topic::new("alerts")).await {
    println!("Received: {:?}", msg.payload);
}
```

### Workflow DSL

```rust
use zylos_bot_system::{parse_workflow, validate_workflow};

let yaml_spec = r#"
name: data_pipeline
steps:
  - id: fetch
    action: http_get
    params:
      url: https://api.example.com/data
  - id: transform
    action: json_transform
    depends_on: [fetch]
    params:
      operation: normalize
"#;

let spec = parse_workflow(yaml_spec)?;
validate_workflow(&spec)?; // Validates structure and dependencies
```

## Para Dev Junior

### Passo a Passo: Criando um Agente

1. **Defina o agent loop**: Função async que roda até CancellationToken ser sinalizado
2. **Registre no LifecycleManager**: Use `manager.spawn()` com child token
3. **Implemente graceful shutdown**: Cheque `token.is_cancelled()` no loop
4. **Publique estado**: Atualize AgentState para observabilidade
5. **Comunique via P2P**: Use topics para categorizar mensagens

### Pitfalls Comuns

- **Nunca use `tokio::spawn` sem JoinSet**: Perde tracking e graceful shutdown
- **Não esqueça de propagar CancellationToken**: Child agents precisam do child token
- **Não faça busy-wait**: Use `tokio::select!` ou channels para esperar eventos
- **Não ignore AgentState transitions**: State machine valida transições; inválidas retornam erro
- **Não envie mensagens sem topic**: P2P requer topic para routing

### Debugging Tips

- `AgentState` é serializable — log state transitions para debugging
- DAG executor logs execution order em nível `debug`
- P2P messenger tem métricas de message count e latency
- Use `tracing::span` para correlacionar logs de um agente específico

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `BotError::SpawnFailed` | Token já cancelado ou resource exhaustion | Verifique CancellationToken state, reduza concurrency |
| `BotError::InvalidTransition` | State transition inválida | Consulte state machine em `lifecycle.rs`, siga transições permitidas |
| DAG deadlock | Ciclo em dependências | `validate_workflow()` detecta ciclos; corrija edges |
| P2P message lost | Subscriber desconectado | Implemente retry ou dead-letter queue |
| Agent hang após cancel | Loop não checa token | Adicione `if token.is_cancelled() { break; }` no loop |
| Workflow validation fail | Spec malformado | Verifique YAML syntax, required fields, dependency refs |

## Referências

- **Source**: `crates/zylos-bot-system/src/` (9 módulos)
- **Key files**: `lib.rs:1-22`, `lifecycle.rs`, `dag.rs`, `p2p.rs`, `workflow_dsl.rs`, `cerebelo.rs`, `kanban_bridge.rs`
- **Master Plan**: Linhas 184-188 (Fase 2 overview), Sprint 2.x detalhado no plano
- **Related**: `zylos-cerebelo` crate para DAG execution avançada