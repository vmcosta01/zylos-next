# Plano Detalhado de Correção de Gaps — Zylos-Next v6

**Data:** 2026-11-04
**Status:** Planejamento Final Consolidado
**Fonte:** Master Plan Original + SUPPLEMENTAL_GAP_CORRECTION_PLAN.md + Análise de Gaps de Interface/Fase 7C

---

## 1. Escopo e Objetivo

Este plano consolida **todos os gaps restantes** entre o estado atual do Zylos-Next e o Master Plan original (`zylos-next-master-plan.md`), com foco especial nas áreas que não foram cobertas pelo plano suplementar inicial:

1. **Fase 7C Completa** — HITL Approval Cards, Task Durability, Outcome Uncertainty, Two-Stage Triage, Equivalence Hashing, Sentinel AIOps
2. **Interface/Cockpit (Sprint 6.2)** — WASM Frontend Leptos, Lazy-Roster, AG-UI Streaming Integration
3. **Integração Final (Fase 7)** — Orquestrador Global, E2E Tests, Integração Hermes Real, Deploy Produção
4. **Documentação e Artefatos Operacionais** — Runbooks, Playbooks, OpenAPI, Benchmarks

**Meta:** Levar o Zylos-Next do estado atual (crates parciais + docs) até produção-ready com 65 perfis Hermes, zero data loss, e dupla auditoria aprovada.

---

## 2. Gaps Identificados por Fase

### 2.1 Fase 7C: Padrões Externos (PARCIALMENTE IMPLEMENTADA)

**Estado atual:** ⚠️ Módulos implementados em `zylos-live-comm` + crates dedicados; faltam tipos AG-UI completos e integração E2E
**Referência Master Plan:** Linhas 2071-2246

| Task | Descrição | Target LOC | Status | Notas |
|------|-----------|------------|--------|-------|
| 7C.1.1 | HITL Approval Cards via AG-UI | ~800L | ✅ 614L + 13 testes | `approval.rs` completo com CAS, TTL, secret masking |
| 7C.1.2 | Task Durability via SQL Leases | ~1.000L | ✅ 723L + 15 testes | `task_durability.rs` + crate dedicado (2.290L) |
| 7C.1.3 | Outcome Uncertainty Tri-State | ~200L | ✅ 471L + 18 testes | `outcome.rs` com dedup e classification |
| 7C.2.1 | Two-Stage Triage Engine | ~400L | ✅ 866L + 18 testes | `triage.rs` + crate dedicado (589L) |
| 7C.2.2 | Equivalence-Based Approval Hashing | ~700L | ✅ 957L + 18 testes | `equivalence_approval.rs` completo |
| 7C.2.3 | Sentinel Health Scanner / AIOps | ~350L | ✅ 815L + 19 testes | `sentinel.rs` + crate dedicado (595L) |
| 7C.Audit | Dupla Auditoria (adversarial + isolated) | 2 docs | ❌ | Pendente |
| 7C.AGUI | Tipos AG-UI faltantes (RunStarted/Finished/ToolCallEnd/CustomEvent) | ~150L | ✅ Completo | 4 tipos + RunStatus enum + 6 testes roundtrip adicionados |

**Critérios de Saída da Fase 7C:**
- [x] HITL approval cards funcionais via AG-UI com storage server-side
- [x] Task durability via SQL leases com maintenance loop e circuit breaker
- [x] Outcome uncertainty tri-state implementado
- [x] Two-stage triage com decided_by auditável
- [x] Equivalence-based approval hashing funcional
- [x] Sentinel health scanner com detectors pure functions
- [ ] Adicionar tipos AG-UI faltantes: `RunStarted`, `RunFinished`, `ToolCallEnd`, `CustomEvent`
- [ ] Dupla Auditoria: `evals/audits/fase7c-audit-adversarial.md` + `evals/audits/fase7c-audit-isolated.md` ambos APPROVED
- [ ] Integração E2E com SOAR gateway verificada

### 2.2 Sprint 6.2: Cockpit Visual (PARCIALMENTE IMPLEMENTADA)

**Estado atual:** ⚠️ 2.426 LOC implementados com componentes básicos; faltam SSR, PWA, i18n completo, streaming AG-UI integration
**Referência Master Plan:** Linhas 1776-1851

| Task | Descrição | Target | Status | Notas |
|------|-----------|--------|--------|-------|
| 6.2.1 | WASM Frontend Reactive (Leptos 0.7) | ~1.200L + assets | ⚠️ 2.426L total | Componentes existem; SSR/hydration/PWA/i18n pendentes |
| 6.2.2 | Lazy-Roster para 50+ Agentes | ~300L | ⚠️ Existe `lazy_roster.rs` | Virtualização e performance budget não verificados |
| 6.2.3 | AG-UI Streaming Integration | ~600L | ⚠️ Existe `streaming.rs` | Integração completa com tipos AG-UI faltantes pendente |

**Componentes Existentes no Cockpit:**
- `app.rs` — DashboardState enum, routing
- `agent_card.rs` — AgentCard component
- `lazy_roster.rs` — Lazy-Roster component
- `soar_dashboard.rs` — SOAR dashboard view
- `theme_toggle.rs` — Dark/light theme toggle
- `streaming.rs` — Streaming client (parcial)
- `i18n.rs` — i18n setup (parcial)
- `benchmarks.rs` — Benchmark views

