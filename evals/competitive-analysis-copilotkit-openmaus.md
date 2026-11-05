# Competitive Analysis: Zylos-Next vs. State-of-the-Art Agent Ecosystems
**Date:** November 02, 2026
**Scope:** Zylos-Next (Fases 0-6) vs. OpenMausBot, OpenDots, OpenMuse, OpenDuet
**Objective:** Identify architectural advantages, critical UX gaps, and strategic features to adapt for Zylos-Next Fase 7+.

---

## 1. Executive Summary

The current agent ecosystem is bifurcating into two camps: **Infrastructure-Heavy TS/Node stacks** (CopilotKit ecosystem, OpenMausBot) that rely on Docker containers and hosted intelligence APIs, and **Sovereign Rust/WASM stacks** (Zylos-Next, OpenFANG) that prioritize local-first execution, strict sandboxing, and auditable DAGs.

**Zylos-Next holds a decisive technical advantage** in tool execution speed (WASM vs Docker), memory sovereignty (SQLite Graph vs CopilotKit Intelligence), and orchestration rigor (petgraph DAGs vs conversational delegation). 

However, the CopilotKit ecosystem (specifically **OpenDuet** and **OpenMuse**) has pioneered **UX patterns for proactivity and human-in-the-loop (HITL) security** that Zylos-Next currently lacks. To dominate, Zylos-Next must adapt the "Morning Brief / Triage" pattern, persistent browser profiles, and masked secret inputs into the Leptos Cockpit.

---

## 2. Feature Matrix

| Capability | Zylos-Next (Fase 0-6) | OpenMausBot | OpenDots | OpenMuse | OpenDuet |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Core Runtime** | Rust (Tokio/Axum/SQLx) | TS / Electron / Docker | TS / Hono / Docker | TS / React Native / Hono | TS / OpenBot Gateway |
| **Tool Isolation** | **WASM (wasmtime) + Docker** | Docker / VM | Docker + Persistent Browser | Docker + Persistent Browser | Docker + OpenBot Sandbox |
| **Orchestration** | **A2A + MCP + petgraph DAG** | Bot Mode / Group Chat | Specialist Dots | Personal Agent | Proactive Routines |
| **Memory Architecture** | **Local SQLite Graph + 4-Scope** | Local Config/Files | CopilotKit Intelligence | CopilotKit Intelligence | CopilotKit Intelligence |
| **Proactivity** | Cron + SOAR/AIOPS | Manual / Triggered | Spaces / Delegation | Goals / Tracking | **Morning Brief / Triage** |
| **HITL Approvals** | SOAR Gates | Manual | CopilotKit UI | CopilotKit UI | **Explicit Gateway (Allow/Refuse)** |
| **Mobile / Cross-Platform**| Web (Leptos WASM) | PWA / Phone Web App | **Expo / React Native** | **Expo / React Native** | OpenBot Mobile |

---

## 3. Where Zylos-Next Already Wins (The Moat)

### 3.1. WASM Sandboxing vs. Docker Bloat
OpenMuse, OpenDots, and OpenMausBot all provision a **full Docker container or Linux workspace** for an agent to execute code or use a browser. This incurs massive memory overhead and cold-start latency. 
* **Zylos Advantage:** `zylos-sandbox` uses `wasmtime` to execute untrusted tools in microseconds with strict memory limits, falling back to Docker only when full OS syscalls are required. This allows Zylos to run hundreds of concurrent agent tools on hardware that would choke a CopilotKit stack.

### 3.2. Sovereign Memory vs. Hosted Intelligence
The entire CopilotKit ecosystem (OpenDots, OpenMuse, OpenDuet) relies on **CopilotKit Intelligence**—a hosted or licensed self-hosted API—to manage threads, memory, and context. If the API goes down or the license expires, the agent loses its memory.
* **Zylos Advantage:** Zylos uses a local SQLite database with `petgraph` for temporal context graphs and 4-scope isolation. Memory is sovereign, zero-latency, and survives entirely offline.

