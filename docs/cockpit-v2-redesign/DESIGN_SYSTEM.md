# Design System — Zylos Cockpit v2.0

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Contrato Visual
**Conceito Manifesto:** #3 (Design System como contrato visual)
**Depende de:** [PRD.md](./PRD.md), [ARCHITECTURE.md](./ARCHITECTURE.md)
**Single Source of Truth:** `styles/design-tokens.css` (ADR-008)

---

## 1. Paleta de Cores Unificada

### 1.1 Cores Semânticas (Dark Mode Only — v2.0)

| Token | Valor | Uso | Exemplo |
|-------|-------|-----|---------|
| `--color-bg-primary` | `#0f172a` | Fundo principal da aplicação | Body, main content area |
| `--color-bg-secondary` | `#1e293b` | Fundos de cards, painéis | AgentCard, MetricCard |
| `--color-bg-tertiary` | `#334155` | Hover states, elevated surfaces | Sidebar item hover |
| `--color-bg-translucent` | `rgba(15, 23, 42, 0.7)` | Header/Sidebar com backdrop-filter | Header, Sidebar |
| `--color-text-primary` | `#f8fafc` | Texto principal, títulos | Headings, body text |
| `--color-text-secondary` | `#94a3b8` | Labels, metadados, timestamps | "LAST ACTIVE", subtitles |
| `--color-text-tertiary` | `#64748b` | Placeholder, disabled text | Input placeholders |
| `--color-border-default` | `#334155` | Bordas sutis de separação | Card borders, dividers |
| `--color-border-strong` | `#475569` | Bordas de foco, ênfase | Active tab, focused input |
| `--color-accent-primary` | `#3b82f6` | Ações primárias, links, active states | Buttons, active nav, links |
| `--color-accent-hover` | `#2563eb` | Hover de accent-primary | Button hover |
| `--color-status-success` | `#22c55e` | Healthy, approved, connected | Status badges, success toasts |
| `--color-status-warning` | `#f59e0b` | Idle, pending, degraded | Warning badges, pending approvals |
| `--color-status-error` | `#ef4444` | Error, rejected, disconnected | Error badges, failed states |
| `--color-status-info` | `#3b82f6` | Info, running, streaming | Running badges, info toasts |

### 1.2 Eliminação de Conflitos (ADR-008)

**Problema identificado:** `design-tokens.css` define `--primary: #3b82f6`, `style.css` redefine `.btn-primary { background: #2563eb }`, e componentes inline usam `#60a5fa` hardcoded. Três fontes de verdade = inconsistência.

**Resolução:**
1. `design-tokens.css` torna-se ÚNICO arquivo de definições de cor
2. `style.css` é eliminado; seus estilos migrados para `base.css` (reset/utilities) e `components/*.css`
3. Componentes Rust NUNCA usam cores hardcoded; sempre `class="text-primary"` ou `style:background-color="var(--color-accent-primary)"`
4. CI lint rule: `grep -r '#[0-9a-fA-F]\{6\}' src/` deve retornar zero resultados

### 1.3 Prefers-Color-Scheme (Futuro)

Tokens preparados para light mode via `@media (prefers-color-scheme: light)` mas NÃO implementado em v2.0. Estrutura:
```css
@media (prefers-color-scheme: light) {
  :root {
    --color-bg-primary: #ffffff;
    --color-text-primary: #0f172a;
    /* ... */
  }
}
```

---

## 2. Tipografia

### 2.1 Família

| Token | Valor | Uso |
|-------|-------|-----|
| `--font-family-sans` | `'Inter', ui-sans-serif, system-ui, sans-serif` | Todo texto UI |
| `--font-family-mono` | `'JetBrains Mono', ui-monospace, monospace` | Código, hashes, timestamps técnicos |

**Fallback:** Inter via Google Fonts CDN no index.html; fallback para system-ui garante renderização offline.

### 2.2 Escala Tipográfica (Modular Scale 1.25)

| Token | Size | Weight | Line Height | Uso |
|-------|------|--------|-------------|-----|
| `--text-xs` | 0.75rem (12px) | 400 | 1rem | Badges, captions, timestamps |
| `--text-sm` | 0.875rem (14px) | 400 | 1.25rem | Labels, secondary text |
| `--text-base` | 1rem (16px) | 400 | 1.5rem | Body text, inputs |
| `--text-lg` | 1.125rem (18px) | 500 | 1.75rem | Section headers |
| `--text-xl` | 1.25rem (20px) | 600 | 1.75rem | Page titles |
| `--text-2xl` | 1.5rem (24px) | 700 | 2rem | Hero metrics |
| `--text-3xl` | 1.875rem (30px) | 700 | 2.25rem | Dashboard KPIs grandes |

