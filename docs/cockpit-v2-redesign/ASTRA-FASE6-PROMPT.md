# Zylos Cockpit v2.0 — Fase 6: Command Palette (ADR-009)

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `042a0de` (Fase 1-5 complete)

## Context
Fases 1-5 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável
- Fase 3 (`09d87fa`): spring animation system (ADR-007)
- Fase 4 (`c959c37`): inline approval cards reais (ADR-001)
- Fase 5 (`042a0de`): VNC stream real + screenshot fallback (ADR-003)

Agora você implementa a **Fase 6: Command Palette (ADR-009)**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 6" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Tokens, spring params, breakpoints
3. `docs/cockpit-v2-redesign/APP_FLOW.md` — Seção AF-10 (Command Palette overlay)
4. `docs/cockpit-v2-redesign/DECISIONS.md` — ADR-009 (Command Palette Primary Nav)

## Problema Atual
O cockpit tem uma sidebar com 30+ itens de navegação (Agentes, Tarefas, Memória, Metas, DAG, Sentinela, etc). A navegação é lenta — usuário precisa clicar em cada item. A ADR-009 decidiu implementar um **omnibar Cmd+K** estilo Linear/Vercel para busca rápida e execução de ações.

## Fase 6 — O que implementar

### 6.1 Componente CommandPalette
- Overlay modal full-screen com backdrop-filter blur
- Spring animation entrance (scale 0.95→1.0, opacity 0→1)
- Centralizado, max-width 600px, border-radius 12px
- Input field no topo com placeholder "Buscar telas, agentes, comandos..."
- Resultados em lista agrupada abaixo do input

### 6.2 Keyboard Shortcut
- `Cmd+K` (Mac) / `Ctrl+K` (Win/Linux) abre/fecha a palette
- `Esc` fecha a palette
- Listener global no `window` com `keydown` event
- Prevenir default do browser (Cmd+K = address bar em alguns browsers)

### 6.3 Index Construction
- Build index on mount from available data:
  - **Pages:** Agentes, Tarefas, Memória, Metas, DAG, Sentinela, Phone Relay, Personalidades, Settings
  - **Agents:** From GET /api/v1/phase2/status (agent list with name + status)
  - **Commands:** "Toggle Dark Mode", "Clear Notifications", "Export Data", "Open Settings", "Refresh All"
  - **Skills:** From GET /api/v1/hermes/skills (skill names)
- Each item: `{ id, label, category, icon, action }` where action is either a route path or a callback

### 6.4 Fuzzy Search
- Implement simple fuzzy matching in pure Rust/WASM (no external crate)
- Algorithm: split query into chars, find each char in order in target string
- Score: lower distance between matched chars = better match
- Filter results where score > 0, sort by score descending
- Debounce input 50ms to avoid excessive re-searches

### 6.5 Grouped Results
- Categories with headers: "Recent" (from history), "Pages", "Agents", "Commands", "Skills"
- Max 8 results per category, total max 20 visible
- Empty category = hidden (no empty headers)
- "No results found" message when search returns nothing

### 6.6 Keyboard Navigation
- `↑` / `↓` arrows move selection highlight
- `Enter` executes selected item
- Selection wraps at boundaries (↓ on last → first, ↑ on first → last)
- Selected item has accent border-left + background highlight

### 6.7 Recent History
- Store last 10 executed items in localStorage key `zylos_command_history`
- Show "Recent" group first when palette opens with empty search
- Each history entry: `{ id, label, category, timestamp }`
- Deduplicate: same item executed twice moves to top

### 6.8 Execute Action
- Navigate items: use `leptos_router` to push new route, then close palette
- Command items: execute callback (toggle theme, refresh data, etc.)
- After execution: add to history, close palette with spring exit animation

### 6.9 Debounce + Loading
- 50ms debounce on input using `set_timeout_with_handle`
- Show subtle spinner/dot when search is pending
- Clear previous timeout on new keystroke

### 6.10 Close Behavior
- `Esc` key closes palette
- Click outside (backdrop) closes palette
- Spring exit animation (scale 1.0→0.95, opacity 1→0)
- Clear search input on close

## Arquivos a modificar/criar
- `apps/zylos-cockpit/src/components/command_palette.rs` — **novo**: CommandPalette component
- `apps/zylos-cockpit/src/components/mod.rs` — exportar novo componente
- `apps/zylos-cockpit/src/main.rs` ou `app.rs` — montar CommandPalette no layout shell + add keydown listener
- `apps/zylos-cockpit/styles/base.css` — classes CSS para command palette overlay

## Componentes/Sistemas Existentes a Usar
- **Spring system** (Fase 3): `create_spring()` para entrance/exit animations
- **Design tokens** (Fase 1): `var(--color-*)` para cores, `var(--space-*)` para espaçamento
- **Backdrop-filter** (Fase 2): overlay translúcido estilo `backdrop-filter: blur(20px)`
- **Auth helper**: `crate::auth::authed()` para requests autenticados
- **Gateway config**: `crate::gateway_config::gateway_base()` para URL base

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 6 — command palette Cmd+K omnibar (ADR-009)`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- **Zero mocks** — usar API real; se agente/skill list vazia, mostrar empty state honesto
- **Bundle budget** — adicionar <5KB ao WASM gzipped (fuzzy search é pure Rust, sem deps)

## Verificação obrigatória (gate)
1. `cargo check -p zylos-gateway` — compila
2. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
3. `cargo test -p zylos-cockpit --lib` — testes passam
4. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — limpo
5. `cargo fmt --all --check` — formatado
6. Grep para stubs: zero `todo!()` ou `unimplemented!()` nos arquivos modificados

## Out of Scope (NÃO fazer nesta fase)
- Preview pane lateral ao selecionar item
- Subcommands aninhados (ex: "agents:stop:name")
- Custom plugin commands registration
- Voice input
- Search within file contents
- AgentCard redesign (Fase 7)
- Toast system (Fase 7)

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação (test result lines)
- Bundle size delta vs commit anterior
- Lista de arquivos criados/modificados
- Nota sobre performance (search 500 items benchmark)

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 6 e o código existente em `src/main.rs` ou `src/app.rs` para entender onde montar o CommandPalette no layout.**