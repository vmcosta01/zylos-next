# Zylos Cockpit v2.0 Implementation — GPT Astra Prompt

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Language:** Portuguese (Brazilian) for commits/reports, English for code.

## Context
You are implementing the complete redesign of the Zylos Cockpit v2.0. The planning phase is complete and all 11 planning documents are in `docs/cockpit-v2-redesign/`. You MUST read and follow them strictly:

1. `IMPLEMENTATION_PLAN.md` — 8 gated phases with atomic tasks (YOUR PRIMARY GUIDE)
2. `DESIGN_SYSTEM.md` — Tokens, 6 states per component, spring params, translucent materials
3. `APP_FLOW.md` — 11 screens × 6 states each, critical flows
4. `ARCHITECTURE.md` — C4 diagram, tech stack, data flows, ADRs
5. `PRD.md` — Personas, features, DoD, Out of Scope
6. `TESTING.md` — 8 test types, mordida protocol
7. `SECURITY.md` — Threat model, CSP, token policy
8. `DECISIONS.md` — ADR-007 to ADR-010 (plus inherited ADR-001 to ADR-006)

## Current State (Fase 1 WIP)
- `styles/design-tokens.css` created (single source of truth per ADR-008)
- `styles/base.css` created (reset + utilities + layout primitives)
- Old duplicate `design-tokens.css` and `style.css` at root DELETED
- `index.html` updated to load `styles/design-tokens.css` + `styles/base.css`
- A trunk build was started in background — check `E:/zylos-target/trunk-fase1.log` for status
- If build fails, fix CSS/HTML issues first before proceeding to Fase 2

## Implementation Rules

### Environment
- **Repo:** `C:/Users/Paulo/zylos-next`
- **Cockpit app:** `apps/zylos-cockpit/`
- **Gateway:** `apps/zylos-gateway/`
- **Shell:** bash (git-bash/MSYS) on Windows — use POSIX syntax
- **CARGO_TARGET_DIR:** `E:/zylos-target/cockpit-v2` (always set this)
- **Foreground timeout:** ~420s — for long builds (trunk, cargo test), use background with log file + polling:
  ```bash
  cd C:/Users/Paulo/zylos-next/apps/zylos-cockpit && CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2 trunk build --dist E:/zylos-target/dist-v2 > E:/zylos-target/trunk-v2.log 2>&1; echo exit=$? >> E:/zylos-target/trunk-v2.log
  ```
  Then poll: `for i in $(seq 1 20); do grep -q '^exit=' E:/zylos-target/trunk-v2.log && break; sleep 20; done; tail -3 E:/zylos-target/trunk-v2.log`

### Code Quality
- **Zero hardcoded colors** — all colors via `var(--color-*)` tokens from `design-tokens.css`
- **6 states per component** — loading, empty, error, success, validation, disabled (per DESIGN_SYSTEM.md §9)
- **Spring animations** — use CSS transitions as fallback; implement spring solver if leptos-spring incompatible (ADR-007)
- **Translucent materials** — `backdrop-filter: var(--blur-header)` on header/sidebar (per DESIGN_SYSTEM.md §7)
- **Accessibility** — `prefers-reduced-motion`, `prefers-reduced-transparency`, `prefers-contrast` media queries
- **No mocks in production** — real API calls to gateway; empty state when no data; `zylos_cockpit::ui_error::user_message` for errors
- **Bundle budget** — WASM gzipped < 800KB (CI gate will fail otherwise)

### Git Workflow
- **Commit after EACH phase** — atomic commits per IMPLEMENTATION_PLAN.md
- **Push after each commit:**
  ```bash
  GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master
  ```