**Critérios de Aceite Sprint 6.2:**
- [ ] Bundle inicial <200KB gzipped (CI gate)
- [ ] Time to Interactive <1.5s em 4G
- [ ] Lighthouse score >95 (Performance, Accessibility, Best Practices, SEO)
- [ ] SSR funcional com zero hydration mismatches
- [ ] Streaming client recupera conexão em <2s sem event loss
- [x] PWA funciona offline com queue replay on reconnect (sw.js + manifest.json + index.html criados em `apps/zylos-cockpit/static/`)
- [ ] i18n cobre ≥98% das strings visíveis
- [ ] 500 agentes renderizados em <100ms (Lazy-Roster)
- [ ] Latência end-to-end <100ms (agent → UI)
- [ ] 1000 eventos/segundo processados sem perda
- [ ] Tipos AG-UI completos integrados (RunStarted, RunFinished, ToolCallEnd, CustomEvent)

### 2.3 Fase 7: Integração Final e Produção (NÃO IMPLEMENTADA)

**Estado atual:** ❌ Zero código de integração
**Referência Master Plan:** Linhas 2249-2441

| Task | Descrição | Horas | Status |
|------|-----------|-------|--------|
| 7.1.1 | Orquestrador Global de Crates | 32h | ❌ |
| 7.1.2 | Testes End-to-End (20 cenários) | 40h | ❌ |
| 7.1.3 | Integração com Hermes Real (65 perfis) | 40h | ❌ |
| 7.2.1 | Benchmark Final Publicado | 32h | ❌ |
| 7.2.2 | Documentação Completa (README, ARCHITECTURE, DEPLOYMENT, etc.) | 40h | ❌ |
| 7.2.3 | Ecosystem Launch (crates.io, Helm, Docker, blog) | 24h | ❌ |
| 7.3.1 | Deploy Produção em Hermes | 32h | ❌ |
| 7.3.2 | Treinamento e Handoff | 16h | ❌ |
| 7.3.3 | Post-Mortem e Roadmap Futuro | 8h | ❌ |

### 2.4 Artefatos Operacionais Faltantes (Parcialmente Implementados)

**Estado atual:** ⚠️ Runbooks criados por subagentes, mas playbooks e OpenAPI pendentes
**Referência:** SUPPLEMENTAL_GAP_CORRECTION_PLAN.md §4 (linhas 397-473)

| Artefato | Status | Nota |
|----------|--------|------|
| Runbooks (10 arquivos) | ✅ Criados | Via deleg_709cfa31 task-1 |
| Playbook Disaster Recovery | ✅ Criado | 309 linhas, 11.369 bytes |
| Runbook Canary Deployment | ✅ Criado | 346 linhas, 12.013 bytes |
| Playbook Incident Response SOAR | ✅ Criado | 542 linhas, 16.973 bytes |
| Runbook Backup e Restore | ✅ Criado | 329 linhas, 11.723 bytes |
| OpenAPI Spec (gateway-openapi.yaml) | ✅ Criado | 1.060 linhas, 30.682 bytes, validado |
| Cockpit API Docs | ⚠️ Incluído no OpenAPI | Endpoints /api/v1/ cobertos na spec |
| Módulo Documentation Status Tracker | ✅ Criado | Via deleg_709cfa31 task-3 |
| Gaps Analysis Runbooks/Playbooks | ✅ Criado | Via deleg_709cfa31 task-3 |

---

## 3. Plano de Execução Detalhado

### Fase A: Fundação para Interface e 7C (Semana 1-2)

**Objetivo:** Garantir que zylos-live-comm e zylos-memory-system têm os tipos e APIs necessários para Cockpit e HITL.

#### Task A.1: Expandir zylos-live-comm com tipos AG-UI completos
- **Target:** Adicionar tipos `ApprovalRequest`, `ApprovalResponse`, `RunStarted`, `RunFinished`, `ToolCallStart`, `ToolCallEnd`, `TextMessageContent`, `StateSnapshot`, `CustomEvent`
- **Arquivo:** `crates/zylos-live-comm/src/ag_ui/types.rs`
- **Testes:** 15 (serialização/deserialização, ordering guarantees, sequence numbering)
- **Dependências:** Nenhuma
- **Horas:** 16h

#### Task A.2: Preparar zylos-memory-system para SQL Leases
- **Target:** Adicionar schema `tasks` com campos `lease_id`, `lease_until`, `status`, `attempts`; implementar `FOR UPDATE SKIP LOCKED` (feature-flagged para Postgres vs SQLite polling)
- **Arquivo:** `crates/zylos-memory-system/src/task_store.rs`
- **Testes:** 20 (schema migration, CAS race, skip_locked concurrency)
- **Dependências:** Nenhuma
- **Horas:** 24h

#### Task A.3: Configurar projeto Leptos para Cockpit
- **Target:** Scaffold `apps/zylos-cockpit/` com Leptos 0.7, SSR, wasm-bindgen, trunk/Dioxus build pipeline
- **Arquivos:** `apps/zylos-cockpit/Cargo.toml`, `src/main.rs`, `index.html`, `Trunk.toml`
- **Testes:** 5 (build passa, SSR renderiza, hydration sem mismatch)
- **Dependências:** Rust toolchain, wasm32-unknown-unknown target
- **Horas:** 12h

**Total Fase A:** 52h (~1 semana útil)

---

### Fase B: HITL Approval + Task Durability (Semana 3-4)

**Objetivo:** Implementar core da Fase 7C.1 (Tasks 7C.1.1, 7C.1.2, 7C.1.3)

#### Task B.1: HITL Approval Cards via AG-UI (Task 7C.1.1)
- **Target:** ~800L implementando todos os 17 subtasks do master plan
- **Arquivo:** `crates/zylos-soar/src/hitl/approval_service.rs`
- **Subtasks críticos:**
  - [ ] `ApprovalRequest` type com id, summary, tool_call, args_hash, ttl
  - [ ] Storage server-side em SQLite (approval_pendings table)
  - [ ] Claim atômico (CAS) para prevenir double-approve
  - [ ] Idempotent result recovery para re-renders
  - [ ] Exclusão de approval-gated tools em contextos headless/Slack
  - [ ] Secret masking com distinção whole/inbox
  - [ ] Execution authority via `tokio::task_local!`
  - [ ] Validation probe task-local
  - [ ] Carry-on multi-decision step
  - [ ] Withdraw allowed selective
  - [ ] Abandon para retries exauridos
  - [ ] Is_permanent(error) detection
  - [ ] Public_approval() strip internals