### 3.3. Auditable DAGs vs. Conversational Drift
OpenDots and OpenMuse rely on "Specialist Dots" delegating tasks to each other via chat. This is flexible but prone to infinite loops, context loss, and hallucinated handoffs.
* **Zylos Advantage:** Zylos compiles multi-agent workflows into **petgraph DAGs**. Every node execution is checkpointed, rolled back on failure, and logged to the SOAR event bus. It is deterministic and auditable.

---

## 4. Critical Gaps to Steal & Adapt (The Roadmap)

To achieve product-market fit against these repositories, Zylos-Next Fase 7+ must implement the following patterns:

### Gap 1: The "Morning Brief" & Triage UX (Source: OpenDuet)
* **What they do:** OpenDuet does not just stream raw events. It processes overnight emails/calendar events and presents a **Morning Brief** categorized into: *"Needs Your Decision"* vs *"Can Wait"*.
* **Zylos Implementation:** The Leptos Cockpit needs a **Triage Dashboard**. Instead of a raw SOAR event feed, an AI router should classify incoming webhooks/emails into `Action_Required` (renders an approval card) and `FYI` (folds into a daily digest). 

### Gap 2: Persistent Browser Profiles (Source: OpenMuse / OpenDots)
* **What they do:** Agents in OpenMuse have a persistent Chromium profile. If they log into Gmail or AWS, the cookies are saved to a volume. Next time they run, they are already logged in.
* **Zylos Implementation:** Currently, Zylos browser runbooks are headless and ephemeral. We need a `zylos-browser-vault` crate that mounts persistent cookie/localStorage volumes to the WASM/Docker browser worker, allowing agents to maintain authenticated sessions across reboots.

### Gap 3: Masked Secret Inputs (Source: OpenDuet)
* **What they do:** When an agent hits a 2FA wall or login prompt, OpenDuet renders a **masked input box** in the chat UI. The user types the password, and it is piped directly to the browser via CDP. **The LLM never sees the password, and it never enters the chat transcript.**
* **Zylos Implementation:** Implement a `SecureInput` tool in the MCP registry. When called, the Zylos Gateway pushes a specialized UI component to the Leptos frontend that masks input and writes directly to the browser worker's stdin, bypassing the LLM context window entirely.

### Gap 4: Document "Spaces" (Source: OpenDots)
* **What they do:** OpenDots separates ephemeral chat from durable work via "Spaces"—a wiki-like structure where agents draft, revise, and save long-form documents.
* **Zylos Implementation:** Integrate a Markdown/Obsidian-style vault directly into the Cockpit, backed by the local SQLite filesystem. Agents should be able to `write_space_page(title, content)` which creates a durable, version-controlled artifact separate from the chat thread.

---

## 5. Strategic Recommendations for Fase 7

1. **Pivot the Cockpit from "Observability" to "Proactivity":** 
   Move beyond just showing DAG execution graphs. Build the **Triage Layer** that intercepts SOAR events and formats them as OpenDuet-style approval cards.
   
2. **Implement the "Ghost Typist" Security Pattern:**
   Adopt OpenDuet's masked credential pipeline. This is a massive selling point for enterprise security compliance (SOC2) that raw headless browsers cannot provide.

3. **Build a Mobile Approval Companion:**
   While Leptos is great for desktop, OpenMuse proves that React Native/Expo is the standard for mobile agent interaction. Consider a lightweight companion app (or PWA) strictly for receiving push notifications and tapping "Approve/Refuse" on Zylos SOAR gates.

4. **Productize the "Offline-First" Memory Graph:**
   Market Zylos-Next aggressively against CopilotKit's reliance on hosted APIs. The ability to run a fully autonomous, memory-rich agent swarm on an air-gapped laptop via SQLite + WASM is a unique differentiator.

---
*Report generated via comparative analysis of GitHub repositories: OpenMausBot (10.4k stars), OpenDots (6.3k stars), OpenMuse (6k stars), OpenDuet (CopilotKit).*