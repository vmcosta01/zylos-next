# Zylos Cockpit v2.0 — Fase 3: Spring Animation System

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `4671e86` (Fase 2 complete — layout shell translúcido + sidebar colapsável)

## Context
Fases 1 e 2 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável (ADR-008, ADR-010)

Agora você implementa a **Fase 3: Spring Animation System**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 3" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Seção §8 (Spring Animation System), §8.3 (Reduced Motion)
3. `docs/cockpit-v2-redesign/DECISIONS.md` — ADR-007 (Spring system choice — PENDENTE, resolver agora)
4. Skill `apple-design` (já carregada no Hermes) — princípios 3-6 sobre animações interruptíveis

## Fase 3 — O que implementar

### 3.1 Resolver ADR-007: Spring System Choice
- **Opção A:** `leptos-spring` crate — verificar compatibilidade com Leptos 0.7
- **Opção B:** Custom WASM solver — implementar se leptos-spring incompatível
- **Opção C:** CSS transitions apenas — NÃO aceitável, sem interruptibilidade nem velocity handoff
- **Critérios:** compatibilidade Leptos 0.7, interruptibilidade, velocity handoff, bundle <5KB

### 3.2 Implementar Spring Animator
- Componente/sistema reutilizável que aceita: `damping`, `response`, `initial_value`, `target_value`, `initial_velocity`
- Spring parameters do DESIGN_SYSTEM.md:
  - UI springs: damping=1.0, response=0.35s (crítico, sem overshoot)
  - Gesture springs: damping=0.8, response=0.4s (leve bounce)
- Cada frame atualiza via `requestAnimationFrame`
- Interrupção: quando target muda mid-animation, spring continua da posição e velocidade atuais
- Velocity handoff: ao soltar um gesto, a velocidade do gesto vira initial_velocity do spring

### 3.3 Aplicar springs em componentes existentes
- **Sidebar collapse** (Fase 2): transição width usa spring em vez de CSS transition
- **Modal entrances** (auth modal, command palette futuro): scale+opacity via spring
- **Toast stacking** (preparar para Fase 7): slide-in from right via spring
- **Tab transitions**: content fade via spring

### 3.4 Respeitar prefers-reduced-motion
- `@media (prefers-reduced-motion: reduce)`: springs são substituídos por crossfade instantâneo (sem movimento)
- No JS: detectar via `window.matchMedia` e pular spring animator

### 3.5 Testes
- Teste unitário: spring converge para target em <2s com damping=1.0
- Teste unitário: spring interruptível (mudar target mid-flight não causa jump)
- Teste unitário: velocity handoff (initial_velocity ≠ 0 → overshoot controlado)
- Teste wasm-bindgen-test: reduced-motion desabilita spring

## Arquivos a criar/modificar
- `apps/zylos-cockpit/src/spring.rs` — **novo**: spring solver module
- `apps/zylos-cockpit/src/components/translucent_header.rs` — aplicar spring no collapse
- `apps/zylos-cockpit/src/components/collapsible_sidebar.rs` — aplicar spring na animação de width
- `apps/zylos-cockpit/src/app.rs` — integrar spring nas transições de tab
- `apps/zylos-cockpit/styles/base.css` — remover CSS transitions que serão substituídas por springs
- `docs/cockpit-v2-redesign/DECISIONS.md` — atualizar ADR-007 com decisão final

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 3 — spring animation system (ADR-007)`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- Bundle overhead do spring system deve ser <5KB gzipped

## Verificação obrigatória (gate)
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
2. `trunk build --dist E:/zylos-target/dist-v2` — build passa
3. `cargo test -p zylos-cockpit --lib` — testes passam (incluindo testes de spring)
4. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — limpo
5. `cargo fmt --all --check` — formatado

## Out of Scope (NÃO fazer nesta fase)
- Inline approval cards (Fase 4)
- VNC stream (Fase 5)
- Command palette funcional (Fase 6)
- AgentCard redesign (Fase 7)
- Performance optimization final (Fase 8)

## Entrega
Ao completar, escrever relatório curto com:
- Decisão ADR-007 (qual opção foi escolhida e por quê)
- Commit hash
- Resultado de cada verificação (test result lines)
- Bundle size delta vs commit anterior
- Lista de arquivos criados/modificados

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 3 e `DECISIONS.md` ADR-007.**