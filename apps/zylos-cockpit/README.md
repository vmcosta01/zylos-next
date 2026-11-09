# zylos-cockpit

**Fase:** 2.1 + ADR-042 - Dashboard Web (Leptos WASM)
**Status:** ✅ Implementado (2264 LOC, 11 arquivos, 32 testes)

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

## Integracao com Gateway

O cockpit consome:
- `GET /api/v1/events` (SSE)
- `GET /api/v1/ws` (WebSocket)
- `GET /api/v1/phase2/status` (subsystems)
- `POST /api/v1/approval/resolve` (HITL)

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