# Security Model — Zylos-Next v6.0 (Task 0.1.6)

## Context
Defines the security architecture for the 6 sovereign core crates. This document specifies the trust boundaries, capability model, sandbox isolation guarantees, secret management, and audit requirements. All security controls are enforced at compile-time via Rust's type system where possible, with runtime validation as defense-in-depth.

## Trust Boundaries

```
┌─────────────────────────────────────────────────────────┐
│                    UNTRUSTED ZONE                        │
│  External clients, MCP tools, LLM outputs, user input   │
└──────────────────────┬──────────────────────────────────┘
                       │ Validation Gate (serde + jsonschema)
┌──────────────────────▼──────────────────────────────────┐
│                  GATEWAY BOUNDARY                         │
│  AuthN/AuthZ, rate limiting, envelope signing            │
└──────────────────────┬──────────────────────────────────┘
                       │ Typed Envelope<T> (validated)
┌──────────────────────▼──────────────────────────────────┐
│                 TRUSTED CORE ZONE                        │
│  BotSystem, MemorySystem, LiveComm, Verification         │
│  (Rust type safety, no raw string parsing)               │
└──────────────────────┬──────────────────────────────────┘
                       │ Capability Token (scoped)
┌──────────────────────▼──────────────────────────────────┐
│                SANDBOX ISOLATION ZONE                     │
│  WasmSandbox (WASI), process isolation, resource limits  │
│  No network, no filesystem, no ambient authority         │
└─────────────────────────────────────────────────────────┘
```

## Capability-Based Security Model

All cross-system access is mediated by capability tokens, not ambient authority:

```rust
/// Scoped capability token — grants specific access to a resource
#[derive(Serialize, Deserialize, Clone)]
pub struct Capability {
    /// Unique capability identifier
    pub id: String,
    /// Resource this capability grants access to
    pub resource: Resource,
    /// Allowed operations on the resource
    pub permissions: Vec<Permission>,
    /// Expiration timestamp (UTC)
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// HMAC-SHA256 signature from issuing authority
    pub signature: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "id")]
pub enum Resource {
    MemoryScope(String),        // e.g., "agent:bot-01:long_term"
    SandboxInstance(String),    // e.g., "sandbox:exec-7f3a"
    Skill(String),              // e.g., "skill:web-search:v2"
    DagWorkflow(String),        // e.g., "dag:email-triage"
    HermesEndpoint(String),     // e.g., "hermes:kanban:dispatch"
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Read,
    Write,
    Execute,
    Create,
    Delete,
    Stream,
}
```

### Capability Issuance Rules
| Issuer | Can Issue For | Max TTL | Revocation |
|--------|--------------|---------|------------|
| Gateway | All resources | 24h | Immediate via revocation list |
| BotSystem | Own agent scopes, spawned sandboxes | 1h | On agent termination |
| MemorySystem | Own scopes, vector stores | 4h | On scope deletion |
| Verification | Proof read-only, panel results | 30min | Auto-expire |
| Skills | Own skill schemas, eval results | 2h | On skill deregistration |

## Sandbox Isolation Guarantees

The `zylos-sandbox` crate enforces hard isolation for untrusted code execution:

| Guarantee | Mechanism | Enforcement |
|-----------|-----------|-------------|
| No network access | WASI preview2 without `wasi:sockets` | Compile-time (no import) |
| No filesystem access | Virtual FS only, no `wasi:filesystem` | Runtime + compile-time |
| CPU time limit | Epoch-based interruption (10ms granularity) | Wasmtime config |
| Memory limit | Linear memory cap (64MB default) | Wasmtime config |
| No ambient authority | Capability passing only | Type system |
| Deterministic execution | No clocks, no randomness (unless granted) | WASI shim |
| Snapshot integrity | SHA-256 hash of snapshot bytes | Runtime verification |

### Sandbox Resource Defaults
```toml
[sandbox.defaults]
max_memory_mb = 64
max_cpu_time_ms = 5000
max_wall_time_ms = 30000
max_open_files = 0          # No file access by default
max_network_connections = 0 # No network by default
epoch_interruption = true
epoch_ticks = 10_000        # ~10ms per tick
```

