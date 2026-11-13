# Plano de Implementação — Fase 10 (Evolução do Cockpit v2)

**Versão:** 1.0.0
**Data:** 2026-11-12
**Base:** Auditoria adversarial fase9-audit-adversarial.md + Opções A/B/C consolidadas
**Pré-requisito:** Fases 1–4 do IMPLEMENTATION_PLAN_DETAILED.md completas (8/8 tasks)

---

## Visão Geral

Este plano transforma o cockpit Zylos de **protótipo funcional com dados mock** em **sistema de produção com persistência real, qualidade verificada e features avançadas**. Integra três estratégias em sequência lógica:

- **Opção A (Fases 10.1–10.5):** Persistência real, integração Hermes, interatividade
- **Opção B (Fase 10.6):** Qualidade, testes E2E reais, build otimizado, docs de usuário
- **Opção C (Fases 10.7–10.8):** Multi-tenant, CRDT, rich-text editor, analytics

**Duração total estimada:** 10–12 semanas
**Stack:** Rust 1.82+, Axum 0.7, Leptos 0.7, SQLite WAL, wasm-bindgen, Yjs/CRDT (Fase 10.7)

---

## Fase 10.1: Persistência Real para Channels

**Objetivo:** Migrar `channels_api` de DashMap em memória para SQLite persistente, consistente com o stack Zylos.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.1.1 | Criar crate `zylos-channels` com schema SQL (channels, messages, memberships) | — | Médio |
| 10.1.2 | Implementar `ChannelsStore` com sqlx + migrations individuais | 10.1.1 | Médio |
| 10.1.3 | Refatorar `channels_api.rs` no gateway para usar `ChannelsStore` ao invés de DashMap | 10.1.2 | Baixo |
| 10.1.4 | Adicionar endpoints `POST /channels/:id/join`, `DELETE /channels/:id/leave` | 10.1.3 | Baixo |
| 10.1.5 | Testes unitários (CRUD channels, mensagens, paginação) | 10.1.2 | Médio |
| 10.1.6 | Atualizar `channel_panel.rs` para consumir novos endpoints (join/leave) | 10.1.4 | Baixo |

### Entregável Verificável
- Crate `zylos-channels` com ≥15 testes passando
- Gateway responde `/api/v1/channels` com dados persistentes em SQLite
- Mensagens sobrevivem a restart do gateway

### Critérios de Aceitação
- [ ] `cargo test -p zylos-channels` passa com ≥15 testes
- [ ] `cargo check -p zylos-gateway` passa com zero erros
- [ ] Dados persistem após `kill` e restart do gateway
- [ ] `channel_panel.rs` consome endpoints reais sem fallback para mock

### Out of Scope
- Notificações push em tempo real (SSE/WebSocket para channels)
- Upload de arquivos/imagens em mensagens
- Busca full-text em mensagens

### Gate de Verificação
```bash
cargo test -p zylos-channels && cargo check -p zylos-gateway && echo "GATE 10.1 VERDE"
```

---

## Fase 10.2: Integração Completa com Hermes Agent

**Objetivo:** Conectar todos os componentes cockpit aos endpoints `/api/v1/hermes/*` existentes, eliminando dados mock.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.2.1 | Integrar `skill_editor_panel.rs` com `/api/v1/hermes/skill-evolution/contracts` | — | Médio |
| 10.2.2 | Integrar `swarm_topology_panel.rs` com `/api/v1/hermes/bot/spawn-crew` + status polling | — | Médio |
| 10.2.3 | Integrar `fleet_auditor_panel.rs` com `/api/v1/hermes/bot/*` métricas reais | — | Médio |
| 10.2.4 | Integrar `federation_dashboard.rs` com endpoints HLC/VectorClock reais (ou criar stub backend) | — | Alto |
| 10.2.5 | Integrar `subagent_status_cards.rs` com `/api/v1/hermes/bot/dispatch-task` status | 10.2.2 | Baixo |
| 10.2.6 | Remover todas as funções `mock_*()` E `placeholder_*()` dos componentes integrados, substituindo por fetch real ou fallback documentado | 10.2.1–10.2.5 | Médio |

### Entregável Verificável
- Zero funções `mock_*()` restantes nos componentes integrados
- Todos os componentes fazem fetch real ao gateway com error handling