- **Testes:** 30 (lifecycle completo, equivalence, tier hierarchy, timeout, headless)
- **Dependências:** Task A.1, Task A.2
- **Horas:** 48h

#### Task B.2: Task Durability via SQL Leases (Task 7C.1.2)
- **Target:** ~1.000L implementando todos os 18 subtasks do master plan
- **Arquivo:** `crates/zylos-orchestrator/src/task_worker.rs`
- **Subtasks críticos:**
  - [ ] TaskWorker com lease SQL (lease_id UUID + lease_until TIMESTAMPTZ)
  - [ ] Compare-and-swap para single-owner execution
  - [ ] State machine completa (9 estados)
  - [ ] Maintenance loop 60s (recover, reclaim, refresh)
  - [ ] Database-as-clock (NUNCA Utc::now() no Rust)
  - [ ] Heartbeat por task (lease_ms/3 interval)
  - [ ] LostLeaseError revert
  - [ ] Trio guard()/checkpoint()/event()
  - [ ] Run() com registro em tabela runs
  - [ ] Answer() preservando histórico Q&A
  - [ ] Select for update skip locked
  - [ ] Pg_notify inside transaction
  - [ ] Attempts semantics (ONE vs >ONE)
  - [ ] Exponential backoff com jitter
  - [ ] Circuit breaker (5 falhas → pause)
  - [ ] 100-task active cap
  - [ ] Control() CAS com precondition
- **Testes:** 40 (lease steal, CAS race, maintenance, heartbeat, skip_locked, pg_notify, backoff, circuit breaker, 100-cap)
- **Dependências:** Task A.2
- **Horas:** 56h

#### Task B.3: Outcome Uncertainty Tri-State (Task 7C.1.3)
- **Target:** ~200L
- **Arquivo:** `crates/zylos-orchestrator/src/outcome.rs`
- **Subtasks:**
  - [ ] Enum Outcome { Succeeded, Failed, OutcomeUnknown }
  - [ ] Classificar network error/5xx → OutcomeUnknown
  - [ ] Retry policy: OutcomeUnknown → confirmação humana
  - [ ] Audit log de transições
  - [ ] Notification dedup key
- **Testes:** 10 (5xx→unknown, timeout→unknown, 4xx→failed, 200→succeeded, dedup)
- **Dependências:** Task B.2
- **Horas:** 12h

**Total Fase B:** 116h (~2.5 semanas úteis)

---

### Fase C: Cockpit WASM + Streaming (Semana 5-6)

**Objetivo:** Implementar Sprint 6.2 completa (Tasks 6.2.1, 6.2.2, 6.2.3)

#### Task C.1: WASM Frontend Reactive Cockpit (Task 6.2.1)
- **Target:** ~1.200L + HTML/CSS/JS assets
- **Arquivos:** `apps/zylos-cockpit/src/` (componentes, rotas, stores)
- **Subtasks:**
  - [ ] Leptos 0.7 SSR + Hydration config
  - [ ] Component library: AgentCard, TaskNode, DAGView, MemoryTimeline, ApprovalCard, EventStream
  - [ ] Dashboard consolidado com tabs URL-synced + drag-and-drop
  - [ ] Tema dark/light com CSS variables (Hermes-compatible)
  - [ ] i18n lazy-loaded pt-BR/en-US com ICU MessageFormat
  - [ ] PWA offline-first (service worker, offline queue, background sync)
  - [ ] Streaming SSE client com auto-reconnect + optimistic UI
  - [ ] Performance budget CI gate (<200KB gzipped, TTI <1.5s, Lighthouse >95)
  - [ ] Accessibility testing (axe-core scan, ARIA labels, keyboard nav)
- **Testes:** 20 (SSR correctness, accessibility, theme switching, i18n coverage, PWA flow, streaming reconnect, performance budget)
- **Dependências:** Task A.3, Task B.1 (para ApprovalCard)
- **Horas:** 48h

#### Task C.2: Lazy-Roster para 50+ Agentes (Task 6.2.2)
- **Target:** ~300L
- **Arquivo:** `apps/zylos-cockpit/src/components/lazy_roster.rs`
- **Subtasks:**
  - [ ] Virtualização de lista (apenas visíveis renderizados)
  - [ ] Paginação infinita com IntersectionObserver
  - [ ] Busca local fuzzy
  - [ ] Agrupamento por tenant/project
  - [ ] Filtro por status
  - [ ] Cache de metadados com TTL invalidation
- **Testes:** 10 (500 agentes <100ms, scroll 60fps, memória <150MB, busca <50ms)
- **Dependências:** Task C.1
- **Horas:** 20h

#### Task C.3: AG-UI Streaming Integration (Task 6.2.3)
- **Target:** ~600L
- **Arquivo:** `apps/zylos-cockpit/src/streaming/ag_ui_client.rs`
- **Subtasks:**
  - [ ] Client AG-UI em Leptos (Rust WASM → browser)
  - [ ] Session Events SDK (RunStarted, RunFinished, ToolCallStart, etc.)
  - [ ] Streaming de tool calls com incremental rendering
  - [ ] Streaming de thinking/reasoning colapsável
  - [ ] Streaming de memory updates via StateSnapshot
  - [ ] Approval cards inline no stream
  - [ ] Reconexão automática com exponential backoff + jitter
  - [ ] Compressão delta (JSON Patch RFC 6902)
  - [ ] Sequence numbering para gap detection + ResyncRequest
  - [ ] Event buffering 16ms batch (60fps)
  - [ ] Multi-run fusion cronológica
