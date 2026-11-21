# PRD — Zylos Cockpit v2.0 Redesign

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Planejamento Aprovado
**Conceito Manifesto:** #1 (PRD antes de código)
**Referências:** ADR-001, ADR-003, CONCEITOS-DESENVOLVIMENTO-IA.md v3.1

---

## 1. Visão do Produto e Problema

### Problema
O Zylos Cockpit atual (v0.1.0) possui backend de classe mundial (5-level proofs, triple sandbox, SQL lease durability, AIOps sentinel, 338+ skills) mas frontend com nota **2.2/10** nos princípios Apple Design. Três gaps críticos bloqueiam a adoção:

1. **Dois sistemas CSS conflitantes** — `design-tokens.css` e `style.css` com 3 paletas diferentes geram inconsistência visual e dívida técnica
2. **HITL Approval Panel é placeholder textual** — `hitl_approval_panel.rs:56-58` exibe texto estático apesar da ADR-001 ter decidido inline approvals copiado de OpenDuet/OpenDots
3. **Phone Relay é div estática** — `phone_relay_panel.rs:241-243` não implementa VNC stream nem screenshot fallback conforme ADR-003

### Visão
Transformar o Cockpit em dashboard de orquestração de agentes IA com qualidade visual equivalente ao backend: materiais translúcidos, animações fluidas com springs, manipulação direta, feedback imediato e sistema de design unificado. Referência: Linear, Vercel Dashboard, Arc Browser.

---

## 2. Target Users Específicos

### Persona Primária: "Ana, SRE de Agentes IA"
- **Nome:** Ana Rodrigues, 34 anos
- **Cargo:** Site Reliability Engineer especializada em multi-agent systems
- **Dor:** Precisa aprovar 50+ ações HITL/dia enquanto monitora 20+ agentes simultâneos. Interface atual exige cliques excessivos, perde contexto conversacional em modais, e não oferece feedback visual imediato sobre estado do swarm.
- **Comportamento Atual:** Usa cockpit apenas para tarefas críticas; prefere CLI para bulk operations porque UI é lenta. Abandona aprovação se levar >3s para renderizar card.
- **Necessidade:** Inline approvals que mantêm contexto, command palette para navegação rápida, sidebar agrupada para reduzir carga cognitiva.

### Persona Secundária: "Carlos, Product Manager de AI"
- **Nome:** Carlos Mendes, 41 anos
- **Cargo:** PM responsável por produto de agentes autônomos
- **Dor:** Não consegue demonstrar valor do sistema para stakeholders porque interface parece "protótipo abandonado". Métricas estão espalhadas, sem narrativa visual.
- **Comportamento Atual:** Tira screenshots e monta decks manualmente. Evita mostrar cockpit em reuniões.
- **Necessidade:** Dashboard com métricas claras, estados visuais consistentes, estética profissional que transmita confiança.

### Anti-Persona (Out of Scope)
- Usuário casual que acessa <1x/semana — otimizar para power users
- Mobile-first user — responsividade necessária mas não primária

---

## 3. Features Core

| # | Feature | Referência APP_FLOW | Prioridade | ADR Relacionada |
|---|---------|---------------------|------------|-----------------|
| F1 | Sistema de Design Unificado | AF-01 | P0 | ADR-008 (nova) |
| F2 | Inline Approval Cards Reais | AF-02 | P0 | ADR-001 |
| F3 | Materiais Translúcidos (Header/Sidebar) | AF-03 | P0 | ADR-010 (nova) |
| F4 | Spring Animation System | AF-04 | P1 | ADR-007 (nova) |
| F5 | Sidebar Agrupada Colapsável | AF-05 | P1 | — |
| F6 | Command Palette Expandida | AF-06 | P1 | ADR-009 (nova) |
| F7 | VNC Stream Real + Screenshot Fallback | AF-07 | P0 | ADR-003 |
| F8 | Toast Notification System | AF-08 | P2 | — |
| F9 | AgentCard Micro-interações | AF-09 | P2 | — |
| F10 | Acessibilidade Preferências (reduced-motion/transparency/contrast) | AF-10 | P1 | — |