### Critérios de Aceitação
- [ ] `grep -rE "(mock_|placeholder_)" apps/zylos-cockpit/src/components/` retorna apenas comentários ou fallbacks documentados com justificativa
- [ ] Cada componente tem error state visível quando gateway está offline
- [ ] `cargo check --workspace` passa com zero erros

### Out of Scope
- Caching client-side de respostas do gateway
- Retry logic com exponential backoff (adicionar na Fase 10.6)

### Gate de Verificação
```bash
grep -rn "fn mock_" apps/zylos-cockpit/src/components/ | grep -v "// " | wc -l | xargs test 0 -eq && cargo check --workspace && echo "GATE 10.2 VERDE"
```

---

## Fase 10.3: WebRTC Básico para Phone Relay

**Objetivo:** Substituir placeholder QR/screen por conexão WebSocket real com streaming de canvas.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.3.1 | Implementar WebSocket relay no gateway (`/ws/computer-view/:session_id`) | — | Alto |
| 10.3.2 | Implementar captura de tela no agente (screenshot → base64 → WS frame) | 10.3.1 | Alto |
| 10.3.3 | Implementar renderização de canvas no `phone_relay_panel.rs` via WS | 10.3.1 | Médio |
| 10.3.4 | Implementar controle remoto básico (click/touch events → WS → agente) | 10.3.3 | Médio |
| 10.3.5 | Indicadores de latência, FPS e qualidade de conexão | 10.3.3 | Baixo |

### Entregável Verificável
- Streaming de tela funcional com ≤500ms de latência
- Controle remoto básico (click) funcional

### Critérios de Aceitação
- [ ] WebSocket conecta e desconecta gracefulmente
- [ ] Canvas renderiza frames recebidos via WS
- [ ] Click no canvas envia coordenadas ao agente via WS
- [ ] Fallback para screenshot estático quando WS falha

### Out of Scope
- WebRTC P2P direto (usar WS relay como intermediário)
- Áudio bidirecional
- Compressão de vídeo (H.264/VP8)

### Gate de Verificação
```bash
cargo check -p zylos-gateway && cargo check -p zylos-cockpit && echo "GATE 10.3 VERDE"
```

---

## Fase 10.4: Skill Editor com Syntax Highlighting

**Objetivo:** Adicionar highlighting YAML básico no preview do skill editor e validação em tempo real.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.4.1 | Implementar parser YAML simples em WASM (key-value, lists, nested) | — | Médio |
| 10.4.2 | Adicionar colorização sintática no preview (keywords, strings, numbers) | 10.4.1 | Médio |
| 10.4.3 | Validação em tempo real do YAML gerado (erros inline) | 10.4.1 | Médio |
| 10.4.4 | Botão "Save to Backend" que chama `/api/v1/hermes/skill-evolution/contracts` | 10.2.1 | Baixo |
| 10.4.5 | Undo/redo básico (stack de estados) | — | Baixo |

### Entregável Verificável
- Preview YAML com cores sintáticas
- Erros de validação exibidos inline
- Save funcional via API

### Critérios de Aceitação
- [ ] YAML válido renderiza com ≥3 cores distintas (keys, strings, values)
- [ ] YAML inválido exibe erro com linha/coluna
- [ ] Save cria/atualiza contrato via `/api/v1/hermes/skill-evolution/contracts`
- [ ] Undo/redo funciona para ≥5 níveis

### Out of Scope
- Editor WYSIWYG completo (TipTap/ProseMirror — Fase 10.7)
- Autocomplete de schemas
- Diff visual entre versões

### Gate de Verificação
```bash
cargo check -p zylos-cockpit && echo "GATE 10.4 VERDE"
```

---

## Fase 10.5: Swarm Topology Interativo

**Objetivo:** Adicionar drag-and-drop de nós, zoom/pan no SVG e atualização em tempo real.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.5.1 | Implementar drag-and-drop de nós SVG (mousedown/mousemove/mouseup) | — | Médio |
| 10.5.2 | Implementar zoom/pan com transformações SVG (scale + translate) | — | Médio |
| 10.5.3 | Persistir posições customizadas no localStorage | 10.5.1 | Baixo |
| 10.5.4 | Polling automático de status dos nós (5s interval) | 10.2.2 | Baixo |
| 10.5.5 | Tooltip com detalhes do nó on hover | 10.5.1 | Baixo |
| 10.5.6 | Animação de arestas ativas (pulse/dash animation) | — | Baixo |

