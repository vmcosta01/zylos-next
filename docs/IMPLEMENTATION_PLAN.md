# Zylos-Gateway Competitive Gap Implementation Plan

**Date:** 2026-11-05
**Source:** `COMPETITIVE_GAP_ANALYSIS.md` (9 repos, 37 gaps)
**Owner:** Victor M. Costa
**Status:** Draft — awaiting execution approval

---

## Overview

This document provides an exhaustive, actionable implementation plan for every gap identified in the competitive analysis. Each gap includes: crate/module location, public API surface, data model changes, dependencies, test strategy, acceptance criteria, estimated effort, and integration points with existing zylos layers.

Gaps are grouped into 4 phases matching the priority matrix. Within each phase, items are ordered by dependency (foundational crates first).

---

## Phase 1: Critical Gaps (Weeks 1–4)

### 1.1 `zylos-integrations` Crate — SaaS OAuth Catalog

| Field | Detail |
|-------|--------|
| **Gap** | 🔴 Critical — Pre-built SaaS integration catalog (200+) |
| **Source** | Rerun, OpenBot |
| **Crate** | `crates/zylos-integrations/` |
| **Effort** | 8 days |

#### Architecture

```
zylos-integrations/
├── src/
│   ├── lib.rs              # Public API: IntegrationRegistry, OAuthFlow
│   ├── oauth/
│   │   ├── mod.rs          # OAuth2 PKCE + device flow engine
│   │   ├── token_store.rs  # AES-256-GCM encrypted token persistence
│   │   └── refresh.rs      # Auto-refresh with jitter + retry
│   ├── providers/
│   │   ├── mod.rs          # Provider trait + registry macro
│   │   ├── gmail.rs        # Gmail API v1 scopes
│   │   ├── slack.rs        # Slack Web API + Socket Mode
│   │   ├── notion.rs       # Notion API v2022-06-28
│   │   ├── linear.rs       # Linear GraphQL API
│   │   ├── jira.rs         # Jira Cloud REST API v3
│   │   ├── figma.rs        # Figma REST API v1
│   │   ├── stripe.rs       # Stripe API v2024-12-18
│   │   └── shopify.rs      # Shopify Admin API 2024-10
│   └── manifest.rs         # YAML spec parser for declarative integrations
├── specs/                  # Declarative YAML integration definitions
│   ├── gmail.yaml
│   ├── slack.yaml
│   └── ...
└── tests/
    ├── oauth_flow_test.rs
    └── provider_registry_test.rs
```

#### Public API

```rust
pub struct IntegrationRegistry { /* ... */ }

impl IntegrationRegistry {
    pub fn from_specs_dir(path: &Path) -> Result<Self>;
    pub fn get(&self, slug: &str) -> Option<&IntegrationSpec>;
    pub fn list(&self) -> Vec<&IntegrationSpec>;
    pub async fn initiate_oauth(&self, slug: &str, redirect_uri: &str) -> Result<OAuthSession>;
    pub async fn complete_oauth(&self, session_id: &str, code: &str) -> Result<TokenBundle>;
    pub async fn execute_action(&self, slug: &str, action: &str, params: Value) -> Result<Value>;
}

pub trait Provider: Send + Sync {
    fn slug(&self) -> &str;
    fn scopes(&self) -> &[&str];
    fn base_url(&self) -> &str;
    async fn execute(&self, token: &TokenBundle, action: &str, params: Value) -> Result<Value>;
}
```

#### Data Model (SQLite WAL)

```sql
CREATE TABLE integration_tokens (
    id TEXT PRIMARY KEY,           -- UUID
    user_id TEXT NOT NULL,
    provider_slug TEXT NOT NULL,
    access_token_encrypted BLOB NOT NULL,  -- AES-256-GCM
    refresh_token_encrypted BLOB,
    expires_at INTEGER NOT NULL,
    scopes TEXT NOT NULL,          -- JSON array
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(user_id, provider_slug)
);

CREATE TABLE integration_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT NOT NULL,
    provider_slug TEXT NOT NULL,
    action TEXT NOT NULL,
    decided_by TEXT NOT NULL,      -- 'user' | 'policy' | 'auto'
    decision_hash TEXT NOT NULL,   -- SHA-256 of (action + params)
    executed_at INTEGER NOT NULL,
    status TEXT NOT NULL           -- 'approved' | 'denied' | 'error'
);
```

#### Dependencies

- `oauth2 = "4"` — OAuth2 PKCE/device flow
- `aes-gcm = "0.10"` — Token encryption at rest
- `reqwest = { version = "0.12", features = ["json"] }` — HTTP client
- `serde_yaml = "0.9"` — Manifest parsing
- `zylos-hitl` — Approval gates for sensitive actions
- `zylos-sentinel` — PII stripping on outbound requests

#### Test Strategy

- Unit: Token encrypt/decrypt round-trip, refresh jitter logic
- Integration: Mock OAuth server testing full PKCE flow
- E2E: Real Gmail/Slack sandbox accounts verifying token lifecycle
- Security: Verify tokens never appear in logs or error messages

#### Acceptance Criteria