**Cross-link:** Ver [APP_FLOW.md](./APP_FLOW.md) para jornada completa e estados por tela.

---

## 4. Research Summary

### 9 Repositórios Pares Analisados

| Repo | Aprendizado Chave | Aplicação no Cockpit v2 |
|------|-------------------|------------------------|
| AnyModelBot (modelflare-ai) | Tier strategy multi-modelo por conversa | Badge de nível de config (global/bot/session) em AgentCard |
| Paperclip (paperclipai) | Modal de aprovação multi-tenant | REJEITADO — modal perde contexto (ver ADR-001) |
| Ruflo (ruvnet) | Painel lateral bulk approval | Complementar: bulk actions via command palette, não painel fixo |
| ECC CLI (affaan-m) | Toast notifications para eventos assíncronos | Base para F8 (Toast System) |
| agency-agents (msitarzewski) | Spaces/documents como filesystem markdown | REJEITADO — ADR-004 decidiu SQLite |
| OpenDuet (CopilotKit) | Approval gates INLINE no chat | BASE DA ADR-001 — implementação obrigatória |
| OpenMuse (CopilotKit) | VNC + static screenshot fallback | BASE DA ADR-003 — progressive enhancement |
| OpenDots (CopilotKit) | PageReviewCard.tsx inline, persistent sessions | Refinamento de ADR-001: receipt persistence |
| OpenMausBot (milind-soni) | VNC stream com controle real-time | Refinamento de ADR-003: take/release control com lease |

### Onde Zylos Já Supera Todos os Pares
- 5-level ground proofs (nenhum par tem verificação formal)
- Adversarial panel multi-juiz
- Triple sandbox (WASM+Docker+Browser)
- SQL lease task durability
- AIOps sentinel + PII detection
- 4-tier HITL escalation
- Shadow-git audit trail
- 338+ skills registry

**Gap único:** Frontend. Este PRD resolve exclusivamente o gap de UX/UI.

---

## 5. Definition of Done (Critérios Objetivos)

### Critérios Funcionais
Estado em 2026-11-19, verificado no navegador contra o gateway real (ver "Evidências" abaixo).

- [x] Inline approval cards renderizam no fluxo de chat com ações funcionais (v2.1: **Recusar / Permitir uma vez / Sempre permitir**; "Escalar" foi removido porque o backend nunca aceitou `escalate`)
- [ ] VNC stream conecta via WebSocket e exibe frames em Canvas com <200ms de latência (WS conecta e mostra "Ao vivo"; **frames e latência não foram medidos**, não há fonte de frames no ambiente de teste)
- [x] Screenshot fallback ativa automaticamente quando não chega frame em 3 s: verificado no navegador (imagem 1920x1080 do gateway, com tentativa de reconexão a cada 5 s)
- [x] Command palette abre com Cmd/Ctrl+K e executa ação sem sair do contexto (tema, atualizar, limpar notificações, aprovar em lote com confirmação, navegação). **Tempo de busca <100ms não foi medido**
- [x] Sidebar agrupa itens com colapso persistente (implementada com 6 grupos, não 4)
- [x] Toasts para eventos assíncronos: aprovação pedida, resolvida e expirada e alerta do Sentinela chegam do gateway pelo SSE; erro de agente é tratado quando o backend emitir `AgentStateChanged` com estado de erro (ainda não emitido)

### Critérios de Performance
Medido em `trunk build --release`, Lighthouse mobile padrão (Slow 4G, CPU 4x), **servido com brotli** como o Cloudflare Pages serve (a primeira medição usou um servidor sem compressão e exagerava o download).

| Métrica | Wasm único | + correção do `:has()` | + dois módulos wasm |
|---|---|---|---|
| Performance | 47 (sem brotli) / 69 | 69 | **91** |
| FCP | 4,5 s | 4,5 s | **0,9 s** |
| LCP | 13,8 s (sem brotli) / 5,4 s | 5,4 s | **3,5 s** |
| TBT | 2.180 ms | 60 ms | **40 ms** |
| Transferido na abertura | ~700 KB | ~700 KB | **395 KB** |
| Wasm da abertura (bruto / brotli-9) | 2,22 MB / 677 KB | 2,22 MB / 677 KB | **958 KB / 309 KB** |

