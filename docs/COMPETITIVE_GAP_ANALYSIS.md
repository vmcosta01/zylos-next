# Zylos-Gateway Competitive Gap Analysis

**Date:** 2026-11-04
**Repos Analyzed:** 9 (via REA plugin verification)
**Methodology:** Parallel subagent analysis of each repo's README/features against zylos-gateway architecture layers (Verification, Memory, Sandbox, Integration)

---

## Executive Summary

Zylos-gateway holds decisive advantages in **formal verification** (5-level proofs, adversarial panels), **sandbox isolation** (WASM + Docker + Browser triple-tier), **task durability** (SQL leases, circuit breaker), and **safety** (AIOps sentinel, PII stripping, 4-tier HITL). The primary competitive gaps are in **integration breadth**, **desktop UX**, **agent interop protocols**, and **distribution channels**.

### Priority Matrix

| Priority | Gap Category | Action Items | Source Repos |
|----------|-------------|--------------|--------------|
| 🔴 Critical | SaaS Integration Catalog | Build zylos-integrations crate with OAuth flows for Gmail/Slack/Notion/Stripe/Shopify/Linear/Jira/Figma | Rerun |
| 🔴 Critical | WeChat/Feishu Adapters | Add zylos-router channel plugins for CN market distribution | AnyModelBot |
| 🔴 Critical | Per-Agent Persistent Browser | Extend zylos-sandbox Browser layer with cookie/session persistence across turns | OpenBot |
| 🟠 High | AG-UI Protocol Adapter | Implement AG-UI protocol in zylos-gateway for universal agent interop | OpenBot, OpenMuse |
| 🟠 High | Browser Takeover Console | Add CDP-based handoff allowing users to control agent's browser mid-task | OpenMuse |
| 🟠 High | Goals/Milestones Tracking | Build zylos-goals crate with SQL-backed entities and cron-driven page watchers | OpenMuse |
| 🟠 High | Multi-Framework Adapters | Build adapter crates for ADK/AG2/Claude SDK/CrewAI/LangGraph/LlamaIndex | OpenBot |
| 🟠 High | Electron Desktop Shell | Build zylos-desktop Tauri wrapper with tray persistence and per-bot workspaces | AnyModelBot, nightly-labs/openbot |
| 🟠 High | Cross-Harness Memory Sync | Enable memory portability across Claude Code/Codex/Cursor/Hermes sessions | ECC |
| 🟠 High | AgentShield Security Integration | Integrate runtime security scanning into zylos-sandbox execution pipeline | ECC |
| 🟡 Medium | Composio/App Marketplace | Integrate Composio SDK or equivalent for 2000+ app connectors | Rerun |
| 🟡 Medium | Gmail/Calendar OAuth Adapters | Create zylos-google-oauth crate with token encryption and HITL approval cards | OpenMuse |
| 🟡 Medium | Encrypted Credential Vault | Build zylos-vault crate with AES-256-GCM at rest, write-only admin API | OpenBot |
| 🟡 Medium | Decided-Before Audit Trail | Add pre-execution policy decision point to zylos-hitl writing audit row before dispatch | OpenBot |
| 🟡 Medium | Per-Workspace Isolated Computer | Extend zylos-sandbox with VNC/screen-share and user takeover per workspace | Rerun |
| 🟡 Medium | Template Library | Create zylos-templates crate with shareable agent blueprints | Rerun |
| 🟡 Medium | Finance CSV Import | Add finance skill parsing CSVs with spending/savings artifacts for cockpit | OpenMuse |
| 🟡 Medium | Editable Personal Context UI | Expose zylos-memory-system personal-context API in cockpit settings panel | OpenMuse |
| 🟢 Low | Web Research Integrations | Add Parallel/Exa/Firecrawl skill adapters with zylos-proof grounding | OpenBot |
| 🟢 Low | Supabase Backend Adapter | Provide Supabase adapter alongside SQLite WAL for team adoption | Rerun |
| 🟢 Low | Cache Hit-Rate Telemetry | Implement cache effectiveness tracking in zylos-router | AnyModelBot |

---

## Detailed Gap Analysis by Repository

### 1. ruvnet/ruflo (80.8k ⭐) — Swarm/Federation/RAG

