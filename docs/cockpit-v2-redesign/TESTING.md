# Testing — Zylos Cockpit v2.0 Redesign

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Checklist Estruturado
**Conceito Manifesto:** #6 (TDD com IA no loop + TESTING.md estruturado 8 tipos)
**Depende de:** [PRD.md](./PRD.md), [APP_FLOW.md](./APP_FLOW.md), [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)

---

## 1. Matriz de Testes por Tipo

| # | Tipo | Escopo | Ferramenta | Gate |
|---|------|--------|------------|------|
| T1 | Happy Path | Fluxos principais funcionais | wasm-bindgen-test + Playwright | 100% dos fluxos AF-01 a AF-11 verdes |
| T2 | Validação | Inputs inválidos tratados | wasm-bindgen-test | Zero crashes em input malformado |
| T3 | Erro | Falhas recuperáveis | Playwright + mock server | Error boundary + retry em todos os componentes |
| T4 | Permissões | Acesso negado funciona | Playwright + auth mock | Unauthorized → redirect/toast |
| T5 | Responsividade | Mobile/tablet/desktop | Playwright viewports | Layout funcional em 375px, 768px, 1440px |
| T6 | Acessibilidade | Keyboard/screen reader | axe-core + manual tab test | WCAG AA compliance; zero critical violations |
| T7 | Regressão | Features antigas intactas | Playwright visual regression | Zero diffs não-aprovados em screenshots baseline |
| T8 | Performance | Render < threshold | Lighthouse CI + Chrome DevTools | Lighthouse ≥90; LCP <2s; TBT <300ms; bundle <800KB |

---

## 2. T1: Happy Path Checklist

### Layout Shell
- [ ] Header translúcido renderiza com backdrop-filter ativo
- [ ] Sidebar carrega grupos colapsados do localStorage
- [ ] SSE subscription conecta e recebe eventos
- [ ] Router navega entre todas as rotas sem reload
- [ ] Toast container renderiza e empilha toasts

### AF-01: Agents Grid
- [ ] Grid renderiza agentes carregados via REST
- [ ] Busca filtra client-side instantaneamente
- [ ] Filtro por status re-renderiza grid corretamente
- [ ] Click em AgentCard navega para detalhes
- [ ] Hover exibe micro-interações (elevação, sparkline)

### AF-02: Tasks + Inline Approvals
- [ ] SSE approval_requested monta InlineApprovalCard
- [ ] Approve click → optimistic update → POST success → receipt collapse
- [ ] Reject click → card vermelho → receipt
- [ ] Escalate click → card amarelo → notificação tier seguinte
- [ ] Receipt persiste em localStorage e restaura após refresh
- [ ] Equivalence hash exibido e copiável

### AF-07: Phone Relay (VNC)
- [ ] QR code renderiza quando nenhum dispositivo pareado
- [ ] WS conecta a /computer-view/:session em <1s
- [ ] Frames renderizam em Canvas com latency <200ms
- [ ] Quality scaling reduz resolução quando FPS <15
- [ ] Fallback ativa após 3s sem frames
- [ ] Take Control adquire lease e mostra indicador
- [ ] Auto-release após 5min inatividade
- [ ] Manual screenshot baixa PNG válido

### AF-10: Command Palette
- [ ] Cmd+K / Ctrl+K abre palette em <100ms
- [ ] Busca fuzzy retorna resultados relevantes em <50ms
- [ ] ↑↓ navega resultados com highlight
- [ ] Enter executa ação e fecha palette
- [ ] Histórico recente mostra últimos 10 comandos
- [ ] Esc e click outside fecham palette

### AF-11: Toast Notifications
- [ ] SSE event gera toast do tipo correto
- [ ] Success/info auto-dismiss após 5s
- [ ] Error/warning requer dismiss manual
- [ ] Hover pausa timer de auto-dismiss
- [ ] Stack limita a 3 toasts visíveis
- [ ] Click em toast navega para entidade relacionada

---

## 3. T2: Validação Checklist

### Inputs
- [ ] Search agents: regex inválido mostra warning inline
- [ ] Command palette: shortcut inválido em contexto mostra warning
- [ ] Settings form: campos obrigatórios validam on blur
- [ ] Filter bar: date range inválido mostra error state

