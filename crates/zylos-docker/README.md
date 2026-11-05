# zylos-docker

Docker-based sandbox isolation for Zylos agents.

## Architecture

### Modules

- **`container`** — `DockerSandbox`, `SandboxConfig`, `ContainerStatus` for per-agent container lifecycle management and filesystem sandboxing
- **`approval`** — `ApprovalWorkflow`, `ApprovalDecision`, `RiskLevel` for approval workflows gating high-risk operations
- **`error`** — `DockerError` enum for container operation failures

### Key Exports

```rust
pub use container::{DockerSandbox, SandboxConfig, ContainerStatus};
pub use approval::{ApprovalWorkflow, ApprovalDecision, RiskLevel};
pub use error::DockerError;
```

## Usage

```rust
use zylos_docker::{DockerSandbox, SandboxConfig, ApprovalWorkflow, RiskLevel};

// Configure a sandboxed container for an agent
let config = SandboxConfig::default();
let sandbox = DockerSandbox::new(config);

// Check approval before executing a high-risk operation
let workflow = ApprovalWorkflow::default();
let decision = workflow.evaluate(RiskLevel::High).await?;
```

## Dependencies

| Crate | Features | Purpose |
|-------|----------|---------|
| `bollard` | chrono | Docker API client |
| `tokio` | full | Async runtime |
| `serde` | derive | Serialization |
| `serde_json` | — | JSON handling |
| `uuid` | v4, serde | Container/session IDs |
| `chrono` | serde | Timestamps |
| `thiserror` | — | Error types |
| `tracing` | — | Structured logging |
| `sha2` | — | Content hashing |
| `hex` | — | Hash encoding |
| `futures` | — | Stream combinators |

## Testing

```bash
cargo test -p zylos-docker
```

Dev dependencies: `tempfile` for temporary directory fixtures. Note: integration tests require a running Docker daemon.

## Related Crates

- None (standalone crate with no internal workspace dependencies)