- **Testes:** 15 (ordering guarantee, gap detection, approval round-trip, incremental rendering, buffer flush, multi-run fusion)
- **Dependências:** Task C.1, Task B.1
- **Horas:** 40h

**Total Fase C:** 108h (~2.5 semanas úteis)

---

### Fase D: Triage + Equivalence + Sentinel (Semana 7-8)

**Objetivo:** Completar Fase 7C.2 (Tasks 7C.2.1, 7C.2.2, 7C.2.3) + Auditoria

#### Task D.1: Two-Stage Triage Engine (Task 7C.2.1)
- **Target:** ~400L
- **Arquivo:** `crates/zylos-soar/src/triage/two_stage.rs`
- **Subtasks:**
  - [ ] Stage 1: rules-only para safe negatives
  - [ ] Stage 2: model fallback com prompt estruturado + Zod parsing
  - [ ] Campo decided_by: "rule:<id>" | "model:<name>"
  - [ ] Framing pattern wrappers fixos
  - [ ] Initiator tracking (person/routine/responsibility/memory/handoff)
  - [ ] Eval measurement contra corpus rotulado
- **Testes:** 15 (rule match→skip model, model failure→throw, decided_by populated, framing correctness)
- **Dependências:** Nenhuma (paralelo com Fase C)
- **Horas:** 20h

#### Task D.2: Equivalence-Based Approval Hashing (Task 7C.2.2)
- **Target:** ~700L
- **Arquivo:** `crates/zylos-soar/src/hitl/equivalence.rs`
- **Subtasks:**
  - [ ] Blake3 hash de [actor_id, bot_id, tool_ref, effect, scope, stable(target), stable(args)]
  - [ ] Trait ApprovalDigest (blake3/sha256 intercambiáveis)
  - [ ] stable() stripa campos voláteis, ordena keys recursivamente (depth limit 6)
  - [ ] Four-tier decision via decide_action
  - [ ] 7 mensagens de recusa distintas
  - [ ] Continuation capture via tokio::task_local!
  - [ ] carry_on() multi-decision step
  - [ ] withdraw_allowed() selective
  - [ ] abandon() resultado sintético
  - [ ] is_permanent(error) detection
  - [ ] public_approval() strip internals
  - [ ] Escalation como ferramenta ungated
  - [ ] Policy engine como WASM sandbox (wasmtime 25.0)
- **Testes:** 35 (same-action reuse, tier override, refusal messages, carry_on, withdraw, abandon, permanent error, WASM policy load)
- **Dependências:** Task B.1
- **Horas:** 36h

#### Task D.3: Sentinel Health Scanner / AIOps (Task 7C.2.3)
- **Target:** ~350L
- **Arquivo:** `crates/zylos-sentinel/src/health_scanner.rs`
- **Subtasks:**
  - [ ] Detectors como pure functions sobre stores read-only
  - [ ] Normalized signatures para dedup
  - [ ] Output separado (fora do data dir principal)
  - [ ] Scheduling via tokio interval (120s default)
  - [ ] Privacy-first: hash PII/IDs, strip sensitive fields
  - [ ] Issue upsert por signature em JSONL append-only
  - [ ] Team incident reporting com rate limit (5/hour mute)
- **Testes:** 15 (detector pure function, signature dedup, privacy stripping, rate limiting, JSONL format)
- **Dependências:** zylos-live-comm
- **Horas:** 16h

#### Task D.4: Dupla Auditoria da Fase 7C
- **Target:** 2 documentos completos
- **Arquivos:**
  - `evals/audits/fase7c-audit-adversarial.md`
  - `evals/audits/fase7c-audit-isolated.md`
- **Conteúdo obrigatório:**
  - [ ] Verificação independente de cada task B.1-B.3, D.1-D.3 contra critérios de aceite
  - [ ] Testes de integração E2E com SOAR gateway
  - [ ] Validação de security boundaries (approval bypass, lease stealing, PII leakage)
  - [ ] Performance benchmarks (approval latency, lease renewal throughput, triage speed)
  - [ ] Assinatura formal: APPROVED / REJECTED com justificativa
- **Dependências:** Todas as tasks da Fase 7C
- **Horas:** 24h

**Total Fase D:** 96h (~2 semanas úteis)

---

### Fase E: Integração Final + Produção (Semana 9-12)

**Objetivo:** Implementar Fase 7 completa (Tasks 7.1.1 a 7.3.3)

#### Task E.1: Orquestrador Global de Crates (Task 7.1.1)
- **Target:** ~500L
- **Arquivo:** `apps/zylos-gateway/src/orchestrator.rs`
- **Subtasks:**
  - [ ] Binário que une todos os crates
  - [ ] Startup order (memory → orchestrator → verification → cockpit)
  - [ ] Graceful shutdown com drain de requests
  - [ ] Health checks integrados (/health, /ready, /live)
  - [ ] Configuração via TOML + env vars + CLI flags
  - [ ] Hot-reload de configuração sem restart
- **Testes:** 15 (startup <3s, shutdown <5s, health <100ms, hot-reload 80%)
- **Dependências:** Todas as fases anteriores
- **Horas:** 32h

#### Task E.2: Testes End-to-End (Task 7.1.2)
- **Target:** 20 cenários E2E
- **Arquivos:** `tests/e2e/` (20 arquivos .rs)
- **Cenários obrigatórios:**
  - [ ] Memory (consolidação, contradição, decay)
  - [ ] Orchestrator (DAG 100 nós, rollback, recovery)
  - [ ] Verification (proof, panel, ToT)
  - [ ] Sandbox (WASM, Docker, browser)
  - [ ] Skills (registry, eval, export)
  - [ ] Cockpit (UI + streaming + lazy-roster)
  - [ ] HITL (approval lifecycle, equivalence reuse)
  - [ ] Task Durability (lease steal, maintenance recovery)
