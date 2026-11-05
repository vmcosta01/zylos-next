# Zylos-Next v6.0

**Multi-agent orchestration platform for Hermes** — Rust-native, memory-first, verification-gated.

[![Tests](https://img.shields.io/badge/tests-592+-green)]() [![Crates](https://img.shields.io/badge/crates-18-blue)]() [![License](https://img.shields.io/badge/license-MIT-orange)]()

## Quick Start (<5 min)

### Prerequisites

- Rust 1.80+ (stable toolchain)
- Docker 24+ (optional, for sandbox isolation)
- SQLite 3.40+ (bundled via rusqlite, no external install needed)
- 4GB RAM minimum (~30MB per agent idle)

### Build and Run

```bash
# Clone and build
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next
cargo build --release

# Run the gateway with default config
./target/release/zylos-gateway

# Verify health (in another terminal)
curl http://localhost:3000/gateway/health
# {"service":"zylos-gateway","status":"healthy","version":"0.1.0"}
```

### Custom Configuration

Create a `config.toml` in the project root:

```toml
[server]
listen_addr = "0.0.0.0:3000"
shutdown_timeout_secs = 5

[health]
ready_enabled = true
live_enabled = true

[soar]
audit_key = "your-production-key-here"
event_bus_capacity = 256

[skills]
index_path = "./skills"
```

Run with custom config:

```bash
./target/release/zylos-gateway --config config.toml
```

Environment variables override TOML values (priority: CLI > env > TOML > defaults):

```bash
ZYLOS_LISTEN="0.0.0.0:8080" ZYLOS_SOAR_AUDIT_KEY="prod-key" ./target/release/zylos-gateway
```

### Using Zylos-Next as a Library

Add crates to your `Cargo.toml`:

```toml
[dependencies]
zylos-core = { path = "../zylos-next/crates/zylos-core" }
zylos-memory = { path = "../zylos-next/crates/zylos-memory" }
zylos-proof = { path = "../zylos-next/crates/zylos-proof" }
zylos-orchestrator = { path = "../zylos-next/crates/zylos-orchestrator" }
tokio = { version = "1", features = ["full"] }
```

Example: creating a verified claim with the proof system:

```rust
use zylos_proof::{Ground, ProofBuilder};
use zylos_core::Claim;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let claim = Claim::new("user-authenticated")
        .with_evidence("oauth-token-validated")
        .with_ground(Ground::Confirmed);

    let proof = ProofBuilder::new(claim)
        .add_witness("auth-service", Ground::Verified)
        .add_witness("token-validator", Ground::Confirmed)
        .build()?;

    println!("Proof ground level: {:?}", proof.ground());
    Ok(())
}
```

Example: storing and querying memory with consolidation:

```rust
use zylos_memory::{MemoryStore, MemoryEntry};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = MemoryStore::open("data/memory.db")?;

    let entry = MemoryEntry::builder()
        .agent_id("assistant-01")
        .content("User prefers dark mode in all IDEs")
        .tags(vec!["preference".into(), "ui".into()])
        .build();
    store.insert(entry).await?;

    let results = store.search("editor theme settings", 5).await?;
    for r in &results {
        println!("[{:.2}] {}", r.score, r.content);
    }

    let stats = store.consolidate().await?;
    println!("Consolidated: {} merged, {} decayed, {} contradicted",
        stats.merged, stats.decayed, stats.contradicted);
    Ok(())
}
```

Example: running a DAG orchestration task:

```rust
use zylos_orchestrator::{DagRunner, TaskNode};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut dag = DagRunner::new();

    dag.add_node(TaskNode::new("fetch-data", async {
        Ok(serde_json::json!({"records": 42}))
    }));

    dag.add_node(TaskNode::new("process", async {
        Ok(serde_json::json!({"processed": true}))
    }));

    dag.add_edge("fetch-data", "process");

    let result = dag.execute().await?;
    println!("DAG completed: {} tasks in {:.2}s",
        result.completed, result.duration.as_secs_f64());
    Ok(())
}
```

## Architecture Overview

```text
+-----------------------------------------------------+
|                  zylos-gateway                       |
|  (Axum HTTP/SSE + Health Checks + Config Hot-Reload) |
+----------+----------+-----------+-------------------+
|  Memory  |Orchestr. |Verification|   Live Comm       |
|(zylos-   |(zylos-   |(zylos-    |(zylos-live-comm)  |
| memory)  |orchest.) |proof/etc) | AG-UI, Triage,    |
|          |          |           | Approval, Sentinel |
+----------+----------+-----------+-------------------+
|              Sandbox Layer                           |
|  WASM (wasmtime) | Docker (bollard) | Browser       |
+-----------------------------------------------------+
|              Skills Registry (338 skills)            |
+-----------------------------------------------------+
```

See [Architecture Documentation](docs/ARCHITECTURE.md) for C4 diagrams and detailed component descriptions.

## Crates

| Crate | Description | Tests |
|-------|-------------|-------|
| `zylos-core` | Shared types, traits, errors | 21 |
| `zylos-memory` | Memory store, consolidation, decay | 25 |
| `zylos-orchestrator` | DAG execution, checkpointing | 6 |
| `zylos-proof` | 5-level Ground enum, ProofBuilder | 17 |
| `zylos-resolver` | BFS frontier, wave scheduling | 6 |
| `zylos-panel` | Adversarial judging, refutation | 5 |
| `zylos-router` | Grounded routing, leash, sacred targets | 4 |
| `zylos-cerebelo` | Shadow-git, audit chain SHA256 | 3 |
| `zylos-learn` | Feedback loops, reward shaping | 5 |
| `zylos-live-comm` | AG-UI, EventBus, approval, triage, sentinel | 157 |
| `zylos-sandbox` | WASM + Docker sandbox isolation | 13 |
| `zylos-search-mcp` | Parallel search MCP server | 8 |
| `zylos-soar` | SOAR orchestrator, playbook engine | 34 |
| `zylos-hitl` | HITL outcome uncertainty (standalone) | 12 |
| `zylos-triage` | Two-stage triage engine (standalone) | 18 |
| `zylos-sentinel` | AIOps health scanner (standalone) | 17 |
| `zylos-verification` | Verification pipeline integration | 21 |
| `zylos-gateway` | API gateway binary + E2E tests | 23 |

**Total: 592+ tests, zero failures.**

## Key Features

- **Memory-First Architecture**: SQLite-backed persistent memory with automatic consolidation, contradiction detection, and temporal decay. Agents retain context across restarts without external databases.
- **Verification-Gated Execution**: 5 ground levels (Assumed -> Observed -> Confirmed -> Verified -> Proven). Claims must meet minimum ground thresholds before acceptance. Adversarial panels can refute weak evidence.
- **HITL Approval Cards**: Equivalence-based SHA-256 hashing ensures identical actions reuse approvals. 4-tier decision model (approve/deny/escalate/safety). Safety-tier decisions are immutable and cannot be overridden.
- **Two-Stage Triage**: Deterministic rules engine runs first (zero latency), falls back to model-based classification only when rules are inconclusive. Every decision includes `decided_by` audit field.
- **AIOps Sentinel**: Pure-function detectors with zero side effects. PII stripping on all output. Rate-limited incident reporting (5/hr per signature) prevents alert storms. Escalations are never rate-limited.
- **Sandbox Isolation**: Three-tier sandbox: WASM (wasmtime 25.0) for untrusted code, Docker (bollard 0.17) for system access, browser anti-detect for web automation. Each tier has independent resource limits.
- **SOAR Integration**: Playbook lifecycle management, incident publishing via EventBus, full audit trails with SHA-256 chaining. Compatible with standard SOAR platforms.
- **338 Skills Registry**: All Hermes v1 skills migrated with zero functionality loss. Eval harness validates skill behavior against test suites.
- **Configuration Hot-Reload**: ~80% of config keys support live reload via SIGHUP or file watcher. No restart needed for tuning parameters.
- **Leptos 0.7 Frontend**: SSR-native WASM UI replacing React. Cockpit dashboard for monitoring, approval management, and skill browsing.

## API Endpoints

The gateway exposes REST and SSE endpoints:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/gateway/health` | GET | Service health check |
| `/gateway/ready` | GET | Readiness probe (subsystems initialized) |
| `/gateway/live` | GET | Liveness probe |
| `/api/v1/agents` | GET/POST | Agent CRUD |
| `/api/v1/skills` | GET | List registered skills |
| `/api/v1/memory/search` | POST | Semantic memory search |
| `/api/v1/approvals` | GET/POST | HITL approval cards |
| `/api/v1/incidents` | POST | Report incidents |
| `/events` | SSE | Real-time event stream |

Full OpenAPI specification: [`docs/api/gateway-openapi.yaml`](docs/api/gateway-openapi.yaml)

## Documentation

- [Architecture (C4 Diagrams)](docs/ARCHITECTURE.md) - Component diagrams, data flows, deployment views
- [Deployment Guide](docs/DEPLOYMENT.md) - Docker, Kubernetes, bare metal with production checklist
- [Contributing Guidelines](docs/CONTRIBUTING.md) - Dev setup, code standards, review process
- [Migration from Hermes v1](docs/MIGRATION.md) - Step-by-step upgrade path with state preservation
- [FAQ](docs/FAQ.md) - Top 20 questions about architecture, operations, and troubleshooting
- [Benchmarks](docs/BENCHMARKS.md) - Performance comparisons and methodology
- [Runbooks](runbooks/) - Operational guides for all phases
- [Playbooks](playbooks/) - SOAR workflows and procedures
- [Handoffs](docs/handoffs/) - Phase transition documents

## Configuration Reference

Gateway supports TOML files, environment variables, and CLI flags (priority: CLI > env > TOML > defaults).

```toml
# config.toml - full reference
[server]
listen_addr = "0.0.0.0:3000"         # Bind address
shutdown_timeout_secs = 5             # Graceful shutdown window
max_connections = 1000                # Concurrent connection limit

[health]
ready_enabled = true                  # Enable /gateway/ready endpoint
live_enabled = true                   # Enable /gateway/live endpoint

[soar]
audit_key = "your-key-here"           # SOAR audit trail signing key
event_bus_capacity = 256              # EventBus channel buffer size
incident_rate_limit = 5               # Max incidents per hour per signature

[skills]
index_path = "./skills"               # Skills registry directory
auto_reload = true                    # Watch for skill file changes

[memory]
db_path = "data/memory.db"            # SQLite database location
consolidation_interval_secs = 3600    # Auto-consolidation period
decay_half_life_days = 30             # Temporal decay half-life

[sandbox]
wasm_enabled = true                   # Enable WASM sandbox tier
docker_enabled = true                 # Enable Docker sandbox tier
browser_enabled = false               # Enable browser anti-detect tier
```

### Environment Variable Overrides

| Variable | Default | Description |
|----------|---------|-------------|
| `ZYLOS_LISTEN` | `0.0.0.0:3000` | Listen address |
| `ZYLOS_SHUTDOWN_TIMEOUT_SECS` | `5` | Graceful shutdown timeout |
| `ZYLOS_EVENT_BUS_CAPACITY` | `256` | EventBus channel capacity |
| `ZYLOS_SKILLS_PATH` | `./skills` | Skills index directory |
| `ZYLOS_SOAR_AUDIT_KEY` | `zylos-gateway-soar-key` | SOAR audit trail key |
| `ZYLOS_MEMORY_DB` | `data/memory.db` | SQLite database path |
| `RUST_LOG` | `info` | Log level (trace/debug/info/warn/error) |

## Troubleshooting

### Gateway will not start

```bash
# Check if port is already in use
ss -tlnp | grep 3000

# Run with debug logging
RUST_LOG=debug ./target/release/zylos-gateway --config config.toml

# Validate config syntax
./target/release/zylos-gateway --validate-config config.toml
```

### Memory issues

```bash
# Check database integrity
sqlite3 data/memory.db "PRAGMA integrity_check;"

# Force consolidation
./target/release/zylos-gateway --force-consolidation

# Export memory for inspection
sqlite3 data/memory.db ".dump" > memory-backup.sql
```

### High memory usage

Each agent uses ~30MB idle. If usage exceeds expectations:
1. Check active agent count: `curl http://localhost:3000/api/v1/agents | jq '.[].id'`
2. Review consolidation stats in logs
3. Enable memory decay: set `decay_half_life_days` in config
4. Check for memory leaks: `RUST_LOG=zylos_memory=trace`

## License

MIT