**Key Features:** Queen-led swarm coordination, Raft/Byzantine/Gossip consensus, SONA self-learning neural patterns, zero-trust federation (mTLS + ed25519), HNSW-indexed vector memory, plugin marketplace (33 native + 21 npm).

**Gaps vs Zylos:**
- **High:** Swarm consensus protocols (Raft/Byzantine) for multi-node orchestration → Add distributed consensus option to zylos-cerebelo for HA deployments
- **High:** Vector RAG with HNSW indexing and hybrid retrieval → Integrate HNSW index into zylos-memory-system alongside SQLite WAL
- **Medium:** Plugin marketplace with in-console catalog → Build zylos-plugin-marketplace with versioned distribution
- **Medium:** SONA self-learning with trajectory contrastive learning → Enhance zylos-learn with trajectory-based pattern extraction

**Zylos Advantages:** Formal proof system, adversarial panel verification, WASM sandbox, SQL lease durability, AIOps sentinel, 338+ skills registry.

---

### 2. affaan-m/ecc (293k ⭐) — Skills/Hooks/Security

**Key Features:** AgentShield runtime security scanning, cross-harness memory sync (Claude Code/Codex/Cursor/Hermes), PR-triggered security audits, Plan Canvas visual planning, hook dependency isolation, SONA model routing.

**Gaps vs Zylos:**
- **High:** AgentShield integration for runtime security scanning → Wire into zylos-sandbox execution pipeline
- **High:** Cross-harness memory portability → Build memory export/import adapters for external harnesses
- **High:** PR-triggered automated audits → Add git webhook listener to zylos-soar triggering verification pipelines
- **Medium:** Plan Canvas visual DAG builder → Add visual editor to zylos-cockpit
- **Medium:** Hook dependency isolation → Add dependency graph validation to zylos-skills loader

**Zylos Advantages:** 5-level ground proofs, adversarial panel, WASM+Docker sandbox, SQL lease task durability, 4-tier HITL escalation, config hot-reload via TOML+SIGHUP.

---

### 3. msitarzewski/agency-agents (166k ⭐) — Agent Personalities

**Key Features:** 230+ specialized agent personalities with unique voice/deliverables/metrics, division-based organization (Engineering/Design/Sales/etc.), runbook rosters for team workflows, native desktop installer for 9+ tools, multi-tool integration scripts.

**Gaps vs Zylos:**
- **High:** Pre-built personality templates with success metrics → Create zylos-personalities crate with structured persona definitions
- **High:** Native installer for multiple AI tools → Build zylos-installer with cross-platform binary distribution
- **Medium:** Division-based agent organization → Add taxonomy/category system to zylos-skills registry
- **Medium:** Runbook roster coordination → Enhance zylos-soar playbooks with multi-agent roster templates

**Zylos Advantages:** Formal verification layer, WASM sandbox isolation, SQL lease durability, adversarial panel, AIOps sentinel, SOAR orchestration.

---

### 4. CopilotKit/OpenDuet (1.9k ⭐) — Proactive Agent/Triage

**Key Features:** Proactive morning brief, triage classification (needs-you vs can-wait), agent computer (browser/files/shell), approval gates (allow-once/always/refuse), masked credentials, outbound PII checking, scheduled routines.

**Gaps vs Zylos:**
- **High:** Proactive morning brief generation → Build zylos-brief crate aggregating calendar/email/tasks into daily summary
- **High:** Scheduled routine engine with cron-like syntax → Add scheduler to zylos-soar with persistent cron expressions
- **Medium:** Masked credential display in UI → Add credential masking to zylos-cockpit rendering layer
- **Low:** Outbound PII checking on responses → Already covered by zylos-sentinel; verify coverage completeness

**Zylos Advantages:** 5-level proof system, equivalence hashing for approval reuse, WASM+Docker sandbox, SQL lease circuit breaker, 338+ skills, two-stage triage (rules + model fallback).

---

### 5. CopilotKit/openmuse (6k ⭐) — Personal Agent/Browser

**Key Features:** Persistent Chromium with takeover console, optional Linux workspace, PDF transfer, goals/milestones tracking, finance CSV import, Gmail/Calendar OAuth, editable personal context, AG-UI protocol, Rich Threads.