### Entregável Verificável
- Nós arrastáveis com persistência de posição
- Zoom/pan funcional
- Status atualizado automaticamente

### Critérios de Aceitação
- [ ] Drag-and-drop funciona em desktop e touch devices
- [ ] Zoom range: 0.25x–4x com smooth transition
- [ ] Posições persistem após refresh da página
- [ ] Status dos nós atualiza sem reload manual

### Out of Scope
- Force-directed layout automático (d3-force)
- Export/import de topologia como JSON
- Edição de conexões (criar/remover arestas via UI)

### Gate de Verificação
```bash
cargo check -p zylos-cockpit && echo "GATE 10.5 VERDE"
```

---

## Fase 10.6: Qualidade e Estabilidade (Opção B)

**Objetivo:** Estabilizar o cockpit com testes E2E reais, build otimizado, limpeza de warnings e documentação de usuário.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.6.1 | Executar testes E2E em browser headless (`wasm-pack test --headless --chrome`) | 10.1–10.5 | Médio |
| 10.6.2 | Corrigir todos os warnings dead_code/unused (8 warnings atuais) | — | Baixo |
| 10.6.3 | Build de produção otimizado (`trunk build --release`) e validar tamanho do WASM | 10.6.2 | Baixo |
| 10.6.4 | Adicionar retry logic com exponential backoff em todos os `fetch_json()` | 10.2 | Médio |
| 10.6.5 | Criar guia de usuário final (`docs/USER_GUIDE.md`) com screenshots | 10.1–10.5 | Médio |
| 10.6.6 | Adicionar health check endpoint específico para cockpit (`/api/v1/cockpit/health`) | — | Baixo |
| 10.6.7 | Performance profiling do WASM bundle (identificar hot paths) | 10.6.3 | Médio |
| 10.6.8 | Accessibility audit (WCAG AA): foco visível, aria-labels, navegação por teclado | — | Médio |

### Entregável Verificável
- Zero warnings no workspace
- Build release <2MB WASM gzipped
- Guia de usuário com ≥10 páginas
- Testes E2E passando em CI

### Critérios de Aceitação
- [ ] `cargo clippy --workspace -- -D warnings` passa
- [ ] `trunk build --release` produz dist/ <2MB gzipped
- [ ] `wasm-pack test --headless` passa com ≥8 testes
- [ ] USER_GUIDE.md cobre todas as 31 tabs
- [ ] Todos os elementos interativos têm aria-label

### Out of Scope
- CI/CD pipeline completo (GitHub Actions)
- Monitoramento de produção (Axiom/Grafana)
- Load testing

### Gate de Verificação
```bash
cargo clippy --workspace -- -D warnings && trunk build --release && echo "GATE 10.6 VERDE"
```

---

## Fase 10.7: Multi-Tenant Auth + CRDT Collaboration (Opção C Parte 1)

**Objetivo:** Adicionar autenticação multi-usuário e colaboração em tempo real via CRDT para edição simultânea de skills/spaces.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.7.1 | Criar crate `zylos-auth` com JWT tokens, roles, permissions | 10.6 | Alto |
| 10.7.2 | Implementar middleware de auth no gateway (Bearer token validation) | 10.7.1 | Médio |
| 10.7.3 | Adicionar login/register UI no cockpit | 10.7.2 | Médio |
| 10.7.4 | Integrar Yjs/CRDT via WASM para edição colaborativa de skills | 10.7.2 | Alto |
| 10.7.5 | Implementar WebSocket sync server para CRDT updates | 10.7.4 | Alto |
| 10.7.6 | Presence indicators (quem está editando agora) | 10.7.5 | Baixo |
| 10.7.7 | Conflict resolution UI (merge conflicts visuais) | 10.7.4 | Médio |

### Entregável Verificável
- Login/registro funcional com JWT
- Dois usuários editando mesma skill simultaneamente sem perda de dados
- Presence indicators funcionais

### Critérios de Aceitação
- [ ] Token JWT expira corretamente (configurável)
- [ ] Usuário sem permissão recebe 403 em endpoints protegidos
- [ ] Edição simultânea de 2+ clientes converge para mesmo estado
- [ ] Conflict resolution exibe diff visual quando merge automático falha

### Out of Scope
- OAuth2/OIDC providers (Google, GitHub)
- RBAC granular por recurso individual
- Audit log de operações colaborativas