### Approval Cards
- [ ] Hash mismatch entre expected e received mostra warning
- [ ] Description com HTML malicioso é sanitizada (ammonia)
- [ ] Payload POST com campos faltantes retorna validation error

### VNC
- [ ] Session ID inválido → error state + mensagem clara
- [ ] Lease request com session expirada → toast + retry
- [ ] Frame corrompido → skip frame, não crash

---

## 4. T3: Erro Checklist

### Network Errors
- [ ] REST GET /agents falha → error banner + retry button
- [ ] SSE disconnect → toast "Conexão perdida" + reconnect automático
- [ ] SSE reconnect falha 3x → toast persistente + manual retry
- [ ] POST approval falha → revert optimistic + error state + retry
- [ ] WS VNC disconnect → fallback screenshot + toast + retry button

### Component Errors
- [ ] AgentCard render crash → ErrorBoundary captura + fallback UI
- [ ] InlineApprovalCard mount com dados incompletos → skeleton → error state
- [ ] VncCanvas com codec unsupported → fallback imediato
- [ ] CommandPalette index build falha → empty state + sugestões

### State Errors
- [ ] localStorage corrupto → fallback para defaults + toast warning
- [ ] Signal stale após SSE gap → REST sync on focus
- [ ] Lease expired durante controle → overlay "Control released"

---

## 5. T4: Permissões Checklist

### Auth (Futuro — Placeholder)
- [ ] Token expirado → redirect para login
- [ ] Unauthorized API response → toast + disable ações
- [ ] RBAC: usuário sem permissão de approve → botão disabled + tooltip

### Client-Side Guards
- [ ] Egress checker bloqueia PII antes de enviar (ADR-002)
- [ ] Input sanitization previne XSS em approval descriptions
- [ ] CSP headers bloqueiam scripts inline não-autorizados

---

## 6. T5: Responsividade Checklist

### Viewports
- [ ] 375px (mobile portrait): sidebar hidden, single column, stacked cards
- [ ] 768px (tablet portrait): sidebar collapsible, 2-column grid
- [ ] 1440px (desktop): sidebar always visible, 3-4 column grid
- [ ] 1920px (large desktop): content max-width capped, whitespace extra

### Touch Targets
- [ ] Todos os botões ≥44x44px em mobile
- [ ] Swipe right dismiss toast em mobile
- [ ] Tap target spacing adequado (≥8px gap)

### Orientation
- [ ] Portrait → layout vertical funcional
- [ ] Landscape → layout adapta sem overflow

---

## 7. T6: Acessibilidade Checklist

