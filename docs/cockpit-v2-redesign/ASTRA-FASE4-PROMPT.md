# Zylos Cockpit v2.0 — Fase 4: Inline Approval Cards (ADR-001)

**Role:** Senior Frontend Engineer (Leptos 0.7 / Rust WASM)
**Model:** GPT-6 Astra (via ghostcli)
**Starting commit:** `81fc8df` (Fase 1-3 complete)

## Context
Fases 1-3 do redesign do Cockpit v2.0 estão completas e commitadas no master:
- Fase 1 (`47e8608`): design system unificado (ADR-008)
- Fase 2 (`4671e86`): layout shell translúcido + sidebar colapsável (ADR-008, ADR-010)
- Fase 3 (`09d87fa`): spring animation system (ADR-007)

Agora você implementa a **Fase 4: Inline Approval Cards (ADR-001)**.

**Leia ANTES de começar:**
1. `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` — Seção "Fase 4" com tarefas atômicas
2. `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` — Seção §10.2 (InlineApprovalCard), §9 (6 estados obrigatórios)
3. `docs/cockpit-v2-redesign/APP_FLOW.md` — Seção AF-02 (Tasks + Inline Approvals)
4. `docs/cockpit-v2-redesign/DECISIONS.md` — ADR-001 (Approval Gates Inline no Chat vs Modal Separado)

## Problema Atual
O arquivo `apps/zylos-cockpit/src/components/hitl_approval_panel.rs` atualmente mostra **texto placeholder** (linhas ~56-58) dizendo "Cards interativos requerem integração completa". A ADR-001 decidiu que aprovações devem ser **inline no fluxo de chat**, não em modais separados — copiado de OpenDuet/OpenDots.

## Fase 4 — O que implementar

### 4.1 Substituir placeholder por InlineApprovalCard real
- Criar componente `InlineApprovalCard` em `hitl_approval_panel.rs` (ou arquivo novo)
- Card mostra: título da ação, descrição, contexto (quem pediu, quando), detalhes (JSON expandível)
- 3 botões: **Aprovar** (verde), **Rejeitar** (vermelho), **Escalar** (amarelo)
- Cada botão tem estado `:active` com spring scale (0.96, damping 1.0, response 0.35s)

### 4.2 Fluxo de aprovação completo
1. Card aparece com estado `pending` (borda azul pulsante)
2. Usuário clica Approve/Reject/Escalate
3. **Optimistic UI**: card imediatamente muda para estado `resolving` (spinner + texto "Resolvendo...")
4. POST `/api/v1/approval/resolve` com `{approval_id, decision, reason?}`
5. Se sucesso (200): card colapsa para **receipt** (compacto, verde/vermelho/amarelo, com timestamp)
6. Se falha (4xx/5xx): card reverte para `pending` + toast de erro com botão "Tentar novamente"

### 4.3 Receipt persistence
- Ao aprovar/rejeitar com sucesso, salvar receipt em `localStorage` (`zylos_approval_receipts`)
- Formato: `{approval_id, decision, timestamp, summary}`
- Na próxima visita, receipts persistidos aparecem em estado `resolved` (colapsados)
- Limpar receipts com mais de 7 dias

### 4.4 Integração com spring system (Fase 3)
- Card entrada: spring entrance (scale 0.95→1, opacity 0→1, damping 0.8, response 0.4s)
- Card colapso para receipt: spring (height atual→height receipt, damping 1.0, response 0.35s)
- Button press: spring scale(1→0.96→1, damping 1.0, response 0.35s)
- Respeitar `prefers-reduced-motion`: sem springs, crossfade instantâneo

### 4.5 6 estados obrigatórios (DESIGN_SYSTEM.md §9)
| Estado | Comportamento |
|--------|--------------|
| Loading | Skeleton do card (3 linhas shimmer) |
| Empty | "Nenhuma aprovação pendente" com ícone |
| Error | Banner vermelho + retry button |
| Success | Receipt colapsado com checkmark |
| Validation | Input de reason (para Reject/Escalate) com validação inline |
| Disabled | Overlay cinza quando auth pendente |

