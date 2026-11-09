# zylos-memory-sync

Cross-harness memory synchronization for zylos-gateway. Unified memory layer that synchronizes agent memory across frameworks (ADK, Claude SDK, CrewAI, LangGraph, LlamaIndex) using content-addressed storage with SHA-256 deduplication and sync operations for eventual consistency.

## Quick Start

```rust
use zylos_memory_sync::{MemoryStore, MemoryEntry, MemoryQuery, SyncOpType};

// Open store (SQLite WAL, auto-migrations)
let store = MemoryStore::open("memory-sync.db").await?;

// Upsert a memory entry (content-addressed by SHA-256)
let entry = MemoryEntry::new(
    "agent:claude-sdk:researcher",
    "user-preferences",
    r#"{"theme":"dark","lang":"pt-BR"}"#,
    "claude-sdk",
);
let changed = store.upsert(&entry).await?; // true if content changed

// Retrieve by namespace + key
let retrieved = store.get("agent:claude-sdk:researcher", "user-preferences").await?;

// Query with filters
let results = store.query(&MemoryQuery {
    namespace: Some("agent:claude-sdk:researcher".into()),
    tag: Some("preferences".into()),
    limit: Some(20),
    ..Default::default()
}).await?;

// Record sync operation for cross-harness propagation
let op_id = store.record_sync_op(&entry.id, SyncOpType::Upsert, "claude-sdk").await?;

// Purge expired entries (TTL-based)
let purged = store.purge_expired().await?;

// Stats
let stats = store.stats().await?;
```

## Types

| Type | Description |
|------|-------------|
| `MemoryEntry` | Content-addressed entry: SHA-256 id, namespace, key, content, version, source_harness, tags, TTL |
| `SyncOp` | Propagation operation: op_id, entry_id, type, timestamp, source harness |
| `SyncOpType` | Enum: `Upsert`, `Delete`, `Touch` |
| `SyncResult` | Reconciliation outcome: upserted/deleted/conflicts/errors counts |
| `MemoryQuery` | Filter: namespace, key_prefix, tag substring, limit, offset |
| `MemoryStats` | Aggregate: total entries, pending sync ops, namespace breakdown |
| `MemoryStore` | SQLite WAL-backed store with content-addressed dedup + sync op tracking |

## Error Variants (`MemorySyncError`)

| Variant | Cause |
|---------|-------|
| `Database(sqlx::Error)` | SQLite failure |
| `Serialization(serde_json::Error)` | JSON encode/decode of tags or content |
| `NotFound(String)` | Entry lookup miss |
| `Conflict(String)` | Divergent versions detected during merge |
| `Io(std::io::Error)` | Filesystem error |
| `Internal(String)` | Invariant violation |

## Gaps

- `Conflict` error variant exists but is never returned — current `upsert` uses last-writer-wins silently.
- `SyncResult` type is defined but no reconciliation function produces it.
- `mark_ops_applied` uses lexicographic comparison on UUID op_ids (`op_id <= ?`) — not chronological ordering.
- Tags stored as JSON string; substring LIKE matching can false-positive on partial tag names.
- `purge_expired` uses SQLite datetime arithmetic; timezone edge cases possible with non-UTC timestamps.
- No batch upsert API — each entry requires a separate round-trip.
- `get_pending_ops` has no ordering guarantee beyond timestamp ASC; concurrent writers may produce non-deterministic order.
- Content-addressed ID includes content hash, but `upsert` looks up by (namespace, key) — changing content generates a new ID while the old row is updated in-place, making the ID field misleading after updates.
- No encryption at rest for sensitive memory content.
- Cursor management (`update_cursor` / `get_cursor`) is manual; no automatic cursor advancement tied to `mark_ops_applied`.

## License

This crate is part of the zylos-next workspace. See the repository root for licensing terms.
