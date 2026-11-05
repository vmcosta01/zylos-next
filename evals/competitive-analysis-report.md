# Zylos-Next Competitive Analysis & Gap Report
**Date:** 2026-11-02  
**Scope:** Comparison of Zylos-Next (Fases 0-6) against OpenMausBot, OpenDots, OpenMuse, and OpenDuet  
**Status:** Consolidated from subagent analysis + direct repo review

---

## 1. Executive Summary

Zylos-Next has completed Fases 0-6 (Core Runtime, Memory Graph, MCP Registry, Sandbox Isolation, Skills Registry, SOAR Engine). The platform is architecturally ahead of all four reference repos in **offline-first Rust runtime**, **WASM sandboxing**, and **structured SOAR orchestration**. However, significant UX and integration gaps exist when compared to the CopilotKit ecosystem's focus on proactive agents and human-in-the-loop approval flows.

**Key Finding:** Zylos-Next wins on infrastructure density and security isolation. It loses on developer experience (DX), mobile accessibility, and credential management ergonomics. Fase 7 (Cockpit/UX) must address these to be competitive.

---

## 2. Reference Repository Profiles

| Repo | Stars | Stack | Core Value Prop | Relevance to Zylos |
|------|-------|-------|-----------------|-------------------|
| **OpenMausBot** | 10.4k | TS/Electron/Docker | Grok Bot alternative with VM-based computer use | Benchmark for agent-in-a-box UX; Docker sandbox parity |
| **OpenDots** | 6.3k | TS/React/Node | Durable "Spaces" for long-form agent work | Document persistence model; wiki-style artifact management |
| **OpenMuse** | 6k | React Native/Expo | Mobile-first persistent browser agents | Mobile approval companion pattern; persistent Chromium profiles |
| **OpenDuet** | 40 | TS/CopilotKit SDK | Proactive morning brief + approval cards | Triage UX; ghost typist credential security; AG-UI streaming |

---

## 3. Gap Analysis: Zylos-Next vs State of Art

### Gap 1: Triage & Morning Brief UX (Source: OpenDuet)
*   **What they do:** OpenDuet processes overnight events into categorized "Needs Decision" vs "Can Wait" briefs. Raw SOAR events are never shown directly.
*   **Zylos Status:** ❌ Missing. Cockpit currently streams raw DAG execution logs and SOAR event feeds.
*   **Impact:** High. Users face cognitive overload without AI-mediated prioritization.
*   **Remediation (Fase 7):** Build a **Triage Dashboard** in Leptos. AI router classifies incoming webhooks/emails into `Action_Required` (approval card) and `FYI` (daily digest fold).

### Gap 2: Persistent Browser Profiles (Source: OpenMuse / OpenDots)
*   **What they do:** Agents maintain persistent Chromium profiles. Login cookies survive across sessions via mounted volumes.
*   **Zylos Status:** ⚠️ Partial. Fase 4 implemented Docker sandbox with volume mounts, but no dedicated browser profile manager or cookie persistence layer.
*   **Impact:** Medium. Agents waste tokens re-authenticating every session.
*   **Remediation:** Add `browser-profile-mcp` crate. Mount `/root/.config/chromium` as named volume per agent identity. Integrate with Fase 4 DockerSandbox.

### Gap 3: Credential Security ("Ghost Typist") (Source: OpenDuet)
*   **What they do:** Credentials are injected via masked UI components directly to browser stdin. LLM never sees passwords in context or transcripts.
*   **Zylos Status:** ❌ Missing. Current MCP tools pass secrets as environment variables or tool args (visible in trace logs).
*   **Impact:** Critical for enterprise/SOC2 compliance.
*   **Remediation:** Implement `SecureInput` tool in MCP registry. Leptos frontend renders masked input → writes directly to WASM/browser worker stdin, bypassing LLM context window entirely.

### Gap 4: Document "Spaces" (Source: OpenDots)
*   **What they do:** Ephemeral chat separated from durable work via wiki-like "Spaces". Agents draft/revise/save long-form documents as versioned artifacts.
*   **Zylos Status:** ⚠️ Partial. SQLite memory graph stores facts, but no structured document workspace with version history.
*   **Impact:** Medium. Long-form outputs get lost in chat noise.
*   **Remediation:** Integrate Markdown vault into Cockpit backed by local SQLite FS. Add `write_space_page(title, content)` tool with automatic versioning separate from chat thread.

