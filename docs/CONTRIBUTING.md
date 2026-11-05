# Contributing to Zylos-Next

Thank you for your interest in contributing to Zylos-Next! This document provides guidelines for setting up your development environment, coding standards, and the review process.

## Getting Started

### Prerequisites

- Rust 1.80+ (stable toolchain)
- Git 2.40+
- Docker 24+ (for sandbox tests)
- SQLite 3.40+ (bundled via rusqlite)
- `just` command runner (optional, but recommended)

### Initial Setup

```bash
# Clone repository
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next

# Install Rust toolchain (if not already installed)
rustup install stable
rustup component add clippy rustfmt

# Build the workspace
cargo build

# Run all tests
cargo test --workspace

# Run clippy lints
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all
```

### Development Tools

Install recommended tools:

```bash
# just - task runner
cargo install just

# cargo-watch - auto-rebuild on changes
cargo install cargo-watch

# cargo-expand - macro expansion debugging
cargo install cargo-expand

# sqlx-cli - database migrations (if working on schema)
cargo install sqlx-cli
```

### Project Structure

```
zylos-next/
├── crates/                 # Library crates
│   ├── zylos-core/         # Shared types, traits, errors
│   ├── zylos-memory/       # Memory store, consolidation, decay
│   ├── zylos-orchestrator/ # DAG execution, checkpointing
│   ├── zylos-proof/        # Ground levels, ProofBuilder
│   ├── zylos-resolver/     # BFS frontier, wave scheduling
│   ├── zylos-panel/        # Adversarial judging, refutation
│   ├── zylos-router/       # Grounded routing, leash
│   ├── zylos-cerebelo/     # Shadow-git, audit chain
│   ├── zylos-learn/        # Feedback loops, reward shaping
│   ├── zylos-live-comm/    # AG-UI, EventBus, approval, triage
│   ├── zylos-sandbox/      # WASM + Docker isolation
│   ├── zylos-search-mcp/   # Parallel search MCP server
│   ├── zylos-soar/         # SOAR orchestrator, playbooks
│   ├── zylos-hitl/         # HITL outcome uncertainty
│   ├── zylos-triage/       # Two-stage triage engine
│   ├── zylos-sentinel/     # AIOps health scanner
│   └── zylos-verification/ # Verification pipeline
├── apps/                   # Binary applications
│   ├── zylos-gateway/      # API gateway
│   └── zylos-cockpit/      # Leptos frontend
├── docs/                   # Documentation
├── runbooks/               # Operational guides
├── playbooks/              # SOAR workflows
├── skills/                 # Skill registry
└── tests/                  # Integration/E2E tests
```

## Development Workflow

### Branch Naming

Use descriptive branch names with type prefix:

```bash
feat/add-memory-consolidation
fix/sentinel-pii-boundary-check
docs/update-deployment-guide
test/approval-equivalence-hash
refactor/orchestrator-dag-runner
perf/reduce-memory-allocations
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `ci`, `chore`

Scopes: crate name or area (`memory`, `triage`, `sentinel`, `gateway`, etc.)

Examples:

```
feat(triage): add VIP sender detection rule
fix(sentinel): correct PII stripping boundary check
docs(handoff): add Fase 7B->7C transition document
test(approval): add equivalence hash collision test
refactor(memory): extract consolidation into separate module
perf(orchestrator): reduce allocations in DAG traversal
```

### Making Changes

1. **Create a branch** from latest `main`:
   ```bash
   git checkout main
   git pull origin main
   git checkout -b feat/your-feature
   ```

2. **Implement changes** with tests:
   - Write tests first when possible (TDD encouraged)
   - All new functionality requires tests
   - Update documentation for user-facing changes

3. **Run checks locally**:
   ```bash
   cargo fmt --all
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```

4. **Commit with conventional message**:
   ```bash
   git add -A
   git commit -m "feat(scope): description"
   ```

5. **Push and open PR**:
   ```bash
   git push origin feat/your-feature
   # Open PR against main via GitHub UI
   ```

## Code Standards

### Rust Style

- Edition 2021, MSRV 1.80
- `clippy -- -D warnings` must pass (no exceptions)
- `rustfmt` formatting enforced in CI
- All public APIs require rustdoc comments
- No `unwrap()` in non-test code — use `?` or explicit error handling
- Prefer `thiserror` for library errors, `anyhow` for application errors
- Use workspace dependencies when available

### Module Structure

```rust
//! Module-level doc comment explaining purpose and design decisions.
//! Include rationale for non-obvious architectural choices.

use serde::{Deserialize, Serialize};

/// Public types with full rustdoc.
/// Explain invariants and usage patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyType {
    /// Field documentation.
    pub field: String,
}

impl MyType {
    /// Constructor with validation.
    pub fn new(field: String) -> Result<Self, MyError> {
        if field.is_empty() {
            return Err(MyError::InvalidInput("field cannot be empty".into()));
        }
        Ok(Self { field })
    }
}