- **Branch:** `master` (PR #2 already merged; we're building v2.0 on master)

### Verification Gates (per phase)
Before committing each phase, verify:
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compiles
2. `trunk build --profile wasm-release` — builds successfully
3. `cargo test -p zylos-cockpit --lib` — tests pass
4. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — clean
5. `cargo fmt --all --check` — formatted
6. Visual check: serve with `npx http-server E:/zylos-target/dist-v2 -p 8088` and screenshot

## Phase Execution Order

### Phase 1: Design System Unificado (VERIFY CURRENT WIP)
- Check trunk build log `E:/zylos-target/trunk-fase1.log`
- Fix any CSS/HTML issues
- Verify zero hardcoded colors: `grep -r '#[0-9a-fA-F]\{6\}' apps/zylos-cockpit/src/ | wc -l` should be 0
- Commit: `feat(cockpit-v2): Fase 1 — design system unificado (ADR-008)`

### Phase 2: Layout Shell + Translucency
- Implement `CollapsibleSidebar` with grouped nav (Core/Orquestração/Comunicação/Configuração)
- Implement `TranslucentHeader` with `backdrop-filter`
- Sidebar collapse state persists in localStorage
- Commit: `feat(cockpit-v2): Fase 2 — layout shell translúcido + sidebar colapsável`

### Phase 3: Spring Animation System
- Implement spring solver (WASM custom if leptos-spring incompatible — ADR-007)
- Apply springs to: sidebar collapse, modal entrances, toast stacking
- Respect `prefers-reduced-motion`
- Commit: `feat(cockpit-v2): Fase 3 — spring animation system (ADR-007)`

### Phase 4: Inline Approval Cards (ADR-001)
- Replace placeholder in `hitl_approval_panel.rs` with real inline cards
- Approve/Reject/Escalate buttons with optimistic UI
- Receipt persistence in localStorage
- Commit: `feat(cockpit-v2): Fase 4 — inline approval cards reais (ADR-001)`

### Phase 5: VNC Stream + Fallback (ADR-003)
- **⚠️ Backend dependency:** Endpoints `/api/v1/computer-view/:session/lease` and `/api/v1/screenshots/:session/latest` DO NOT EXIST yet. Create them in `apps/zylos-gateway/src/computer_view_api.rs` first.
- Implement `VncCanvas` component with WebSocket stream to `/ws/computer-view/{session_id}`
- Screenshot fallback on timeout
- Take Control button with lease management
- Commit: `feat(cockpit-v2): Fase 5 — VNC stream real + fallback (ADR-003)`

### Phase 6: Command Palette (ADR-009)
- Implement `Cmd+K` overlay with fuzzy search
- Search across all 30+ tabs + actions
- Keyboard navigation (↑↓ Enter)
- Commit: `feat(cockpit-v2): Fase 6 — command palette Cmd+K (ADR-009)`

### Phase 7: AgentCard + Toast + Polish
- Redesign `AgentCard` with sparklines, hover drill-down, skeleton loading
- Implement `ToastContainer` with success/error/warning/info variants
- Auto-dismiss 5s, pause on hover
- Commit: `feat(cockpit-v2): Fase 7 — AgentCard micro-interações + toast system`

### Phase 8: Accessibility + Performance
- Audit WCAG AA compliance (axe-core)
- Optimize bundle size (wasm-opt -Oz, tree shaking)
- Verify Lighthouse mobile ≥90, LCP <2s, TBT <300ms
- Commit: `feat(cockpit-v2): Fase 8 — acessibilidade WCAG AA + performance budget`

## Deliverables
After completing all 8 phases:
1. Write `C:/Users/Paulo/zylos-next/docs/cockpit-v2-redesign/IMPLEMENTATION_REPORT.md` with:
   - List of all commits with hashes
   - Verification results per phase (test output, clippy, bundle size)
   - Screenshots of key screens (Agentes, Phone Relay, Command Palette)
   - Any deviations from plan with justification
2. Push final commit to master

## Critical Reminders
- **Read IMPLEMENTATION_PLAN.md FIRST** — it has the exact task breakdown
- **Do NOT skip gates** — each phase must pass verification before next
- **Do NOT invent APIs** — check gateway routes before calling
- **Do NOT use mocks** — real data or honest empty state
- **Commit small** — one phase = one commit
- **Push frequently** — don't let work sit unpushed

**Start now by reading `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` and checking the Fase 1 build log.**