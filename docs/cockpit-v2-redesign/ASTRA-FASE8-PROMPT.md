# Zylos Cockpit v2.0 — Fase 8: Acessibilidade + Performance

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `fd48ae0` (Fase 1-7 complete)

## Context
Fases 1-7 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável
- Fase 3 (`09d87fa`): spring animation system (ADR-007)
- Fase 4 (`c959c37`): inline approval cards reais (ADR-001)
- Fase 5 (`042a0de`): VNC stream real + screenshot fallback (ADR-003)
- Fase 6 (`d6860da`): command palette Cmd+K omnibar (ADR-009)
- Fase 7 (`fd48ae0`): AgentCard micro-interactions + Toast + ErrorBoundary + EmptyState

Agora você implementa a **Fase 8: Acessibilidade + Performance** — a última fase do redesign.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 8" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Seções 11 (Acessibilidade), 7 (Motion)
3. `docs/cockpit-v2-redesign/TESTING.md` — Seção 6 (Acessibilidade), Seção 9 (Performance)

## Auditoria do Estado Atual

### O que JÁ existe (não recriar):
- `@media (prefers-reduced-motion: reduce)` em `base.css:516` — desativa animações
- `@media (prefers-reduced-transparency: reduce)` em `base.css:524` — remove backdrop-filter
- `@media (prefers-contrast: more)` em `base.css:532` — bordas mais grossas
- `:focus-visible` em `base.css:32` — outline básico
- `<main role="main">` em `app.rs:108`
- `role="alert"` no error banner em `app.rs:148`
- `<html lang="pt-BR">` em `index.html:2`
- `<meta name="description">` em `index.html:7`
- `<meta name="viewport">` em `index.html:5`
- `aria-label` no VNC canvas button

### O que FALTA (implementar):
- **`<nav>` landmark** — sidebar usa `<div>`, precisa ser `<nav aria-label="Navegação principal">`
- **`<header>` landmark** — TranslucentHeader usa `<div>`, precisa ser `<header>`
- **`aria-live="polite"`** no ToastContainer — screen readers não anunciam toasts
- **ARIA labels** em icon-only buttons (dark mode toggle, sidebar collapse, etc.)
- **`tabindex`** e focus management no CommandPalette (focus trap)
- **`skip-to-content` link** — permite pular navegação direto para main
- **`prefers-reduced-motion`** nos spring animations (WASM-side check)
- **Trunk.toml wasm-opt flags** agressivos para bundle <800KB
- **Route-based code splitting** via `lazy()` do Leptos

## Fase 8 — Tarefas Específicas

### 8.1 Semantic HTML Landmarks
- `apps/zylos-cockpit/src/components/collapsible_sidebar.rs`: trocar container de `<div>` para `<nav aria-label="Navegação principal">`
- `apps/zylos-cockpit/src/components/translucent_header.rs`: trocar container de `<div>` para `<header>`
- `apps/zylos-cockpit/src/app.rs`: adicionar `<aside>` ao redor da sidebar, `<main>` já existe
- `apps/zylos-cockpit/src/components/command_palette.rs`: adicionar `role="dialog"` e `aria-label="Command palette"`

### 8.2 Skip-to-Content Link
- Adicionar em `app.rs` como primeiro elemento no DOM:
```rust
view! {
    <a href="#main-content" class="skip-link">"Pular para o conteúdo"</a>
    // ... resto do layout
    <main id="main-content" class="shell-main" role="main">
```
- CSS: `.skip-link { position: absolute; top: -40px; left: 0; ... }` visível apenas no `:focus`

### 8.3 ARIA Labels em Icon-Only Buttons
- Auditar todos os `<button>` sem texto visível e adicionar `aria-label`
- Componentes a verificar:
  - `translucent_header.rs`: dark mode toggle button
  - `collapsible_sidebar.rs`: collapse/expand toggle
  - `command_palette.rs`: close button
  - `toast_container.rs`: close (×) button
  - `vnc_canvas.rs`: take control, release, screenshot buttons
  - `hitl_approval_panel.rs`: approve, reject, escalate buttons

### 8.4 Toast Container Live Region
- Em `toast_container.rs`, adicionar `aria-live="polite"` e `aria-atomic="true"` no container:
```rust
view! {
    <div class="toast-container" aria-live="polite" aria-atomic="false" role="log">
        // toasts aqui
    </div>
}
```
- Cada toast item: `role="status"` e `aria-label` com variante

### 8.5 Focus Management no CommandPalette
- Quando palette abre: focus no input
- Quando palette fecha: restore focus ao elemento anterior
- Focus trap: Tab/Shift+Tab cicla dentro da palette
- Esc fecha e restore focus

