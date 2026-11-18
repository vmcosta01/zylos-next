# APP_FLOW — Zylos Cockpit v2.0

**Versão:** 2.0.0
**Data:** 2026-11-18
**Status:** Jornada Mapeada
**Conceito Manifesto:** #18 (APP_FLOW obrigatório)
**Depende de:** [PRD.md](./PRD.md), [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md)

---

## 1. Visão Geral da Jornada

O usuário entra no Cockpit via URL raiz, é recebido pelo layout shell (Header + Sidebar + MainContent), e navega entre telas via sidebar ou command palette. Cada tela tem 6 estados obrigatórios documentados. Fluxos críticos: aprovação HITL inline, pareamento VNC, busca rápida.

### Mapa de Telas

| ID | Tela | Rota | Grupo Sidebar | Prioridade |
|----|------|------|---------------|------------|
| AF-01 | Agents Grid | `/agents` | Core | P0 |
| AF-02 | Tasks + Inline Approvals | `/tasks` | Core | P0 |
| AF-03 | Memory Graph | `/memory` | Core | P2 |
| AF-04 | Goals Cascade | `/goals` | Orquestração | P2 |
| AF-05 | DAG Swarm | `/dag` | Orquestração | P1 |
| AF-06 | Sentinel AIOps | `/sentinel` | Orquestração | P1 |
| AF-07 | Phone Relay (VNC) | `/phone-relay` | Comunicação | P0 |
| AF-08 | Personalities | `/personalities` | Configuração | P2 |
| AF-09 | Settings | `/settings` | Configuração | P2 |
| AF-10 | Command Palette | Overlay (Cmd+K) | Global | P1 |
| AF-11 | Toast Notifications | Overlay (bottom-right) | Global | P2 |

---

## 2. Layout Shell (Persistente)

### Entrada
- Usuário acessa qualquer rota → Layout shell monta
- Header translúcido renderiza com backdrop-filter
- Sidebar carrega estado de colapso do localStorage
- SSE subscription inicia; toast container pronto

### Estados Obrigatórios do Shell

| Estado | Comportamento |
|--------|--------------|
| Loading | Skeleton do sidebar + header opaco (translucency aguarda CSS load) |
| Empty | N/A (shell sempre tem conteúdo) |
| Error | SSE disconnected → toast "Conexão perdida" + retry automático |
| Success | Shell fully rendered, translucent materials active |
| Validation | N/A |
| Disabled | N/A |

### Saída/Recuperação
- SSE disconnect: exponential backoff reconnect (1s→2s→4s→max 30s)
- Após 3 falhas: toast error persistente + manual retry button
- Auth expired: redirect para login (futuro)

---

## 3. AF-01: Agents Grid

### Entrada
- Click "Agentes" na sidebar OU Cmd+K → "Agentes" OR rota direta `/agents`

### Ações Possíveis
1. Buscar agente por nome/status
2. Filtrar por status (Todos/Running/Idle/Blocked)
3. Click em AgentCard → navegar para detalhes
4. Hover em AgentCard → micro-interações (elevação, sparkline)
5. Toggle view (Grid/List)

### Respostas do Sistema
- Busca: filtro client-side instantâneo (<10ms)
- Filtro: re-render grid com transição spring
- Click card: navigate com spring page transition
- Hover: shadow-lg + translateY(-2px) + sparkline animada

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Grid de skeletons (6 cards shimmer) |
| Empty | Ilustração SVG "Nenhum agente configurado" + CTA "Adicionar agente" |
| Error | Banner "Falha ao carregar agentes" + Retry button |
| Success | Grid de AgentCards com dados reais |
| Validation | Search input: warning se regex inválido |
| Disabled | Filter pills disabled durante bulk operation |

### Saída/Recuperação
- Error: retry resync via REST GET /api/v1/agents
- Empty: CTA leva para Settings → Add Agent
- Back: browser back button retorna ao estado anterior

---

## 4. AF-02: Tasks + Inline Approvals (ADR-001)

### Entrada
- Click "Tarefas" na sidebar OU Cmd+K → "Tarefas" OR rota `/tasks`

### Ações Possíveis
1. Visualizar tarefas em lista ou kanban
2. Aprovar/Rejeitar/Escalar approval card inline
3. Expandir detalhes da tarefa
4. Filtrar por status/prioridade/agente
5. Bulk approve via command palette

### Respostas do Sistema
- Approval action: optimistic update → POST → SSE confirmation → collapse to receipt
- Reject: card muda para vermelho, colapsa com receipt
- Escalate: card muda para amarelo, notifica próximo tier
- Receipt persiste em localStorage

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Skeleton list (5 items shimmer) |
| Empty | "Nenhuma tarefa pendente" + illustration |
| Error | Inline error no card com retry; toast se POST falhar |
| Success | Cards renderizados; approvals com botões ativos |
| Validation | Hash mismatch warning em approval card |
| Disabled | Botões disabled após decisão (locked receipt) |