- **Critérios:** 100% coverage user journeys críticos, flakiness <1%, CI <30min
- **Dependências:** Task E.1
- **Horas:** 40h

#### Task E.3: Integração com Hermes Real (Task 7.1.3)
- **Target:** Staging com 5 perfis reais → validação 2 semanas
- **Subtasks:**
  - [ ] Deploy staging com 5 perfis Hermes
  - [ ] Testar integração com Crew, Bot HQ, Operations, Boardstate, Kanban, MAX Platform
  - [ ] Logging detalhado para debugging
  - [ ] Validar 2 semanas sem crashes/data loss
- **Dependências:** Task E.2
- **Horas:** 40h

#### Task E.4: Benchmark Final Publicado (Task 7.2.1)
- **Target:** Resultados em docs/BENCHMARKS.md + raw data
- **Subtasks:**
  - [ ] Executar benchmark suite em c5.4xlarge
  - [ ] Comparar com baselines (Hindsight, Rig, AutoAgents, Mem0, Hermes nativo)
  - [ ] Publicar gráficos e raw data
  - [ ] Submeter para review comunidade Rust
- **Critérios:** Supera/iguala baseline em 80%, RAM idle <30MB, TTFD <50ms, reproduzível 3x
- **Dependências:** Task E.3
- **Horas:** 32h

#### Task E.5: Documentação Completa (Task 7.2.2)
- **Target:** 7 documentos + rustdoc + vídeo tutorial
- **Arquivos:**
  - README.md (quick start <5min)
  - docs/ARCHITECTURE.md (diagramas C4)
  - docs/DEPLOYMENT.md (Docker, K8s, bare metal)
  - docs/CONTRIBUTING.md
  - docs/MIGRATION.md (Hermes v1 → Zylos-Next)
  - docs/FAQ.md (top 20)
  - rustdoc completo (100% coverage API pública)
  - Vídeo tutorial 10min
- **Dependências:** Todas as fases anteriores
- **Horas:** 40h

#### Task E.6: Ecosystem Launch (Task 7.2.3)
- **Target:** Crates publicados + Helm + Docker + blog + Discord
- **Subtasks:**
  - [ ] Publicar crates em crates.io
  - [ ] Publicar Helm chart (validado AWS/GCP/Azure)
  - [ ] Publicar Docker images GHCR + Docker Hub (<500MB)
  - [ ] GitHub Discussions + Discord server
  - [ ] Blog post Medium + dev.to (>1000 views/sem)
  - [ ] Talk RustConf/AgentCon preparada
- **Dependências:** Task E.5
- **Horas:** 24h

#### Task E.7: Deploy Produção em Hermes (Task 7.3.1)
- **Target:** 65 perfis em produção, zero downtime/data loss
- **Subtasks:**
  - [ ] Deploy produção 65 perfis
  - [ ] Migrar memória existente → zylos-memory
  - [ ] Migrar 338 skills → novo registry
  - [ ] Migrar workflows → DAG runtime
  - [ ] Configurar Prometheus + Grafana + alertas + backups
  - [ ] Validar 1 semana sem incidentes P0/P1
- **Critérios:** Zero downtime, zero data loss, SLA 99.9%
- **Dependências:** Task E.6
- **Horas:** 32h

#### Task E.8: Treinamento e Handoff (Task 7.3.2)
- **Target:** 4 sessões 2h + runbook operacional + on-call
- **Dependências:** Task E.7
- **Horas:** 16h

#### Task E.9: Post-Mortem e Roadmap Futuro (Task 7.3.3)
- **Target:** Retrospective + roadmap v2.0 (12 meses) + arquivamento
- **Dependências:** Task E.8
- **Horas:** 8h

**Total Fase E:** 264h (~6 semanas úteis)

---

### Fase F: Artefatos Operacionais Pendentes (✅ CONCLUÍDA)

**Objetivo:** Completar S4 do plano suplementar
**Status:** ✅ Todos os 5 artefatos criados e verificados em disco

#### Task F.1: Playbook Disaster Recovery (S4.1) ✅
- **Arquivo:** `playbooks/disaster-recovery.md` (309 linhas, 11.369 bytes)
- **Conteúdo:** Declaração formal, RTO/RPO por componente, sequência recuperação, integridade dados, comunicação regulatória LGPD, teste anual

#### Task F.2: Runbook Canary Deployment (S4.2) ✅
- **Arquivo:** `runbooks/canary-deployment.md` (346 linhas, 12.013 bytes)
- **Conteúdo:** Estratégia 5%→25%→50%→100%, métricas health, rollback automático, validação schema, approval gates

#### Task F.3: Playbook Incident Response SOAR (S4.3) ✅
- **Arquivo:** `playbooks/incident-response-soar.md` (542 linhas, 16.973 bytes)
- **Conteúdo:** Severidade P1-P4, triage automatizado zylos-triage, escalação humana, forensics checklist, post-mortem 5 Whys

#### Task F.4: Runbook Backup e Restore (S4.4) ✅
- **Arquivo:** `runbooks/backup-restore.md` (329 linhas, 11.723 bytes)
- **Conteúdo:** Schedule incremental/full, comandos SQLite WAL, PRAGMA integrity_check, restore parcial/completo, retenção LGPD, criptografia at-rest

#### Task F.5: OpenAPI Spec + Cockpit API Docs (S4.5) ✅
- **Arquivo:** `docs/api/gateway-openapi.yaml` (1.060 linhas, 30.682 bytes, YAML validado)
- **Conteúdo:** Todos endpoints /api/v1/, auth Bearer JWT, rate limiting, error codes, exemplos curl, versioning

**Total Fase F:** ✅ Concluída (44h estimadas → executada via subagentes)

---