1. ✅ 8 providers ship with working OAuth flows
2. ✅ Tokens encrypted at rest with AES-256-GCM, key derived from machine secret
3. ✅ Every action logged to `integration_audit_log` with `decided_by` field
4. ✅ HITL approval required for write operations on first use per provider
5. ✅ Auto-refresh handles expired tokens transparently with <1% failure rate
6. ✅ New providers addable via YAML spec without Rust code changes

---

### 1.2 WeChat/Feishu Channel Adapters

| Field | Detail |
|-------|--------|
| **Gap** | 🔴 Critical — CN market distribution channels |
| **Source** | AnyModelBot |
| **Module** | `crates/zylos-router/src/channels/wechat.rs`, `feishu.rs` |
| **Effort** | 5 days |

#### Architecture

```rust
// crates/zylos-router/src/channels/mod.rs
pub trait ChannelAdapter: Send + Sync {
    fn name(&self) -> &str;
    async fn receive(&self, webhook_payload: Bytes) -> Result<InboundMessage>;
    async fn send(&self, target: &str, message: OutboundMessage) -> Result<()>;
    async fn verify_webhook(&self, challenge: &str) -> Result<String>;
}
```

#### WeChat Official Account Adapter

- **Webhook verification**: SHA-1 signature validation per WeChat docs
- **Message types**: text, image, voice, video, file, template_message
- **API calls**: `cgi-bin/message/send`, `cgi-bin/media/upload`, `cgi-bin/user/info`
- **Rate limits**: 48h customer service window enforcement, template message quota tracking
- **Encryption**: AES-CBC message body decryption with EncodingAESKey

#### Feishu (Lark) Adapter

- **Event subscription**: v2.0 event schema with Encrypt + VerificationToken
- **Message card**: Interactive card JSON builder with button/callback support
- **Bot API**: `im/v1/messages`, `contact/v3/users`, `drive/v1/files`
- **Approval**: Feishu native approval instance creation for HITL escalation

#### Data Model

```sql
CREATE TABLE channel_sessions (
    id TEXT PRIMARY KEY,
    channel_type TEXT NOT NULL,     -- 'wechat' | 'feishu' | 'slack' | 'telegram'
    external_user_id TEXT NOT NULL,
    zylos_user_id TEXT NOT NULL,
    metadata TEXT,                  -- JSON: locale, timezone, preferences
    last_active_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
```

#### Test Strategy

- Unit: Message parsing, signature verification, encryption/decryption
- Integration: WeChat/Feishu sandbox environments with real webhook delivery
- Load: 1000 msg/s ingestion benchmark with backpressure

#### Acceptance Criteria

1. ✅ WeChat text/image/file messages received and routed to zylos-agent
2. ✅ Feishu interactive cards with callback buttons functional
3. ✅ Rate limit enforcement prevents API bans
4. ✅ Session persistence across reconnects
5. ✅ HITL approvals routable back through originating channel

---

### 1.3 Per-Agent Persistent Browser Sessions

| Field | Detail |
|-------|--------|
| **Gap** | 🔴 Critical — Cookie/session persistence across turns |
| **Source** | OpenBot, OpenMuse |
| **Module** | `crates/zylos-sandbox/src/browser/persistent_session.rs` |
| **Effort** | 6 days |

#### Architecture

```rust
pub struct PersistentBrowserSession {
    agent_id: String,
    profile_dir: PathBuf,        // /data/browser-profiles/{agent_id}/
    cookie_jar: EncryptedCookieJar,
    cdp_session: CdpSession,
}

impl PersistentBrowserSession {
    pub async fn open(agent_id: &str, config: &BrowserConfig) -> Result<Self>;
    pub async fn navigate(&self, url: &str) -> Result<PageState>;
    pub async fn cookies(&self) -> Vec<Cookie>;
    pub async fn set_cookies(&self, cookies: &[Cookie]) -> Result<()>;
    pub async fn local_storage(&self) -> HashMap<String, String>;
    pub async fn screenshot(&self) -> Result<Vec<u8>>;
    pub async fn close(self) -> Result<()>;  // Persists profile to disk
}
```

#### Storage Layout

```
/data/browser-profiles/{agent_id}/
├── cookies.db          # SQLite encrypted cookie store
├── local_storage.db    # localStorage persistence
├── cache/              # Chromium cache partition
├── gpu_cache/
└── session_state.json  # Tab history, scroll positions, form autofill
```

#### Security Boundaries

- Each agent gets isolated Chromium `--user-data-dir`
- Cookies encrypted with per-agent key derived from `zylos-vault` master key
- Network egress filtered through `zylos-sentinel` PII detector
- No cross-agent cookie sharing; explicit transfer requires HITL approval
- Profile directories chowned to sandbox UID, mode 0700

#### Dependencies

- `chromiumoxide = "0.7"` — CDP client
- `rusqlite = { version = "0.32", features = ["bundled"] }` — Cookie storage
- `zylos-vault` — Key derivation for cookie encryption
- `zylos-sentinel` — Outbound request inspection

#### Test Strategy

- Unit: Cookie encrypt/decrypt, profile dir isolation
- Integration: Login to test site → close browser → reopen → verify session alive
- Security: Attempt cross-agent cookie access → must fail
- Persistence: Kill process mid-session → restart → verify state recovery

#### Acceptance Criteria