**Gaps vs Zylos:**
- **High:** Browser takeover console (CDP handoff) → Add CDP-based session sharing to zylos-browser
- **High:** Goals/milestones with recurring page monitors → Build zylos-goals crate with cron-driven watchers
- **High:** Gmail/Calendar OAuth adapters → Create zylos-google-oauth with HITL approval cards
- **Medium:** Finance CSV import and spending summaries → Add finance skill to zylos-skills
- **Medium:** Editable personal context UI → Expose memory-system profile API in cockpit
- **Medium:** PDF bidirectional transfer → Extend sandbox volume API with signed upload/download

**Zylos Advantages:** 5-level proofs, adversarial panels, WASM sandbox, AIOps sentinel, shadow-git audit trail, SQL lease durability, safety tier immutability.

---

### 6. modelflare-ai/AnyModelBot (155 ⭐) — Multi-Model/Desktop

**Key Features:** Electron desktop app with tray, 70+ models via single key, DSH runtime, computer-use driver, WeChat/Feishu/Telegram/Slack/Discord adapters, local SQLite storage, background task queue, cache compensation guarantees.

**Gaps vs Zylos:**
- **Critical:** WeChat/Feishu native channel adapters → Add zylos-router channel plugins for CN market
- **High:** Electron desktop shell with tray persistence → Build zylos-desktop Tauri/Electron wrapper
- **High:** Pay-per-use unified model gateway with fallback → Extend zylos-router with multi-provider aggregation
- **Medium:** Computer-use driver with per-action approval → Integrate into zylos-sandbox with HITL gates
- **Medium:** Background task queue with async delivery → Expose cerebelo tasks as persistent background jobs
- **Medium:** Per-bot persona/workspace/memory isolation → Add bot entity to zylos-memory-system

**Zylos Advantages:** Rust-native triple sandbox, 5-level proofs, adversarial panel, AIOps sentinel, SQL lease durability, 4-tier HITL, 338+ skills, SOAR orchestration.

---

### 7. rerun-build/rerun (153 ⭐) — AI Coworkers/Integrations

**Key Features:** 200+ native SaaS integrations, Composio 2000+ apps, MCP server support, scheduled work/webhooks, template library, per-workspace computer with screen share, Foreman orchestrator, AGPL-3.0 self-hostable.

**Gaps vs Zylos:**
- **Critical:** Pre-built SaaS integration catalog (200+) → Build zylos-integrations crate with declarative YAML specs
- **High:** Composio/unified app connector marketplace → Integrate Composio SDK or build equivalent adapter layer
- **High:** Per-workspace isolated computer with VNC → Extend zylos-sandbox with screen-share and takeover
- **Medium:** Template library for agent roles → Create zylos-templates crate
- **Medium:** Webhook-triggered scheduled work → Add webhook ingress to zylos-soar
- **Medium:** Native MCP server hosting → Implement MCP server mode in zylos-gateway
- **Low:** Supabase-compatible backend option → Provide adapter alongside SQLite WAL

**Zylos Advantages:** 5-level proofs, adversarial panel, WASM+Docker sandbox, 4-tier HITL, AIOps sentinel, SQL lease durability, 338+ skills, DAG cerebelo engine, SOAR orchestration, zylos-learn cross-session improvement.

---

### 8. CopilotKit/openbot (8.4k ⭐) — AG-UI/Governance

**Key Features:** AG-UI protocol governance, per-coworker browser with real logins, decided-before/recorded-after audit trail, encrypted credential store, agent-computer Docker isolation (gVisor), Helm chart, 10+ agent framework adapters, automatic learning, web research (Parallel/Exa/Firecrawl).

**Gaps vs Zylos:**
- **Critical:** Per-agent persistent browser with real login sessions → Extend zylos-sandbox Browser with cookie/session persistence
- **High:** AG-UI protocol as universal interop layer → Implement AG-UI adapter in zylos-gateway
- **High:** Decided-before/recorded-after audit trail → Add pre-execution policy decision point to zylos-hitl
- **High:** Native adapters for 10+ agent frameworks → Build framework-specific adapter crates or AG-UI bridge
- **Medium:** Encrypted credential vault (AES-256-GCM) → Build zylos-vault crate
- **Medium:** Helm chart for K8s deployment → Already created; verify completeness against openbot charts
- **Medium:** Automatic learning with per-agent memory containers → Integrate zylos-learn with per-agent namespaces
- **Low:** Web research via Parallel/Exa/Firecrawl → Add skill adapters with zylos-proof grounding

