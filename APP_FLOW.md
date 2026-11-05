# User Journey — Zylos-Next Cockpit

## Visão Geral
Este documento mapeia a jornada completa do usuário no cockpit web do Zylos-Next. Cada tela lista seus estados obrigatórios (Loading, Empty, Error, Success, Validation, Disabled) conforme DESIGN_SYSTEM.md.

## Persona Principal
**Victor Costa** — Arquiteto de Sistemas Multi-Agente que precisa orquestrar 50+ agentes simultaneamente com observabilidade em tempo real e verificação adversarial de resultados.

---

## Tela 1: Dashboard Overview

### Estados Obrigatórios
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Loading** | Skeleton cards (3 placeholders) | Pulse animation 1.5s |
| **Empty** | Ícone + "No agents configured" + CTA "Create First Agent" | Focus no CTA |
| **Success** | Grid de cards com métricas em tempo real | Auto-refresh 5s |
| **Error** | Banner vermelho + "Connection lost" + Retry button | Retry com exponential backoff |
| **Validation** | N/A (tela read-only) | - |
| **Disabled** | Overlay cinza + "System maintenance" | Tooltip com ETA |

### Ações do Usuário
1. **Ver métricas globais:** Total agents, active workflows, success rate, avg latency
2. **Click em card de agente:** Navega para Agent Detail
3. **Click em "Create Agent":** Abre modal de criação
4. **Filtrar por status:** Dropdown (All/Running/Idle/Error)
5. **Buscar agente:** Input com debounce 300ms

### Fluxo de Dados
```
WebSocket Connect → Subscribe /api/v1/dashboard/stream
          ↓
Event Stream (SSE) → Update React Signals
          ↓
Re-render Cards → Métricas em tempo real
```

---

## Tela 2: Agent Roster (Lazy-Loaded)

### Estados Obrigatórios
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Loading** | Skeleton list (10 rows) | Fade-in staggered |
| **Empty** | "No agents match filter" + Reset filters link | - |
| **Success** | Virtualized list (react-virtuoso) com badges | Infinite scroll |
| **Error** | Inline error + "Load failed" + Retry row | Retry individual |
| **Validation** | Filter combo com "Invalid date range" toast | Focus no filtro |
| **Disabled** | Rows grayed + "Batch operation in progress" | Spinner overlay |

### Ações do Usuário
1. **Scroll infinito:** Carrega 50 agentes por batch
2. **Click em agente:** Expande inline ou navega detail
3. **Batch select:** Checkbox + barra de ações flutuante
4. **Drag reorder:** Reordena prioridade de execução
5. **Right-click menu:** Park, Delete, Clone, Export

### Lazy Loading Strategy
```
Initial Load: 50 agentes (top by activity)
          ↓
On Scroll: Fetch next 50 via cursor pagination
          ↓
On Search: Reset + fetch filtered set
          ↓
On Filter Change: Debounce 300ms + refetch
```

---

## Tela 3: Agent Detail / Workflow View

### Estados Obrigatórios
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Loading** | Full-page skeleton com tabs | Tab content placeholder |
| **Empty** | "Agent has no runs yet" + "Trigger First Run" | CTA primário |
| **Success** | Tabs: Overview, DAG, Memory, Logs, Proof | Default tab: Overview |
| **Error** | Error boundary + stack trace collapsible | Copy to clipboard |
| **Validation** | Form fields com inline errors | Shake on submit fail |
| **Disabled** | Read-only mode + "Archived agent" badge | Tooltip: reason |

### Sub-Telas (Tabs)

#### Tab: Overview
- Status badge (Running/Idle/Error/Parked)
- Métricas: Uptime, Success Rate, Avg Latency, Cost
- Últimos 5 runs com status
- Quick actions: Trigger, Park, Edit Config

#### Tab: DAG Visualization
- Canvas interativo (react-flow)
- Nós coloridos por status (pending/running/success/error/skipped)
- Click em nó: Side panel com input/output/logs
- Zoom/Pan/Minimap
- Rollback button (se shadow-git disponível)