### Gate de Verificação
```bash
cargo test -p zylos-auth && cargo check -p zylos-gateway && cargo check -p zylos-cockpit && echo "GATE 10.7 VERDE"
```

---

## Fase 10.8: Rich-Text Editor + Analytics Dashboard (Opção C Parte 2)

**Objetivo:** Adicionar editor rich-text WASM para Spaces e dashboard de analytics de uso dos bots.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.8.1 | Integrar TipTap/ProseMirror via WASM bindings para Spaces editor | 10.7 | Alto |
| 10.8.2 | Implementar slash commands (/heading, /list, /code, /table) | 10.8.1 | Médio |
| 10.8.3 | Autosave com debounce 2s + dirty-page guard | 10.8.1 | Baixo |
| 10.8.4 | Criar crate `zylos-analytics` com agregação de métricas de uso | 10.7 | Alto |
| 10.8.5 | Implementar AnalyticsDashboard no cockpit (gráficos, tabelas, filtros) | 10.8.4 | Alto |
| 10.8.6 | Export de relatórios (CSV/PDF) | 10.8.5 | Médio |
| 10.8.7 | Retention policy configurável para dados de analytics | 10.8.4 | Baixo |

### Entregável Verificável
- Editor rich-text funcional com markdown support
- Dashboard de analytics com ≥5 tipos de gráficos
- Export CSV funcional

### Critérios de Aceitação
- [ ] Editor suporta headings, lists, code blocks, tables
- [ ] Autosave funciona sem perda de dados em disconnect
- [ ] Dashboard renderiza gráficos com dados reais do `zylos-analytics`
- [ ] Export CSV inclui todos os campos visíveis no dashboard
- [ ] Retention policy deleta dados antigos automaticamente

### Out of Scope
- Colaboração em tempo real no rich-text editor (CRDT já cobre skills)
- Alertas/anomalias automáticas (AIOps — futura Fase 11)
- Integração com ferramentas externas (Slack, Notion, Linear)

### Gate de Verificação
```bash
cargo test -p zylos-analytics && cargo check -p zylos-cockpit && echo "GATE 10.8 VERDE"
```

---

## Timeline e Recursos

| Fase | Duração Estimada | Dependências | Prioridade |
|------|-----------------|--------------|------------|
| 10.1 Persistência Channels | 1 semana | — | Crítica |
| 10.2 Integração Hermes | 1.5 semanas | — | Crítica |
| 10.3 WebRTC Phone Relay | 2 semanas | — | Alta |
| 10.4 Skill Editor Highlight | 1 semana | 10.2 | Média |
| 10.5 Swarm Topology Interativo | 1 semana | 10.2 | Média |
| 10.6 Qualidade e Estabilidade | 1.5 semanas | 10.1–10.5 | Crítica |
| 10.7 Multi-Tenant + CRDT | 3 semanas | 10.6 | Alta |
| 10.8 Rich-Text + Analytics | 2 semanas | 10.7 | Média |
| 10.9 UX Polish & Resilience | 1.5 semanas | 10.6 | Alta |

**Total:** ~13.5 semanas (~3.5 meses)

### Paralelismo Possível
- 10.1 e 10.2 podem ser executadas em paralelo (sem dependências mútuas)
- 10.3 pode ser executada em paralelo com 10.4 e 10.5
- 10.7 e 10.8 são sequenciais (10.8 depende de auth de 10.7)
- 10.9 depende de 10.6 mas pode iniciar parcialmente em paralelo com 10.7 (tasks 10.9.5–10.9.10 não dependem de auth)

---

## Riscos e Mitigações

| Risco | Probabilidade | Impacto | Mitigação |
|-------|--------------|---------|-----------|
| Yjs/CRDT WASM bindings instáveis | Média | Alto | Avaliar alternativas (Automerge, Diamond-types) antes de iniciar 10.7 |
| TipTap/ProseMirror WASM muito pesado (>5MB) | Média | Médio | Avaliar editors mais leves (Milkdown, BlockNote) como fallback |
| WebRTC latency >500ms em redes ruins | Alta | Médio | Adaptive quality scaling + fallback para screenshot polling |
| SQLite write contention em channels multi-user | Baixa | Alto | WAL mode + connection pool + write batching |
| JWT token leakage via XSS | Média | Crítico | HttpOnly cookies + CSP headers + token rotation |

---

## Decisões Arquiteturais Pendentes