O módulo extras (1,6 MB bruto, 508 KB em brotli) só é baixado ao abrir uma aba secundária. Detalhes em `DECISIONS.md` (ADR-011).

- [x] Lighthouse Mobile Performance ≥90: **91**. Accessibility 100 no desktop e **90 no mobile** (falham `aria-required-children` e `target-size`, a corrigir), Best Practices 100. **SEO 63**: o `robots.txt` bloqueia indexação, de propósito (painel privado); liberar a indexação é decisão sua
- [x] LCP <2s no desktop: **0,80 a 0,85 s** no Lighthouse desktop (`docs/fase11-reports/lighthouse-2026-11-20-desktop.json`)
- [x] LCP <=4s no mobile simulado (Slow 4G + CPU 4x): **3,89 a 3,95 s**, quase no limite (`lighthouse-2026-11-20-mobile.json`; a tabela acima, de uma medição anterior sem JSON arquivado, dizia 3,5 s). A meta original "<2s em 3G" não é alcançável sem trocar de framework ou pré-renderizar; foi revista em 2026-11-20 (`PERFORMANCE_WASM.md`)
- [x] TBT <300ms: **40 ms**
- [x] CLS <0.1: **0**
- [x] Bundle WASM da abertura <800KB comprimido: **309 KB** em brotli-9 (gzip -9: veja o CI, que agora mede cada módulo)
- [ ] Spring animations a 60fps em aparelho intermediário (não medido; a v2.1 removeu a maior parte da animação)

### Critérios de Design
- [x] Cores só em `styles/design-tokens.css` (sem hex em `base.css`; `index.html` só tem os `meta theme-color`; skins são dados de tema)
- [x] Vidro (`backdrop-filter`) no cabeçalho flutuante (v2.1 revisou: blur 16px, só no cabeçalho; sidebar e palette são opacos)
- [ ] 6 estados documentados e implementados em cada componente crítico (cobertos: carregando, vazio, erro e sucesso nas telas novas; a matriz completa não foi auditada componente a componente)
- [x] `prefers-reduced-motion` respeitado
- [x] `prefers-reduced-transparency` respeitado

### Evidências (2026-11-19)
- Gateway real iniciado com `ZYLOS_AUTH_SECRET`; login, criar aprovação (201), SSE com `ApprovalRequested`, `ApprovalResolved`, `ApprovalTimeout` e `SentinelAlert`, resolver sem token (401) e com admin (200).
- Navegador: aprovação criada pelo backend aparece no painel e no chat sem recarregar; Permitir uma vez e Sempre permitir (grava regra no SQLite; Revogar remove); Inbox e Activity; computador do agente expande e conecta (101 no WebSocket).
- Spaces (criar, definir agente, Inbox filtrado, persistência da escolha) e pedido resolvido por regra com Revogar verificados no navegador contra o gateway real.
- Testes: cockpit 161 (lib) + 20 (bin) + 25 (wasm-pack em Chrome headless, rodado antes de Spaces), gateway 130, live-comm 183, computer-view 24. Clippy sem avisos novos.

### Critérios de Qualidade
- [ ] TESTING.md com checklist de 8 tipos completo e verde
- [ ] Auditoria isolada (agente-auditar) aprova sem findings críticos
- [ ] Code review automático passa sem blockers
- [ ] Zero TODOs/FIXMEs/HACKs em código final

---

## 6. Out of Scope (Explícito)

### NÃO será feito nesta versão
- Multi-user authentication / RBAC server-side (futuro, pós-v2.0)
- Edição de Spaces/Documents inline (somente leitura via API)
- Mobile app nativo (responsividade web é suficiente)
- Internacionalização i18n completa (PT-BR como default, EN como fallback)
- Dark/Light mode toggle dinâmico (dark mode only nesta versão)
- Custom theme engine (tokens fixos)
- Plugin system para extensões de terceiros
- Analytics dashboard próprio (usar Axiom/Grafana externo)
- Voice commands na command palette
- Drag-and-drop reordenação de sidebar items
- Export de relatórios PDF/CSV
- Integração com Slack/Discord notificações (usar SOAR playbooks)
- Refatoração do backend gateway
- Novos endpoints REST/SSE
- Mudanças no protocolo AG-UI
- Migração de Leptos 0.7 para outra framework