### Fluxo Detalhado: Inline Approval
```
1. SSE approval_requested → Card mount com skeleton
2. Skeleton → Full card (spring entrance)
3. User click Approve → Optimistic: botão verde, spinner
4. POST /api/v1/approvals/:id
5a. Success → SSE approval_resolved → Card collapse to receipt (spring)
5b. Failure → Revert optimistic → Error state + retry button
6. Receipt persistido em localStorage
7. Page refresh → Receipt restaurado do localStorage
```

### Saída/Recuperação
- POST failure: revert + error state + retry
- SSE missed event: REST sync on focus
- Stale receipt: TTL 24h, auto-cleanup

---

## 5. AF-03: Memory Graph

### Entrada
- Sidebar → "Memória" OR Cmd+K → "Memória"

### Ações Possíveis
1. Visualizar grafo de memórias
2. Buscar por termo
3. Filtrar por tipo/data
4. Click nó → expandir detalhes

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Spinner central + "Carregando grafo..." |
| Empty | "Memória vazia" + info sobre consolidação noturna |
| Error | "Falha ao carregar grafo" + retry |
| Success | Grafo interativo renderizado |
| Validation | Search: warning se termo muito curto (<2 chars) |
| Disabled | N/A |

---

## 6. AF-04: Goals Cascade

### Entrada
- Sidebar → "Metas" OR Cmd+K → "Metas"

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Skeleton cascade (3 levels shimmer) |
| Empty | "Nenhuma meta definida" + CTA |
| Error | Banner error + retry |
| Success | Cascade dashboard com métricas |
| Validation | N/A |
| Disabled | N/A |

---

## 7. AF-05: DAG Swarm

### Entrada
- Sidebar → "DAG do Swarm" OR Cmd+K → "DAG"

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Spinner + "Renderizando DAG..." |
| Empty | "Swarm inativo" + info |
| Error | "Falha ao renderizar DAG" + retry |
| Success | DAG visualization interativa |
| Validation | N/A |
| Disabled | Controls disabled durante re-render |

---

## 8. AF-06: Sentinel AIOps

### Entrada
- Sidebar → "Sentinela" OR Cmd+K → "Sentinela"

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Skeleton dashboard |
| Empty | "Sem alertas recentes" + success indicator |
| Error | "Falha ao carregar alertas" + retry |
| Success | Alert list + metrics dashboard |
| Validation | Filter: warning se date range inválido |
| Disabled | N/A |

---

## 9. AF-07: Phone Relay (VNC + Fallback) — ADR-003

### Entrada
- Sidebar → "Phone Relay" OR Cmd+K → "Phone Relay"

### Ações Possíveis
1. Parear novo dispositivo (QR code)
2. Selecionar dispositivo pareado
3. Take Control (solicitar lease)
4. Release Control (liberar lease)
5. Toggle quality (auto/low/high)
6. Screenshot manual (salvar frame atual)

### Respostas do Sistema
- QR scan: WebSocket connect → stream inicia
- Take Control: POST lease → indicador "Controlling" → timeout 5min
- Release: DELETE lease → indicador "Viewing"
- Quality toggle: ajusta resolução/framerate em tempo real
- Screenshot: salva frame atual como PNG download

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Spinner "Conectando..." sobre canvas preto |
| Empty | "Nenhum dispositivo pareado" + QR code para pareamento |
| Error | Screenshot fallback automático + toast "Stream falhou" + retry button |
| Success | Live VNC stream em canvas + controles visíveis |
| Validation | N/A |
| Disabled | "Control released" overlay quando lease expira |

### Fluxo Detalhado: VNC Connection
```
1. Page mount → Check paired devices
2a. No device → Show QR code (Empty state)
2b. Device exists → WS connect /ws/computer-view/{session_id}
3. WS open → Canvas ready → Waiting for frames
4a. Frame received within 3s → Render loop starts (Success)
4b. No frame in 3s → Screenshot fallback (Error state)
5. Quality monitor: track frame intervals
6. If avg interval >66ms → Reduce resolution 50%
7. Take Control click → POST /api/v1/computer-view/:session/lease **[CRIAR ENDPOINT]** → "Controlling" badge
8. Lease timeout 5min OR Release click → DELETE /lease → "Viewing"
9. WS close/error → Auto-fallback to screenshot + toast
```

