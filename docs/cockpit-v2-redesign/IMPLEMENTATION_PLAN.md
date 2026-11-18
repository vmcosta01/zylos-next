# Implementation Plan — Zylos Cockpit v2.0 Redesign

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Fases Gateadas
**Conceito Manifesto:** #19 (Implementation Plan por fases gateadas)
**Depende de:** [PRD.md](./PRD.md), [ARCHITECTURE.md](./ARCHITECTURE.md), [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md), [APP_FLOW.md](./APP_FLOW.md)

---

## Visão Geral das Fases

| Fase | Nome | Objetivo | Dependências | Gate |
|------|------|----------|--------------|------|
| 1 | Design System Unificado | Eliminar conflitos CSS, estabelecer tokens como SSoT | Nenhuma | Zero hardcoded colors; design-tokens.css completo |
| 2 | Layout Shell + Translucency | Header/Sidebar translúcidos, sidebar agrupada colapsável | Fase 1 | backdrop-filter funcional; sidebar collapse persiste |
| 3 | Spring Animation System | Physics-based animations interruptíveis | Fase 1 | Springs 60fps; reduced-motion respeitado |
| 4 | Inline Approval Cards (ADR-001) | Approval cards reais no fluxo de chat | Fases 1,3 | Approve/Reject/Escalate funcionais; receipt persiste |
| 5 | VNC Stream + Fallback (ADR-003) | Phone Relay com stream real e fallback automático | Fases 1,2 | WS connect <3s; fallback ativa em timeout |
| 6 | Command Palette (ADR-009) | Omnibar Cmd+K com busca fuzzy | Fases 1,3 | Abre <100ms; busca <50ms; executa ação |
| 7 | AgentCard + Toast + Polish | Micro-interações, toasts, estados completos | Fases 1-6 | 6 estados por componente; Lighthouse ≥90 |
| 8 | Acessibilidade + Performance | WCAG AA, reduced-motion/transparency/contrast, bundle budget | Fase 7 | Lighthouse mobile ≥90; bundle <800KB gzipped |

---

## Fase 1: Design System Unificado

### Objetivo
Eliminar os dois sistemas CSS conflitantes e estabelecer `design-tokens.css` como single source of truth (ADR-008).

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 1.1 | Auditar todos os arquivos CSS existentes (`design-tokens.css`, `style.css`) e mapear conflitos de cores | — | Relatório de conflitos em `docs/cockpit-v2-redesign/css-audit.md` |
| 1.2 | Escrever `styles/design-tokens.css` completo com todas as variáveis do DESIGN_SYSTEM.md §12 | — | Arquivo criado com todas as variáveis |
| 1.3 | Criar `styles/base.css` com reset, tipografia base, utilities | 1.2 | Arquivo criado |
| 1.4 | Migrar componentes existentes para consumir tokens via `var()` | 1.2, 1.3 | Zero hardcoded colors em `src/` |
| 1.5 | Eliminar `style.css` após migração completa | 1.4 | Arquivo deletado |
| 1.6 | Adicionar CI lint rule: `grep -r '#[0-9a-fA-F]\{6\}' src/` deve retornar zero | 1.5 | Pre-commit hook ou CI step configurado |
| 1.7 | Atualizar `index.html` para carregar Inter font + design-tokens.css + base.css | 1.2, 1.3 | Fonts renderizam corretamente |

### Critérios de Aceitação
- [ ] `design-tokens.css` contém todas as variáveis listadas em DESIGN_SYSTEM.md §12
- [ ] `style.css` não existe mais no repositório
- [ ] `grep -r '#[0-9a-fA-F]\{6\}' src/` retorna zero resultados
- [ ] Todos os componentes renderizam com cores corretas via CSS variables
- [ ] Inter font carrega e aplica como `--font-family-sans`
- [ ] Build trunk passa sem erros CSS

### Out of Scope da Fase
- Componentes novos (AgentCard, InlineApprovalCard, etc.)
- Animações ou transições
- Funcionalidade nova
- Mudanças no backend

### Gate de Verificação
```bash
# Deve retornar exit code 0
test ! -f styles/style.css
grep -r '#[0-9a-fA-F]\{6\}' src/ | wc -l | grep '^0$'
trunk build --profile wasm-release
```

---

## Fase 2: Layout Shell + Translucency