1. ✅ Agent logs into site A, closes browser, reopens → still logged in
2. ✅ Cookies encrypted at rest, unreadable without vault key
3. ✅ Agent A cannot access Agent B's cookies or localStorage
4. ✅ PII in outbound requests flagged by sentinel before transmission
5. ✅ Session state survives gateway restart (graceful shutdown persists profile)
6. ✅ <50ms overhead per navigation vs non-persistent browser

---

### 1.4 AG-UI Protocol Adapter

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Universal agent interop protocol |
| **Source** | OpenBot, OpenMuse |
| **Crate** | `crates/zylos-ag-ui/` |
| **Effort** | 7 days |

#### Architecture

```rust
// AG-UI Protocol Types (per spec v0.1)
pub enum AgUiEvent {
    TextMessageStart { message_id: String, role: Role },
    TextMessageContent { message_id: String, delta: String },
    TextMessageEnd { message_id: String },
    ToolCallStart { tool_call_id: String, tool_name: String, args: Value },
    ToolCallResult { tool_call_id: String, result: Value, error: Option<String> },
    StateSnapshot { snapshot: Value },
    StateDelta { patch: JsonPatch },
    RunStarted { run_id: String, input: Value },
    RunFinished { run_id: String, output: Value },
    Error { code: String, message: String },
}

pub struct AgUiAdapter {
    gateway: GatewayHandle,
    codec: AgUiCodec,
}

impl AgUiAdapter {
    pub fn new(gateway: GatewayHandle) -> Self;
    pub async fn handle_sse_stream(&self, stream: impl Stream<Item = AgUiEvent>) -> Result<()>;
    pub async fn emit_event(&self, event: AgUiEvent) -> Result<()>;
    pub fn to_zylos_message(&self, event: &AgUiEvent) -> Option<ZylosMessage>;
    pub fn from_zylos_message(&self, msg: &ZylosMessage) -> Vec<AgUiEvent>;
}
```

#### Integration Points

- **zylos-gateway**: Mount `/api/v1/ag-ui/stream` SSE endpoint
- **zylos-live-comm**: Translate AG-UI events ↔ internal EventBus messages
- **zylos-cockpit**: Render AG-UI events in real-time UI
- **zylos-proof**: Attach proof metadata to `ToolCallResult` events

#### Wire Format

```
POST /api/v1/ag-ui/stream
Accept: text/event-stream
Content-Type: application/json

event: TEXT_MESSAGE_START
data: {"message_id":"msg_abc","role":"assistant"}

event: TEXT_MESSAGE_CONTENT  
data: {"message_id":"msg_abc","delta":"Hello "}

event: TOOL_CALL_START
data: {"tool_call_id":"tc_123","tool_name":"web_search","args":{"q":"zylos"}}
```

#### Test Strategy

- Unit: Event serialization/deserialization round-trip
- Integration: Bidirectional translation between zylos messages and AG-UI events
- Interop: Connect to reference AG-UI server/client implementations
- Fuzz: Malformed event handling, oversized payloads, invalid UTF-8

#### Acceptance Criteria

1. ✅ Full AG-UI v0.1 event set implemented (12 event types)
2. ✅ Bidirectional translation preserves semantic meaning
3. ✅ SSE streaming with <50ms TTFD
4. ✅ Proof metadata embedded in tool results
5. ✅ Graceful degradation on unknown event types (log + skip)
6. ✅ Reference interop test passes against 2+ external AG-UI implementations

---

## Phase 2: High-Priority UX (Weeks 5–8)

### 2.1 Electron/Tauri Desktop Shell (`zylos-desktop`)

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Native desktop app with tray persistence |
| **Source** | AnyModelBot, OpenBot |
| **Crate** | `apps/zylos-desktop/` (Tauri v2) |
| **Effort** | 10 days |

#### Architecture

```
apps/zylos-desktop/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Tauri entrypoint
│   │   ├── gateway_embed.rs  # Embed zylos-gateway binary
│   │   ├── tray.rs           # System tray with status/menu
│   │   ├── workspace.rs      # Per-bot workspace management
│   │   └── notifications.rs  # OS-native notification bridge
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                      # SolidJS frontend
│   ├── App.tsx
│   ├── components/
│   │   ├── ChatPanel.tsx
│   │   ├── AgentRoster.tsx
│   │   ├── WorkspaceManager.tsx
│   │   └── SettingsPanel.tsx
│   └── ipc.ts                # Tauri invoke wrappers
└── package.json
```

#### Features

- **Embedded gateway**: `zylos-gateway` binary bundled, auto-started on app launch
- **System tray**: Status indicator (green/yellow/red), quick actions (new chat, pause all, quit)
- **Per-bot workspaces**: `~/Zylos/Agents/{bot_id}/` with isolated memory/skills/browser profiles
- **Auto-update**: Tauri updater checking GitHub Releases every 4 hours
- **Deep links**: `zylos://chat/{bot_id}`, `zylos://approve/{request_id}`
- **Offline mode**: Queue messages when gateway unreachable, replay on reconnect

#### IPC Bridge