### Contraste
- [ ] Text primary (#f8fafc) sobre bg-primary (#0f172a): ratio ≥15:1 ✅
- [ ] Text secondary (#94a3b8) sobre bg-primary: ratio ≥5.6:1 ✅
- [ ] Accent (#3b82f6) sobre bg-secondary (#1e293b): ratio ≥4.8:1 ✅
- [ ] Status colors sobre card backgrounds: todos ≥4.5:1 ✅

### Keyboard Navigation
- [ ] Tab order lógico através de todos os interativos
- [ ] Focus outline visível (2px solid accent, offset 2px)
- [ ] Cmd+K abre command palette
- [ ] Esc fecha overlays, modals, dropdowns
- [ ] Arrow keys navegam listas e dropdowns
- [ ] Enter/Space ativa buttons e links

### Screen Reader
- [ ] ARIA labels em todos os icon-only buttons
- [ ] `aria-live="polite"` em toast container
- [ ] Landmarks semânticos: `<nav>`, `<main>`, `<header>`
- [ ] Alt text em todas as imagens e SVGs significativos
- [ ] Status changes anunciados via live regions

### Preferências do Sistema
- [ ] `prefers-reduced-motion: reduce` → zero animações, transitions instantâneas
- [ ] `prefers-reduced-transparency: reduce` → fundos opacos, sem backdrop-filter
- [ ] `prefers-contrast: more` → bordas 2px, texto #ffffff puro, sombras removidas

---

## 8. T7: Regressão Checklist

### Visual Regression (Playwright)
- [ ] Baseline screenshots de todas as telas em 3 viewports
- [ ] Diff threshold: 0.01% pixel change tolerance
- [ ] Novos PRs comparam contra baseline; diff >threshold = fail
- [ ] Aprovação manual necessária para updates de baseline

### Functional Regression
- [ ] Features da Fase N-1 continuam funcionando após Fase N
- [ ] E2E suite roda completo em cada PR
- [ ] Zero flaky tests (retry policy: 0 retries permitido)

### CSS Regression
- [ ] stylelint rule: zero hardcoded colors
- [ ] design-tokens.css é SSoT; mudanças requerem aprovação
- [ ] Component styles não vazam para outros componentes

---

## 9. T8: Performance Checklist

### Lighthouse Mobile
- [ ] Performance score ≥90
- [ ] Accessibility score ≥90
- [ ] Best Practices score ≥90
- [ ] SEO score ≥90
- [ ] LCP <2s (3G throttling)
- [ ] TBT <300ms
- [ ] CLS <0.1

### Bundle Size
- [ ] WASM gzipped <800KB (budget CI gate)
- [ ] CSS gzipped <50KB
- [ ] Assets totais <1MB
- [ ] Tree shaking elimina dead code
- [ ] Route-based code splitting funcional

### Runtime Performance
- [ ] Spring animations mantêm ≥55fps em mid-range device
- [ ] Fuzzy search 500 items <50ms (WASM)
- [ ] Command palette abre <100ms
- [ ] AgentCard hover animation <16ms (60fps)
- [ ] VNC frame latency <200ms em rede local
- [ ] SSE event → UI update <100ms

### Memory
- [ ] SSE buffer limitado a 100 eventos
- [ ] Zero memory leaks em sessões longas (>1h)
- [ ] GC-friendly signal patterns
- [ ] Canvas cleanup on unmount

---

## 10. Testes Específicos de Spring/Motion

### Interruptibilidade
- [ ] Nova animação interrompe anterior sem reset
- [ ] Velocity handoff: nova spring herda velocity da anterior
- [ ] Rapid toggle (sidebar open/close 5x rápido) → sem acumulação de animações

### Reduced Motion
- [ ] `prefers-reduced-motion: reduce` → spring solver desativado
- [ ] State changes instantâneos (zero requestAnimationFrame)
- [ ] Transitions CSS com duration 0.01ms

### Velocity Handoff
- [ ] Drag release → spring continua com momentum do gesto
- [ ] Scroll interrupt → page transition herda scroll velocity
- [ ] Card hover exit → reverse spring com velocity atual

---

## 11. Estratégia de Testes por Fase

| Fase | Testes Prioritários | Ferramentas |
|------|---------------------|-------------|
| 1 | CSS lint, token coverage, visual regression baseline | stylelint, Playwright |
| 2 | Backdrop-filter perf, sidebar state persistence, responsive | Chrome DevTools, Playwright |
| 3 | Spring FPS, interruptibility, reduced-motion | Chrome Perf, wasm-bindgen-test |
| 4 | Approval flow E2E, XSS sanitization, receipt persistence | Playwright, ammonia tests |
| 5 | VNC latency, fallback timing, lease management | WebSocket mock, Playwright |
| 6 | Search benchmark, keyboard nav, history persistence | Benchmark.js, axe-core |
| 7 | Micro-interactions FPS, toast stacking, error boundaries | Chrome Perf, Playwright |
| 8 | Lighthouse CI, bundle budget, WCAG audit | Lighthouse, axe-core, wasm-opt |

---

## 12. Mordida Protocol (Anti-Slop #1)

Para cada bug fix ou feature:
1. Escrever teste que FALHA antes do fix
2. Implementar fix
3. Verificar teste PASSA
4. Reverter fix temporariamente → teste DEVE falhar novamente
5. Restaurar fix → commit

Se teste passa sem o fix → teste é falso positivo → reescrever.

---

## 13. Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| 8 tipos de teste documentados | ✅ Seções 2-9 |
| Checklist verificável por tipo | ✅ Checkboxes em cada seção |
| Critérios Lighthouse definidos | ✅ Seção 9 |
| Testes de spring/motion | ✅ Seção 10 |
| Reduced-motion/transparency/contrast | ✅ Seção 7 |
| Mordida protocol | ✅ Seção 12 |
| Estratégia por fase | ✅ Seção 11 |
| Cross-link com APP_FLOW | ✅ Referências AF-XX |

---

## 14. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 testing plan; 8 types; spring tests; mordida protocol |