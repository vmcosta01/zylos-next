# zylos-core

Foundational types and shared utilities for Zylos-Next v6.

## Architecture

This is a minimal foundation crate providing common error types, serialization primitives, and tracing setup used across the Zylos workspace. It intentionally has no modules beyond the root `lib.rs`.

### Current Exports

The crate currently re-exports from its single-module root:

```rust
//! zylos-core — parte do Zylos-Next v6
```

As the foundational crate, new shared types (error enums, trait definitions, config structs) should be added here to avoid circular dependencies between higher-level crates.

## Usage

```rust
use zylos_core::*;

// Shared error types and utilities available workspace-wide
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `serde` | Serialization framework |
| `thiserror` | Ergonomic error type derivation |
| `tracing` | Structured diagnostic logging |

## Testing

```bash
cargo test -p zylos-core
```

No dev-dependencies currently configured.

## Related Crates

- Depended on by: `zylos-skills`, `zylos-vlm-ocr`
- This crate has no internal dependencies (it is a leaf dependency)