```typescript
// Frontend → Rust
invoke('start_gateway', { configPath: string }): Promise<void>
invoke('stop_gateway'): Promise<void>
invoke('get_gateway_status'): Promise<{ running: boolean, pid: number, uptime: number }>
invoke('list_workspaces'): Promise<Workspace[]>
invoke('create_workspace', { botId: string }): Promise<Workspace>
invoke('open_browser_takeover', { sessionId: string }): Promise<void>

// Rust → Frontend (events)
'gateway-status-changed': { running: boolean }
'notification': { title: string, body: string, action?: string }
'hitl-request': { requestId: string, summary: string, urgency: string }
```

#### Test Strategy

- Unit: IPC serialization, workspace path resolution
- Integration: Gateway embed start/stop lifecycle
- E2E: Install on macOS/Windows/Linux, verify tray, deep links, auto-update
- Performance: Cold start <3s, memory <150MB idle

#### Acceptance Criteria

1. ✅ Single-click install on macOS (.dmg), Windows (.exe/.msi), Linux (.AppImage/.deb)
2. ✅ Gateway auto-starts with app, auto-stops on quit
3. ✅ System tray shows real-time status, quick actions functional
4. ✅ Per-bot workspace isolation verified (no cross-contamination)
5. ✅ Auto-update downloads and installs without user intervention
6. ✅ Deep links open correct chat/approval screens
7. ✅ Offline queue replays correctly on reconnect

---

### 2.2 `zylos-goals` Crate — Milestones & Page Watchers

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Goals/milestones with recurring monitors |
| **Source** | OpenMuse |
| **Crate** | `crates/zylos-goals/` |
| **Effort** | 5 days |

#### Data Model

```sql
CREATE TABLE goals (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    target_date INTEGER,
    status TEXT NOT NULL DEFAULT 'active',  -- active|completed|paused|archived
    parent_goal_id TEXT REFERENCES goals(id),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE milestones (
    id TEXT PRIMARY KEY,
    goal_id TEXT NOT NULL REFERENCES goals(id),
    title TEXT NOT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    completed_at INTEGER,
    sort_order INTEGER NOT NULL,
    evidence_url TEXT,                     -- Link to proof artifact
    verified_by TEXT                       -- 'user' | 'agent' | 'proof_system'
);

CREATE TABLE page_watchers (
    id TEXT PRIMARY KEY,
    goal_id TEXT REFERENCES goals(id),
    url TEXT NOT NULL,
    css_selector TEXT,                     -- Optional: monitor specific element
    check_interval_secs INTEGER NOT NULL DEFAULT 3600,
    last_checked_at INTEGER,
    last_value TEXT,                       -- Hash of monitored content
    alert_on_change BOOLEAN NOT NULL DEFAULT TRUE,
    alert_threshold TEXT,                  -- JSON: { operator: "gt", value: 100 }
    created_at INTEGER NOT NULL
);
```

#### Public API

```rust
pub struct GoalService { /* ... */ }

impl GoalService {
    pub async fn create_goal(&self, req: CreateGoalRequest) -> Result<Goal>;
    pub async fn add_milestone(&self, goal_id: &str, req: AddMilestoneRequest) -> Result<Milestone>;
    pub async fn mark_milestone_complete(&self, milestone_id: &str, evidence: Option<&str>) -> Result<()>;
    pub async fn register_watcher(&self, req: RegisterWatcherRequest) -> Result<PageWatcher>;
    pub async fn check_watchers(&self) -> Vec<WatcherAlert>;  // Called by cron
    pub async fn get_progress(&self, goal_id: &str) -> GoalProgress;
}
```

#### Cron Integration

- Register watcher checks as `zylos-soar` scheduled tasks
- On change detection → create SOAR incident or notify via `zylos-live-comm`
- Watcher results stored in `page_watcher_history` table for trend analysis

#### Test Strategy

- Unit: Progress calculation, threshold evaluation
- Integration: Real page monitoring against test HTTP server
- E2E: Create goal → add milestones → complete → verify progress dashboard

#### Acceptance Criteria

1. ✅ CRUD for goals and milestones with parent-child hierarchy
2. ✅ Page watchers execute on schedule, detect changes within 5% of interval
3. ✅ Alerts routed through zylos-live-comm to cockpit and messaging channels
4. ✅ Milestone completion supports evidence URLs and proof system verification
5. ✅ Progress percentage calculated correctly across nested goals
6. ✅ Watcher history retained for 90 days for trend visualization

---

### 2.3 Browser Takeover Console (CDP Handoff)

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — User controls agent's browser mid-task |
| **Source** | OpenMuse, OpenBot |
| **Module** | `crates/zylos-sandbox/src/browser/takeover.rs` |
| **Effort** | 4 days |

#### Architecture

```rust
pub struct BrowserTakeover {
    session_id: String,
    cdp_endpoint: String,       // ws://localhost:{port}/devtools/page/{target}
    auth_token: String,         // One-time bearer token
    expires_at: Instant,
}

impl BrowserTakeover {
    pub async fn initiate(session_id: &str, ttl: Duration) -> Result<Self>;
    pub fn devtools_url(&self) -> String;  // chrome-devtools:// URL for user
    pub async fn revoke(self) -> Result<()>;
}
```

#### Flow

