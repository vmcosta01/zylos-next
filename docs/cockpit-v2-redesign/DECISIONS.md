# Decisions — Zylos Cockpit v2.0 Redesign

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** ADRs Novas Documentadas
**Conceito Manifesto:** #7 (Documentos vivos — DECISIONS.md)
**Herdado de:** [DECISIONS_FASE9.md](../DECISIONS_FASE9.md) (ADR-001 a ADR-006)

---

## ADRs Existentes (Herdadas — Não Modificar)

| ADR | Título | Status | Origem |
|-----|--------|--------|--------|
| ADR-001 | Approval Gates Inline no Chat vs Modal Separado | ✅ Aceito | DECISIONS_FASE9.md |
| ADR-002 | Egress Checker Síncrono no Frontend | ✅ Aceito | DECISIONS_FASE9.md |
| ADR-003 | Computer View VNC Primário + Static Screenshot Fallback | ✅ Aceito | DECISIONS_FASE9.md |
| ADR-004 | Spaces/Documents em SQLite vs Arquivos Markdown | ✅ Aceito | DECISIONS_FASE9.md |
| ADR-005 | Multi-Model Config Per-Bot com Herança Global | ✅ Aceito | DECISIONS_FASE9.md |
| ADR-006 | Migrations SQL com Statements Individuais | ✅ Aceito | DECISIONS_FASE9.md |

---

## ADR-007: Spring Animation System Choice

**Data:** 2026-11-18
**Status:** ⏳ Pendente (Resolver na Fase 3)
**Contexto:** Como implementar physics-based animations interruptíveis em Leptos 0.7 WASM?

**Opções Avaliadas:**

| Opção | Prós | Contras | Bundle Overhead |
|-------|------|---------|-----------------|
| A: leptos-spring crate | Comunidade, mantida, API ergonômica | Pode não suportar Leptos 0.7 signals; dependência externa | ~3-5KB |
| B: Custom WASM solver | Controle total, zero deps externas, otimizado para nosso uso | Manutenção própria, math complexa, risco de bugs | ~2-4KB |
| C: CSS transitions apenas | Zero overhead, nativo | Sem interruptibilidade, sem velocity handoff, não é spring real | 0KB |

**Critérios de Decisão:**
1. Compatibilidade comprovada com Leptos 0.7 signals
2. Interruptibilidade + velocity handoff funcionais
3. Bundle overhead <5KB gzipped
4. prefers-reduced-motion desabilita completamente
5. ≥55fps em mid-range device

**Decisão:** ✅ Opção B — Custom WASM solver (`apps/zylos-cockpit/src/spring.rs`).

**Racional:**
- `leptos-spring` não existe como crate independente compatível com Leptos 0.7; `leptos-use` (v0.14-0.15) oferece `use_transition` mas sem velocity handoff nem interruptibilidade real
- Custom solver dá controle total sobre physics params (damping, response, mass), interruptibilidade e velocity handoff
- Bundle overhead estimado ~2-3KB gzipped (solver + RAF loop), dentro do budget <5KB
- Zero dependências externas além de `web-sys` (já presente)
- Testes unitários validam convergência, overshoot e interrupção sem necessidade de browser

**Alternativas Rejeitadas:**
1. Opção A (leptos-spring): crate inexistente para Leptos 0.7; leptos-use insuficiente
2. Opção C (CSS transitions): sem interruptibilidade, sem velocity handoff — rejeitada por não atender DESIGN_SYSTEM.md §8.2

**Consequências:**
- Manutenção própria do solver (~300 LOC)
- Physics params configuráveis via `SpringConfig` presets (UI/GESTURE/MICRO/ENTRANCE)
- `prefers-reduced-motion` detectado via `window.matchMedia` → snap instantâneo
- Integrado em `CollapsibleSidebar` (width animation); pronto para modals/toasts/tabs nas fases futuras

**Validação:** Testes unitários passam (convergência <2s, interruptibilidade, velocity handoff); bundle check na Fase 3 gate.

---

## ADR-008: Unificação de CSS (Single Source of Truth)

**Data:** 2026-11-18
**Status:** ✅ Aceito
**Contexto:** Dois sistemas CSS conflitantes (`design-tokens.css` vs `style.css`) com 3 paletas diferentes causam inconsistência visual e dívida técnica.

**Decisão:** `design-tokens.css` como ÚNICO arquivo de definições de cor/tipografia/espaçamento. Eliminar `style.css`. Componentes consomem tokens exclusivamente via CSS variables (`var(--token-name)`).