| # | Decisão | Opções | Status |
|---|---------|--------|--------|
| ADR-007 | CRDT library para colaboração | Yjs vs Automerge vs Diamond-types | Pendente (avaliar em 10.7) |
| ADR-008 | Rich-text editor WASM | TipTap vs Milkdown vs BlockNote | Pendente (avaliar em 10.8) |
| ADR-009 | Auth strategy | JWT stateless vs Session-based vs OAuth2 | Pendente (avaliar em 10.7) |
| ADR-010 | Analytics storage | SQLite vs DuckDB vs TimescaleDB | Pendente (avaliar em 10.8) |

---

## Fase 10.9: UX Polish, Resilience e Limpeza de Placeholders (Opção B Estendida)

**Objetivo:** Polimento final de UX, resiliência de rede, acessibilidade e remoção sistemática de TODAS as funções `mock_*()` E `placeholder_*()` restantes nos componentes cockpit, substituindo por fetch real ou fallback documentado com justificativa.

### Tarefas

| # | Tarefa | Dependência | Esforço |
|---|--------|-------------|---------|
| 10.9.1 | Auditoria completa de `mock_*` e `placeholder_*` em todos os componentes cockpit | 10.2 | Baixo |
| 10.9.2 | Substituir cada mock/placeholder por fetch real ao gateway OU documentar justificativa de fallback | 10.9.1 | Médio |
| 10.9.3 | Adicionar retry logic com exponential backoff + jitter em todos os `fetch_json()` | 10.6.4 | Médio |
| 10.9.4 | Offline detection + queue local de operações pendentes + replay ao reconectar | 10.9.3 | Alto |
| 10.9.5 | Skeleton loading states para todos os componentes (substituir spinners genéricos) | — | Médio |
| 10.9.6 | Toast notification system (sucesso, erro, warning, info) com auto-dismiss | — | Médio |
| 10.9.7 | Keyboard navigation completa (Tab order, Enter/Escape, arrow keys em listas) | 10.6.8 | Médio |
| 10.9.8 | Focus management em modais/dialogs (trap focus, restore on close) | 10.9.7 | Baixo |
| 10.9.9 | Dark/light theme consistency audit (todas as cores via CSS vars, zero hardcoded) | — | Baixo |
| 10.9.10 | Responsive breakpoint testing (mobile/tablet/desktop) para todas as 31 tabs | — | Médio |

### Entregável Verificável
- Zero funções `mock_*()` ou `placeholder_*()` sem justificativa documentada
- Retry + offline queue funcional em todos os componentes
- Skeleton loading em todos os painéis
- Toast notifications funcionais
- Navegação por teclado completa em todas as tabs
- Tema dark/light consistente sem cores hardcoded

### Critérios de Aceitação
- [ ] `grep -rE "(fn mock_|fn placeholder_)" apps/zylos-cockpit/src/components/` retorna zero resultados OU cada resultado tem comentário `// FALLBACK: <justificativa>` na linha anterior
- [ ] Todos os `fetch_json()` têm retry com max 3 tentativas + exponential backoff
- [ ] Offline queue persiste operações em localStorage e replaya ao reconectar
- [ ] Cada componente tem skeleton state distinto do empty state
- [ ] Toast system suporta ≥4 tipos (success/error/warning/info) com auto-dismiss configurável
- [ ] Todas as tabs navegáveis apenas com teclado (Tab + Enter + Escape + Arrows)
- [ ] Focus trap funcional em todos os modais/dialogs
- [ ] Zero cores hardcoded (`#hex`) em componentes — tudo via `var(--color-*)`
- [ ] Layout responsivo testado em 3 breakpoints (mobile <768px, tablet 768-1024px, desktop ≥1024px)

### Out of Scope
- Service Worker para offline completo (PWA cache-first)
- Internacionalização adicional além de PT-BR/EN-US
- Animações de transição entre tabs

### Gate de Verificação
```bash
grep -rEn "(fn mock_|fn placeholder_)" apps/zylos-cockpit/src/components/ | grep -v "// FALLBACK:" | wc -l | xargs test 0 -eq && cargo check -p zylos-cockpit && echo "GATE 10.9 VERDE"
```

---

## Compliance com Manifesto v3.1