### Justificativa das Exclusões
Cada item excluído foi avaliado contra o princípio "vertical slice funcional". Incluir qualquer um destes expandiria escopo além da capacidade de entrega com qualidade. Backend já é classe mundial; foco exclusivo em frontend.

---

## 7. Métricas de Sucesso Mensuráveis

| Métrica | Baseline (v0.1.0) | Target (v2.0) | Método de Medição |
|---------|-------------------|---------------|-------------------|
| Nota Apple Design Principles | 2.2/10 | ≥8.0/10 | Auditoria manual com rubrica de 17 princípios |
| Lighthouse Mobile Performance | ~45 | ≥90 | Lighthouse CI em pipeline |
| Tempo médio para aprovar HITL | 12s (modal + contexto perdido) | <3s (inline) | Telemetria customizada |
| Taxa de abandono de aprovação | ~35% | <10% | Analytics de sessão |
| Bundle size gzipped | ~1.2MB | <800KB | trunk build --profile wasm-release |
| FPS em animações spring | N/A (zero springs) | 60fps constante | Chrome DevTools Performance |
| Cobertura de estados obrigatórios | ~30% | 100% | proof.mjs + auditoria manual |
| Conflitos CSS detectados | 3 paletas | 0 | stylelint + auditoria |

---

## 8. Requisitos Funcionais, UX e Performance

### RF-01: Inline Approval Cards (ADR-001)
- Renderiza no fluxo de chat como componente `<InlineApprovalCard />`
- Mostra: action description, equivalence hash, risk tier, timestamp
- Botões: Approve (verde), Reject (vermelho), Escalate (amarelo)
- Após decisão: card colapsa para receipt com status e timestamp
- Receipt persiste via localStorage para sobreviver a refresh
- Skeleton loading durante fetch de detalhes
- Estado de erro mostra retry button

### RF-02: VNC Stream + Fallback (ADR-003)
- Canvas element ocupa 100% do container Phone Relay
- WebSocket conecta a `ws://gateway/ws/computer-view/{session_id}` (endpoint existente em `webrtc_relay.rs`)
- Frame rate adaptativo: 30fps padrão, 15fps em low-bandwidth
- Quality scaling: reduz resolução se latency >200ms
- Fallback: após 3s sem frame, exibe último screenshot estático
- Take Control button: solicita lease (timeout 5min auto-release) — **⚠️ ENDPOINT `/api/v1/computer-view/:session/lease` NÃO EXISTE; criar no backend antes da Fase 5**
- Release Control button: libera lease imediatamente
- Indicador visual de status: Connected/Connecting/Fallback/Error

### RF-03: Command Palette (ADR-009)
- Shortcut: Cmd+K (Mac) / Ctrl+K (Windows/Linux)
- Busca fuzzy em: telas, agentes, tarefas, comandos, skills
- Resultados agrupados por categoria
- Preview da ação antes de executar
- Histórico de comandos recentes (últimos 10)
- Fecha com Esc ou clique fora
- Animação spring de entrada/saída

### RF-04: Sidebar Agrupada
- Grupos: Core (Agentes, Tarefas, Memória), Orquestração (DAG, Sentinela, Metas), Comunicação (Phone Relay, Chat), Configuração (Personalidades, Settings)
- Cada grupo colapsável com chevron animado
- Estado de colapso persistido em localStorage
- Badge de contagem pendente por grupo
- Tooltip em cada item descrevendo função

### RF-05: Materiais Translúcidos (ADR-010)
- Header: `backdrop-filter: blur(20px) saturate(180%)`, bg rgba(15,23,42,0.7)
- Sidebar: mesmo tratamento, borda direita sutil
- Respeita prefers-reduced-transparency: bg opaco quando ativo
- Z-index consistente: header=100, sidebar=90, content=1