**Zylos Advantages:** 5-level mathematical proofs, adversarial panel multi-judge validation, WASM+Docker dual sandbox, SQL lease exactly-once execution, 338+ built-in skills, Rust performance/safety, 4-tier HITL granularity, AIOps sentinel, SOAR orchestration.

---

### 9. nightly-labs/openbot (2.4k ⭐) — Desktop Workspace

**Key Features:** Local-first Electron desktop app, dedicated per-agent workspaces (~/OpenBot/Agents/<id>), FIFO message queues with pause/resume/cancellation, agent-to-agent messaging with file transfers, shared desktop channel chats, SolidJS UI, Bun runtime, multi-model support (Codex/Claude/Grok/OpenCode/Gemini/Cursor/Cline via ACP).

**Gaps vs Zylos:**
- **High:** Dedicated per-agent workspace directories with isolated filesystems → Add workspace isolation to zylos-sandbox with per-agent directory trees
- **High:** FIFO message queues with pause/resume/cancellation → Enhance zylos-task-durability with queue semantics beyond DAG ordering
- **High:** Agent-to-agent messaging protocol with file transfers → Build zylos-a2m (agent-to-agent messaging) crate
- **Medium:** Shared desktop channel chats with task ownership → Add shared-channel concept to zylos-live-comm
- **Medium:** SolidJS/Bun-based lightweight UI alternative → Consider zylos-cockpit SSR optimization or alternative frontend
- **Medium:** ACP (Agent Communication Protocol) support → Implement ACP adapter alongside AG-UI

**Zylos Advantages:** Formal verification (proofs + adversarial), triple sandbox (WASM+Docker+Browser), SQL lease durability, AIOps sentinel, SOAR orchestration, 338+ skills, Rust memory safety.

---

## Consolidated Zylos Advantages (Consistent Across All 9 Repos)

1. **5-Level Ground Proof System** — No competitor has formal mathematical verification of agent outputs
2. **Adversarial Panel Validation** — Multi-judge verification absent in all analyzed repos
3. **Triple Sandbox (WASM + Docker + Browser)** — Lighter-weight isolation than container-only approaches
4. **SQL Lease Task Durability** — Exactly-once execution guarantees under failover; unique differentiator
5. **AIOps Sentinel with PII Detection** — Production safety monitoring integrated into orchestration layer
6. **4-Tier HITL Escalation** — More granular than binary approve/deny in competitors
7. **338+ Built-in Skills Registry** — Larger than any single competitor's tool set
8. **SOAR Orchestration** — Security incident response workflows absent in productivity-focused competitors
9. **Rust-Native Performance** — <30MB/agent footprint vs Node.js/Electron memory pressure
10. **Shadow-Git Tamper-Evident Audit** — SHA-256 chained commits for forensic integrity

---

## Recommended Implementation Sequence

### Phase 1: Critical Gaps (Weeks 1-4)
1. `zylos-integrations` crate — Gmail, Slack, Notion, Linear, Jira OAuth flows
2. `zylos-router` WeChat/Feishu channel plugins
3. `zylos-sandbox` Browser layer persistent sessions with cookie storage
4. AG-UI protocol adapter in zylos-gateway

### Phase 2: High-Priority UX (Weeks 5-8)
5. `zylos-desktop` Tauri wrapper with system tray
6. `zylos-goals` crate with cron-driven page watchers
7. Browser takeover console (CDP handoff)
8. Multi-framework adapter crates (ADK, Claude SDK, CrewAI, LangGraph)
9. `zylos-personalities` crate with structured persona definitions

### Phase 3: Ecosystem Expansion (Weeks 9-12)
10. Composio/app marketplace integration
11. `zylos-vault` encrypted credential store
12. `zylos-templates` shareable agent blueprints
13. `zylos-a2m` agent-to-agent messaging protocol
14. Per-workspace VNC/screen-share in sandbox

### Phase 4: Polish & Distribution (Weeks 13-16)
15. Native installer for cross-platform distribution
16. Finance CSV skill, editable personal context UI
17. Web research skill adapters (Parallel/Exa/Firecrawl)
18. Helm chart hardening and K8s operator

---

*Generated by REA plugin verification across 9 competitor repositories. All findings grounded in public README analysis.*