### Objetivo
Implementar header e sidebar translúcidos com backdrop-filter (ADR-010), sidebar agrupada e colapsável.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 2.1 | Implementar `TranslucentHeader` component com backdrop-filter | 1.x | Componente renderiza com blur(20px) saturate(180%) |
| 2.2 | Implementar `CollapsibleSidebar` com grupos Core/Orquestração/Comunicação/Configuração | 1.x | Sidebar renderiza 4 grupos colapsáveis |
| 2.3 | Persistir estado de colapso em localStorage (`sidebar-collapsed-groups`) | 2.2 | Estado sobrevive a refresh |
| 2.4 | Implementar fallback opaco para `prefers-reduced-transparency` | 2.1 | Media query funciona |
| 2.5 | Implementar sticky positioning (header z-100, sidebar z-90) | 2.1, 2.2 | Scroll mantém header/sidebar visíveis |
| 2.6 | Adicionar badges de contagem pendente por grupo sidebar | 2.2 | Badges atualizam via SSE |
| 2.7 | Implementar layout shell em `app.rs` com Router outlet | 2.1, 2.2 | Todas as rotas renderizam dentro do shell |

### Critérios de Aceitação
- [ ] Header exibe backdrop-filter blur(20px) saturate(180%) sobre conteúdo scrollável
- [ ] Sidebar tem 4 grupos distintos com chevrons animados
- [ ] Colapso de grupo persiste entre sessões
- [ ] `prefers-reduced-transparency: reduce` → fundos opacos
- [ ] Header permanece sticky no topo durante scroll
- [ ] Badges mostram contagens corretas de itens pendentes
- [ ] Z-index stacking correto (header > sidebar > content)

### Out of Scope da Fase
- Componentes de página (Agents, Tasks, etc.)
- Animações spring (Fase 3)
- Command palette (Fase 6)
- VNC stream (Fase 5)

### Gate de Verificação
- Screenshot do shell com conteúdo scrollando atrás do header translúcido
- localStorage contém `sidebar-collapsed-groups` após interação
- DevTools → Computed Styles mostra `backdrop-filter: blur(20px)` ativo
- Toggle `prefers-reduced-transparency` no DevTools → fundo muda para opaco

---

## Fase 3: Spring Animation System

### Objetivo
Implementar physics-based animation system interruptível (ADR-007).

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 3.1 | Resolver ADR-007: avaliar leptos-spring compatibilidade com Leptos 0.7 | — | Decisão documentada em DECISIONS.md |
| 3.2a | Se leptos-spring: integrar crate, criar wrapper `SpringAnimator` | 3.1 | Componente reutilizável |
| 3.2b | Se custom: implementar solver WASM em `utils/spring_solver.rs` | 3.1 | Solver funcional com damping/stiffness/mass params |
| 3.3 | Implementar `SpringAnimator` component com velocity handoff | 3.2a/3.2b | Animação interruptível demo |
| 3.4 | Aplicar springs em: sidebar collapse, page transitions, card hover | 3.3 | 3+ use cases funcionando |
| 3.5 | Implementar `prefers-reduced-motion` check → disable springs | 3.3 | Zero animações quando ativo |
| 3.6 | Benchmark: medir FPS em dispositivo mid-range | 3.4 | ≥55fps sustentado |

### Critérios de Aceitação
- [ ] ADR-007 resolvido e documentado
- [ ] Spring animations são interruptíveis (nova animação herda velocity)
- [ ] Parameters configuráveis: damping, stiffness, mass
- [ ] `prefers-reduced-motion: reduce` → zero animações, state changes instantâneos
- [ ] FPS ≥55 em Chrome DevTools Performance tab (throttled 4x CPU)
- [ ] Bundle overhead do spring system <5KB gzipped

### Out of Scope da Fase
- Componentes específicos (approval cards, VNC)
- Gesture-based animations (drag/swipe) — futuro
- Haptic feedback

### Gate de Verificação
- Chrome DevTools Performance recording mostra ≥55fps durante spring animations
- `matchMedia('(prefers-reduced-motion: reduce)')` true → zero requestAnimationFrame calls
- Bundle size diff pré/pós spring system <5KB

---

## Fase 4: Inline Approval Cards (ADR-001)