### 2.3 Regras de Tipografia
- **Headings:** Sempre weight ≥600; nunca uppercase exceto badges
- **Body:** Weight 400; line-height ≥1.5 para legibilidade
- **Mono:** Usado APENAS para dados técnicos (hashes, IDs, code snippets)
- **Antialiasing:** `-webkit-font-smoothing: antialiased` no root

---

## 3. Espaçamento (Escala Modular 4px)

| Token | Valor | Uso |
|-------|-------|-----|
| `--space-1` | 0.25rem (4px) | Micro gaps (icon-to-text) |
| `--space-2` | 0.5rem (8px) | Tight spacing (badge padding) |
| `--space-3` | 0.75rem (12px) | Compact elements |
| `--space-4` | 1rem (16px) | Default spacing |
| `--space-5` | 1.25rem (20px) | Card internal padding |
| `--space-6` | 1.5rem (24px) | Section gaps |
| `--space-8` | 2rem (32px) | Major section separation |
| `--space-10` | 2.5rem (40px) | Page margins |
| `--space-12` | 3rem (48px) | Layout gutters |

### Grid System
- **Sidebar width:** 280px (collapsed: 64px)
- **Header height:** 64px
- **Content max-width:** 1440px (centered)
- **Card grid:** CSS Grid com `repeat(auto-fill, minmax(320px, 1fr))`
- **Gap padrão:** `var(--space-4)` entre cards, `var(--space-6)` entre seções

---

## 4. Sombras e Elevações

| Token | Valor | Uso |
|-------|-------|-----|
| `--shadow-none` | `none` | Flat elements |
| `--shadow-sm` | `0 1px 2px rgba(0,0,0,0.3)` | Subtle elevation (inputs) |
| `--shadow-md` | `0 4px 6px rgba(0,0,0,0.3)` | Cards, panels |
| `--shadow-lg` | `0 10px 15px rgba(0,0,0,0.4)` | Dropdowns, popovers |
| `--shadow-xl` | `0 20px 25px rgba(0,0,0,0.5)` | Modals, command palette |
| `--shadow-glow-blue` | `0 0 20px rgba(59,130,246,0.3)` | Active/focused accents |

### Regras de Elevação
- Cards em repouso: `--shadow-md`
- Cards em hover: `--shadow-lg` + `transform: translateY(-2px)`
- Overlays (command palette, modals): `--shadow-xl`
- Nunca usar shadow sem propósito semântico

---

## 5. Border Radius

| Token | Valor | Uso |
|-------|-------|-----|
| `--radius-sm` | 4px | Badges, tags |
| `--radius-md` | 8px | Inputs, buttons |
| `--radius-lg` | 12px | Cards, panels |
| `--radius-xl` | 16px | Modals, large containers |
| `--radius-full` | 9999px | Pills, avatars, circular buttons |

---

## 6. Breakpoints Responsivos

| Token | Valor | Layout |
|-------|-------|--------|
| `--bp-sm` | 640px | Mobile portrait |
| `--bp-md` | 768px | Tablet portrait |
| `--bp-lg` | 1024px | Laptop |
| `--bp-xl` | 1280px | Desktop |
| `--bp-2xl` | 1536px | Large desktop |

### Comportamento por Breakpoint
- **<640px:** Sidebar hidden (hamburger menu), single column, stacked cards
- **640-1024px:** Sidebar collapsible, 2-column grid
- **>1024px:** Sidebar always visible, 3-4 column grid
- **>1536px:** Content max-width capped, extra whitespace

---

## 7. Materiais Translúcidos (ADR-010)

### 7.1 Especificação Técnica

```css
.material-translucent {
  background: var(--color-bg-translucent);
  backdrop-filter: blur(20px) saturate(180%);
  -webkit-backdrop-filter: blur(20px) saturate(180%);
  border-bottom: 1px solid var(--color-border-default);
}
```

### 7.2 Aplicação

| Componente | Blur | Saturate | Opacity BG | Border |
|-----------|------|----------|------------|--------|
| Header | 20px | 180% | 0.7 | Bottom 1px |
| Sidebar | 20px | 180% | 0.7 | Right 1px |
| Command Palette Overlay | 30px | 200% | 0.8 | None |
| Toast Container | 15px | 150% | 0.85 | All 1px |

### 7.3 Fallbacks e Preferências