## 4. Cronograma Consolidado Final

| Fase | Duração | Semana | Tasks | Bloqueia | Horas |
|------|---------|--------|-------|----------|-------|
| A | 1 semana | 1 | A.1-A.3 | B, C | 52h |
| B | 2.5 semanas | 2-4 | B.1-B.3 | C, D | 116h |
| C | 2.5 semanas | 5-6 | C.1-C.3 | E | 108h |
| D | 2 semanas | 7-8 | D.1-D.4 | E | 96h |
| E | 6 semanas | 9-14 | E.1-E.9 | Release v6.0 | 264h |
| F | 1 semana (paralelo) | 7-8 | F.1-F.5 | Release v6.0 | 44h |
| **Total** | **~14 semanas** | | **35 tasks** | | **680h** |

**Caminho Crítico:** A.1 → A.2 → B.2 → B.3 → C.3 → D.4 → E.1 → E.2 → E.3 → E.7

---

## 5. Matriz de Dependências Visual

```
A.1 ──→ A.2 ──→ A.3
  │       │       │
  ▼       ▼       ▼
B.1 ←─────┘     C.1 ←── A.3
  │               │
  ▼               ▼
B.2             C.2
  │               │
  ▼               ▼
B.3             C.3 ←── B.1
                  │
D.1 (paralelo)    │
  │               │
  ▼               ▼
D.2 ←── B.1     E.1 ←── (todas)
  │               │
  ▼               ▼
D.3             E.2
  │               │
  ▼               ▼
D.4 ←── (todas 7C) E.3
                  │
F.1-F.5 (paralelo) ▼
                E.4 → E.5 → E.6 → E.7 → E.8 → E.9
```

---

## 6. Riscos e Mitigações Atualizados

| Risco | Probabilidade | Impacto | Mitigação |
|-------|---------------|---------|-----------|
| zylos-core tem tipos insuficientes para crates existentes | Alta | Alto | Extrair tipos REAIS antes de definir core (Task A.1-A.2) |
| WASM runtime incompatível com Windows dev environment | Média | Médio | Docker fallback; CI Linux-first |
| Cockpit WASM bundle > 5MB | Baixa | Médio | Code-splitting lazy loading; CI gate <200KB gzipped |
| SQL leases complexos demais para SQLite | Média | Alto | Feature flag postgres-leases vs sqlite-simple-leases |
| Blake3 dependency surface area segurança | Baixa | Médio | Audit blake3 crate; fallback SHA-256 via trait |
| WASM policy engine (wasmtime) binary size | Média | Médio | Feature flag wasm-policy; release-only |
| Leptos 0.7 breaking changes durante desenvolvimento | Baixa | Médio | Pin versão exata; monitorar changelog semanal |
| Integração Hermes Real revela incompatibilidades | Alta | Alto | Staging 2 semanas antes de produção; logging detalhado |

---

## 7. Definição de Done Global

O plano é considerado completo quando:

1. ✅ Todos os 35 tasks têm critério de conclusão verificado
2. ✅ `cargo build --workspace` passa sem errors
3. ✅ `cargo test --workspace` passa com ≥80% coverage
4. ✅ `cargo doc --workspace` gera sem warnings
5. ✅ Cockpit serve via gateway; dashboard mostra dados reais
6. ✅ HITL approval cards renderizam no cockpit; equivalence hash funcional
7. ✅ Task durability com leases; maintenance loop recupera órfãos <120s
8. ✅ Dupla auditoria Fase 7C: ambos APPROVED
9. ✅ 20 cenários E2E passando; flakiness <1%; CI <30min
10. ✅ Integração Hermes Real: 2 semanas staging sem crashes/data loss
11. ✅ Benchmark publicado; supera/iguala baseline em 80%
12. ✅ Documentação completa; rustdoc 100% coverage API pública
13. ✅ Crates publicados crates.io; Helm validado 3 provedores; Docker <500MB
14. ✅ Deploy produção 65 perfis; zero downtime; SLA 99.9%
15. ✅ Todos os runbooks/playbooks testados em simulação
16. ✅ OpenAPI spec valida; ≥90% endpoints com exemplo funcional
17. ✅ Zero definições duplicadas de tipos core em qualquer crate

---

## 8. Métricas de Sucesso

| Métrica | Target | Medição |
|---------|--------|---------|
| RAM idle por agente | <30MB | Benchmark suite |
| TTFD (Time to First Decision) | <50ms | Benchmark suite |
| Bundle Cockpit gzipped | <200KB | CI gate |
| TTI Cockpit 4G | <1.5s | Lighthouse CI |
| Lighthouse Score | >95 | Lighthouse CI |
| Streaming latency E2E | <100ms | Performance API |
| Eventos/segundo sem perda | 1000 | Sequence numbering |
| Reconexão streaming | <2s | Network instability test |
| Lease recovery órfãos | <120s | Maintenance loop test |
| Approval equivalence reuse | >80% same-action | Metrics counter |
| CI E2E duration | <30min | CI pipeline |
| Flakiness E2E | <1% | 100 execuções |
| Uptime produção | 99.9% | Prometheus |
| Data loss | Zero | Checksums + WAL replay |

---

---

## 9. Integração de Padrões Externos (Engenharia Reversa — 31 Repositórios)

**Fonte:** 31 relatórios de engenharia reversa consolidados em `REVERSE_ENGINEERING_INDEX.md`
**Objetivo:** Aplicar lições aprendidas de frameworks e plataformas de referência diretamente nas tasks do plano, evitando reinvenção e incorporando padrões validados pela comunidade.

### 9.1 Mapeamento de Padrões → Tasks do Plano

#### Fase A: Core Types + Foundation

