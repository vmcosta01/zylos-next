# Zylos Cockpit v2.0 — Fase 7: AgentCard + Toast + Polish

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `d6860da` (Fase 1-6 complete)

## Context
Fases 1-6 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável
- Fase 3 (`09d87fa`): spring animation system (ADR-007)
- Fase 4 (`c959c37`): inline approval cards reais (ADR-001)
- Fase 5 (`042a0de`): VNC stream real + screenshot fallback (ADR-003)
- Fase 6 (`d6860da`): command palette Cmd+K omnibar (ADR-009)

Agora você implementa a **Fase 7: AgentCard + Toast + Polish**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 7" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Tokens, spring params, 6 estados obrigatórios
3. `docs/cockpit-v2-redesign/APP_FLOW.md` — Seções AF-01 (Agents Grid), AF-11 (Toast Notifications)

## Componentes Existentes a Melhorar/Criar

### AgentCard (já existe, precisa melhorar)
- Arquivo: `apps/zylos-cockpit/src/components/agent_card.rs`
- Usado por: `lazy_roster.rs`, `agents_panel.rs`, `benchmarks.rs`
- Tipo: `AgentInfo { id, name, status: AgentStatus, tasks, last_active, role, division }`
- **Problema atual:** card básico sem micro-interações, sem sparkline, sem hover effects
- **O que adicionar:**
  - Hover: `shadow-lg` + `translateY(-2px)` + spring animation
  - Click: ripple effect + navigate to agent detail
  - Sparkline: mini chart de atividade (últimas 24h) usando SVG inline
  - 6 estados: Loading (skeleton), Empty (placeholder), Error (retry button), Success (normal), Validation (warning badge), Disabled (grayed out)

### Toast System (NÃO existe, criar do zero)
- **Criar:** `apps/zylos-cockpit/src/components/toast_container.rs`
- ToastContainer: fixed bottom-right, stack vertical, max 3 visíveis
- 4 variantes: success (green), error (red), warning (amber), info (blue)
- Cada toast: ícone + mensagem + botão close (×)
- Auto-dismiss: 5s para success/info, manual para error/warning
- Pause on hover: timer pausa quando mouse está sobre o toast
- Spring animation: entrance (translateX 100%→0), exit (opacity 1→0)
- Integração SSE: `EventBus` events → toast dispatch

### Empty States (melhorar em vários painéis)
- Muitos painéis mostram apenas `<p>"Nenhum dado"</p>`
- **Adicionar:** ilustração SVG inline + CTA button
- Exemplo: "Nenhum agente registrado" → SVG de robot + botão "Criar Primeiro Agente"

### Error Boundaries (adicionar em routes)
- Atualmente crashes mostram tela branca
- **Adicionar:** `<ErrorBoundary>` wrapper em cada route
- Mostra: "Algo deu errado" + error message + botão "Tentar novamente"

## Fase 7 — Tarefas Específicas

### 7.1 Melhorar AgentCard com micro-interações
- Adicionar `on_hover` handler que anima `transform: translateY(-2px)` + `box-shadow: var(--shadow-lg)`
- Spring entrance quando card aparece (scale 0.95→1.0)
- Click handler: ripple effect + `leptos_router::navigate()` para `/agents/:id`
- Sparkline SVG: `<svg width="60" height="20">` com `<polyline>` baseado em dados de atividade
- 6 estados implementados via classes CSS (`.agent-card.loading`, `.agent-card.empty`, etc)

### 7.2 Criar ToastContainer component
- Novo arquivo: `apps/zylos-cockpit/src/components/toast_container.rs`
- Signal global: `RwSignal<Vec<Toast>>` onde `Toast { id, variant, message, created_at }`
- Fixed position: `bottom: var(--space-4); right: var(--space-4)`
- Stack vertical com gap `var(--space-2)`, max 3 items (remove oldest se >3)
- Spring entrance: `translateX(100%)→0` com `SpringConfig::GESTURE`
- Spring exit: `opacity 1→0` com `SpringConfig::UI`

### 7.3 Integrar SSE events → toast dispatch
- Em `app.rs`, subscribe ao `EventBus` SSE
- Quando receber evento `agent_started`, dispatch `Toast::success("Agent X started")`
- Quando receber `approval_resolved`, dispatch `Toast::info("Approval Y resolved")`
- Quando receber `error`, dispatch `Toast::error("Connection lost")`

