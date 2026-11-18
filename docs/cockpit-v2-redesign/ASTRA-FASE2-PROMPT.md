# Zylos Cockpit v2.0 — Fase 2: Layout Shell + Translucency

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `1a83814` (Fase 1 complete — design system unificado)

## Context
Fase 1 do redesign do Cockpit v2.0 está completa e commitada no master. O design system está unificado em `styles/design-tokens.css` (SSoT, ADR-008) e `styles/base.css` (reset + utilitários). Agora você implementa a **Fase 2: Layout Shell + Translucency**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 2" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Seções §7 (Translucent Materials), §10.3 (TranslucentHeader), §10.4 (CollapsibleSidebar)
3. `docs/cockpit-v2-redesign/APP_FLOW.md` — Seção "Layout Shell" com 6 estados obrigatórios
4. `docs/cockpit-v2-redesign/ARCHITECTURE.md` — Diagrama C4 com layout shell

## Fase 2 — O que implementar

### 2.1 CollapsibleSidebar
- Sidebar com grupos colapsáveis: **Core** (Agentes, Tarefas, Memória, Metas), **Orquestração** (DAG Swarm, Sentinel, SOAR), **Comunicação** (Canais, Phone Relay, Chat, Threads), **Configuração** (Personalidades, Temas, Settings)
- Botão de collapse (ícone hamburger → seta) no topo da sidebar
- Estado colapsado/expandido persiste em `localStorage` (`zylos-sidebar-collapsed`)
- Quando colapsado: sidebar mostra apenas ícones (56px), tooltip no hover com nome do grupo
- Animação de collapse via spring (ADR-007: damping 1.0, response 0.35s)
- Grupos também colapsáveis individualmente (estado persiste em localStorage)

### 2.2 TranslucentHeader
- Header com `backdrop-filter: var(--blur-header)` (= `blur(20px) saturate(180%)`)
- Background: `var(--color-bg-translucent)` (= `rgba(15, 23, 42, 0.7)`)
- Sticky no topo (`position: sticky; top: 0; z-index: var(--z-header)`)
- Mostra: título "Zylos Cockpit", status do gateway (healthy/degraded/disconnected), versão, toggle dark/light theme, botão Login
- Fallback opaco quando `prefers-reduced-transparency: reduce`
- Feature detection JS: se `backdrop-filter` não suportado, adiciona classe `.no-backdrop` → fundo opaco

### 2.3 Estados obrigatórios do Shell (APP_FLOW.md)
| Estado | Comportamento |
|--------|--------------|
| Loading | Skeleton do sidebar + header opaco (translucency aguarda CSS load) |
| Empty | N/A (shell sempre tem conteúdo) |
| Error | SSE disconnected → toast "Conexão perdida" + retry button |
| Success | Shell funcional, SSE ativo, todas as navegações operacionais |
| Validation | N/A para shell |
| Disabled | `.disabled-overlay` quando auth pendente em rotas protegidas |

### 2.4 Command Palette stub (preparar para Fase 6)
- Atalho `Cmd+K` / `Ctrl+K` registrado (handler vazio por enquanto)
- Overlay preparado mas não renderizado ainda

## Arquivos a modificar
- `apps/zylos-cockpit/src/app.rs` — Layout shell principal
- `apps/zylos-cockpit/src/components/` — Criar `collapsible_sidebar.rs` e `translucent_header.rs`
- `apps/zylos-cockpit/styles/base.css` — Estender com classes do shell
- `apps/zylos-cockpit/styles/design-tokens.css` — Adicionar tokens de sidebar (collapsed-width, group colors) se necessário
- `apps/zylos-cockpit/index.html` — Ajustar shell HTML se necessário

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final da fase: `feat(cockpit-v2): Fase 2 — layout shell translúcido + sidebar colapsável (ADR-008, ADR-010)`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`

## Verificação obrigatória (gate)
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
2. `trunk build --dist E:/zylos-target/dist-v2` — build passa
3. `cargo test -p zylos-cockpit --lib` — testes passam
4. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — limpo
5. `cargo fmt --all --check` — formatado
6. Visual: servir com `npx http-server E:/zylos-target/dist-v2 -p 8088` e verificar sidebar/header

## Out of Scope (NÃO fazer nesta fase)
- Spring animation system (Fase 3)
- Inline approval cards (Fase 4)
- VNC stream (Fase 5)
- Command palette funcional (Fase 6)
- AgentCard redesign (Fase 7)
- Performance optimization (Fase 8)

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação (test result lines)
- Lista de arquivos modificados
- Qualquer blocker encontrado

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 2.**