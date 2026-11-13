# zylos-cockpit

**Fase:** 2.1 + ADR-042 + Fase 9 (Fases 1–4) - Dashboard Web (Leptos WASM)
**Status:** ✅ Implementado (5000+ LOC, 25+ componentes, 32 testes cockpit + 144 testes crates)

## Descricao

Frontend web em Leptos 0.7 compilado para WebAssembly. Dashboard de monitoramento de agentes, streaming de eventos AG-UI em tempo real, SOAR dashboard, benchmarks e i18n.

## Arquitetura

```
zylos-cockpit/
├── src/
│   ├── main.rs              # mount_to_body / hydrate_body
│   ├── lib.rs               # wasm_bindgen entrypoint
│   ├── app.rs               # App root component (356 LOC)
│   ├── streaming.rs         # SSE/WebSocket event stream (566 LOC)
│   ├── benchmarks.rs        # Performance benchmarks (289 LOC)
│   ├── i18n.rs              # Internationalization (160 LOC)
│   └── components/
│       ├── agent_card.rs    # Agent status card
│       ├── lazy_roster.rs   # Virtualized agent list (302 LOC)
│       ├── soar_dashboard.rs # SOAR event visualization (503 LOC)
│       └── theme_toggle.rs  # Light/dark theme switch
├── index.html
├── style.css
├── sw.js                    # Service worker (offline support)
└── manifest.json            # PWA manifest
```

## Features

- ✅ CSR + SSR + Hydration modes via feature flags
- ✅ AG-UI SSE streaming (real-time agent events)
- ✅ WebSocket fallback para bidirecional
- ✅ SOAR dashboard com event timeline
- ✅ Virtualized roster (100+ agents sem lag)
- ✅ i18n (pt-BR, en-US)
- ✅ PWA com service worker offline
- ✅ Theme toggle (light/dark)

## Build

```bash
# CSR mode (client-only)
trunk serve

# Production build
trunk build --release

# SSR + hydration (requires server)
cargo run --features ssr
```

## Componentes Principais

| Componente | LOC | Descricao |
|------------|-----|-----------|
| `App` | 356 | Root com router e layout |
| `streaming` | 566 | SSE client + reconnect logic |
| `soar_dashboard` | 503 | Event list + playbook status |
| `lazy_roster` | 302 | Windowed list rendering |
| `benchmarks` | 289 | In-browser perf metrics |
| `i18n` | 160 | Translation registry |

## Componentes Novos — Fase 9 (Fases 1–4)

| Componente | Arquivo | Descrição |
|------------|---------|-----------|
| TaskTriagePanel | `task_triage_panel.rs` | InboxSurface-inspired 4-section triage |
| ThreadManager | `thread_manager.rs` | Thread rename/reorder/create/delete |
| SubagentStatusCards | `subagent_status_cards.rs` | Grid de status de subagentes |
| FileManager | `file_manager.rs` | File browser com folder tabs |
| VoicePanel | `voice_panel.rs` | Voice interaction com waveform |
| EmojiReactions | `emoji_reactions.rs` | Message reaction system |
| AgentAvatarsGrid | `agent_avatars.rs` | 21-color division-based avatars |
| SkinThemesPanel | `skin_themes.rs` | 7 temas visuais |
| FleetAuditorPanel | `fleet_auditor.rs` | Bot cost/efficacy metrics |
| FederationDashboard | `federation_dashboard.rs` | HLC, VectorClock, conflict resolution |

## Design System v2

Tokens CSS em `design-tokens.css` (669 linhas) + specs em `docs/DESIGN_SYSTEM.md`.
Paleta contextual dark-first com light theme via `[data-theme="light"]`.

## Integracao com Gateway

O cockpit consome:
- `GET /api/v1/events` (SSE)
- `GET /api/v1/ws` (WebSocket)
- `GET /api/v1/phase2/status` (subsystems)
- `POST /api/v1/approval/resolve` (HITL)
- `GET /api/v1/hermes/security/*` (guardrails, egress, sanitizer)
- `GET /api/v1/hermes/desktop/*` (workspace, awareness, relay)
- `GET /api/v1/hermes/skill-evolution/*` (contracts, graph, miner)
- `GET /api/v1/hermes/employee/*` (roles, routines, plan-gate)
- `GET /api/v1/approvals/*`, `/egress/*`, `/audit/*`, `/brief/*`, `/models/*`, `/computer-view/*`, `/spaces/*`

## Dependencias

- `leptos` 0.7 (signals-based reactivity)
- `leptos_router` 0.7 (client-side routing)
- `gloo-net` 0.6 (HTTP/fetch)
- `wasm-bindgen` 0.2 (JS interop)
- `web-sys` 0.3 (DOM APIs)

## ADR-042: Por que Leptos?

Leptos foi escolhido sobre Yew por:
- 20% mais rapido (fine-grained reactivity)
- Signals nativos (vs hooks)
- SSR nativo sem framework extra
- Hydratacao automatica