### Objetivo
Substituir placeholder textual em `hitl_approval_panel.rs:56-58` por inline approval cards reais.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 4.1 | Implementar `InlineApprovalCard` component com skeleton loading | 1.x, 3.x | Componente renderiza skeleton → full card |
| 4.2 | Integrar SSE `approval_requested` event → mount card | 4.1 | Card aparece automaticamente |
| 4.3 | Implementar botões Approve/Reject/Escalate com optimistic update | 4.2 | UI responde instantaneamente |
| 4.4 | POST `/api/v1/approvals/:id` com decision payload | 4.3 | API call funcional |
| 4.5 | SSE `approval_resolved` → collapse card to receipt (spring) | 4.4 | Receipt renderizado |
| 4.6 | Persistir receipts em localStorage (TTL 24h) | 4.5 | Receipt sobrevive a refresh |
| 4.7 | Error handling: revert optimistic on POST failure + retry button | 4.4 | Error state funcional |
| 4.8 | Equivalence hash display com truncation + copy button | 4.1 | Hash visível e copiável |
| 4.9 | Sanitizar HTML em approval descriptions (ammonia WASM) | 4.1 | XSS test passa |

### Critérios de Aceitação
- [ ] Placeholder textual em `hitl_approval_panel.rs` substituído por `<InlineApprovalCard />`
- [ ] Card renderiza dentro de 200ms após SSE event
- [ ] Optimistic update <50ms de latência percebida
- [ ] POST failure reverte estado e mostra retry
- [ ] Receipt persiste em localStorage e restaura após refresh
- [ ] Equivalence hash exibido em mono font, truncado com ellipsis
- [ ] Ammonia sanitiza HTML malicioso em description field
- [ ] Spring collapse animation funciona (damping 0.9, response 0.3s)

### Out of Scope da Fase
- Bulk approval via UI (usar command palette na Fase 6)
- Approval history page
- Email/push notifications de approval
- Multi-tier escalation UI (backend já suporta)

### Gate de Verificação
- E2E test: SSE event → card render → approve click → POST success → receipt
- E2E test: POST failure → revert → error state → retry → success
- localStorage inspection após refresh mostra receipts
- XSS probe: `<script>alert(1)</script>` em description → sanitized output

---

## Fase 5: VNC Stream + Fallback (ADR-003)

> ⚠️ **DEPENDÊNCIA DE BACKEND:** Esta fase requer endpoints que NÃO existem no gateway atual:
> - `POST /api/v1/computer-view/:session/lease` (lease management)
> - `DELETE /api/v1/computer-view/:session/lease` (release)
> - `GET /api/v1/screenshots/:session/latest` (fallback)
> Esses endpoints devem ser criados ANTES de iniciar a Fase 5 frontend.

### Objetivo
Substituir div estática em `phone_relay_panel.rs:241-243` por VNC stream real com screenshot fallback.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 5.1 | Implementar `VncCanvas` component com WebSocket connection | 1.x, 2.x | Canvas conecta a WS endpoint |
| 5.2 | Renderizar binary frames em Canvas (JPEG decode ou raw RGB) | 5.1 | Frames visíveis |
| 5.3 | Implementar quality monitor: track frame intervals | 5.2 | Métrica de FPS disponível |
| 5.4 | Adaptive quality scaling: reduzir resolução se avg interval >66ms | 5.3 | Resolução ajusta automaticamente |
| 5.5 | Implementar `ScreenshotFallback` component | 5.1 | Static image renderiza |
| 5.6 | Timeout logic: 3s sem frame → activate fallback + toast | 5.2, 5.5 | Fallback automático |
| 5.7 | Take Control: POST `/api/v1/computer-view/:session/lease` | 5.1 | Lease adquirido |
| 5.8 | Release Control: DELETE lease + auto-release timer (5min) | 5.7 | Lease liberado |
| 5.9 | QR code generation para pareamento (Empty state) | 1.x | QR code renderiza |
| 5.10 | Manual screenshot: capture current frame as PNG download | 5.2 | Download funcional |

### Critérios de Aceitação
- [ ] Div estática em `phone_relay_panel.rs` substituída por `<VncCanvas />` + `<ScreenshotFallback />`
- [ ] WS conecta em <1s em rede local
- [ ] Frame latency <200ms em condições normais
- [ ] Quality scaling reduz resolução quando FPS <15
- [ ] Fallback ativa após 3s sem frames
- [ ] Take Control adquire lease e mostra indicador "Controlling"
- [ ] Auto-release após 5min de inatividade
- [ ] QR code gera corretamente para pareamento
- [ ] Manual screenshot baixa PNG válido

