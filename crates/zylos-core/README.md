# zylos-core

Foundational types shared across all Zylos-Next crates.

## Overview

This crate provides the common error types, identifiers, and trait definitions that every other crate in the workspace depends on. It has **no dependencies on other `zylos-*` crates** to avoid circular imports.

## Quick Start

```rust
use zylos_core::{AgentId, CoreError, Result};

// Create an agent identifier
let id = AgentId::new("worker-1");
assert_eq!(id.as_str(), "worker-1");

// Use ergonomic error constructors
let err = CoreError::not_found("agent", "worker-42");
assert!(err.to_string().contains("not found"));

// Workspace-wide Result alias
fn lookup() -> Result<AgentId> {
    Err(CoreError::not_found("agent", "missing"))
}
```

## Core Types

| Type | Description |
|------|-------------|
| `AgentId` | Unique identifier for an agent (wraps `String`, implements `Hash`, `Serialize`, `Deserialize`) |
| `CoreError` | Top-level error enum with `NotFound`, `InvalidState`, `Config`, `External` variants |
| `Result<T>` | Workspace-wide result alias using `CoreError` |

## Error Variants

| Variant | Constructor | Example Message |
|---------|-------------|-----------------|
| `NotFound` | `CoreError::not_found(entity, detail)` | `"agent not found: worker-42"` |
| `InvalidState` | `CoreError::invalid_state(operation, reason)` | `"invalid state for spawn: already running"` |
| `Config` | `CoreError::config(message)` | `"config error: missing ZYLOS_HOME"` |
| `External` | `CoreError::external(source, message)` | `"external error [docker]: connection refused"` |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde` | Serialization/deserialization of `AgentId` |
| `thiserror` | Ergonomic `Display` and `Error` impls for `CoreError` |
| `tracing` | Structured diagnostic logging (re-exported) |

### Dev Dependencies

| Crate | Purpose |
|-------|---------|
| `serde_json` | Serde roundtrip tests for `AgentId` |

## Testing

```bash
cargo test -p zylos-core
```

14 unit tests + 1 doc-test covering:
- `AgentId`: construction, display, `From` conversions, equality/hash, serde roundtrip
- `CoreError`: display format for all 4 variants, clone equality
- `Result`: ok and err paths

## Related Crates

- Depended on by: `zylos-skills`, `zylos-vlm-ocr`, and all higher-level crates
- This crate has no internal dependencies (leaf dependency)