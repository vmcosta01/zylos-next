# HANDOFF_5: Live Communication & Cockpit Frontend

**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Fase:** 5 — Live Communication Layer & Leptos WASM Cockpit
**Antecessor:** HANDOFF_4.md
**Sucessor:** HANDOFF_6.md

---

## 1. O Que Foi Entregue

### 1.1 Live Communication Layer (`crates/zylos-live-comm/`)
- **SSE Gateway:** Server-Sent Events endpoint `/api/v1/stream/events` com reconnect automático e last-event-id tracking
- **AG-UI Protocol:** Implementação completa do protocolo Agent-UI (event types: TEXT_MESSAGE_START/CONTENT/END, TOOL_CALL_START/ARGS/END, STATE_SNAPSHOT, RUN_STARTED/FINISHED)
- **EventBus:** Pub/sub interno com backpressure e bounded channels para desacoplar daemon do gateway SSE
- **SessionManager:** Tracking de sessões ativas com heartbeat e cleanup automático de conexões stale
- **CORS Middleware:** Configuração declarativa de origins permitidos via `config.toml`

### 1.2 Cockpit Frontend (`apps/zylos-cockpit/`)
- **Leptos WASM App:** SPA completa compilada para WebAssembly via trunk
- **6 Estados Obrigatórios:** Empty, Loading, Streaming, Error, Offline, Auth (conforme APP_FLOW.md)
- **Design System:** Componentes reutilizáveis (Button, Card, Badge, Toast, Modal) conforme DESIGN_SYSTEM.md
- **i18n:** Suporte a pt-BR e en-US com lazy loading de traduções
- **Theme Toggle:** Light/Dark mode com persistência em localStorage
- **PWA Support:** Service worker para cache offline de assets estáticos

### 1.3 Integração Daemon ↔ Cockpit
- **Health Endpoint:** `/health` retorna status do daemon + métricas básicas
- **Agents API:** CRUD completo via REST (`/api/v1/agents/*`)
- **Skills API:** Listagem e eval via REST (`/api/v1/skills/*`)
- **Streaming:** Eventos AG-UI em tempo real via SSE
- **Error Handling:** Respostas JSON padronizadas com error codes e mensagens localizáveis

### 1.4 Test Suite
- 38 testes unitários cobrindo SSE serialization, AG-UI event parsing e session management
- 6 testes de integração end-to-end (daemon + cockpit + browser headless)
- Load test: 100 concurrent SSE connections por 5min sem drop

---

## 2. Decisões Técnicas

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| SSE sobre WebSocket | Simplicidade, HTTP/2 multiplexing, reconexão nativa do browser | WebSocket (mais complexo, sem retry built-in) |
| Leptos sobre Yew/Dioxus | Performance, SSR opcional, comunidade ativa, macro ergonomics | Yew (boilerplate), Dioxus (menos maduro) |
| AG-UI como protocolo padrão | Interoperabilidade com outros agent frameworks, spec aberta | Protocolo customizado (lock-in) |
| trunk como build tool | Integração nativa com Leptos, hot reload, PWA support | wasm-pack manual (mais steps, sem dev server) |
| i18n via JSON files | Simplicidade, hot reload, compatível com ferramentas de tradução | Compile-time macros (rigidez, rebuild necessário) |

---

## 3. Gaps Conhecidos e Dívida Técnica

- **SSE reconnection edge cases:** Backoff exponencial implementado mas não testado com network flapping real
- **WASM bundle size:** ~800KB gzipped; code splitting não implementado ainda
- **Accessibility:** ARIA labels parciais; falta keyboard navigation completa
- **E2E test coverage:** Apenas happy path coberto; error states e recovery não testados automaticamente
- **Mobile responsiveness:** Layout funciona mas não é otimizado para telas <768px
- **Real-time metrics:** Dashboard mostra snapshot; falta gráficos temporais (sparklines/charts)

---

## 4. Lições Aprendidas

1. **SSE precisa de last-event-id:** Sem tracking de último evento, reconnect perde mensagens. Implementar desde o início.
2. **WASM panic hook é obrigatório:** Sem `console_error_panic_hook`, panics são silenciosos no browser. Adicionar no main.rs.
3. **CORS errors são confusos:** Browser mostra "network error" genérico. Logar CORS violations no backend com origin detalhado.
4. **i18n keys faltantes quebram UI:** Fallback para key name é melhor que crash. Validar todas as keys no build.
5. **trunk serve ≠ production:** Dev server não testa service worker. Sempre validar PWA com `trunk build --release` + servidor estático.

---

## 5. Próximos Passos (Fase 6+)

- Implementar code splitting para reduzir initial load
- Adicionar gráficos temporais com charts library (e.g., leptos-charts ou chart.js via FFI)
- Completar accessibility audit (WCAG 2.1 AA)
- Integrar E2E tests com Playwright para error states
- Documentar AG-UI protocol extension points em docs/specs/ag-ui-extensions.md
- Adicionar telemetry/analytics opt-in para métricas de uso do cockpit

---

## 6. Referências

- **Código:** `crates/zylos-live-comm/src/`, `apps/zylos-cockpit/src/`
- **Tests:** `crates/zylos-live-comm/tests/`, `apps/zylos-cockpit/tests/`
- **ADR relevante:** ADR-042 (Leptos WASM Frontend), ADR-039 (AG-UI Protocol Adoption)
- **Runbook relacionado:** `zylos-cockpit.md` (operação e troubleshooting)
- **Runbook relacionado:** `fase6-live-communication.md` (detalhes do SSE/AG-UI)
- **Design System:** `DESIGN_SYSTEM.md`
- **App Flow:** `APP_FLOW.md`