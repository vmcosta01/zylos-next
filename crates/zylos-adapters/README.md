# zylos-adapters

Multi-framework agent adapters for [zylos-gateway](../../). Provides a unified
`FrameworkAdapter` trait and concrete implementations for five agent frameworks:

| Framework     | Adapter              | Transport          |
|---------------|----------------------|--------------------|
| Google ADK    | `AdkAdapter`         | HTTP (REST)        |
| Anthropic     | `ClaudeSdkAdapter`   | HTTP (Messages API)|
| CrewAI        | `CrewAiAdapter`      | HTTP (REST)        |
| LangGraph     | `LangGraphAdapter`   | HTTP (Platform API)|
| LlamaIndex    | `LlamaIndexAdapter`  | HTTP (REST)        |

## Quick Start

```rust
use zylos_adapters::{
    AdapterRegistry, ClaudeSdkAdapter, Framework, FrameworkAdapter,
    SpawnRequest, AgentMessage, MessageRole,
};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let claude = ClaudeSdkAdapter::new("sk-ant-...", "claude-sonnet-4-20250514");
    let mut registry = AdapterRegistry::new();
    registry.register(Box::new(claude));

    let request = SpawnRequest {
        framework: Framework::ClaudeSdk,
        name: "researcher".into(),
        config: serde_json::json!({"temperature": 0.7}),
        initial_prompt: Some("You are a research assistant.".into()),
        env_vars: HashMap::new(),
    };

    let adapter = registry.get(Framework::ClaudeSdk).unwrap();
    let agent = adapter.spawn(&request).await?;
    println!("Spawned agent {} with status {:?}", agent.id, agent.status);

    let msg = AgentMessage {
        id: uuid::Uuid::new_v4().to_string(),
        role: MessageRole::User,
        content: "Summarize the latest advances in fusion energy.".into(),
        timestamp: chrono::Utc::now(),
        metadata: HashMap::new(),
    };

    let reply = adapter.send_message(&agent.id, &msg).await?;
    println!("Agent replied: {}", reply.content);
    Ok(())
}
```

## Types

| Type | Purpose |
|------|---------|
| `Framework` | Enum of supported frameworks: `Adk`, `ClaudeSdk`, `CrewAi`, `LangGraph`, `LlamaIndex` |
| `AgentStatus` | Agent lifecycle: `Initializing`, `Ready`, `Running`, `Paused`, `Completed`, `Failed`, `Terminated` |
| `AgentInstance` | Running or configured agent instance managed by an adapter |
| `AgentMessage` | Message exchanged between caller and agent (role, content, metadata) |
| `MessageRole` | Participant role: `User`, `Assistant`, `System`, `Tool` |
| `SpawnRequest` | Parameters for spawning a new agent (framework, name, config, env_vars) |
| `AdapterHealth` | Health report for a single adapter (available, version, active_agents, errors) |
| `FrameworkAdapter` | Async trait every adapter must implement |
| `AdapterRegistry` | Central registry mapping `Framework` to `Box<dyn FrameworkAdapter>` |
| `AdapterError` | Unified error type (see Error Variants below) |

### Health Checking

```rust
let results = registry.health_all().await;
for result in &results {
    match result {
        Ok(health) => println!(
            "{} available={} version={:?}",
            health.framework, health.available, health.version
        ),
        Err(e) => eprintln!("Health check failed: {e}"),
    }
}
```

### Custom Base URLs

```rust
use zylos_adapters::AdkAdapter;
let adk = AdkAdapter::new("http://localhost:8080", Some("my-api-key".into()));

let claude = ClaudeSdkAdapter::new("sk-ant-...", "claude-sonnet-4-20250514")
    .with_base_url("https://proxy.internal.example.com/v1");
```

### Implementing a Custom Adapter

```rust
use async_trait::async_trait;
use zylos_adapters::*;

pub struct MyAdapter { /* ... */ }

#[async_trait]
impl FrameworkAdapter for MyAdapter {
    fn framework(&self) -> Framework { /* return your variant */ todo!() }
    async fn health(&self) -> Result<AdapterHealth, AdapterError> { todo!() }
    async fn spawn(&self, req: &SpawnRequest) -> Result<AgentInstance, AdapterError> { todo!() }
    async fn send_message(&self, id: &str, msg: &AgentMessage) -> Result<AgentMessage, AdapterError> { todo!() }
    async fn get_status(&self, id: &str) -> Result<AgentStatus, AdapterError> { todo!() }
    async fn terminate(&self, id: &str) -> Result<(), AdapterError> { todo!() }
    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> { todo!() }
}

registry.register(Box::new(MyAdapter { /* ... */ }));
```

## Error Variants

`AdapterError` (via `thiserror`):

| Variant | Meaning |
|---------|---------|
| `UnknownFramework(String)` | Framework name not recognized by `Framework::from_str` |
| `AgentNotFound(String)` | No agent with the given ID exists in this adapter |
| `FrameworkUnavailable(String)` | Framework runtime or SDK is not installed or reachable |
| `InvalidConfig(String)` | Supplied configuration is invalid for this framework |
| `Communication(String)` | Network or IPC call to the framework failed |
| `Timeout(u64)` | Operation exceeded its deadline (seconds) |
| `Serialization(serde_json::Error)` | JSON serialization/deserialization failure |
| `Http(reqwest::Error)` | Underlying HTTP request error |
| `Io(std::io::Error)` | Filesystem or I/O error |
| `Internal(String)` | Unclassified internal adapter error |

## Gaps

- **`ClaudeSdkAdapter` is stateless** — `spawn` creates a local tracking handle only; `list_agents` always returns empty; `terminate` is a no-op. No real session management.
- **Sequential health checks** — `health_all()` iterates adapters sequentially, not concurrently.
- **No retry/backoff** — HTTP calls in adapters have no built-in retry or exponential backoff.
- **No streaming responses** — all adapters use request/response only; no support for streaming or SSE.
- **No adapter removal** — `AdapterRegistry` supports `register` but not `unregister` or selective replacement.
- **No standardized auth** — each adapter handles authentication differently (Bearer, API key, none); no unified auth abstraction.
- **No framework discovery** — adapters must be registered manually; no auto-detection of available frameworks.

## License

See the workspace-level LICENSE file.