### 8.6 prefers-reduced-motion nos Springs (WASM-side)
- Em `spring.rs`, verificar `window.matchMedia("(prefers-reduced-motion: reduce)")`
- Se reduce: spring duration = 0 (transições instantâneas)
- Cache o resultado no mount, não verificar a cada frame

### 8.7 Trunk.toml wasm-opt Flags Agressivos
- Editar `apps/zylos-cockpit/Trunk.toml`:
```toml
[build]
# ...existing...

[[hooks]]
stage = "post_build"
command = "wasm-opt"
command_arguments = ["-Oz", "--strip-debug", "--strip-producers", "$WASM_FILE", "-o", "$WASM_FILE"]
```
- Alternativamente, no `[profile.release]` do Cargo.toml:
```toml
[profile.release]
opt-level = "z"      # Optimize for size
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### 8.8 Bundle Analysis
- Rodar `trunk build --release` e verificar tamanho
- Identificar dead code com `wasm2map` ou similar
- Remover imports não usados
- Verificar se há código de debug em release

### 8.9 Route-Based Code Splitting
- Usar `leptos::lazy()` para carregar componentes pesados sob demanda:
```rust
let LazyAgents = lazy(|| async {
    let mod = AgentsPanel::lazy_load().await;
    move || view! { <AgentsPanel /> }
});
```
- Componentes candidatos: EvidenceLedgerPanel, SoarDashboard, SwarmTopologyPanel, AnalyticsPanel

### 8.10 WCAG Contrast Verification
- Verificar todas as combinações de cor do DESIGN_SYSTEM.md §11.1
- Text primary (#f8fafc) on bg primary (#0f172a) = 15.4:1 ✅
- Text secondary (#94a3b8) on bg primary (#0f172a) = 5.6:1 ✅
- Accent (#3b82f6) on bg primary (#0f172a) = 4.8:1 ✅
- Status colors on bg secondary (#1e293b): success 4.6:1 ✅, error 4.5:1 ✅
- Todas passam WCAG AA (4.5:1 text, 3:1 UI)

## Arquivos a Modificar
- `apps/zylos-cockpit/src/app.rs` — skip link, semantic landmarks
- `apps/zylos-cockpit/src/components/collapsible_sidebar.rs` — `<nav>` landmark + ARIA
- `apps/zylos-cockpit/src/components/translucent_header.rs` — `<header>` landmark + ARIA labels
- `apps/zylos-cockpit/src/components/command_palette.rs` — `role="dialog"`, focus trap, ARIA
- `apps/zylos-cockpit/src/components/toast_container.rs` — `aria-live="polite"`, ARIA roles
- `apps/zylos-cockpit/src/components/hitl_approval_panel.rs` — ARIA labels nos botões
- `apps/zylos-cockpit/src/components/vnc_canvas.rs` — ARIA labels nos botões
- `apps/zylos-cockpit/src/spring.rs` — prefers-reduced-motion check
- `apps/zylos-cockpit/styles/base.css` — skip-link CSS, focus-visible enhancements
- `apps/zylos-cockpit/Trunk.toml` — wasm-opt flags (se existir)
- `apps/zylos-cockpit/Cargo.toml` — release profile optimization

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 8 — accessibility (WCAG AA) + performance optimization`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- **Não quebrar nada das Fases 1-7** — apenas adicionar A11Y/performance
- **Zero mocks** — media queries e ARIA reais

## Verificação Obrigatória (gate)
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
2. `cargo test -p zylos-cockpit --lib` — testes passam
3. `cargo clippy -p zylos-cockpit --all-targets` — limpo ou warnings aceitáveis
4. Grep para `<nav`, `<header`, `aria-live`, `aria-label`, `role="dialog"` — todos presentes
5. Grep para `skip-link` em CSS e HTML — presente
6. Bundle size: `trunk build --release` → wasm gzipped <800KB

## Out of Scope (NÃO fazer)
- WCAG AAA compliance (AA é suficiente)
- Sign language video
- Braille display optimization
- Cognitive load testing
- Novas features funcionais
- Mudanças no design system (cores, espaçamento)

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação
- Bundle size final (WASM gzipped)
- Lista de ARIA landmarks adicionados
- Lista de ARIA labels adicionados
- Nota sobre prefers-reduced-motion

**Comece lendo os arquivos atuais e aplique as mudanças incrementais. Não reescreva componentes inteiros — apenas adicione atributos ARIA e semantic HTML.**