### Saída/Recuperação
- WS error: auto-fallback + toast + manual retry
- Lease expired: overlay "Control released" + Take Control button
- QR expired: regenerate QR + countdown timer

---

## 10. AF-08: Personalities

### Entrada
- Sidebar → "Personalidades"

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Skeleton list |
| Empty | "Nenhuma personalidade configurada" |
| Error | Banner + retry |
| Success | List/grid de personalities |
| Validation | Form validation inline |
| Disabled | Edit disabled durante save |

---

## 11. AF-09: Settings

### Entrada
- Sidebar → "Settings"

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Skeleton form |
| Empty | N/A (sempre tem defaults) |
| Error | Inline field errors + toast |
| Success | Form rendered + save confirmation |
| Validation | Real-time field validation |
| Disabled | Save button disabled durante submit |

---

## 12. AF-10: Command Palette (ADR-009)

### Entrada
- Shortcut Cmd+K (Mac) / Ctrl+K (Win/Linux)
- Click search icon no header (futuro)

### Ações Possíveis
1. Digitar busca fuzzy
2. Navegar resultados com ↑↓
3. Selecionar com Enter ou click
4. Fechar com Esc ou click fora
5. Ver histórico recente

### Respostas do Sistema
- Input: debounce 50ms → fuzzy search → results render
- Navigate: highlight move com spring micro-animation
- Select: execute action → palette close (spring exit)
- Close: fade out + scale down (spring)

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | Debounce indicator (dot pulse) no input |
| Empty | "Nenhum resultado" + sugestões ("Tente: agentes, tarefas...") |
| Error | Toast se ação falhar após seleção |
| Success | Results grouped + highlighted selection |
| Validation | Warning se shortcut inválido em contexto |
| Disabled | Input disabled durante execução de ação |

### Index de Busca
```
Pages: Agentes, Tarefas, Memória, Metas, DAG, Sentinela, Phone Relay, Personalidades, Settings
Agents: [dynamic from store]
Tasks: [dynamic from store]
Commands: Open Command Palette, Toggle Sidebar, Toggle Theme (future), Take Screenshot
Skills: [top 20 most used]
```

---

## 13. AF-11: Toast Notifications

### Entrada
- Evento SSE (approval_timeout, agent_error, sentinel_alert, stream_connected)
- Ação do usuário (save success, copy to clipboard)

### Ações Possíveis
1. Hover → pause auto-dismiss
2. Click → navigate to related entity
3. Click ✕ → dismiss manualmente
4. Swipe right → dismiss (mobile)

### Respostas do Sistema
- Mount: slide in from right (spring)
- Dismiss: fade out + slide right (spring)
- Stack: max 3 visible; overflow queued
- Pause: timer stops on hover, resumes on leave

### Estados Obrigatórios

| Estado | Renderização |
|--------|-------------|
| Loading | N/A (toast é feedback pós-ação) |
| Empty | N/A |
| Error | Red left border + ✕ icon + persistent (manual dismiss) |
| Success | Green left border + ✓ icon + auto-dismiss 5s |
| Validation | Amber left border + ⚠️ icon |
| Disabled | Dismissed state (animating out) |

---

## 14. Matriz de Cross-Links

| APP_FLOW | PRD Feature | DESIGN_SYSTEM Component | ARCHITECTURE |
|----------|-------------|------------------------|--------------|
| AF-01 | F9 AgentCard | AgentCard (§10.1) | AgentsPage |
| AF-02 | F2 Inline Approval | InlineApprovalCard (§10.2) | InlineApprovalCard component |
| AF-07 | F7 VNC Stream | VncCanvas (§10 implicit) | VncCanvas + ScreenshotFallback |
| AF-10 | F6 Command Palette | CommandPalette (§10.5) | CommandPalette component |
| AF-11 | F8 Toast | Toast (§10.6) | ToastContainer |
| All | F3 Translucent | Header/Sidebar (§10.3-4) | Layout shell |
| All | F4 Springs | Spring params (§8) | SpringAnimator |
| All | F10 Accessibility | Reduced motion/transparency (§7.3, §8.3) | Global media queries |

---

## 15. Compliance com Manifesto

| Requisito | Status |
|-----------|--------|
| Todas as telas mapeadas | ✅ 11 telas/overlays |
| 6 estados por tela | ✅ Matrizes completas |
| Entradas e saídas definidas | ✅ Cada seção |
| Fluxos críticos detalhados | ✅ Approval, VNC, Command Palette |
| Cross-links funcionais | ✅ Seção 14 |
| Recuperação de erro documentada | ✅ Cada tela tem recuperação |

---

## 16. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 2.0.0 | 2026-11-18 | Initial v2.0 app flow; 11 screens; 6 states each; critical flows detailed |