| Padrão Externo | Origem | Aplicação na Task | Justificativa |
|----------------|--------|-------------------|---------------|
| **Effects as Data** | native (Zig) | A.1 (Core Types) | Retornar enum `Effect` em vez de executar tools diretamente; testabilidade unitária trivial, replay e auditoria completa |
| **Trait-based composability (`Invocable<I,O>`)** | langchainjs | A.2 (Traits) | Equivalente Rust de `Runnable`; generics + trait objects para composição sem herança |
| **Schema validation com serde + schemars** | langchainjs | A.1-A.2 | Validação runtime equivalente a Zod/Joi; derive macros reduzem boilerplate |
| **State machine explícita (enums + match)** | langgraph, lifecycle-workshop | A.2 | FSM tipada com transições verificadas em compile time; evita strings magicas |

#### Fase B: Memory + Verification + SOAR

| Padrão Externo | Origem | Aplicação na Task | Justificativa |
|----------------|--------|-------------------|---------------|
| **RLS patterns no PostgreSQL** | morphic | B.1 (Memory) | Policies declarativas com session variables para multi-tenancy seguro |
| **Durable Memory > Conversational** | hallmark | B.1 | Artefatos externos (stamps, specs) como fonte de verdade que sobrevive compactação |
| **Memory Inbox + Tags assíncronas** | qwenpaw | B.1 | Fila de consolidação + tagging automática melhora UX sem sacrificar verificabilidade |
| **Checkpoint Tuple Estruturada** | langgraph | B.1 | `{id, ts, channel_values, versions_seen, pending_writes}`; SQLite WAL ideal |
| **WAL + snapshot com HMAC** | smithtune | B.1 | State machine baseada em arquivos traduzida para Rust com verificação de integridade |
| **Gate Taxonomy (Floor/Reflex/Finish)** | hallmark | B.2 (Verification) | Tiers de qualidade: Floor inegociável, Reflex waivable com evidência, Finish nunca bloqueante |
| **Fresh-Context Reviews** | hallmark | B.2 | Revisão em contexto isolado para evitar viés de confirmação do builder thread |
| **Error classification (phase/stage)** | morphic | B.2 | Classificar erros por fase (prep/generation/tool) e estágio para debugging estruturado |
| **Pipeline stateless por stage** | anydoc | B.3 (SOAR) | Cada stage puramente funcional; state management fica no orquestrador |
| **Error typing discriminado para routing** | anydoc | B.3 | `ConvertError.code()` estável permite SOAR rotear falhas por tipo automaticamente |
| **Middleware Stack Ordenado** | deepagents | B.3 | Hooks explícitos (before_model, after_tool) em vez de lógica monolítica |
| **Tool output projection** | morphic | B.3 | Trim tool results antes de enviar ao LLM; remover metadata e conteúdo UI-only |

#### Fase C: Cockpit + Live-Comm + Bot System

| Padrão Externo | Origem | Aplicação na Task | Justificativa |
|----------------|--------|-------------------|---------------|
| **OTel span propagation** | morphic, macro | C.1 (Cockpit), C.2 (Live-Comm) | Propagar trace context entre crates via OTel baggage; correlacionar agent turn → sandbox → proof |
| **Stream error diagnostics** | morphic | C.1 | Metadata estruturado em spans de erro: phase, stage, cancelled flag |
| **Streaming via Transformadores** | langgraph | C.2 | Separar stream bruto de projeções ergonômicas; modos `values`, `updates`, `events` |
| **Event-driven architecture** | browser-use | C.3 (Bot System) | Substituir polling por EventBus + Watchdogs com lifecycle próprio |
| **Loop detection como primitiva** | browser-use | C.3 | ActionLoopDetector com hash normalizado + page/state fingerprinting |
| **Agent Brain estruturado** | browser-use | C.3 | Campos obrigatórios: `thinking`, `evaluation_previous_goal`, `memory`, `next_goal` |
| **Channel abstraction unificada** | qwenpaw | C.2, C.3 | Um agente, N transports sob trait comum `channel_protocol` |
| **Factory Pattern para Bots** | lifecycle-workshop | C.3 | `create_<bot>_agent()` com injeção de tools, state schema e model config |

#### Fase D: HITL + Task Durability + Sentinel

| Padrão Externo | Origem | Aplicação na Task | Justificativa |
|----------------|--------|-------------------|---------------|
| **Interrupt/Resume como Primitiva** | langgraph, lifecycle-workshop | D.1 (HITL) | `interrupt()` serializa estado, `Command(resume=...)` retoma; essencial para approval workflows |
| **SuspendedPermission pattern** | qwenpaw | D.1 | Permissões como dados serializáveis no SOAR, não callbacks |
| **Opt-in explícito para ações externas** | anydoc | D.1 | HITL deve exigir consentimento explícito antes de enviar contexto humano |
| **NeedsOcr como template HITL** | anydoc | D.1 | Não pedir "help genérico"; especificar gap preciso e deixar caller decidir |
| **Usage budget + idempotency** | morphic | D.2 (Task Durability) | Sistema de créditos/budget com attempt IDs e refund handlers |
| **Receipt-based resume** | smithtune | D.2 | Progresso granular permite recovery sem reprocessamento total |
| **Seeded Randomness determinística** | hallmark | D.3 (Sentinel) | Hash + salt garante variedade controlada reproduzível para debug |

#### Fase E: Integração Final + Produção

