# zylos-evidence-ledger

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