### Out of Scope da Fase
- Audio streaming
- Clipboard sync bidirecional
- Multi-monitor support
- Recording/replay de sessões VNC
- Mobile touch gestures no canvas

### Gate de Verificação
- WS connection log mostra handshake successful
- Frame counter mostra ≥25fps em rede local
- Desconectar WS → fallback ativa em ≤3.5s
- Lease POST retorna 200 + indicator visual
- 5min timer → DELETE lease chamado automaticamente
- Screenshot download: file é PNG válido (magic bytes check)

---

## Fase 6: Command Palette (ADR-009)

### Objetivo
Implementar omnibar Cmd+K com busca fuzzy indexando telas, agentes, tarefas, comandos e skills.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 6.1 | Implementar `CommandPalette` overlay component | 1.x, 3.x | Overlay abre/fecha com spring |
| 6.2 | Keyboard shortcut listener: Cmd+K / Ctrl+K | 6.1 | Shortcut funcional |
| 6.3 | Construir index em mount: pages, agents, tasks, commands, skills | 6.1 | Index populado |
| 6.4 | Implementar fuzzy search algorithm (Fuse.js-style em WASM) | 6.3 | Busca retorna resultados relevantes |
| 6.5 | Resultados agrupados por categoria com headers | 6.4 | Groups renderizados |
| 6.6 | Keyboard navigation: ↑↓ para navegar, Enter para selecionar | 6.5 | Navegação funcional |
| 6.7 | Histórico recente (últimos 10) em localStorage | 6.6 | Histórico persiste |
| 6.8 | Execute action: navigate/run command → close palette | 6.6 | Ação executa |
| 6.9 | Debounce input 50ms + loading indicator | 6.4 | Sem lag perceptível |
| 6.10 | Esc / click outside fecha palette | 6.1 | Close funcional |

### Critérios de Aceitação
- [ ] Cmd+K (Mac) / Ctrl+K (Win/Linux) abre palette em <100ms
- [ ] Busca fuzzy retorna resultados em <50ms para index de 500 itens
- [ ] Resultados agrupados: Recent, Pages, Agents, Tasks, Commands, Skills
- [ ] ↑↓ navega com highlight spring animation
- [ ] Enter executa ação e fecha palette
- [ ] Histórico mostra últimos 10 comandos primeiro
- [ ] Esc e click outside fecham com spring exit animation
- [ ] Debounce previne busca excessiva durante digitação

### Out of Scope da Fase
- Preview pane lateral
- Subcommands aninhados
- Custom plugin commands
- Voice input
- Search within file contents

### Gate de Verificação
- Performance.now() delta entre keydown e palette visible <100ms
- Benchmark: 500 items search <50ms (WASM)
- E2E: Cmd+K → type "agen" → select "Agentes" → navigates to /agents
- localStorage inspection mostra histórico após uso

---

## Fase 7: AgentCard + Toast + Polish

### Objetivo
Implementar AgentCard com micro-interações, toast notification system, e garantir 6 estados obrigatórios em todos os componentes.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 7.1 | Implementar `AgentCard` com sparkline, status badge, hover effects | 1.x, 3.x | Card com micro-interações |
| 7.2 | Implementar `ToastContainer` com stack vertical bottom-right | 1.x, 3.x | Toasts renderizam e empilham |
| 7.3 | Integrar SSE events → toast dispatch | 7.2 | Eventos geram toasts |
| 7.4 | Implementar 4 variantes de toast: success/error/warning/info | 7.2 | Variantes visuais distintas |
| 7.5 | Auto-dismiss 5s (success/info), manual (error/warning) | 7.4 | Timing correto |
| 7.6 | Pause on hover para toasts | 7.5 | Timer pausa |
| 7.7 | Garantir 6 estados em AgentCard, MetricCard, FilterBar | 1.x | Estados implementados |
| 7.8 | Empty states com ilustrações SVG inline | 7.7 | Empty states visuais |
| 7.9 | Error boundaries em cada route com retry | 7.7 | Error recovery funcional |