| Padrão Externo | Origem | Aplicação na Task | Justificativa |
|----------------|--------|-------------------|---------------|
| **Service Locator com Factory + DI** | langflow | E.1 (Orquestrador) | Trait objects + registry para gateway/cockpit; startup sequencial com gating |
| **Índice Pré-Construído de Componentes** | langflow | E.1 | Skills registry com índice serializado (bincode/postcard) para startup <5ms |
| **Hot-reload de configuração** | langgraph | E.1 | Definição declarativa (YAML/TOML) de grafos para hot-reload sem recompilação |
| **Seed scenarios para E2E** | macro | E.2 (Testes E2E) | `seed/scenarios/` com mundos sintéticos para testing do cockpit e daemon |
| **MCP server nativo** | macro | E.3 (Integração Hermes) | Expor skills/memory/hitl via MCP HTTP; Claude/Codex usam Zylos como tool surface |
| **Regras ast-grep como CI gate** | macro | E.2 | Enforçar boundaries hexagonais; impedir que domain importe wasmtime/bollard/reqwest |
| **Inference contract hash SHA256** | smithtune | E.7 (Deploy) | Bot em produção usa exatamente ferramentas validadas; hash por tool schema |
| **Memory synthesis cron** | macro | E.3 | Consolidação noturna team-wide de signals SOAR/HITL/verification em memória estruturada |
| **Runnable Pipeline (map_err + fallback)** | langchain-rust | E.1 | Composição funcional de stages com error conversion trait; evita unwrap em pipelines |
| **Provider abstraction com retry/backoff** | langchain-nvidia | E.3 | NIM client pattern: typed errors, automatic retry com jitter, model fallback chain |
| **CLI config precedence (flag > env > file)** | langsmith-cli | E.1 | Cobra-style layered config; essential para zylos-cli e gateway multi-env |
| **Async factory pattern (SWE-agent)** | open-swe | E.1 | Agent creation como async fn retornando Result<Agent>; permite lazy init de tools pesadas |
| **Research graph com reflection node** | open-deep-research | E.2 | Self-critique step antes de finalizar; melhora qualidade de outputs longos |
| **SqliteSaver checkpointing** | openwiki | E.1 | Checkpoint serialization format compatível; migration path de Python→Rust |
| **QuickJS como lightweight sandbox** | quickjs-wasmtime | B.3, E.2 | Alternativa a wasmtime para skills JS simples; 10x menor binary size, cold start <5ms |
| **Time-travel debugging via log replay** | rerun | E.2, D.3 | Record all events → replay with different parameters; essencial para debug de HITL e sentinel |
| **Crash handler com state snapshot** | rerun | E.1 | Capturar full agent state on panic; post-mortem automático sem reprodução manual |
| **Multi-modal timeline visualization** | rerun | C.1 | Cockpit pode mostrar agent reasoning como timeline interativa; não apenas texto |

### 9.2 Padrões Transversais (Aplicam-se a Múltiplas Fases)

| Padrão | Origens | Escopo | Implementação |
|--------|---------|--------|---------------|
| **OpenTelemetry nativo** | morphic, macro, qwenpaw, langchainjs | Todas as fases | Tracing estruturado desde o início; não repetir erro Langfuse-only |
| **Serialização binária (MessagePack/CBOR)** | langgraph | Checkpoints, IPC | SQLite armazena blobs eficientemente; reduz overhead vs JSON |
| **Sensitive data redaction integrada** | browser-use | Memory, Live-Comm, Logs | Redaction automática em todos os pontos de saída; domain-scoped credentials |
| **Safety limits hard-coded** | anydoc | Sandbox WASM/Docker | Caps não-configuráveis para memory, execution time, I/O |
| **Delegação via Tool (não grafo hardcoded)** | deepagents, lifecycle-workshop | SOAR, Orquestrador | Subagentes como tools chamáveis; mais flexível que edges fixas |
| **Provider Profiles** | deepagents | Gateway | Ajustar prompts/tool-surface por modelo (Claude vs GPT vs local) |
| **Benchmarks contínuos no CI** | deepagents | Todas as fases | Threshold de regressão automático com Criterion |

### 9.3 Anti-Padrões Identificados (EVITAR)

| Anti-Padrão | Origem da Lição | Mitigação no Zylos-Next |
|-------------|-----------------|------------------------|
| Over-abstraction prematura (3566-line base class) | langchainjs | Composition over inheritance (Rust natural); traits enxutos |
| Dependência de LLM como juiz em hot path | smithtune | Usar apenas para auditoria offline; gates determinísticos em runtime |
| Observabilidade single-vendor | qwenpaw | OTel nativo desde o início; exportadores intercambiáveis |
| Workflow implícito / grafo hardcoded | qwenpaw, deepagents | SOAR como orquestração explícita e verificável |
| Python runtime dependency | qwenpaw | Rust puro; extrair padrões, não código |
| File locks globais | smithtune | Sharding ou MVCC para concorrência real |
| Legacy key-value memory simples | langchainjs | Começar direto com checkpointing/state persistence |
| Dual schema version support | langchainjs | Escolher uma versão e manter; breaking changes aceitáveis em ecossistema jovem |
| Polling em vez de event-driven | browser-use | EventBus + Watchdogs como padrão default |

### 9.4 Referências Cruzadas

Todos os 31 relatórios completos estão indexados em [`REVERSE_ENGINEERING_INDEX.md`](../REVERSE_ENGINEERING_INDEX.md). Para detalhes de implementação de cada padrão, consultar o relatório correspondente:

- **Arquitetura/Orquestração:** langgraph, langflow, lifecycle-workshop, deepagents
- **Memória/Estado:** morphic, hallmark, smithtune, browser-use
- **HITL/Aprovações:** qwenpaw, anydoc, langgraph
- **Sandbox/WASM:** anydoc, quickjs-rs
- **Observabilidade:** macro, morphic, langchainjs
- **UI/Cockpit:** assistant-ui, morphic, rerun
- **Segurança/Gates:** hallmark, qwenpaw, smithtune

---

*Seção adicionada em 2026-11-04. Integra 13 relatórios com lições aplicáveis extraídas em tarefas anteriores.*

*Documento gerado automaticamente em 2026-11-04. Última atualização: consolidação final de gaps Fase 7C + Interface + Integração + Padrões Externos.*