1. User clicks "Take Over" in cockpit → POST `/api/v1/browser/{session_id}/takeover`
2. Gateway generates one-time auth token, returns Chrome DevTools URL
3. User opens URL in local Chrome → sees agent's browser tab live
4. User interacts (navigate, click, type) → changes reflected in agent's session
5. User clicks "Return Control" or TTL expires → agent resumes with updated state
6. All takeover actions logged to audit trail with user identity

#### Security

- Auth token: 32-byte random, SHA-256 hashed for storage, valid max 10 minutes
- CDP endpoint bound to localhost only; tunnel via SSH/Cloudflare for remote access
- Takeover requires HITL Tier 2+ approval if agent is handling sensitive data
- All user actions during takeover recorded in shadow-git audit

#### Acceptance Criteria

1. ✅ User can take over agent's browser within 2 seconds of clicking button
2. ✅ Changes made during takeover persist when control returns to agent
3. ✅ Takeover auto-expires after configured TTL
4. ✅ All takeover sessions logged with user identity and action count
5. ✅ Revocation immediately disconnects user's DevTools session
6. ✅ Works with persistent browser sessions (Phase 1.3)

---

### 2.4 Multi-Framework Adapter Crates

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — ADK/AG2/Claude SDK/CrewAI/LangGraph/LlamaIndex |
| **Source** | OpenBot |
| **Crates** | `crates/zylos-adapter-{adk,claude-sdk,crewai,langgraph,llamaindex}/` |
| **Effort** | 8 days (2 days each × 4 adapters; ADK + AG2 share structure) |

#### Common Adapter Trait

```rust
#[async_trait]
pub trait FrameworkAdapter: Send + Sync {
    fn framework_name(&self) -> &str;
    async fn spawn_agent(&self, config: AgentConfig) -> Result<AgentHandle>;
    async fn send_message(&self, handle: &AgentHandle, msg: ZylosMessage) -> Result<()>;
    async fn receive_events(&self, handle: &AgentHandle) -> impl Stream<Item = ZylosEvent>;
    async fn shutdown(&self, handle: AgentHandle) -> Result<()>;
}
```

#### Per-Framework Notes

| Framework | Transport | Auth | Notes |
|-----------|-----------|------|-------|
| Google ADK | gRPC + stdio | Service account JSON | Vertex AI or local runtime |
| Claude SDK | stdio JSONL | ANTHROPIC_API_KEY env | Subprocess management |
| CrewAI | HTTP REST | CREWAI_API_KEY | Cloud-hosted agents |
| LangGraph | HTTP + WebSocket | LANGSMITH_API_KEY | Graph state sync |
| LlamaIndex | Python subprocess | None (local) | pyo3 bridge or stdio JSON |

#### Test Strategy

- Unit: Message format translation, error mapping
- Integration: Spawn real agent per framework, exchange 10 messages
- Resilience: Framework crash → adapter detects, reports, allows restart

#### Acceptance Criteria

1. ✅ All 5 frameworks spawnable from zylos-gateway config
2. ✅ Bidirectional message translation preserves tool calls and proofs
3. ✅ Framework crashes detected within 5s, reported to sentinel
4. ✅ Credentials managed via zylos-vault, never in plaintext config
5. ✅ Adapter health exposed via `/api/v1/adapters/status`

---

### 2.5 Cross-Harness Memory Sync

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Memory portability across Claude Code/Codex/Cursor/Hermes |
| **Source** | ECC |
| **Module** | `crates/zylos-memory-system/src/sync/` |
| **Effort** | 5 days |

#### Export Formats

```rust
pub enum MemoryExportFormat {
    MarkdownVault,      // ECC-compatible .md files
    ClaudeCodeMemory,   // .claude/memory.md format
    CursorRules,        // .cursorrules format
    CodexInstructions,  // CODEX.md format
    HermesMemory,       // Hermes MEMORY.md format
    JsonL,              // Generic structured export
}

pub struct MemorySyncService { /* ... */ }

impl MemorySyncService {
    pub async fn export(&self, user_id: &str, format: MemoryExportFormat) -> Result<Vec<u8>>;
    pub async fn import(&self, user_id: &str, format: MemoryExportFormat, data: &[u8]) -> Result<ImportReport>;
    pub async fn watch_and_sync(&self, user_id: &str, path: &Path, format: MemoryExportFormat) -> Result<WatchHandle>;
}
```

#### Conflict Resolution

- Import uses last-write-wins with timestamp comparison
- Semantic dedup via embedding similarity (>0.92 cosine = duplicate)
- Conflicting entries flagged for user review in cockpit

#### Acceptance Criteria

1. ✅ Export/import round-trip for all 6 formats preserves >95% of entries
2. ✅ File watcher detects external changes within 2 seconds
3. ✅ Duplicate detection prevents memory bloat on repeated imports
4. ✅ Conflict resolution UI in cockpit shows diff and merge options
5. ✅ Sync works bidirectionally (zylos → external AND external → zylos)

---

### 2.6 AgentShield Security Integration

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Runtime security scanning in sandbox pipeline |
| **Source** | ECC |
| **Module** | `crates/zylos-sandbox/src/security_scan.rs` |
| **Effort** | 4 days |

#### Scan Points