// Private implementation details below.
// Group related private functions together.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_valid() {
        let result = MyType::new("valid".into());
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_empty_rejected() {
        let result = MyType::new(String::new());
        assert!(matches!(result, Err(MyError::InvalidInput(_))));
    }
}
```

### Error Handling

Library crates use `thiserror`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("operation failed")]
    OperationFailed {
        #[source]
        source: anyhow::Error,
    },

    #[error("resource not found: {id}")]
    NotFound { id: String },
}
```

Application crates use `anyhow`:

```rust
fn main() -> anyhow::Result<()> {
    let config = load_config()?;
    let store = MemoryStore::open(&config.db_path)?;
    // ...
    Ok(())
}
```

### Testing

#### Unit Tests

Inline in same file under `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consolidation_merges_duplicates() {
        let mut store = MemoryStore::in_memory();
        store.insert(entry("user prefers dark mode"));
        store.insert(entry("user likes dark theme"));

        let stats = store.consolidate().unwrap();

        assert_eq!(stats.merged, 1);
        assert_eq!(store.count(), 1);
    }
}
```

#### Integration Tests

Cross-module scenarios in `tests/` directory:

```rust
// tests/memory_integration.rs
use zylos_memory::{MemoryStore, MemoryEntry};

#[tokio::test]
async fn test_search_across_agents() {
    let store = MemoryStore::open(":memory:").unwrap();
    // ... test setup and assertions
}
```

#### E2E Tests

Full system tests in `apps/zylos-gateway/tests/e2e_integration.rs`:

```rust
#[tokio::test]
async fn test_health_endpoint() {
    let app = spawn_test_app().await;
    let resp = reqwest::get(format!("{}/gateway/health", app.url))
        .await.unwrap();
    assert_eq!(resp.status(), 200);
}
```

### Documentation

- Update `README.md` for user-facing changes
- Update `docs/ARCHITECTURE.md` for structural changes
- Add runbook entries for operational changes
- Update handoff docs when completing a phase
- All public items need rustdoc comments
- Examples in rustdoc should compile (use `cargo test --doc`)

## Crate Conventions

### Dependencies

```toml
# Cargo.toml
[dependencies]
# Use workspace versions
serde = { workspace = true }
tokio = { workspace = true }
rusqlite = { workspace = true }

# Never add sqlx — workspace uses rusqlite exclusively
# Avoid tokio-test unless already in workspace deps
# Prefer thiserror for lib crates, anyhow for bin crates
```

### Adding a New Crate

1. Create directory under `crates/zylos-your-crate/`
2. Add `Cargo.toml` with workspace inheritance
3. Register in workspace `Cargo.toml` members list
4. Add to CI test matrix
5. Document purpose in crate-level rustdoc

### Feature Flags

Use feature flags for optional functionality:

```toml
[features]
default = []
docker-sandbox = ["bollard"]
wasm-sandbox = ["wasmtime"]
full = ["docker-sandbox", "wasm-sandbox"]
```

Gate conditional compilation:

```rust
#[cfg(feature = "docker-sandbox")]
pub mod docker;
```

## Review Process

### Automated Checks

CI runs on every PR:

- `cargo fmt --check` — formatting
- `cargo clippy --workspace -- -D warnings` — lints
- `cargo test --workspace` — all tests
- `cargo doc --workspace` — documentation builds without warnings
- Coverage report (new code must maintain or improve coverage)

### Human Review

- At least 1 maintainer approval required
- Adversarial audit for verification-related changes
- Architecture review for cross-cutting changes
- Security review for sandbox/auth/network changes

### Merge Criteria

PR can merge when:

- [ ] All CI checks pass
- [ ] Required approvals received
- [ ] No unresolved review comments
- [ ] Commit messages follow conventional format
- [ ] Documentation updated if needed
- [ ] Tests cover new functionality

## Special Areas

### Working on Memory System

- Understand consolidation algorithm before modifying
- All memory operations must be idempotent
- Test with large datasets (10k+ entries) for performance
- Verify shadow-git audit chain captures all mutations

### Working on Verification

- Ground level transitions are security-critical
- Adversarial panel changes need extra review
- Test equivalence hashing with edge cases (null fields, unicode, etc.)
- Safety-tier immutability must never be weakened

### Working on Sandbox

- WASM and Docker tiers have independent security models
- Resource limits must be enforced at tier boundary
- Test escape attempts (filesystem, network, process spawning)
- Browser anti-detect changes need fingerprint testing

### Working on Frontend (Cockpit)

- Leptos 0.7 with SSR
- Components in `apps/zylos-cockpit/src/components/`
- State management via signals
- Test with `wasm-pack test --headless`

## Getting Help

- **Questions**: GitHub Discussions
- **Bugs**: GitHub Issues with reproduction steps
- **Architecture**: Check `docs/ARCHITECTURE.md` first
- **Operations**: Check `runbooks/` for troubleshooting
- **Real-time**: Join our Discord (link in README)

## Code of Conduct

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Be respectful, inclusive, and constructive. Report violations to conduct@nousresearch.com.

## License

Contributions are licensed under MIT. By submitting a PR, you agree to license your work under the project's license.
