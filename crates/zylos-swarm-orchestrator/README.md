# zylos-swarm-orchestrator

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