### RF-06: Spring Animations (ADR-007)
- Library: leptos-spring ou solver WASM custom (decidir em ADR-007)
- Parameters: damping=1.0 para UI transitions, damping=0.8 para gestos
- Response time: 0.3-0.4s para entradas, 0.2s para saídas
- Interruptível: nova animação interrompe anterior com velocity handoff
- Respeita prefers-reduced-motion: zero springs quando ativo

### RF-07: Toast Notifications
- Posição: bottom-right, stack vertical
- Tipos: success (verde), error (vermelho), warning (amarelo), info (azul)
- Auto-dismiss: 5s para info/success, manual dismiss para error/warning
- Pause on hover
- Click to navigate (quando aplicável)
- Max 3 toasts visíveis; overflow em fila

### RF-08: AgentCard Micro-interações
- Hover: elevação sutil (shadow + scale 1.02), sparkline animada
- Click: ripple effect + navegação
- Skeleton loading com shimmer
- Status badge com pulse animation para "Running"
- Last active timestamp relativo ("2m ago") com tooltip absoluto

---

## 9. Compliance com Manifesto v3.1

| Camada | Conceito | Aplicação neste PRD |
|--------|----------|---------------------|
| 1 | #1 PRD antes de código | ESTE DOCUMENTO |
| 1 | #3 Design System como contrato | Seção 8 referencia DESIGN_SYSTEM.md |
| 1 | #18 APP_FLOW obrigatório | Cross-link para APP_FLOW.md |
| 1 | #19 Implementation Plan por fases | IMPLEMENTATION_PLAN.md derivado |
| 1 | #20 DoD e Out of Scope explícitos | Seções 5 e 6 |
| 2 | Princípio 1: Prova = artefato verificável | DoD com critérios objetivos |
| 2 | Princípio 4: Documente ANTES como contrato | Este PRD é contrato |
| 2 | Princípio 7: Out of Scope nunca omitido | Seção 6 explícita |
| 3 | Loop Profissional | READ→UNDERSTAND→PLAN→... |
| 5 | Arquivos Obrigatórios | PRD, ARCH, DS, APP_FLOW, IMPL, TEST, SEC, EVAL, DEC, HANDOFF |
| 9 | Skills Hermes | prd-author, design-system-author, app-flow-author, implementation-plan-author |

---

## 10. Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|--------------|---------|-----------|
| leptos-spring incompatível com Leptos 0.7 | Média | Alto | ADR-007 prevê solver WASM custom como fallback |
| VNC bandwidth excessivo em redes lentas | Alta | Médio | Quality scaling + adaptive framerate no RF-02 |
| backdrop-filter performance em mobile antigo | Média | Médio | Feature flag + fallback opaco automático |
| Bundle size aumenta com novas deps | Alta | Alto | Budget CI gate: fail se >800KB gzipped |
| Conflitos CSS ressurgem | Baixa | Alto | stylelint rule + pre-commit hook |
| Inline approval cards causam scroll jump | Média | Médio | Sticky pending count header + smooth scroll |

---

## 11. Dependências Externas

| Dependência | Versão | Propósito | Alternativa se Indisponível |
|-------------|--------|-----------|----------------------------|
| Leptos | 0.7.x | Framework frontend | Manter versão atual |
| leptos-spring | TBD | Animações spring | Solver WASM custom |
| trunk | 0.22.1 | Build tool | Manter versão atual |
| wasm-bindgen | compat | JS interop | Inclusivo no Leptos |
| gloo-net | compat | WebSocket/fetch | web-sys direto |

---

## 12. Aprovação

| Role | Nome | Data | Status |
|------|------|------|--------|
| Product Owner | — | — | Pendente |
| Tech Lead | — | — | Pendente |
| Design Lead | — | — | Pendente |

**Próximo passo:** Aprovar este PRD → Gerar ARCHITECTURE.md → Gerar DESIGN_SYSTEM.md → Gerar APP_FLOW.md → Gerar IMPLEMENTATION_PLAN.md