| Camada | Conformidade |
|--------|-------------|
| 1. Conceitos Canônicos | ✅ Todas as tarefas mapeadas para conceitos #1–#22 |
| 2. Anti-Slop | ✅ Zero filler, especificações técnicas concretas |
| 3. Loop Profissional | ✅ READ→PLAN→IMPLEMENT→TEST→DOCUMENT por fase |
| 4. Penalidade | ✅ Gate vermelho = fase não avança |
| 5. Arquivos Obrigatórios | ✅ README, DESIGN_SYSTEM, API docs por crate novo |
| 6. Templates de Prompts | N/A (plano de implementação) |
| 7. Matriz de Rastreabilidade | ✅ Tasks mapeadas para gaps da auditoria |
| 8. Integração Harness | ✅ Gates automatizados via bash |
| 9. Skills Hermes | ✅ implementation-plan-author carregada |
| 10. Evolução Semântica | ✅ ADRs pendentes documentados |

---

## Cross-References

- [IMPLEMENTATION_PLAN_DETAILED.md](./IMPLEMENTATION_PLAN_DETAILED.md) — Fases 1–4 completas
- [HANDOFF_FASE9.md](./handoffs/HANDOFF_FASE9.md) — Handoff da fase anterior
- [fase9-audit-adversarial.md](./audits/fase9-audit-adversarial.md) — Auditoria que motivou este plano
- [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) — Tokens e specs de UI
- [MODULE_DOCUMENTATION_STATUS.md](./MODULE_DOCUMENTATION_STATUS.md) — Status dos 14 componentes
- [AGENTS.md](../AGENTS.md) — Regras enforceáveis para agentes

---

## Apêndice A: Compliance Documental Obrigatório (Manifesto v3.1, Camadas 5 e 9)

> **Documento mestre:** [`CONCEITOS-DESENVOLVIMENTO-IA.md`](./CONCEITOS-DESENVOLVIMENTO-IA.md)
> Este apêndice lista TODOS os artefatos documentais que DEVEM ser criados durante a execução da Fase 10, conforme as camadas 5 (Arquivos Obrigatórios) e 9 (Skills Hermes Documentais) do manifesto v3.1.

### A.1 Documentos Transversais (atualizar continuamente)

| Documento | Ação | Quando | Skill |
|-----------|------|--------|-------|
| `TRACKRECORD_FASE10.md` | Criar novo track record com log cronológico por task | Início da Fase 10 | `project-memory-author` |
| `DECISIONS_FASE10.md` | Log contínuo de ADRs (007–010 pendentes + novos) | Durante toda a fase | `project-memory-author` |
| `MODULE_DOCUMENTATION_STATUS.md` | Adicionar novos crates (zylos-channels, zylos-auth, zylos-analytics) | Após cada crate criado | — |
| `DESIGN_SYSTEM.md` | Adicionar tokens de auth/analytics/CRDT quando aplicável | Durante Fases 10.7–10.8 | `design-system-author` |
| `AGENTS.md` | Adicionar auto-invocation rules para novas skills se criadas | Ao final da Fase 10 | `agents-rules-author` |
| Gateway `README.md` | Adicionar endpoints novos por fase | Após cada API criada | — |
| Cockpit `README.md` | Adicionar componentes/tabs novos por fase | Após cada componente criado | — |
| `HANDOFF_FASE10.md` | Handoff completo ao final da Fase 10 | Final | `handoff-author` |

### A.2 Artefatos por Fase