```rust
pub enum ScanPoint {
    PreExecution,    // Before skill/code runs in sandbox
    PostExecution,   // After output generated, before delivery
    FileAccess,      // On file read/write within sandbox
    NetworkEgress,   // On outbound HTTP request
    ToolInvocation,  // Before MCP/tool call dispatched
}

pub struct SecurityScanner { /* ... */ }

impl SecurityScanner {
    pub async fn scan(&self, point: ScanPoint, payload: &ScanPayload) -> ScanResult;
}

pub enum ScanResult {
    Pass,
    Warn(Vec<Finding>),
    Block(Vec<Finding>),  // Execution halted, HITL escalation
}
```

#### Rule Categories

- **Prompt injection**: Detect jailbreak patterns, system prompt extraction attempts
- **Secret exfiltration**: Regex for API keys, tokens, passwords in outbound data
- **Path traversal**: Block `../`, absolute paths outside sandbox root
- **Dangerous tool calls**: Flag shell exec, file delete, network bind operations
- **PII leakage**: SSN, credit card, email patterns in outputs (integrates with sentinel)

#### Acceptance Criteria

1. ✅ All 5 scan points integrated into sandbox execution pipeline
2. ✅ Block results halt execution and escalate to HITL within 100ms
3. ✅ Warning results logged but allow execution with annotation
4. ✅ Custom rules loadable via TOML config without recompilation
5. ✅ False positive rate <2% on legitimate agent workflows
6. ✅ Scan latency <10ms per check (async, non-blocking)

---

### 2.7 `zylos-personalities` Crate

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Structured persona definitions with success metrics |
| **Source** | Agency-Agents |
| **Crate** | `crates/zylos-personalities/` |
| **Effort** | 3 days |

#### Schema

```yaml
# personalities/senior-backend-engineer.yaml
slug: senior-backend-engineer
division: Engineering
display_name: Senior Backend Engineer
voice:
  tone: precise
  verbosity: concise
  formality: professional
  emoji_usage: minimal
deliverables:
  - Production-ready Rust/Python code
  - Architecture decision records
  - Test coverage reports
success_metrics:
  - metric: test_coverage
    target: ">80%"
  - metric: pr_review_turnaround
    target: "<4h"
  - metric: production_incidents
    target: "0 critical/month"
system_prompt_template: |
  You are a senior backend engineer specializing in distributed systems.
  Communication style: {{voice.tone}}, {{voice.verbosity}}.
  Always include tests and document trade-offs.
proven_workflows:
  - name: feature-implementation
    steps: [analyze_requirements, design_api, implement, test, document]
learning_memory:
  consolidation_interval: daily
  decay_half_life_days: 30
```

#### Integration

- Loaded by `zylos-skills` registry as personality overlays
- Cockpit renders personality selector when creating new agent
- Voice parameters injected into system prompt at runtime
- Success metrics tracked in `zylos-goals` and displayed in roster

#### Acceptance Criteria

1. ✅ 20+ starter personalities shipped across 6 divisions
2. ✅ YAML schema validated on load with clear error messages
3. ✅ Personality selection in cockpit applies voice + metrics instantly
4. ✅ Success metrics visible in agent roster cards
5. ✅ Custom personalities creatable via cockpit UI or YAML file drop

---

## Phase 3: Ecosystem Expansion (Weeks 9–12)

### 3.1 Composio/App Marketplace Integration

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — 2000+ app connectors via unified SDK |
| **Source** | Rerun |
| **Module** | `crates/zylos-integrations/src/composio.rs` |
| **Effort** | 5 days |

#### Approach

- Integrate Composio Rust SDK (or HTTP API fallback) as optional feature flag
- Expose Composio apps as zylos-integrations providers dynamically
- Cache app catalog locally, refresh every 24h
- OAuth flows delegated to Composio; tokens stored in zylos-vault

#### Acceptance Criteria

1. ✅ Composio app catalog browsable in cockpit
2. ✅ One-click enable for any Composio app
3. ✅ Token lifecycle managed automatically
4. ✅ Feature-flagged: disabled by default, enabled via config
5. ✅ Fallback to direct integration if Composio unavailable

---

### 3.2 `zylos-vault` Encrypted Credential Store

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — AES-256-GCM credential vault with admin API |
| **Source** | OpenBot |
| **Crate** | `crates/zylos-vault/` |
| **Effort** | 5 days |

#### Architecture

```rust
pub struct Vault {
    master_key: Zeroizing<[u8; 32]>,  // Derived from machine secret + user passphrase
    db: SqlitePool,
}

impl Vault {
    pub fn open(path: &Path, passphrase: &str) -> Result<Self>;
    pub async fn store_secret(&self, key: &str, value: &[u8], tags: &[&str]) -> Result<()>;
    pub async fn retrieve_secret(&self, key: &str) -> Result<Option<Zeroizing<Vec<u8>>>>;
    pub async fn list_keys(&self, tag_filter: Option<&str>) -> Vec<VaultEntry>;
    pub async fn rotate_key(&self, old_passphrase: &str, new_passphrase: &str) -> Result<()>;
    pub async fn delete_secret(&self, key: &str) -> Result<()>;
}
```

#### Security Properties

- Master key never persisted to disk; derived on unlock via Argon2id
- Secrets encrypted with AES-256-GCM, unique nonce per entry
- Memory zeroized after use (`zeroize` crate)
- Admin API requires mTLS client certificate
- Audit log for every access (read/write/delete) with caller identity