### Critérios de Aceitação
- [ ] AgentCard hover: shadow-lg + translateY(-2px) + sparkline animada
- [ ] AgentCard click: ripple effect + navigate
- [ ] Toast stack: max 3 visíveis, newest on top
- [ ] Toast variants: cores e ícones distintos por tipo
- [ ] Auto-dismiss timing correto (5s vs manual)
- [ ] Hover pausa timer de auto-dismiss
- [ ] Todos os componentes críticos têm 6 estados implementados
- [ ] Empty states nunca são tela branca
- [ ] Error boundary captura crash e mostra retry UI

### Out of Scope da Fase
- Drag-and-drop reordenação
- Context menus
- Custom toast actions além de navigate
- Sound notifications

### Gate de Verificação
- Visual inspection: AgentCard hover/click animations suaves
- Toast stress test: 10 toasts rápidos → apenas 3 visíveis
- Kill component via DevTools → error boundary renderiza fallback
- Empty data set → illustration + CTA renderiza

---

## Fase 8: Acessibilidade + Performance

### Objetivo
Garantir WCAG AA compliance, suporte completo a prefers-reduced-motion/transparency/contrast, e bundle budget <800KB gzipped.

### Tarefas Numeradas

| # | Tarefa | Dependência | Entregável Verificável |
|---|--------|-------------|----------------------|
| 8.1 | Auditoria de contraste WCAG AA em todas as combinações de cor | 1.x | Relatório de conformidade |
| 8.2 | Focus states visíveis em todos os interativos (2px solid accent) | 1.x | Tab navigation funcional |
| 8.3 | ARIA labels em todos os icon-only buttons | 7.x | Screen reader test passa |
| 8.4 | Live regions para toast container (`aria-live="polite"`) | 7.2 | Screen reader anuncia toasts |
| 8.5 | Semantic HTML landmarks: `<nav>`, `<main>`, `<header>` | 2.x | Landmarks detectáveis |
| 8.6 | `prefers-contrast: more` → bordas mais grossas, texto mais branco | 1.x | Media query funcional |
| 8.7 | Bundle analysis: identificar e eliminar dead code | 7.x | Report de bundle size |
| 8.8 | wasm-opt flags agressivos: -Oz, --strip-debug | 8.7 | Bundle otimizado |
| 8.9 | Route-based code splitting via `lazy()` | 8.7 | Lazy loading funcional |
| 8.10 | Lighthouse CI integration no pipeline | 8.1-8.9 | Score ≥90 automático |

### Critérios de Aceitação
- [ ] Todas as combinações de cor passam WCAG AA (4.5:1 text, 3:1 UI)
- [ ] Tab order lógico através de todos os interativos
- [ ] Focus outline visível (nunca `outline: none` sem replacement)
- [ ] Screen reader anuncia: nav items, buttons, toasts, status changes
- [ ] `prefers-reduced-motion: reduce` → zero animações
- [ ] `prefers-reduced-transparency: reduce` → fundos opacos
- [ ] `prefers-contrast: more` → bordas 2px, texto #ffffff puro
- [ ] WASM bundle gzipped <800KB
- [ ] Lighthouse Mobile: Performance ≥90, Accessibility ≥90, Best Practices ≥90, SEO ≥90
- [ ] LCP <2s, TBT <300ms, CLS <0.1

### Out of Scope da Fase
- WCAG AAA (AA é suficiente para v2.0)
- Sign language video support
- Braille display optimization
- Cognitive load testing formal

### Gate de Verificação
```bash
# Bundle size check
trunk build --profile wasm-release
gzip -c dist/*.wasm | wc -c  # Must be <819200

# Lighthouse CI
lighthouse http://localhost:8080 --preset=mobile --only-categories=performance,accessibility,best-practices,seo
# All scores ≥90
```

---

## Matriz de Compliance com Manifesto