### 7.4 Implementar 4 variantes de toast
- `ToastVariant::Success`: border-left green, ícone ✓, auto-dismiss 5s
- `ToastVariant::Error`: border-left red, ícone ✕, manual dismiss
- `ToastVariant::Warning`: border-left amber, ícone ⚠, manual dismiss
- `ToastVariant::Info`: border-left blue, ícone ℹ, auto-dismiss 5s

### 7.5 Auto-dismiss com timer
- Para success/info: `set_timeout_with_handle` de 5000ms
- Quando timeout dispara, remove toast do Vec com spring exit
- Para error/warning: sem timeout, usuário clica × para fechar

### 7.6 Pause on hover
- `on:mouseenter` pausa o timer (clear timeout)
- `on:mouseleave` reinicia timer com tempo restante
- Indicador visual sutil: border-left fica mais claro quando paused

### 7.7 Garantir 6 estados em componentes críticos
- AgentCard: loading (skeleton), empty (placeholder), error (retry), success (normal), validation (warning badge), disabled (grayed)
- MetricCard: mesmo padrão
- FilterBar: mesmo padrão
- Usar classes CSS: `.component.loading`, `.component.empty`, etc

### 7.8 Empty states com ilustrações SVG
- Criar componente `EmptyState { icon: &'static str, title: String, description: String, cta: Option<String> }`
- SVG inline simples (robot, document, chart) com `fill: var(--color-text-tertiary)`
- CTA button: `cta-primary` que navega ou executa ação

### 7.9 Error boundaries em routes
- Criar `<ErrorBoundary>` component que captura panics via `std::panic::set_hook`
- Fallback UI: card vermelho com "Algo deu errado" + error message + botão retry
- Aplicar em cada route: `<ErrorBoundary><RouteComponent /></ErrorBoundary>`

## Arquivos a modificar/criar
- `apps/zylos-cockpit/src/components/agent_card.rs` — melhorar com hover/click/sparkline/6 estados
- `apps/zylos-cockpit/src/components/toast_container.rs` — **novo**: ToastContainer + Toast types
- `apps/zylos-cockpit/src/components/empty_state.rs` — **novo**: EmptyState component
- `apps/zylos-cockpit/src/components/error_boundary.rs` — **novo**: ErrorBoundary component
- `apps/zylos-cockpit/src/components/mod.rs` — exportar novos componentes
- `apps/zylos-cockpit/src/app.rs` — integrar ToastContainer no layout shell + SSE event dispatch
- `apps/zylos-cockpit/styles/base.css` — classes CSS para toast, empty-state, error-boundary, agent-card hover
- `apps/zylos-cockpit/src/components/agents_panel.rs` — usar 6 estados em AgentCard
- `apps/zylos-cockpit/src/components/lazy_roster.rs` — usar 6 estados em AgentCard

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 7 — AgentCard micro-interações + Toast system + Polish`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- **Zero mocks** — usar API real; se lista de agentes vazia, mostrar EmptyState honesto
- **Bundle budget** — adicionar <15KB ao WASM gzipped (SVG inline é leve, toast system é simples)

## Verificação obrigatória (gate)
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
2. `cargo test -p zylos-cockpit --lib` — testes passam
3. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — limpo (ou apenas warnings aceitáveis)
4. `cargo fmt --all --check` — formatado
5. Grep para stubs: zero `todo!()` ou `unimplemented!()` nos arquivos modificados
6. Visual inspection (via browser): AgentCard hover animation suave, Toast stack funciona

## Out of Scope (NÃO fazer nesta fase)
- Drag-and-drop reordenação de AgentCards
- Context menus (right-click)
- Custom toast actions além de navigate/close
- Sound notifications
- Fase 8 (Accessibility + Performance) — será próxima fase

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação (test result lines)
- Bundle size delta vs commit anterior
- Lista de arquivos criados/modificados
- Nota sobre performance (toast stress test: 10 toasts rápidos)

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 7 e os arquivos existentes `agent_card.rs` e `app.rs` para entender a integração atual.**