### 4.6 Integração com SSE events
- SSE event `approval_requested` → novo card aparece no topo com spring entrance
- SSE event `approval_resolved` → card colapsa para receipt se outro usuário resolveu
- Se SSE desconectado → polling REST `/api/v1/approvals` a cada 30s

### 4.7 CSS classes necessárias (adicionar a `styles/base.css`)
```css
.inline-approval-card { ... }
.inline-approval-card.pending { border-left: 4px solid var(--color-status-info); }
.inline-approval-card.resolving { opacity: 0.7; }
.inline-approval-card.resolved { border-left: 4px solid var(--color-status-success); }
.inline-approval-card.rejected { border-left: 4px solid var(--color-status-error); }
.inline-approval-card.escalated { border-left: 4px solid var(--color-status-warning); }
.approval-actions { display: flex; gap: var(--space-2); }
.approval-receipt { font-size: var(--text-sm); padding: var(--space-2); }
```

## Arquivos a modificar
- `apps/zylos-cockpit/src/components/hitl_approval_panel.rs` — reescrever com InlineApprovalCard real
- `apps/zylos-cockpit/styles/base.css` — adicionar classes de approval card
- `apps/zylos-cockpit/src/app.rs` — garantir que SSE events de approval são roteados corretamente
- `docs/cockpit-v2-redesign/DECISIONS.md` — marcar ADR-001 como implementado

## API do Gateway (existente)
- `POST /api/v1/approval/resolve` — body: `{approval_id: string, decision: "approve"|"reject"|"escalate", reason?: string}`
- `GET /api/v1/approvals` — retorna lista de aprovações pendentes
- SSE event `approval_requested` — payload: `{id, title, description, context, requested_at}`
- SSE event `approval_resolved` — payload: `{id, decision, resolved_by, resolved_at}`

## Regras
- **CARGO_TARGET_DIR=E:/zylos-target/cockpit-v2** sempre
- **Shell bash** no Windows (POSIX syntax)
- **Foreground timeout ~420s** → builds longos em background com log + polling
- **Commit atômico** ao final: `feat(cockpit-v2): Fase 4 — inline approval cards reais (ADR-001)`
- **Push** após commit: `GIT_TERMINAL_PROMPT=0 git -c credential.helper= -c credential.helper='!gh auth git-credential' push origin master`
- **Zero mocks** — usar API real; se gateway não tem approvals pendentes, mostrar estado Empty honesto
- **Bundle budget** — adicionar <3KB ao WASM gzipped

## Verificação obrigatória (gate)
1. `cargo check -p zylos-cockpit --target wasm32-unknown-unknown` — compila
2. `trunk build --dist E:/zylos-target/dist-v2` — build passa
3. `cargo test -p zylos-cockpit --lib` — testes passam
4. `cargo clippy -p zylos-cockpit --all-targets -- -D warnings` — limpo
5. `cargo fmt --all --check` — formatado
6. Grep para stubs: `grep -rniE 'todo!\(\)|unimplemented!\(\)|placeholder' apps/zylos-cockpit/src/components/hitl_approval_panel.rs` — deve retornar 0 matches

## Out of Scope (NÃO fazer nesta fase)
- VNC stream (Fase 5)
- Command palette funcional (Fase 6)
- AgentCard redesign (Fase 7)
- Performance optimization final (Fase 8)
- Backend changes (gateway approval API já existe)

## Entrega
Ao completar, escrever relatório curto com:
- Commit hash
- Resultado de cada verificação (test result lines)
- Bundle size delta vs commit anterior
- Lista de arquivos criados/modificados
- Screenshot ou descrição visual do card em cada estado

**Comece lendo `IMPLEMENTATION_PLAN.md` seção Fase 4 e o arquivo atual `hitl_approval_panel.rs`.**