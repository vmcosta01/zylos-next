#!/usr/bin/env python3
import os

files = {
    'C:/Users/Paulo/zylos-next/crates/zylos-swarm-orchestrator/README.md': """# zylos-swarm-orchestrator

Multi-agent swarm orchestration for **zylos-gateway**. Provides task decomposition, dependency DAG scheduling, agent assignment with capability matching, and SQLite WAL-backed persistence for crash recovery.

## Features

- **Task lifecycle state machine** — `Pending → Ready → Running → Done/Failed/Cancelled/Blocked` with validated transitions and automatic retry/backoff.
- **Dependency DAG** — topological validation via `petgraph`; tasks auto-promote to `Ready` when all dependencies complete.
- **Capability-based scheduling** — agents declare capabilities; tasks declare requirements; the scheduler matches and load-balances.
- **SQLite persistence** — all state survives process restarts; WAL mode for concurrent reads.
- **Hierarchical tasks** — parent/child relationships for decomposing large work units.

## Quick Start

```rust
use zylos_swarm_orchestrator::{SwarmOrchestrator, Task, SwarmAgent, Priority};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let orch = SwarmOrchestrator::open("sqlite:swarm.db").await?;

    let mut agent = SwarmAgent::new("agent-1", "Backend Worker");
    agent.capabilities = vec!["rust".into(), "backend".into()];
    agent.max_concurrent_tasks = 4;
    orch.register_agent(&agent).await?;

    let build = Task::new("Build API", "Compile the backend")
        .with_priority(Priority::High)
        .with_capabilities(vec!["rust".into(), "backend".into()]);
    orch.add_task(&build).await?;

    let deploy = Task::new("Deploy", "Push to staging")
        .with_dependencies(vec![build.id.clone()])
        .with_priority(Priority::Critical);
    orch.add_task(&deploy).await?;

    orch.initialize_ready_tasks().await?;
    let result = orch.schedule().await?;
    println!("Assigned {} tasks", result.assignments.len());

    orch.complete_task(&build.id, serde_json::json!({"artifacts": ["api.bin"]})).await?;
    let stats = orch.stats().await?;
    println!("Running: {}, Done: {}", stats.running_tasks, stats.done_tasks);
    Ok(())
}
```

## Core Types

| Type | Purpose |
|------|---------|
| `TaskId` / `AgentId` | `String` aliases for task and agent identifiers |
| `TaskState` | Lifecycle enum: `Pending`, `Ready`, `Running`, `Done`, `Failed`, `Cancelled`, `Blocked` |
| `Priority` | Scheduling priority: `Low`, `Normal`, `High`, `Critical` |
| `Task` | A unit of work with state, dependencies, capabilities, payload, and retry config |
| `SwarmAgent` | A worker with capabilities, capacity, heartbeat, and metadata |
| `ScheduleResult` | Output of `schedule()`: assignments, blocked/unassignable tasks, errors |
| `SwarmStats` | Aggregate counters for tasks by state and agent capacity |
| `TaskQuery` | Filter struct for `query_tasks()` with state, agent, parent, tag, pagination |
| `SwarmOrchestrator` | Main entry point; manages tasks, agents, scheduling, and persistence |
| `SwarmError` | Unified error type (see Error Variants below) |

### Task Builder Pattern

```rust
let task = Task::new("Process data", "ETL pipeline step")
    .with_priority(Priority::High)
    .with_dependencies(vec!["upstream-task-id".into()])
    .with_capabilities(vec!["python".into(), "data".into()])
    .with_payload(serde_json::json!({"source": "s3://bucket/input.csv"}))
    .with_parent("parent-task-id".into());
```

### State Machine

Valid transitions:

```text
Pending  → Ready, Cancelled
Ready    → Running, Cancelled, Blocked
Running  → Done, Failed, Cancelled, Blocked
Failed   → Ready  (retry)
Blocked  → Ready, Cancelled
```

Use `transition_task()` for manual transitions or the convenience methods `complete_task()`, `fail_task()`, and `retry_task()`.

## Error Variants

`SwarmError` (via `thiserror`):

| Variant | Meaning |
|---------|---------|
| `Database(sqlx::Error)` | SQLite connection, query, or migration failure |
| `Serialization(serde_json::Error)` | Invalid JSON in payload, dependencies, capabilities, or tags |
| `TaskNotFound(String)` | Referenced task ID does not exist in the store |
| `AgentNotFound(String)` | Referenced agent ID is not registered |
| `CycleDetected` | Task dependency graph contains a cycle (DAG violation) |
| `InvalidTransition(from, to)` | State transition violates the state machine rules |
| `Io(std::io::Error)` | Underlying filesystem or I/O error |
| `Internal(String)` | Unclassified internal orchestrator error |

## Gaps

- **No async message passing** between agents — only task assignment is supported, not inter-agent communication.
- **Single-pass scheduling** — `schedule()` runs once; no periodic re-scheduling or preemption of running tasks.
- **No automatic heartbeat validation** — stale agents remain active in the registry without timeout-based eviction.
- **Fixed retry backoff** — retries use immediate re-queue; no exponential backoff or jitter configuration.
- **No execution history** — task/agent latency metrics and historical performance are not tracked.
- **Limited `TaskQuery`** — no date-range filtering, no multi-tag intersection, no compound filters.
- **No recursive cancellation** — cancelling a parent task does not cascade to child tasks.
- **No priority preemption** — a high-priority task cannot displace a lower-priority running task.

## Testing

```bash
cargo test -p zylos-swarm-orchestrator
```

Tests use in-memory SQLite (`:memory:`) so no external database is required.

## License

Part of the zylos-next workspace. See the root `LICENSE` file.
""",

    'C:/Users/Paulo/zylos-next/crates/zylos-adapters/README.md': """# zylos-adapters

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
""",

    'C:/Users/Paulo/zylos-next/crates/zylos-evidence-ledger/README.md': """# zylos-evidence-ledger

Tamper-evident evidence ledger for [zylos-gateway](https://github.com/nousresearch/zylos-next). Provides an append-only, cryptographically signed log of evidence entries with Merkle tree integrity verification and SQLite WAL-backed persistence.

## Features

- **Append-only log** — entries are immutable once written; no updates or deletes.
- **Ed25519 signing** — every entry is signed with a per-ledger keypair; signatures are verifiable independently.
- **Chain linking** — each entry includes the hash of its predecessor, enabling sequential tamper detection.
- **Merkle proofs** — batch integrity via Merkle root computation and per-entry inclusion proofs.
- **SQLite WAL storage** — concurrent-safe persistence with automatic migrations.
- **JSONL export** — dump the full ledger as newline-delimited JSON for archival or import.

## Quick Start

```rust
use zylos_evidence_ledger::{EvidenceLedger, LedgerKeys, AppendRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ledger = EvidenceLedger::open("sqlite:evidence.db").await?;
    let keys = LedgerKeys::generate();

    let entry = ledger.append(&AppendRequest {
        evidence_type: "tool_output".into(),
        actor: "agent-1".into(),
        description: "Search results for query X".into(),
        payload: serde_json::json!({"query": "X", "results": 42}),
        tags: vec!["search".into()],
    }, &keys).await?;

    println!("Appended entry {} at sequence {}", entry.id, entry.sequence);

    let result = ledger.verify_entry(&entry.id).await?;
    assert!(result.valid);

    if let Some(root) = ledger.merkle_root().await? {
        println!("Current Merkle root: {}", root);
    }

    let proof = ledger.merkle_proof(&entry.id).await?;
    assert!(zylos_evidence_ledger::verify_merkle_proof(&proof));

    let jsonl = ledger.export_jsonl().await?;
    std::fs::write("evidence_backup.jsonl", jsonl)?;
    Ok(())
}
```

## Types

| Type | Purpose |
|------|---------|
| `EvidenceLedger` | Main API: open, append, query, verify, Merkle operations |
| `LedgerKeys` | Ed25519 keypair generation, signing, verification, and hex export/import |
| `LedgerEntry` | A single signed evidence record with chain metadata and hashes |
| `AppendRequest` | Input struct for appending new entries (type, actor, payload, tags) |
| `VerificationResult` | Outcome of verifying an entry (valid flag + individual checks) |
| `VerificationCheck` | Individual check (payload_hash, entry_hash, signature, chain_link) |
| `MerkleProof` | Inclusion proof for a specific entry against the batch root |
| `MerklePathNode` | Node in the Merkle proof path (hash + direction) |
| `Direction` | Enum `Left` / `Right` for Merkle tree navigation |
| `LedgerStats` | Aggregate statistics (counts, latest sequence, Merkle root) |
| `LedgerError` | Unified error type (see Error Variants below) |

### Verification Checks

When you call `verify_entry`, four independent checks run:

1. **payload_hash** — SHA-256 of the canonical payload matches the stored hash.
2. **entry_hash** — Combined hash of `(payload_hash, previous_hash, sequence)` matches.
3. **signature** — Ed25519 signature over `entry_hash` verifies against the stored public key.
4. **chain_link** — `previous_hash` matches the predecessor's `entry_hash` (genesis uses 64 zeros).

Use `verify_full_chain()` to run all four checks on every entry in sequence order.

### Key Management

```rust
let keys = LedgerKeys::generate();
let secret_hex = keys.secret_hex();
let restored = LedgerKeys::from_secret_hex(&secret_hex)?;

let valid = LedgerKeys::verify_with_public_key(
    &entry.signer_public_key,
    entry.entry_hash.as_bytes(),
    &entry.signature,
)?;
```

### Standalone Verification

```rust
let proof: MerkleProof = /* ... */;
assert!(verify_merkle_proof(&proof));
```

## Error Variants

`LedgerError` (via `thiserror`):

| Variant | Meaning |
|---------|---------|
| `Database(sqlx::Error)` | SQLite connection, query, or migration failure |
| `Serialization(serde_json::Error)` | Invalid JSON in payload or tags |
| `Signing(String)` | Error during Ed25519 signing (invalid key, etc.) |
| `VerificationFailed(String)` | Integrity verification failed (hash mismatch, bad signature) |
| `NotFound(String)` | Entry not found by ID or hash |
| `InvalidKey(String)` | Malformed hex key/signature or wrong byte length |
| `Io(std::io::Error)` | Filesystem or I/O error |
| `Internal(String)` | Unclassified internal ledger error |

## Gaps

- **No import** — `export_jsonl` exists but there is no corresponding import function to restore from a backup.
- **O(n) chain verification** — `verify_full_chain` runs individual queries per entry; no batch verification.
- **Single signer** — no key rotation or support for multiple concurrent signers.
- **In-memory private keys** — `LedgerKeys` holds the signing key in memory as raw bytes; no secure storage or HSM integration.
- **No pruning** — old entries cannot be archived or pruned; the ledger grows unbounded.
- **No Merkle cache** — `merkle_root()` recomputes from scratch on every call; no incremental update.
- **No event notifications** — no webhooks or pub/sub for new entries.
- **No entry types registry** — `evidence_type` is a free-form string with no validation or enumeration.

## Testing

```bash
cargo test -p zylos-evidence-ledger
```

Tests use an in-memory SQLite database (`:memory:`) and cover key management, hash computation, chain integrity, Merkle proofs, stats, and JSONL export.

## License

Same license as the parent `zylos-next` workspace.
""",

    'C:/Users/Paulo/zylos-next/crates/zylos-goals/README.md': """# zylos-goals

Goal tracking system for **zylos-gateway** with hierarchical goals, OKR key results, milestone checkpoints, cron-based progress watchers, and a full audit trail — all persisted in SQLite WAL mode.

## Features

- **Hierarchical goals** — parent/child relationships with status lifecycle (`draft → active → on_track / at_risk / blocked → completed / cancelled`)
- **OKR key results** — measurable outcomes with automatic status transitions when targets are met
- **Milestones** — timeline checkpoints with due-date tracking
- **Progress watchers** — cron-scheduled metric collectors (skill execution, integration actions, SQL queries, HTTP endpoints, or manual updates)
- **Audit log** — every mutation is recorded with actor, old/new values, and timestamp
- **Dashboard summary** — aggregate metrics across all entities
- **Async-safe** — internal `RwLock` around the SQLite connection for concurrent access

## Quick Start

```rust
use chrono::Utc;
use zylos_goals::{Goal, GoalStatus, GoalStore, KeyResult, KrStatus, MetricType, Priority};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = GoalStore::open(std::path::Path::new("goals.db"))?;
    let now = Utc::now();

    let goal = Goal {
        id: "launch-mvp".into(),
        title: "Launch MVP by Q2".into(),
        description: Some("Ship the first public release".into()),
        owner: Some("alice".into()),
        status: GoalStatus::Active,
        priority: Priority::High,
        created_at: now,
        updated_at: now,
        due_date: None,
        parent_goal_id: None,
        tags: vec!["product".into(), "q2".into()],
    };
    store.create_goal(&goal).await?;

    let kr = KeyResult {
        id: "kr-users".into(),
        goal_id: "launch-mvp".into(),
        title: "Reach 1,000 signups".into(),
        metric_type: MetricType::Count,
        target_value: 1000.0,
        current_value: 0.0,
        unit: Some("users".into()),
        status: KrStatus::NotStarted,
        created_at: now,
        updated_at: now,
    };
    store.create_key_result(&kr).await?;

    store.update_kr_progress("kr-users", 1000.0, Some("bot")).await?;

    let summary = store.dashboard_summary().await?;
    println!("Total goals: {}, Avg KR progress: {:.1}%",
        summary.total_goals, summary.avg_kr_progress_pct);
    Ok(())
}
```

### In-Memory Store (Testing)

```rust
let store = GoalStore::in_memory()?;
```

## Types

| Type | Purpose |
|------|---------|
| `Goal` | High-level objective with status, priority, tags, and optional parent |
| `GoalStatus` | Lifecycle: `Draft`, `Active`, `OnTrack`, `AtRisk`, `Blocked`, `Completed`, `Cancelled` |
| `Priority` | Importance level: `Low`, `Medium`, `High`, `Critical` |
| `KeyResult` | Measurable outcome (OKR KR) with metric_type, target/current value, auto-status |
| `MetricType` | Kind of metric: `Percentage`, `Count`, `Currency`, `Duration`, `Boolean`, `Custom` |
| `KrStatus` | KR completion: `NotStarted`, `InProgress`, `Achieved`, `Missed` |
| `Milestone` | Timeline checkpoint with due date and completion status |
| `MilestoneStatus` | Milestone state: `Pending`, `Completed`, `Overdue`, `Skipped` |
| `ProgressWatcher` | Cron-scheduled metric collector for a key result |
| `WatcherSourceType` | Watcher source: `SkillExecution`, `IntegrationAction`, `SqlQuery`, `HttpEndpoint`, `Manual` |
| `GoalAuditEntry` | Immutable audit log entry for entity mutations |
| `DashboardSummary` | Aggregate snapshot: totals by status, average KR progress |
| `GoalStore` | SQLite WAL store with `RwLock` for async-safe access |
| `GoalsError` | Unified error type (see Error Variants below) |

All enums implement `Display` (lowercase snake_case) and `from_str()` for parsing.

### GoalStore Methods

**Goals:** `create_goal`, `get_goal`, `list_goals(parent_id)`, `update_goal_status`, `delete_goal` (cascade).

**Key Results:** `create_key_result`, `list_key_results(goal_id)`, `update_kr_progress` (auto-updates status).

**Milestones:** `create_milestone`, `list_milestones(goal_id)`, `complete_milestone`.

**Watchers:** `create_watcher` (validates cron), `list_enabled_watchers`, `record_watcher_run`.

**Audit & Dashboard:** `get_audit_log(entity_id, limit)`, `dashboard_summary`.

## Error Variants

`GoalsError` (via `thiserror`):

| Variant | Meaning |
|---------|---------|
| `Database(rusqlite::Error)` | SQLite connection, query, or migration failure |
| `NotFound(String)` | Entity (goal, KR, milestone, watcher) not found by ID |
| `InvalidStatus(String)` | Invalid string for `GoalStatus` parsing |
| `InvalidPriority(String)` | Invalid string for `Priority` parsing |
| `InvalidMetricType(String)` | Invalid string for `MetricType` parsing |
| `InvalidKrStatus(String)` | Invalid string for `KrStatus` parsing |
| `InvalidMilestoneStatus(String)` | Invalid string for `MilestoneStatus` parsing |
| `InvalidWatcherSource(String)` | Invalid string for `WatcherSourceType` parsing |
| `InvalidCron(String)` | Invalid cron expression in watcher |
| `Serialization(serde_json::Error)` | Invalid JSON in source_config or tags |
| `Io(std::io::Error)` | Filesystem or I/O error when opening database |

## Gaps

- **Watchers not auto-executed** — watchers are stored with cron expressions but require an external scheduler to trigger `record_watcher_run`.
- **No title uniqueness** — goals/KRs/milestones can have duplicate titles within the same parent.
- **KR progress not audited** — `update_kr_progress` updates the value but does not log an audit entry for the change.
- **No bulk operations** — no batch update or bulk create for KRs, milestones, or watchers.
- **Dashboard is flat** — `DashboardSummary` does not break down metrics by owner, tag, or priority.
- **Hard deletes only** — `delete_goal` is permanent with cascade; no soft-delete or archival.
- **Cron validation is basic** — depends on the `cron` crate; non-standard expressions may be silently rejected.
- **No KR weights** — all KRs within a goal contribute equally to average progress; no relative weighting.
- **No overdue detection** — milestones past their due date are not automatically marked `Overdue`.

## Dependencies

`serde`, `serde_json`, `tokio`, `thiserror`, `tracing`, `uuid`, `chrono`, `rusqlite`, `cron`

## License

Same as the workspace root.
"""
}

for path, content in files.items():
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)
    print(f'Wrote {path} ({len(content)} bytes)')