**Racional:**
- Elimina ambiguidade de 3 fontes de verdade
- CSS variables permitem theming futuro sem refatoração
- CI lint pode enforce zero hardcoded colors
- Alinha com DESIGN_SYSTEM.md como contrato visual (Conceito #3)

**Alternativas Rejeitadas:**
1. Manter ambos e documentar qual usar quando → confusão persistente
2. CSS-in-Rust (leptos-style) → runtime overhead, perde cacheability
3. Tailwind/CSS framework → bundle bloat, não necessário para escopo

**Consequências:**
- Migração única na Fase 1 (todas as cores hardcoded → var())
- Pre-commit hook bloqueia regressão
- style.css deletado permanentemente

---

## ADR-009: Command Palette como Primary Navigation

**Data:** 2026-11-18
**Status:** ✅ Aceito
**Contexto:** Sidebar com 30+ itens planos causa sobrecarga cognitiva. Power users precisam de navegação rápida sem mouse.

**Decisão:** Cmd+K (Mac) / Ctrl+K (Win/Linux) como método primário de navegação para power users. Sidebar torna-se secundária/discoverability. Command palette indexa: telas, agentes, tarefas, comandos, skills.

**Racional:**
- Linear, Vercel, Arc provaram eficácia do padrão
- Reduz carga cognitiva da sidebar (não precisa mostrar tudo sempre)
- Busca fuzzy supera navegação hierárquica para usuários experientes
- Histórico recente acelera workflows repetitivos

**Alternativas Rejeitadas:**
1. Apenas sidebar melhorada → ainda lenta para power users
2. Search bar no header → menos discoverable, sem keyboard-first
3. Menu hamburger mobile-only → inadequado para desktop power users

**Consequências:**
- Sidebar pode ser mais simples (menos itens visíveis por padrão)
- Index deve ser construído em mount (<100ms)
- Fuzzy search em WASM para performance
- Telemetria de uso para otimizar ranking futuro

---

## ADR-010: Material Translúcido (Backdrop-Filter)

**Data:** 2026-11-18
**Status:** ✅ Aceito
**Contexto:** Header e sidebar opacos perdem contexto visual do conteúdo atrás. Princípio Apple #2 (Materials) recomenda translucency para criar profundidade.

**Decisão:** `backdrop-filter: blur(20px) saturate(180%)` em header e sidebar, com fundo `rgba(15, 23, 42, 0.7)`. Fallback opaco automático para `prefers-reduced-transparency: reduce` e dispositivos low-perf.

**Racional:**
- Mantém contexto visual durante scroll
- Cria hierarquia de profundidade sem perder legibilidade
- Padrão estabelecido em macOS/iOS/visionOS
- Saturate(180%) compensa desaturação natural do blur

**Alternativas Rejeitadas:**
1. Fundo totalmente opaco → perde contexto, flat appearance
2. Blur sem saturate → cores lavadas, ilegível
3. Gradient overlay → artifício visual, não material real

**Consequências:**
- Performance monitoring necessário (backdrop-filter é GPU-intensive)
- Feature detection JS para adicionar `.low-perf` class
- Media query `prefers-reduced-transparency` obrigatória
- Z-index management crítico (header=100, sidebar=90)

---

## Log de Decisões Futuras

| # | Data | Título | Status |
|---|------|--------|--------|
| 007 | 2026-11-18 | Spring Animation System Choice | ⏳ Pendente (Fase 3) |
| 008 | 2026-11-18 | Unificação CSS (SSoT) | ✅ Aceito |
| 009 | 2026-11-18 | Command Palette Primary Nav | ✅ Aceito |
| 010 | 2026-11-18 | Material Translúcido | ✅ Aceito |

---

## Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| ADRs documentadas com contexto/decisão/rationale | ✅ Todas acima |
| Alternativas consideradas e rejeitadas | ✅ Cada ADR |
| Consequências explícitas | ✅ Cada ADR |
| Status claro (Aceito/Pendente/Rejeitado) | ✅ Tabela log |
| Cross-link com ARCHITECTURE.md | ✅ Seção 7 Architecture referencia ADRs |
| ADRs herdadas preservadas | ✅ Tabela no topo |

---

## Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 decisions; ADR-007 a ADR-010; herdadas ADR-001 a ADR-006 |

---

## ADR-011: Dois módulos wasm (principal + extras sob demanda)

**Data:** 2026-11-20 · **Status:** Implementado

**Contexto.** O wasm único do cockpit tinha 2,2 MB (600 KB em brotli) e todo o download e compilação aconteciam antes da primeira tela. A análise do code section mostrou ~38% em instâncias de view do `tachys`, espalhadas pelos 36 painéis; não havia uma dependência grande para cortar.

**Decisão.** `src/main.rs` (módulo principal) fica com o shell, Chat, Inbox, Activity, aprovações, Agentes, Skills e Logs. Os outros 29 painéis vão para `src/bin/extras.rs`, que o Trunk compila como segundo wasm (`data-type="worker"`: gera `extras.js` e `extras_bg.wasm` **sem hash e sem carregar na abertura**). `ExtraHost` carrega o módulo na primeira aba secundária (`extras-loader.js`) e chama `mount_extra`, que monta o painel num elemento da página com um runtime Leptos próprio; sair da aba libera a instância.

**Consequências.**
- Abertura: ~395 KB transferidos (antes ~700 KB), Lighthouse mobile Performance 91.
- Contextos não atravessam os módulos: o extras recria tema, toast e i18n. Spaces e o contador de aprovações não existem no extras (os painéis já os tratavam como opcionais).
- O estado de um painel secundário se perde ao trocar de aba (já era assim).
- `extras.js` e `extras_bg.wasm` não têm hash: `_headers` os serve com `no-cache` (revalidação por ETag).
- Depende de o Trunk manter o comportamento de `data-type="worker"`; o CI falha se `extras.js` não for gerado.
- O runtime Leptos fica duplicado (o extras tem ~1,6 MB), mas só é baixado se alguém abrir uma aba secundária.