#### Acceptance Criteria

1. ✅ Secrets encrypted at rest, unreadable without passphrase
2. ✅ Master key derivation uses Argon2id with OWASP-recommended params
3. ✅ Memory zeroized after every cryptographic operation
4. ✅ Admin API enforces mTLS, rejects unauthenticated requests
5. ✅ Full audit trail with tamper-evident logging
6. ✅ Key rotation re-encrypts all secrets atomically

---

### 3.3 `zylos-templates` Shareable Agent Blueprints

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — Template library for agent roles |
| **Source** | Rerun |
| **Crate** | `crates/zylos-templates/` |
| **Effort** | 4 days |

#### Template Schema

```yaml
slug: code-reviewer
version: "1.2.0"
display_name: Code Reviewer
description: Reviews PRs for security, performance, and style compliance
personality: senior-backend-engineer
skills:
  - github-pr-review
  - security-scan
  - performance-analysis
tools:
  - github
  - sonarqube
  - semgrep
config_overrides:
  hitl_tier: 2
  sandbox_mode: docker
  proof_level: confirmed
tags: [engineering, security, quality]
```

#### Distribution

- Built-in templates shipped in `templates/` directory
- Community templates installable via CLI: `zylos template install @community/code-reviewer`
- Template registry at `templates.zylos.dev` (future)
- Templates versioned with semver, backward-compatible updates auto-applied

#### Acceptance Criteria

1. ✅ 15+ built-in templates across engineering, ops, research, creative
2. ✅ Template instantiation creates fully configured agent in <1s
3. ✅ Community template install/download works via CLI
4. ✅ Version conflict detection warns before breaking updates
5. ✅ Template preview in cockpit before instantiation

---

### 3.4 `zylos-a2m` Agent-to-Agent Messaging

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — Inter-agent messaging with file transfers |
| **Source** | OpenBot |
| **Crate** | `crates/zylos-a2m/` |
| **Effort** | 6 days |

#### Protocol

```rust
pub enum A2mMessage {
    Text { from: AgentId, to: AgentId, body: String, reply_to: Option<MessageId> },
    File { from: AgentId, to: AgentId, filename: String, content_hash: Sha256, size: u64 },
    Reaction { from: AgentId, message_id: MessageId, emoji: String },
    Receipt { message_id: MessageId, status: DeliveryStatus },
}

pub struct A2mBus { /* ... */ }

impl A2mBus {
    pub async fn send(&self, msg: A2mMessage) -> Result<MessageId>;
    pub async fn subscribe(&self, agent_id: &AgentId) -> impl Stream<Item = A2mMessage>;
    pub async fn transfer_file(&self, from: &AgentId, to: &AgentId, path: &Path) -> Result<MessageId>;
}
```

#### File Transfer

- Files stored in shared volume `/data/a2m-transfers/{message_id}`
- Content-addressed: filename includes SHA-256 hash for integrity verification
- Max file size: 50MB (configurable)
- Transfer progress events emitted for large files
- Recipient sandbox mounts transfer directory read-only

#### Acceptance Criteria

1. ✅ Text messages delivered within 100ms between co-located agents
2. ✅ File transfers up to 50MB with integrity verification
3. ✅ Reactions and reply threading functional
4. ✅ Delivery receipts confirm message consumption
5. ✅ Messages persisted in SQLite for offline agents, delivered on reconnect
6. ✅ Cross-sandbox file transfer respects isolation boundaries

---

### 3.5 Per-Workspace VNC/Screen Share

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — Screen share and user takeover per workspace |
| **Source** | Rerun |
| **Module** | `crates/zylos-sandbox/src/screen_share.rs` |
| **Effort** | 5 days |

#### Architecture

- noVNC web client embedded in cockpit
- TigerVNC server spawned per sandbox workspace on demand
- WebSocket proxy in gateway tunnels VNC protocol
- Authentication via one-time token (same pattern as browser takeover)

#### Acceptance Criteria

1. ✅ Screen share starts within 3 seconds of user request
2. ✅ Mouse/keyboard input from user transmitted to sandbox
3. ✅ Session auto-disconnects after 30 minutes idle
4. ✅ VNC password never exposed to agent or logs
5. ✅ Works with both Docker and WASM sandboxes

---

## Phase 4: Polish & Distribution (Weeks 13–16)

### 4.1 Native Cross-Platform Installer

| Field | Detail |
|-------|--------|
| **Gap** | 🟠 High — Zero-friction onboarding for non-technical users |
| **Source** | Agency-Agents |
| **Tooling** | Tauri bundler + WiX (Windows) + create-dmg (macOS) + nfpm (Linux) |
| **Effort** | 4 days |

#### Deliverables

- macOS: `.dmg` with drag-to-Applications install
- Windows: `.exe` NSIS installer + `.msi` for enterprise deployment
- Linux: `.AppImage` (universal), `.deb` (Debian/Ubuntu), `.rpm` (Fedora/RHEL)
- All installers signed with code signing certificates
- Post-install: Launch app, create first agent, connect first integration wizard

#### Acceptance Criteria