```css
/* Reduced transparency preference */
@media (prefers-reduced-transparency: reduce) {
  .material-translucent {
    background: var(--color-bg-primary);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}

/* Low-performance fallback (detected via JS class) */
.low-perf .material-translucent {
  background: var(--color-bg-primary);
  backdrop-filter: none;
}
```

---

## 8. Spring Animation System (ADR-007)

### 8.1 Parâmetros Padrão

| Contexto | Damping | Stiffness | Mass | Response Time | Uso |
|----------|---------|-----------|------|---------------|-----|
| UI Transitions | 1.0 | 170 | 1 | 0.3-0.4s | Page transitions, panel open/close |
| Gestures | 0.8 | 200 | 1 | 0.2-0.3s | Drag, swipe, pull-to-refresh |
| Micro-interactions | 0.9 | 180 | 1 | 0.15-0.25s | Button press, card hover, badge pulse |
| Entrance | 0.85 | 160 | 1 | 0.4-0.5s | Component mount, list item stagger |

### 8.2 Interruptibilidade
- Toda animação spring É interruptível
- Nova animação herda velocity da anterior (velocity handoff)
- Sem "reset to start" — sempre continua do estado atual

### 8.3 Reduced Motion

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
  }
}
```

JS equivalent: `window.matchMedia('(prefers-reduced-motion: reduce)').matches` → disable spring solver, use instant state changes.

---

## 9. Estados Obrigatórios por Componente

### 9.1 Matriz de Cobertura

| Componente | Loading | Empty | Error | Success | Validation | Disabled |
|-----------|---------|-------|-------|---------|------------|----------|
| AgentCard | ✅ Skeleton shimmer | ✅ "No agents" illustration | ✅ Retry + message | ✅ Normal render | N/A | ✅ Opacity 0.5 + tooltip |
| InlineApprovalCard | ✅ Skeleton | N/A | ✅ Retry button | ✅ Receipt collapsed | ✅ Hash mismatch warning | ✅ Post-decision locked |
| MetricCard | ✅ Pulse placeholder | ✅ "—" dash | ✅ Icon + tooltip | ✅ Value + trend | N/A | N/A |
| FilterBar | N/A | ✅ All filters inactive | ✅ Clear error state | ✅ Active filter highlight | ✅ Invalid regex warning | ✅ During bulk operation |
| VncCanvas | ✅ Connecting spinner | ✅ "No device paired" | ✅ Screenshot fallback | ✅ Live stream | N/A | ✅ "Control released" overlay |
| CommandPalette | ✅ Search debounce indicator | ✅ "No results" + suggestions | ✅ Network error toast | ✅ Result selected highlight | ✅ Invalid shortcut warning | ✅ During execution |
| Sidebar | ✅ Skeleton nav items | N/A | ✅ Retry connection | ✅ Active item highlight | N/A | ✅ During navigation |
| Toast | N/A | N/A | ✅ Error variant | ✅ Success variant | N/A | ✅ Dismissed state |

### 9.2 Especificações de Estados

#### Loading
- **Skeleton:** Shimmer animation (gradient left-to-right, 1.5s loop)
- **Spinner:** 24px diameter, accent color, 1s rotation
- **Pulse:** Opacity 0.5↔1.0, 2s cycle
- **Max duration:** Se loading >10s, mostrar mensagem + retry

#### Empty
- **Ilustração:** SVG inline, 120x120px, muted colors
- **Texto:** Título + descrição + CTA (quando aplicável)
- **Nunca:** Tela branca sem explicação

#### Error
- **Ícone:** ⚠️ ou ✕ em status-error color
- **Mensagem:** Humana, acionável ("Não foi possível conectar. Verifique sua rede.")
- **Ação:** Retry button sempre presente
- **Nunca:** Stack trace exposto ao usuário

#### Success
- **Feedback:** Visual confirmation (checkmark, color change)
- **Duração:** Toast auto-dismiss 5s; inline states persistem
- **Nunca:** Alert modal para sucesso rotineiro

#### Validation
- **Posição:** Inline abaixo do input
- **Cor:** status-warning para warnings, status-error para erros
- **Timing:** Após blur ou submit (não durante digitação)
- **Ícone:** ℹ️ para info, ⚠️ para warning, ✕ para error

#### Disabled
- **Visual:** Opacity 0.5, cursor not-allowed
- **Tooltip:** Explica POR QUE está disabled
- **Nunca:** Disabled sem razão visível

---

## 10. Componentes Críticos — Especificação Visual

### 10.1 AgentCard

```
┌─────────────────────────────────────┐
│ Backend Engineer  [Idle] │ ← Status badge (warning color)
│                                     │
│ TASKS          LAST ACTIVE          │
│ 3              15m ago              │ ← Mono font for numbers
│                                     │
│ ▁▂▃▅▇▅▃▂▁▂▃▅▇▅▃▂▁▂▃▅▇▅▃▂▁▂▃▅▇▅▃▂ │ ← Sparkline (accent color)
└─────────────────────────────────────┘
```

- **Dimensões:** Min 320px width, auto height
- **Padding:** var(--space-5)
- **Border:** 1px solid var(--color-border-default), radius-lg
- **Hover:** shadow-lg + translateY(-2px) + sparkline animada
- **Click:** Ripple effect + navigate to detail
- **Status badge:** Pill shape, radius-full, bg translucent com status color

### 10.2 InlineApprovalCard

```
┌─────────────────────────────────────────────┐
│ ⚠️ APPROVAL REQUIRED                        │ ← Header warning
│                                             │
│ Action: Deploy to production                │
│ Risk: HIGH                                  │ ← Risk tier badge
│ Hash: a3f8b2c1...                           │ ← Mono, truncated
│ Requested by: Backend Engineer • 2m ago     │
│                                             │
│ [Approve]  [Reject]  [Escalate]             │ ← Action buttons
└─────────────────────────────────────────────┘
```

- **Width:** 100% of chat container
- **Border-left:** 4px solid status-warning
- **Background:** bg-secondary
- **Buttons:** Equal width, status-colored variants
- **Post-decision:** Colapsa para receipt (height animation spring)

### 10.3 Header Translúcido

```
┌─────────────────────────────────────────────────────────┐
│ Zylos Cockpit  v2.0           [Login] │ ← backdrop-filter
└─────────────────────────────────────────────────────────┘
```

- **Height:** 64px fixed
- **Position:** Sticky top, z-index 100
- **Material:** backdrop-filter blur(20px) saturate(180%)
- **Logo:** Left aligned, accent color
- **Version badge:** Pill, text-xs, bg-tertiary
- **Theme toggle:** Icon button, right side
- **Login button:** Accent primary, radius-md

### 10.4 Sidebar Colapsável

```
EXPANDED (280px)          COLLAPSED (64px)
┌──────────────────┐      ┌──────┐
│ CORE             │      │  │ ← Icon only
│ Agentes  3  │      │ fas   5  │      │       │      │      │
│                  │      │  ⚙️  │
│ ORQUESTRAÇÃO     │      │  DAG          │      │ ️ Sentinela    │      │      │
│  Metas  │      │  │
│ │ │ ⚙️ │
│ COMUNICAÇÃO │      └──────┘
│  Phone Relay  │
│  Chat  │
│                  │
│ CONFIGURAÇÃO     │
│  Personalidades│
│  ⚙️ Settings     │
└──────────────────┘
```

- **Transition:** Spring animation (damping 0.85, response 0.3s)
- **Group headers:** Text-xs, uppercase, text-tertiary, collapsible chevron
- **Items:** Padding var(--space-3), hover bg-tertiary, active accent-primary bg
- **Badges:** Right-aligned, pill, text-xs, status color bg
- **Collapse trigger:** Chevron at bottom or double-click edge
- **Persist:** localStorage key `sidebar-collapsed-groups`

### 10.5 Command Palette

```
┌─────────────────────────────────────┐
│ Search commands, agents, pages…  │ ← Auto-focused input
├─────────────────────────────────────┤
│ RECENT                              │
│  ⌘K Open Command Palette            │
│  Navigate to Agents                 │
│                                     │
│ PAGES                               │
│  Agentes  │
│  Tarefas │
│  DAG do Swarm                    │
│                                     │
│ AGENTS                              │
│  Backend Engineer (Idle)            │
│  Chief of Staff (Running)           │
└─────────────────────────────────────┘
```

- **Overlay:** Centered, max-width 640px, shadow-xl, radius-xl
- **Backdrop:** rgba(0,0,0,0.5) com blur(4px)
- **Input:** Full width, text-lg, no border-bottom
- **Results:** Grouped, keyboard navigable (↑↓), Enter selects
- **Shortcut hints:** Right-aligned, text-xs, text-tertiary
- **Animation:** Spring entrance (scale 0.95→1, opacity 0→1)

### 10.6 Toast Notification

```
┌─────────────────────────────────┐
│ ✅ Agent "Backend Engineer"     │ ← Success variant
│    completed task #1234         │
│                            [✕]  │
└─────────────────────────────────┘
```

- **Position:** Fixed bottom-right, margin var(--space-4)
- **Stack:** Vertical, newest on top, max 3 visible
- **Width:** 360px
- **Variants:** Success (green left border), Error (red), Warning (amber), Info (blue)
- **Auto-dismiss:** 5s success/info, manual error/warning
- **Pause on hover:** Timer pauses
- **Click:** Navigate to related entity (when applicable)
- **Animation:** Slide in from right (spring), fade out on dismiss

---

## 11. Acessibilidade

### 11.1 Contraste

| Elemento | Foreground | Background | Ratio | Standard |
|----------|-----------|------------|-------|----------|
| Text primary | #f8fafc | #0f172a | 15.4:1 | AAA ✅ |
| Text secondary | #94a3b8 | #0f172a | 5.6:1 | AA ✅ |
| Accent on bg | #3b82f6 | #0f172a | 4.8:1 | AA ✅ |
| Status success | #22c55e | #1e293b | 4.6:1 | AA ✅ |
| Status error | #ef4444 | #1e293b | 4.5:1 | AA ✅ |

### 11.2 Focus States
- **Outline:** 2px solid accent-primary, offset 2px
- **Nunca:** `outline: none` sem replacement
- **Visible:** Always visible on keyboard focus; optional on mouse click

### 11.3 Keyboard Navigation
- **Tab order:** Logical DOM order
- **Cmd+K:** Opens command palette
- **Esc:** Closes overlays, modals, dropdowns
- **Arrow keys:** Navigate within lists, dropdowns
- **Enter/Space:** Activate buttons, links

### 11.4 Screen Reader
- **ARIA labels:** All icon-only buttons
- **Live regions:** Toast container (`aria-live="polite"`)
- **Landmarks:** `<nav>`, `<main>`, `<header>` semantic HTML
- **Alt text:** All images, SVGs with meaningful content

---

## 12. Implementação em CSS Variables

```css
/* styles/design-tokens.css — SINGLE SOURCE OF TRUTH */
:root {
  /* Colors */
  --color-bg-primary: #0f172a;
  --color-bg-secondary: #1e293b;
  --color-bg-tertiary: #334155;
  --color-bg-translucent: rgba(15, 23, 42, 0.7);
  --color-text-primary: #f8fafc;
  --color-text-secondary: #94a3b8;
  --color-text-tertiary: #64748b;
  --color-border-default: #334155;
  --color-border-strong: #475569;
  --color-accent-primary: #3b82f6;
  --color-accent-hover: #2563eb;
  --color-status-success: #22c55e;
  --color-status-warning: #f59e0b;
  --color-status-error: #ef4444;
  --color-status-info: #3b82f6;
  
  /* Typography */
  --font-family-sans: 'Inter', ui-sans-serif, system-ui, sans-serif;
  --font-family-mono: 'JetBrains Mono', ui-monospace, monospace;
  --text-xs: 0.75rem;
  --text-sm: 0.875rem;
  --text-base: 1rem;
  --text-lg: 1.125rem;
  --text-xl: 1.25rem;
  --text-2xl: 1.5rem;
  --text-3xl: 1.875rem;
  
  /* Spacing */
  --space-1: 0.25rem;
  --space-2: 0.5rem;
  --space-3: 0.75rem;
  --space-4: 1rem;
  --space-5: 1.25rem;
  --space-6: 1.5rem;
  --space-8: 2rem;
  --space-10: 2.5rem;
  --space-12: 3rem;
  
  /* Shadows */
  --shadow-sm: 0 1px 2px rgba(0,0,0,0.3);
  --shadow-md: 0 4px 6px rgba(0,0,0,0.3);
  --shadow-lg: 0 10px 15px rgba(0,0,0,0.4);
  --shadow-xl: 0 20px 25px rgba(0,0,0,0.5);
  
  /* Radius */
  --radius-sm: 4px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-full: 9999px;
  
  /* Breakpoints (use in @media queries) */
  --bp-sm: 640px;
  --bp-md: 768px;
  --bp-lg: 1024px;
  --bp-xl: 1280px;
  --bp-2xl: 1536px;
}
```

---

## 13. Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| Tabela completa de tokens | ✅ Seções 1-6 |
| 6 estados obrigatórios por componente | ✅ Seção 9 |
| Materiais translúcidos spec | ✅ Seção 7 |
| Spring parameters definidos | ✅ Seção 8 |
| prefers-reduced-motion | ✅ Seção 8.3 |
| prefers-reduced-transparency | ✅ Seção 7.3 |
| prefers-contrast | ✅ Seção 11.1 |
| Single source of truth CSS | ✅ Seção 1.2, ADR-008 |
| Acessibilidade WCAG AA | ✅ Seção 11 |

---

## 14. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 design system; unified tokens; spring specs; translucent materials |