| Fase | READMEs | Playbooks | POPs | Runbooks | SOARs | POTs | Handoff | Track Record |
|------|---------|-----------|------|----------|-------|------|---------|--------------|
| 10.1 Persistência Channels | 1 (zylos-channels) | 1 (pb-channels-migration) | 1 (pop-channel-recovery) | 1 (rb-zylos-channels) | 1 (soar-channel-data-loss) | 1 (pot-channels-schema-evolution) | — | TRACKRECORD_FASE10.md |
| 10.2 Integração Hermes | — | 1 (pb-hermes-integration-testing) | 1 (pop-endpoint-fallback) | — | 1 (soar-hermes-disconnect) | — | — | TRACKRECORD_FASE10.md |
| 10.3 WebRTC Phone Relay | — | 1 (pb-webrtc-setup) | 1 (pop-relay-emergency-stop) | 1 (rb-webrtc-relay) | 1 (soar-relay-latency-alert) | 1 (pot-relay-quality-tuning) | — | TRACKRECORD_FASE10.md |
| 10.4 Skill Editor Highlight | — | — | 1 (pop-skill-save-validation) | — | — | — | — | TRACKRECORD_FASE10.md |
| 10.5 Swarm Topology Interativo | — | — | 1 (pop-topology-reset-layout) | — | — | — | — | TRACKRECORD_FASE10.md |
| 10.6 Qualidade e Estabilidade | — | 1 (pb-e2e-test-setup) | 1 (pop-wasm-build-troubleshoot) | 1 (rb-cockpit-release) | — | 1 (pot-wasm-size-optimization) | — | TRACKRECORD_FASE10.md |
| 10.7 Multi-Tenant + CRDT | 1 (zylos-auth) | 2 (pb-auth-setup, pb-crdt-onboarding) | 2 (pop-token-revoke, pop-crdt-conflict-resolve) | 1 (rb-zylos-auth) | 1 (soar-auth-breach) | 1 (pot-crdt-sync-tuning) | — | TRACKRECORD_FASE10.md |
| 10.8 Rich-Text + Analytics | 1 (zylos-analytics) | 1 (pb-analytics-dashboard-config) | 2 (pop-editor-recovery, pop-analytics-export) | 1 (rb-zylos-analytics) | 1 (soar-analytics-anomaly) | 1 (pot-retention-policy-tuning) | HANDOFF_FASE10.md | TRACKRECORD_FASE10.md |
| 10.9 UX Polish & Resilience | — | 1 (pb-ux-polish-checklist) | 2 (pop-placeholder-cleanup, pop-offline-queue-recovery) | 1 (rb-cockpit-ux-release) | 1 (soar-offline-sync-failure) | 1 (pot-theme-consistency-audit) | — | TRACKRECORD_FASE10.md |

### A.3 Totais Estimados de Documentos Novos

| Tipo | Quantidade | Skills Envolvidas |
|------|-----------|-------------------|
| READMEs de crates | 3 | — |
| Playbooks | 8 | `playbook-author` |
| POPs | 12 | `pop-author` |
| Runbooks | 6 | `runbook-builder` |
| SOARs | 6 | `soar-orchestration` |
| POTs | 6 | — |
| Handoffs | 1 | `handoff-author` |
| Track Records | 1 | `project-memory-author` |
| Decisions (ADRs) | ≥4 | `project-memory-author` |
| **Total** | **≥47** | |

### A.4 Gate Criteria Documental

Nenhuma fase pode ser declarada completa sem:

1. ✅ Todos os playbooks, POPs, runbooks e SOARs listados na tabela A.2 criados
2. ✅ README do crate novo (se aplicável) criado com arquitetura, schema e API docs
3. ✅ `TRACKRECORD_FASE10.md` atualizado com entrada cronológica da fase
4. ✅ `DECISIONS_FASE10.md` atualizado com ADRs relevantes da fase
5. ✅ `MODULE_DOCUMENTATION_STATUS.md` atualizado com novos módulos
6. ✅ Cross-links tridirecionais funcionais (playbook ↔ runbook ↔ SOAR)
7. ✅ Auditoria adversarial executada e aprovada antes de avançar à próxima fase

### A.5 Skills Auto-Invocadas por Fase

| Gatilho Detectado | Skill Auto-Carregada | Fases Aplicáveis |
|-------------------|---------------------|-----------------|
| Criar novo crate | `spec-author` + `testing-checklist-author` | 10.1, 10.7, 10.8 |
| Criar playbook | `playbook-author` | Todas |
| Criar POP | `pop-author` | Todas |
| Criar runbook | `runbook-builder` | 10.1, 10.3, 10.6, 10.7, 10.8 |
| Criar SOAR workflow | `soar-orchestration` | 10.1, 10.2, 10.3, 10.7, 10.8 |
| Completar fase | `handoff-author` + `isolated-audit` | Todas |
| Definir auth/RBAC | `access-control-author` | 10.7 |
| Criar dashboard analytics | `dashboard-author` | 10.8 |
| Configurar observabilidade | `aiops-observability` | 10.6, 10.8 |
| Entregar artefato para review | `anti-slop-filter` | Todas |
| Tarefa >15 turnos | `subagent-driven-dev` | Todas |
| Prova por mutação | `mordida-protocol` | Todas |
| UX polish / skeleton / toast | `design-system-author` + `anti-slop-filter` | 10.9 |
| Limpeza mock/placeholder | `isolated-audit` | 10.9 |
| Offline queue / retry | `soar-orchestration` | 10.9 |