#### Tab: Memory Context
- Timeline vertical de eventos
- Filtro por scope (global/team/agent/task)
- Search full-text com highlight
- Export context graph (JSON/SVG)

#### Tab: Live Logs
- Terminal-like view com ANSI colors
- Auto-scroll (toggleable)
- Filtro por level (trace/debug/info/warn/error)
- Download log file

#### Tab: Verification Proof
- Ground status (Verified/Refuted/Pending/Uncertain)
- Panel adversarial: Juiz A vs Juiz B verdicts
- Guardrail metrics (<1ms check)
- Tree-of-Thoughts visualization (se aplicado)

---

## Tela 4: Create/Edit Agent Modal

### Estados Obrigatórios
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Loading** | Spinner no botão Save | Disable form |
| **Empty** | Form vazio com placeholders | Focus no nome |
| **Success** | Toast "Agent created" + redirect to detail | Close modal |
| **Error** | Banner topo modal + field errors | Focus primeiro erro |
| **Validation** | Inline errors abaixo de cada campo | Real-time validation |
| **Disabled** | Form readonly + "Editing archived agent" | Unlock button |

### Campos do Formulário
1. **Name** (required, unique, max 64 chars)
2. **Description** (optional, max 500 chars)
3. **System Prompt** (textarea, markdown support)
4. **Skills** (multi-select from registry)
5. **Memory Scope** (dropdown: global/team/agent/task)
6. **Sandbox Mode** (radio: WASM/Docker/Hybrid)
7. **Verification Level** (slider: none/guardrail/panel/full)
8. **Max Concurrent Runs** (number, min 1, max 100)

### Validação
```
Name: required + unique + pattern ^[a-z0-9-]+$
System Prompt: required + min 10 chars
Skills: at least 1 selected
Max Concurrent: integer 1-100
```

---

## Tela 5: Skills Registry Browser

### Estados Obrigatórios
| Estado | Visual | Comportamento |
|--------|--------|---------------|
| **Loading** | Grid skeleton (12 cards) | Staggered fade |
| **Empty** | "No skills found" + "Import Skill" CTA | - |
| **Success** | Card grid com search/filter/sort | Responsive columns |
| **Error** | "Registry unavailable" + Retry + Offline mode | Cache fallback |
| **Validation** | Import form com schema errors | Highlight invalid fields |
| **Disabled** | Cards dimmed + "Maintenance mode" | - |

### Ações
1. **Search:** Full-text + tag filter
2. **Filter:** Category, version, compatibility
3. **Sort:** Name, popularity, updated date
4. **Click card:** Expand details + Install button
5. **Import:** Upload SKILL.md + validate schema

---

## Jornada Completa do Usuário
```
Login → Dashboard Overview
          ↓
Click "Create Agent" → Modal preenchido → Save
          ↓
Redirect to Agent Detail → Tab Overview
          ↓
Tab DAG → Trigger workflow → Watch nodes execute
          ↓
Tab Verification → Review proof → Approve/Reject
          ↓
Tab Memory → Inspect context → Export if needed
          ↓
Dashboard → Batch select agents → Park for maintenance
          ↓
Skills Registry → Browse → Install new skill → Assign to agent
```

## Recuperação de Erros
- **WebSocket disconnect:** Auto-reconnect com exponential backoff (1s, 2s, 4s, max 30s)
- **API timeout:** Retry 3x com jitter, depois show error banner
- **Stale data:** ETag/Last-Modified validation, refetch se mismatch
- **Auth expired:** Redirect to login, preserve navigation state

## Acessibilidade
- **Keyboard:** Tab order lógico, Enter ativa, Esc fecha modals
- **Screen Reader:** aria-live regions para updates em tempo real
- **Color Blind:** Icons + text labels (não só cor)
- **Reduced Motion:** Respeita prefers-reduced-motion

## Referências
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) para tokens e componentes
- [PRD.md](./PRD.md) para features e requisitos
- [ARCHITECTURE.md](./ARCHITECTURE.md) para fluxo de dados backend