1. ✅ Install completes in <60 seconds on all platforms
2. ✅ No command-line interaction required for basic setup
3. ✅ Code signing verified by OS (no security warnings)
4. ✅ Uninstaller removes all traces including workspace data (with confirmation)
5. ✅ First-run wizard guides through agent creation and integration setup

---

### 4.2 Finance CSV Skill + Editable Personal Context UI

| Field | Detail |
|-------|--------|
| **Gap** | 🟡 Medium — Financial data import + personal context editing |
| **Source** | OpenMuse |
| **Modules** | `skills/finance-csv/`, cockpit settings panel |
| **Effort** | 3 days |

#### Finance CSV Skill

- Parse CSV/OFX/QFX formats
- Categorize transactions via ML classifier (local model)
- Generate spending/savings artifacts for cockpit dashboard
- Support recurring transaction detection

#### Personal Context UI

- Cockpit settings panel exposing `zylos-memory-system` personal-context API
- Editable fields: name, role, preferences, communication style, timezone
- Changes propagate to all active agents within 5 seconds
- Import/export personal context as JSON

#### Acceptance Criteria

1. ✅ CSV import handles 10+ bank export formats
2. ✅ Transaction categorization accuracy >85%
3. ✅ Spending summaries render in cockpit charts
4. ✅ Personal context edits reflect in agent behavior within 5s
5. ✅ Context export/import preserves all fields

---

### 4.3 Web Research Skill Adapters

| Field | Detail |
|-------|--------|
| **Gap** | 🟢 Low — Parallel/Exa/Firecrawl integration with proof grounding |
| **Source** | OpenBot |
| **Skills** | `skills/web-research-parallel/`, `web-research-exa/`, `web-research-firecrawl/` |
| **Effort** | 3 days |

#### Integration Pattern

Each adapter wraps the respective API and pipes results through `zylos-proof`:
- Search results → Observed level
- Extracted content → Confirmed level (cross-source verification)
- Synthesized answer → Verified level (adversarial panel optional)

#### Acceptance Criteria

1. ✅ All 3 providers functional with API key configuration
2. ✅ Results grounded through zylos-proof pipeline
3. ✅ Source attribution preserved in final output
4. ✅ Rate limiting respected per provider terms
5. ✅ Fallback chain: primary → secondary → tertiary provider

---

### 4.4 Helm Chart Hardening + K8s Operator

| Field | Detail |
|-------|--------|
| **Gap** | 🟢 Low — Production-grade Kubernetes deployment |
| **Source** | OpenBot |
| **Files** | `helm/zylos-gateway/`, `operator/` |
| **Effort** | 5 days |

#### Enhancements

- HPA based on custom metrics (active agents, queue depth)
- PodDisruptionBudget for rolling updates
- NetworkPolicy restricting egress to approved domains
- ConfigMap hot-reload via sidecar watcher
- Prometheus ServiceMonitor with Grafana dashboard JSON
- K8s Operator (optional) for automated backup, scaling, and certificate rotation

#### Acceptance Criteria

1. ✅ Helm chart passes `helm lint` and `kubeval` validation
2. ✅ HPA scales from 1→10 replicas under load test
3. ✅ Rolling update completes with zero downtime
4. ✅ NetworkPolicy blocks unauthorized egress
5. ✅ Grafana dashboard shows all key metrics
6. ✅ Operator reconciles desired state within 30 seconds

---

## Cross-Cutting Concerns

### Dependency Graph

```
zylos-vault ← zylos-integrations ← zylos-browser-persistent
     ↑                                      ↓
zylos-a2m ← zylos-sandbox ← zylos-ag-ui ← zylos-desktop
     ↑              ↑
zylos-goals    zylos-security-scan
     ↑
zylos-templates ← zylos-personalities
```

### Shared Infrastructure

- **Error handling**: `thiserror` + `anyhow` consistently across all crates
- **Logging**: `tracing` with structured fields, no PII in logs
- **Testing**: `cargo nextest` for parallel execution, 80% coverage minimum
- **CI**: GitHub Actions matrix (Linux/macOS/Windows), clippy + rustfmt mandatory
- **Docs**: `cargo doc` published to docs.zylos.dev on merge to main

### Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| OAuth provider API changes | Medium | High | Version-pin API clients, monitor changelogs |
| Tauri v2 breaking changes | Low | Medium | Pin Tauri version, test upgrade path quarterly |
| Composio SDK discontinuation | Low | High | Feature-flag, maintain direct integration fallback |
| VNC security exposure | Medium | Critical | Default disabled, require explicit config + mTLS |
| Memory sync conflicts | High | Medium | Semantic dedup + user review UI |

---

## Effort Summary

| Phase | Items | Total Days | Calendar Weeks |
|-------|-------|-----------|----------------|
| Phase 1 | 4 | 26 | 4 |
| Phase 2 | 7 | 39 | 4 |
| Phase 3 | 5 | 25 | 4 |
| Phase 4 | 4 | 15 | 4 |
| **Total** | **20** | **105** | **16** |

*Note: Days assume 1 senior Rust developer. Parallel work possible where dependency graph allows.*

---

*Generated 2026-11-05 from COMPETITIVE_GAP_ANALYSIS.md. All 37 gaps addressed. Plan ready for execution approval.*