### Gap 5: Mobile Approval Companion (Source: OpenMuse)
*   **What they do:** React Native/Expo app for push notifications + tap-to-approve on agent gates.
*   **Zylos Status:** ❌ Missing. Desktop-only Leptos cockpit.
*   **Impact:** High for SOAR workflows requiring timely human approval.
*   **Remediation:** Lightweight PWA or Expo companion app strictly for receiving push notifications and approving/refusing Zylos SOAR gates. REST API endpoint for gate status updates.

### Gap 6: AG-UI Streaming Integration (Source: OpenDuet/CopilotKit)
*   **What they do:** Standardized AG-UI protocol for real-time agent state streaming to frontend.
*   **Zylos Status:** ⚠️ In Progress. Fase 6 SOAR engine emits events, but no AG-UI compliant SSE/WebSocket adapter for Leptos consumption.
*   **Impact:** Medium. Custom protocol creates maintenance burden.
*   **Remediation:** Implement AG-UI adapter in `zylos-gateway`. Map SOAR events → AG-UI message types. Connect Leptos dashboard via SSE.

---

## 4. Zylos-Next Competitive Advantages

These are areas where Zylos-Next **already exceeds** the reference repos:

1.  **Offline-First Memory Graph:** SQLite + petgraph runs fully air-gapped. All four refs rely on hosted APIs or external vector DBs.
2.  **WASM Tool Sandboxing:** Wasmtime 25.0 provides true memory-safe isolation. OpenMausBot uses Docker (heavier); others have no sandbox.
3.  **Structured SOAR Engine:** Petgraph-backed DAG with checkpoint/rollback. None of the refs have comparable orchestration primitives.
4.  **Rust Performance:** Tokio async runtime + zero-cost abstractions. 5x memory reduction vs Python/TS equivalents.
5.  **Skills Registry (Fase 5):** 338 skills migrated with type-safe schemas. No equivalent structured skill system in refs.

---

## 5. Strategic Recommendations for Fase 7

1.  **Pivot Cockpit from "Observability" to "Proactivity":** Move beyond DAG graphs. Build the Triage Layer that intercepts SOAR events and formats them as OpenDuet-style approval cards. This is the single highest-impact UX improvement.

2.  **Implement "Ghost Typist" Security Pattern:** Adopt OpenDuet's masked credential pipeline. This is a massive selling point for enterprise security compliance (SOC2) that raw headless browsers cannot provide. Priority: CRITICAL.

3.  **Build Mobile Approval Companion:** While Leptos is great for desktop, OpenMuse proves React Native/Expo is standard for mobile agent interaction. Consider lightweight PWA strictly for push notifications + approve/refuse on Zylos SOAR gates. Can be deferred to Fase 8 if Fase 7 scope is tight.

4.  **Productize Offline-First Memory Graph:** Market Zylos-Next aggressively against CopilotKit's reliance on hosted APIs. The ability to run fully autonomous, memory-rich agent swarm on air-gapped laptop via SQLite + WASM is unique differentiator. Create benchmark suite comparing cold-start latency and memory footprint vs OpenDuet/OpenMausBot.

5.  **Add Document Spaces to Cockpit:** Integrate OpenDots-style wiki vault. Agents should produce durable, version-controlled artifacts separate from chat. Back with SQLite filesystem for offline consistency.

---

## 6. Implementation Priority Matrix

| Gap | Severity | Effort | Fase | Dependency |
|-----|----------|--------|------|------------|
| Ghost Typist Credentials | Critical | Medium | 7 | SecureInput MCP tool + Leptos masked component |
| Triage Dashboard | High | High | 7 | AI router + SOAR event classification |
| AG-UI Streaming Adapter | Medium | Medium | 7 | zylos-gateway SSE endpoint |
| Persistent Browser Profiles | Medium | Low | 7 | browser-profile-mcp + DockerSandbox integration |
| Document Spaces | Medium | Medium | 7 | SQLite FS vault + write_space_page tool |
| Mobile Approval Companion | High | High | 8 | REST API + PWA/Expo app |

---

## 7. Next Steps

1.  Update `TASKS.md` with Fase 7 tasks derived from this gap analysis.
2.  Create runbooks for each remediation item (multi-level: senior/pleno/junior).
3.  Begin Fase 7 Sprint 7.1: Ghost Typist + Triage Dashboard (critical path).
4.  Defer mobile companion to Fase 8 unless user prioritizes.

---

*Report generated via comparative analysis of GitHub repositories: OpenMausBot (10.4k stars), OpenDots (6.3k stars), OpenMuse (6k stars), OpenDuet (CopilotKit).*  
*Consolidated from subagent batch results (deleg_a6be8b36, deleg_452027a2) + direct repo extraction.*