| Camada | Conceito | Fase(s) | Artefato |
|--------|----------|---------|----------|
| 1 | #1 PRD antes de código | Pré-Fase 1 | PRD.md ✅ |
| 1 | #2 Arquitetura definida | Pré-Fase 1 | ARCHITECTURE.md ✅ |
| 1 | #3 Design System contrato | Fase 1 | DESIGN_SYSTEM.md ✅ |
| 1 | #5 Spec antes de gerar código | Cada tarefa | Specs inline neste plano |
| 1 | #6 TDD com IA no loop | Todas | TESTING.md ✅ |
| 1 | #7 Documentos vivos | Contínuo | MEMORY.md, DECISIONS.md, TASKS.md |
| 1 | #8 Handoff estruturado | Entre fases | HANDOFF_faseN.md |
| 1 | #9 Code review automático | Todas | CI gate |
| 1 | #10 Análise qualidade | Fase 8 | Auditoria isolada |
| 1 | #12 Segurança camada zero | Todas | SECURITY.md ✅ |
| 1 | #18 APP_FLOW obrigatório | Pré-Fase 1 | APP_FLOW.md ✅ |
| 1 | #19 Implementation Plan fases | Este doc | IMPLEMENTATION_PLAN.md ✅ |
| 1 | #20 DoD e Out of Scope | Cada fase | Seções acima |
| 2 | Anti-Slop #1: Prova = artefato | Gates | Scripts de verificação |
| 2 | Anti-Slop #3: Nunca pule erro | Todas | Error states obrigatórios |
| 2 | Anti-Slop #4: Documente ANTES | Pré-Fase 1 | Docs criados antes de código |
| 5 | 15 Arquivos obrigatórios | Pré+Fases | Checklist abaixo |

---

## Matriz de Documentos Obrigatórios por Fase

| Documento | Pré-Fase | F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 |
|-----------|----------|----|----|----|----|----|----|----|-----|
| PRD.md | ✅ | | | | | | | | |
| ARCHITECTURE.md | ✅ | | | | | | | | |
| DESIGN_SYSTEM.md | ✅ | 🔄 | | | | | | | |
| APP_FLOW.md | ✅ | | | | | | | | |
| IMPLEMENTATION_PLAN.md | ✅ | | | | | | | | |
| TESTING.md | ✅ | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 |
| SECURITY.md | ✅ | | | | 🔄 | 🔄 | | | 🔄 |
| EVALUATION.md | ✅ | | | | | | | | 🔄 |
| DECISIONS.md | ✅ | | | 🔄 | | | | | |
| TASKS.md | ✅ | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 |
| MEMORY.md | ✅ | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 | 🔄 |
| HANDOFF_faseN.md | | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| .env.example | ✅ | | | | | | | | |
| AGENTS.md/RULES.md | ✅ | | | | | | | | |
| PROJECT_STRUCTURE.md | ✅ | 🔄 | | | | | | | |

Legenda: ✅ = criado/atualizado, 🔄 = atualizado incrementalmente

---

## Riscos e Mitigações por Fase

| Fase | Risco | Prob | Impacto | Mitigação |
|------|-------|------|---------|-----------|
| 1 | Migração quebra estilos existentes | Média | Alto | Visual regression tests antes/depois |
| 2 | backdrop-filter lento em mobile antigo | Média | Médio | Feature detection + fallback opaco |
| 3 | leptos-spring incompatível com Leptos 0.7 | Média | Alto | Custom solver como fallback (3.2b) |
| 4 | SSE events perdidos durante reconnect | Baixa | Alto | REST sync on focus + receipt persistence |
| 5 | VNC bandwidth excessivo | Alta | Médio | Quality scaling + adaptive framerate |
| 6 | Fuzzy search lento com index grande | Baixa | Médio | WASM implementation + debounce |
| 7 | Sparkline animation causa jank | Baixa | Médio | requestAnimationFrame + will-change |
| 8 | Bundle budget impossível de atingir | Baixa | Alto | Code splitting + tree shaking agressivo |

---

## Timeline Estimada

| Fase | Duração Estimada | Paralelismo Possível |
|------|-----------------|---------------------|
| 1 | 2-3 dias | Nenhum (fundação) |
| 2 | 2-3 dias | Paralelo com Fase 3 |
| 3 | 2-3 dias | Paralelo com Fase 2 |
| 4 | 3-4 dias | Após Fase 3 |
| 5 | 3-4 dias | Após Fase 2, paralelo com Fase 4 |
| 6 | 2-3 dias | Após Fase 3 |
| 7 | 2-3 dias | Após Fases 4-6 |
| 8 | 2-3 dias | Após Fase 7 |
| **Total** | **18-26 dias** | Com paralelismo: ~14-18 dias |

---

## Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 implementation plan; 8 phases; gates defined; compliance matrix |