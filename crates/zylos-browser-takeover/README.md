# zylos-browser-takeover

Browser takeover console for zylos-gateway: live CDP WebSocket proxy with full message audit, session handoff protocol (agent ↔ human), SQLite-backed audit trail, HITL approval workflow, and post-incident replay.

## Quick Start

```rust
use std::path::Path;
use std::sync::Arc;
use zylos_browser_takeover::{
    TakeoverStore, TakeoverRequest, TakeoverDecision, Initiator, CdpProxy,
};

// Open persistent store (WAL mode, auto-migrations)
let store = Arc::new(TakeoverStore::open(Path::new("takeover.db"))?);

// Or in-memory for tests
// let store = Arc::new(TakeoverStore::in_memory()?);

// Create a takeover request (starts as PendingApproval)
let req = TakeoverRequest {
    agent_id: "agent-42".into(),
    browser_target_url: "https://app.example.com".into(),
    cdp_ws_url: "ws://localhost:9222/devtools/page/abc123".into(),
    reason: "Payment form requires human verification".into(),
    initiated_by: Initiator::Agent,
};
let session = store.create_session(&req).await?;

// Human approves → transitions to Active
let decision = TakeoverDecision {
    session_id: session.id.clone(),
    approved: true,
    approver: "alice".into(),
    comment: None,
};
store.approve_session(&decision).await?;

// Proxy CDP traffic with full audit
let proxy = CdpProxy::new(store.clone(), session.id.clone());
let live_rx = proxy.subscribe_live(); // real-time broadcast
// proxy.run(client_ws, &session.cdp_ws_url).await?;
```

## Types

| Type | Description |
|------|-------------|
| `TakeoverSession` | Full lifecycle of a handoff session (id, agent, status, timestamps, message count) |
| `TakeoverStatus` | Enum: `PendingApproval`, `Active`, `Rejected`, `Completed`, `Terminated` |
| `Initiator` | Who requested the handoff: `Agent`, `Human`, `System` |
| `MessageDirection` | CDP message flow: `Incoming` (client→browser) or `Outgoing` (browser→client) |
| `AuditedMessage` | Single captured CDP frame with method, payload size, timestamp, direction |
| `TakeoverRequest` | Payload to initiate a new session |
| `TakeoverDecision` | Human approve/reject decision with optional comment |
| `TakeoverDashboard` | Aggregate stats: total/active/pending/completed/rejected sessions + message count |
| `TakeoverStore` | Channel-based SQLite WAL backend; all DB ops serialized on blocking thread |
| `CdpProxy` | Bidirectional WebSocket proxy that audits every frame to store + broadcast |

## Error Variants (`TakeoverError`)

| Variant | Cause |
|---------|-------|
| `Database(rusqlite::Error)` | SQLite failure |
| `NotFound(String)` | Session ID does not exist |
| `InvalidStatus(String)` | Unrecognized status string during parsing |
| `InvalidInitiator(String)` | Unrecognized initiator string during parsing |
| `WebSocket(tungstenite::Error)` | CDP WebSocket transport failure |
| `Io(std::io::Error)` | Filesystem / I/O error |
| `Serialization(serde_json::Error)` | JSON encode/decode failure |
| `AlreadyActive(String)` | A session for the same agent is already active |
| `ChannelClosed` | Internal mpsc/oneshot channel dropped unexpectedly |

## Gaps

- No automatic session timeout / TTL enforcement — `Terminated` status exists but nothing transitions sessions to it automatically.
- `CdpProxy::run` always ends sessions as `Completed`; no way to signal `Terminated` from the proxy itself.
- No authentication or authorization on the proxy endpoint — relies entirely on upstream gateway.
- Broadcast channel has fixed capacity (1024); slow subscribers silently drop messages.
- No pagination on `get_messages` beyond `limit`; no cursor-based streaming for large sessions.
- `extract_cdp_method` only handles top-level `"method"` field; nested or non-standard CDP frames return `None`.
- No replay API exposed — messages are stored but there is no public method to reconstruct a session timeline beyond `get_messages`.
- Migrations are embedded; no versioned migration framework for schema evolution.

## License

Same as the parent workspace.