## Secret Management

Secrets never touch disk in plaintext and never appear in logs or events:

| Secret Type | Storage | Access Pattern | Rotation |
|-------------|---------|----------------|----------|
| LLM API keys | OS keyring (Windows Credential Manager / macOS Keychain) | In-memory only, zeroized on drop | Manual via CLI |
| Capability signing keys | Embedded in binary at build time (HMAC key) | Const generic, never serialized | Rebuild binary |
| Agent session tokens | In-memory HashMap, zeroized on agent stop | Per-agent scope, non-transferable | On agent lifecycle end |
| Hermes OAuth tokens | OS keyring via `manage_connections` | Refreshed automatically | Via Hermes connector |
| Encryption keys (memory-at-rest) | Derived from master key + scope salt | AES-256-GCM, nonce per record | On scope reset |

### Zeroization Policy
All types holding secrets implement `zeroize::Zeroize` and `zeroize::ZeroizeOnDrop`:
```rust
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct ApiKey(Vec<u8>);

impl ApiKey {
    pub fn as_str(&self) -> &str {
        // Safe: Vec<u8> is valid UTF-8 by construction
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }
}
```

## Audit & Observability

All security-relevant events are logged to an append-only audit log:

| Event | Logged Fields | Retention |
|-------|--------------|-----------|
| Capability issued | issuer, resource, permissions, ttl, correlation_id | 90 days |
| Capability revoked | capability_id, reason, revoked_by | 90 days |
| Sandbox created/terminated | sandbox_id, agent_id, resource_limits, duration | 90 days |
| Validation failure | endpoint, field, reason, source_ip (if external) | 30 days |
| AuthN/AuthZ decision | principal, resource, action, verdict, latency_ms | 90 days |
| Secret access | key_type, accessor_system, success/failure | 90 days |
| Contradiction detected | node_ids, resolution_strategy, verifier | Permanent |

Audit logs are structured JSON, written to `~/.zylos/audit.jsonl`, rotated at 100MB, compressed after rotation.

## Threat Model Summary

| Threat | Mitigation | Crate |
|--------|-----------|-------|
| Prompt injection → unauthorized tool use | Capability scoping + Verification guardrails | BotSystem + Verification |
| Memory poisoning via contradictory claims | Contradiction detection + Proof requirement | MemorySystem + Verification |
| Sandbox escape via Wasm vulnerability | Wasmtime pinned version + epoch interruption + no unsafe imports | Sandbox |
| Secret leakage via logs/events | Zeroization + structured logging filter | All |
| Replay attack on inter-crate messages | UUIDv7 + timestamp + HMAC envelope | Core |
| Denial of service via unbounded DAG | Node timeout + global concurrency limit | BotSystem |
| Cross-agent memory bleed | Scope isolation + capability enforcement | MemorySystem |
| Hermes plugin compromise | Adapter-only integration + schema validation | Gateway |

## Acceptance Criteria
- [ ] Capability type compiles with serde + zeroize dependencies
- [ ] Sandbox defaults enforce no-network, no-fs at Wasmtime config level
- [ ] All secret-holding types derive ZeroizeOnDrop
- [ ] Audit logger writes valid JSONL to configured path
- [ ] Integration test: capability without Write permission rejected on write attempt
- [ ] Integration test: sandbox cannot open TCP socket (returns WASI error)
- [ ] Security review checklist signed off in `evals/audits/fase0-security-review.md`

## Out of Scope
- Multi-tenant isolation (single-user for v1.0)
- Hardware-backed key storage (TPM/Secure Enclave)
- End-to-end encryption of inter-crate messages (trusted core zone)
- External penetration testing (deferred to post-v1.0)
- Formal verification of sandbox isolation (research track)

## References
- [COMMUNICATION-CONTRACT.md](./COMMUNICATION-CONTRACT.md) — Envelope & transport security
- [DESIGN.md](./DESIGN.md) — Typed schemas for capabilities and audit events
- [ARCHITECTURE.md](../ARCHITECTURE.md) — Trust boundary diagram
- [Master Plan §1.6](../IMPLEMENTATION_PLAN.md) — Security-first design principles