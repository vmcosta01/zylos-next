# Gap Analysis: Master Plan Expandido vs Implementação Atual
**Data:** 2026-11-03  
**Base:** zylos-next-master-plan.md (2987 linhas, expandido com REA)  
**Repositório:** C:/Users/Paulo/zylos-next  

---

## Resumo Executivo

| Métrica | Valor |
|---------|-------|
| **Crates existentes** | 24 crates com Cargo.toml |
| **Linhas de código Rust** | ~30.000+ linhas em src/ |
| **Testes unitários passando** | 630 testes, 0 falhas |
| **Fases concluídas** | 0-4 completas, 5-6 parciais, 7B/7C em andamento |
| **Gaps críticos identificados** | 18 subtasks não implementadas ou incompletas |

---

## Status por Crate (Implementação vs Plano)

### ✅ Crates Maduros (>80% do plano)

| Crate | Linhas | Testes | Status | Notas |
|-------|--------|--------|--------|-------|
| `zylos-live-comm` | 6.859 | 157 | ✅ Maduro | Módulos equivalence_approval (957L), triage (866L), sentinel (815L), task_durability (723L), approval (614L) presentes |
| `zylos-skills` | 4.117 | 93 | ✅ Maduro | Registry, eval, export, schema, debt, glossary, scheduler |
| `zylos-soar` | 3.501 | 48 | ✅ Maduro | Orchestrator, executor, playbook, router, triage, audit, webhook |
| `zylos-memory-system` | 1.952 | 34 | ✅ Maduro | VectorStore, context_graph, rerank, types |
| `zylos-bot-system` | 1.404 | 21 | ✅ Maduro | Bot lifecycle management |
| `zylos-browser` | 1.506 | 25 | ✅ Maduro | Browser automation + anti-detect |
| `zylos-verification` | 1.221 | 42 | ✅ Maduro | Judge, proof integration |
| `zylos-task-durability` | 1.154 | 16 | ✅ Maduro | Lease SQL, LostLeaseError, TaskStatus state machine, backoff, circuit breaker |

### ⚠️ Crates Parciais (30-80% do plano)

| Crate | Linhas | Testes | Gaps Identificados |
|-------|--------|--------|-------------------|
| `zylos-proof` | 529 | 17 | Falta: `peso_de()` com clamp, `leitura_bool()`, `normalizar_lista()` com teto MAX_ITENS, `campo_ex()` com proteção, `decide_delivery()`, `build_report()` com truncamento honesto, `checar_completude()` para screenshots |
| `zylos-resolver` | 283 | 5 | Falta: `resolve()` com fronteira BFS completa, checkpoint por onda via persist_path, paralelismo opt-in max_parallel, `glue()` entre claims, resume com anti-forja, struct `corrida` para validação temporal |
| `zylos-panel` | 227 | 4 | Falta: `guardrail_deterministico()` com curto-circuito, detector de falso-sucesso, `filtrar_auto_julgamento()`, `aggregate_votes()` com refutação aterrada, `vote_confidence()`, lentes canônicas completas |
| `zylos-sandbox` | 736 | 13 | WASM + Docker funcionais; falta browser anti-detect rotação fingerprint e VLM OCR sidecar |
| `zylos-vlm-ocr` | 711 | 17 | Estrutura presente; falta integração com wasmtime PagedAttention e batching INT8 |
| `zylos-triage` | 483 | 18 | Two-stage presente; falta framing pattern, initiator tracking completo, eval measurement contra corpus rotulado |
| `zylos-sentinel` | 413 | 17 | Detectors presentes; falta normalized signatures dedup, privacy-first hashing, team incident reporting com rate limit |
| `zylos-hitl` | 234 | 12 | Outcome tri-state presente; falta approval cards via AG-UI, equivalence-based hashing, four-tier decision, carry_on multi-decision, withdraw_allowed, abandon, execution_authority via task_local |

### ❌ Crates Stub (<30% do plano)

| Crate | Linhas | Testes | Status |
|-------|--------|--------|--------|
| `zylos-cerebelo` | 218 | 6 | Shadow-git não implementado; audit chain SHA256 ausente; test-ratchet ausente; rollback escopado ausente; CLI historia/rewind/fork ausente |
| `zylos-learn` | 236 | 5 | `record_outcome()` básico; falta append_pending JSONL, assinatura normalizada, gate recorrência ≥3×, witness anti-regressão determinístico |
| `zylos-router` | 206 | 6 | `route()` básico; falta fallback general-purpose, `is_real()` garantia dura, `propose_change()` com requires_human, alvos sagrados, desempate por track-record, `route_com_proposta()` com cluster fallbacks |
| `zylos-mobile` | 246 | 5 | Estrutura básica apenas; multi-device sync não implementado |
| `zylos-core` | 1 | 0 | **VAZIO** — placeholder sem implementação |
| `zylos-docker` | 478 | 8 | Docker sandbox funcional mas isolado; falta integração com task-durability lease |
| `zylos-search-mcp` | 464 | 8 | MCP search funcional; falta integração com soar registry |

---

## Gaps Críticos por Fase do Master Plan Expandido

### Fase 7B: Padrões CORTEX (Semanas 39-42)

#### Sprint 7B.1: Proof Engine + Resolver DAG

**Task 7B.1.1 — zylos-proof (40h estimadas)**
- ✅ Ground enum, Evidence struct, ProofResult, ProofBuilder, VerificationGate — IMPLEMENTADO
- ❌ `peso_de()` com clamp 1-5 e detecção stakes malformado — NÃO IMPLEMENTADO
- ❌ `leitura_bool()` com suporte sim/não/true/false/1/0/ininteligível — NÃO IMPLEMENTADO
- ❌ `normalizar_lista()` com teto MAX_ITENS=1000 e contagem omitidos — NÃO IMPLEMENTADO
- ❌ `campo_ex()` com proteção contra getters hostis (try/catch → undefined) — NÃO IMPLEMENTADO
- ❌ `decide_delivery()` com lógica achado negativo bloqueante — NÃO IMPLEMENTADO
- ❌ `build_report()` com truncamento honesto e métricas custo — NÃO IMPLEMENTADO
- ❌ `checar_completude()` para screenshot verification — NÃO IMPLEMENTADO
- ⚠️ Testes: 17 existentes, plano exige 30+ cobrindo edge cases

**Task 7B.1.2 — zylos-resolver (32h estimadas)**
- ✅ BfsFrontier, DagNode, DagGraph, ResolverCheckpoint structs — IMPLEMENTADO
- ❌ `resolve()` completo com fronteira BFS e limites max_depth/max_nodes — PARCIAL
- ❌ Checkpoint por onda via persist_path (JSON atômico tmp+rename) — NÃO IMPLEMENTADO
- ❌ Paralelismo opt-in max_parallel com ordem preservada — NÃO IMPLEMENTADO
- ❌ `glue()` prova da cola entre claims de sub-objetivos — NÃO IMPLEMENTADO
- ❌ Resume com anti-forja (arquivo ausente/corrompido → recomeça) — NÃO IMPLEMENTADO
- ❌ Struct `corrida` para validação temporal screenshots — NÃO IMPLEMENTADO

**Task 7B.1.3 — zylos-panel (24h estimadas)**
- ✅ AdversarialPanel, JudgeVote, PanelConfig, PanelDecision — IMPLEMENTADO
- ❌ `guardrail_deterministico()` com curto-circuito resultado óbvio-ruim — NÃO IMPLEMENTADO
- ❌ Detector de falso-sucesso (encerramento sem vestígio) — NÃO IMPLEMENTADO
- ❌ `filtrar_auto_julgamento()` (juiz ≠ gerador estrutural) — NÃO IMPLEMENTADO
- ❌ `aggregate_votes()` onde 1 refutação aterrada derruba maioria — NÃO IMPLEMENTADO
- ❌ `vote_confidence()` alta/média/baixa baseada em consenso — NÃO IMPLEMENTADO
- ❌ Lentes canônicas completas (funcionalidade, estrutura, performance, segurança, simplicidade, testes) — PARCIAL

#### Sprint 7B.2: Roteamento + Cerebelo + Shadow-Git

**Task 7B.2.1 — zylos-router (24h estimadas)**
- ✅ Route básico com tokenização — IMPLEMENTADO
- ❌ Fallback general-purpose quando score=0 — NÃO IMPLEMENTADO
- ❌ `is_real()` garantia dura nunca inventa capacidade — NÃO IMPLEMENTADO
- ❌ `propose_change()` com requires_human=true sempre — NÃO IMPLEMENTADO
- ❌ Alvos sagrados (freios/verificadores) com gate reforçado — NÃO IMPLEMENTADO
- ❌ Desempate por track-record medido (feature flag opt-in) — NÃO IMPLEMENTADO
- ❌ `route_com_proposta()` com cluster fallbacks + dedup persistente — NÃO IMPLEMENTADO

**Task 7B.2.2 — zylos-cerebelo (32h estimadas)**
- ❌ Shadow-git commits em repo isolado — NÃO IMPLEMENTADO
- ❌ Audit chain SHA256 encadeada (prev_hash + integrity_hash) — NÃO IMPLEMENTADO
- ❌ Test-ratchet snapshot força de teste — NÃO IMPLEMENTADO
- ❌ Rollback escopado só arquivos tracked — NÃO IMPLEMENTADO
- ❌ Exclusão padrões sensíveis (.env, chaves) — NÃO IMPLEMENTADO
- ❌ CLI historia/rewind/fork — NÃO IMPLEMENTADO

**Task 7B.2.3 — zylos-learn (16h estimadas)**
- ✅ `record_outcome()` básico — IMPLEMENTADO
- ❌ `append_pending()` em JSONL append-only — NÃO IMPLEMENTADO
- ❌ Assinatura normalizada para agrupamento recorrência — NÃO IMPLEMENTADO
- ❌ Gate recorrência ≥3× lição madura — NÃO IMPLEMENTADO
- ❌ Witness anti-regressão marcador-em-arquivo + check determinístico — NÃO IMPLEMENTADO

### Fase 7C: Padrões Externos HITL/Durability/Triage (Semanas 43-46)

#### Sprint 7C.1: HITL Approval + Task Durability

**Task 7C.1.1 — HITL Approval Cards (48h estimadas)**
- ✅ Outcome enum, classify_http_status, NotificationDedupKey — IMPLEMENTADO em zylos-hitl
- ✅ equivalence_approval.rs (957 linhas) em zylos-live-comm — IMPLEMENTADO
- ✅ approval.rs (614 linhas) em zylos-live-comm — IMPLEMENTADO
- ✅ ApprovalRequest struct com thread_id + tool_call_id + TTL 60min — IMPLEMENTADO (approval.rs:56-94)
- ✅ Storage server-side in-memory com Arc<RwLock<HashMap>> — IMPLEMENTADO (approval.rs:117)
- ✅ Claim atômico CAS prevent double-approve — IMPLEMENTADO (approval.rs:164-197)
- ✅ DecisionTier enum (Safety > TeamRule > PersonalRule > AutoReview > Default) — IMPLEMENTADO (equivalence_approval.rs:90-110)
- ✅ task_local! CONTINUATION_CTX para captura de continuação — IMPLEMENTADO (equivalence_approval.rs:258-270)
- ✅ stable() strip voláteis para hashing determinístico — IMPLEMENTADO (equivalence_approval.rs:27-55)
- ⚠️ Hash usa SHA-256, NÃO blake3 como especificado no master plan — DIVERGÊNCIA (aceitável se documentado)
- ❌ carry_on() multi-decision step — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ withdraw_allowed() selective close — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ abandon() resultado sintético desbloqueio — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ is_permanent(error) detection — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ public_approval() strip internos — **NÃO IMPLEMENTADO**
- ✅ 7 mensagens recusa distintas — IMPLEMENTADO (RefusalReason: DeclinedByPerson, AlreadyWaiting, SafetyBlock, TeamPolicyBlock, RateLimited, NotAuthorized, Expired)
- ✅ Secret masking via mask_secrets() com secret_fields config — IMPLEMENTADO (approval.rs:326, testes em :508-522)
- ✅ Idempotent result recovery com cached decision — IMPLEMENTADO (approval.rs:259, testes em :527-543)
- ❌ validation_probe task-local — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ Exclusão approval-gated tools headless/Slack no fluxo de approval — **NÃO IMPLEMENTADO** (headless existe em zylos-browser mas não integrado ao approval gate)
- ❌ carry_on() multi-decision step — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ withdraw_allowed() selective close — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ abandon() resultado sintético desbloqueio — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ is_permanent(error) detection — **NÃO IMPLEMENTADO** (grep global zero matches)
- ❌ public_approval() strip internos — **NÃO IMPLEMENTADO**

**Task 7C.1.2 — Task Durability SQL Leases (40h estimadas)**
- ✅ TaskStatus enum com can_transition_to — IMPLEMENTADO
- ✅ LostLeaseError — IMPLEMENTADO
- ✅ Lease struct, Attempts, TaskRecord, TaskAnswer — IMPLEMENTADO
- ✅ TaskDurabilityEngine com new(), compute_backoff(), reset_circuit_breaker() — IMPLEMENTADO
- ✅ MetricsSnapshot, ControlAction — IMPLEMENTADO
- ⚠️ Verificar profundidade: compare-and-swap SQL, maintenance loop 60s, database-as-clock, heartbeat lease_ms/3, guard/checkpoint/event trio, run() com tabela runs, answer() histórico completo, select for update skip locked, pg_notify inside transaction, WorkItem.attempts semantics, exponential backoff jitter, circuit breaker 5 falhas, 100-task active cap, control() CAS precondition

**Task 7C.1.3 — Outcome Uncertainty Tri-State (12h estimadas)**
- ✅ Outcome enum Succeeded/Failed/OutcomeUnknown — IMPLEMENTADO em zylos-hitl
- ✅ classify_http_status, classify_network_error, classify_validation_error — IMPLEMENTADO
- ❌ Retry policy OutcomeUnknown → confirmação humana — VERIFICAR
- ❌ Audit log transição outcome com timestamp — VERIFICAR
- ❌ Notification dedup key evitar alertas duplicados — IMPLEMENTADO (NotificationDedupKey)

#### Sprint 7C.2: Triage Proativo + Sentinel

**Task 7C.2.1 — Two-Stage Triage (20h estimadas)**
- ✅ triage.rs (866 linhas) em zylos-live-comm — IMPLEMENTADO
- ✅ zylos-triage crate (483 linhas, 18 testes) — IMPLEMENTADO
- ✅ Campo decided_by "rule:<id>" | "model:<name>" — IMPLEMENTADO (triage.rs:101)
- ✅ Framing pattern wrap_scheduled() + wrap_handoff() — IMPLEMENTADO (triage.rs:182-196)
- ✅ Initiator tracking enum (Person/Routine/Responsibility/Memory/Handoff) — IMPLEMENTADO (triage.rs:35-39)
- ❌ Stage 1 rules-only safe negatives — VERIFICAR PROFUNDIDADE
- ❌ Stage 2 model fallback prompt estruturado + Zod parsing — VERIFICAR
- ❌ Eval measurement contra corpus rotulado — **NÃO IMPLEMENTADO**

**Task 7C.2.2 — Equivalence-Based Approval Hashing (36h estimadas)**
- ✅ equivalence_approval.rs (957 linhas) — IMPLEMENTADO
- ✅ stable() strip voláteis para hashing determinístico — IMPLEMENTADO (equivalence_approval.rs:27-55)
- ✅ DecisionTier four-tier (Safety > TeamRule > PersonalRule > AutoReview > Default) — IMPLEMENTADO (equivalence_approval.rs:90-110)
- ✅ 7 RefusalReason variantes — IMPLEMENTADO (equivalence_approval.rs:131-148)
- ✅ Continuation capture via task_local! CONTINUATION_CTX — IMPLEMENTADO (equivalence_approval.rs:258-270)
- ⚠️ Hash usa SHA-256, NÃO blake3 como especificado no master plan — DIVERGÊNCIA
- ❌ carry_on() step_calls/step_continuation — **NÃO IMPLEMENTADO**
- ❌ withdraw_allowed() selective close — **NÃO IMPLEMENTADO**
- ❌ abandon() synthetic result — **NÃO IMPLEMENTADO**
- ❌ is_permanent() error types — **NÃO IMPLEMENTADO**
- ❌ public_approval() preview whole — **NÃO IMPLEMENTADO**
- ❌ Escalation ungated tool — VERIFICAR
- ❌ WASM policy engine wasmtime 25.0 — VERIFICAR

**Task 7C.2.3 — Sentinel Health Scanner (20h estimadas)**
- ✅ sentinel.rs (815 linhas) em zylos-live-comm — IMPLEMENTADO
- ✅ zylos-sentinel crate (413 linhas, 17 testes) — IMPLEMENTADO
- ✅ Detectors pure functions sobre stores read-only — IMPLEMENTADO (sentinel.rs:45, 281+)
- ✅ Normalized signatures normalize_signature() com SHA-256 dedup — IMPLEMENTADO (sentinel.rs:269)
- ✅ Privacy-first PII stripping strip_pii() — IMPLEMENTADO (sentinel.rs:81)
- ❌ Output separado fora data dir principal — VERIFICAR
- ❌ Scheduling tokio interval 120s — VERIFICAR
- ❌ Issue upsert signature JSONL append-only — VERIFICAR
- ❌ Team incident reporting rate limit 5/hour mute — VERIFICAR

---

## Priorização de Trabalho (Ordem de Execução)

### 🔴 Prioridade 1: Completar Fase 7B (CORTEX Patterns)

1. **zylos-proof** — Implementar 6 funções faltantes (peso_de, leitura_bool, normalizar_lista, campo_ex, decide_delivery, build_report, checar_completude) + expandir testes para 30+
2. **zylos-resolver** — Implementar resolve() completo, checkpoint persist_path, paralelismo opt-in, glue(), resume anti-forja, struct corrida
3. **zylos-panel** — Implementar guardrail_deterministico, falso-sucesso detector, filtrar_auto_julgamento, aggregate_votes, vote_confidence, lentes canônicas
4. **zylos-cerebelo** — Implementar shadow-git completo, audit chain SHA256, test-ratchet, rollback escopado, CLI
5. **zylos-router** — Implementar fallback, is_real(), propose_change(), alvos sagrados, desempate track-record, route_com_proposta()
6. **zylos-learn** — Implementar append_pending JSONL, assinatura normalizada, gate recorrência, witness anti-regressão

### 🟡 Prioridade 2: Completar Fase 7C (External Patterns)

7. **zylos-hitl + live-comm/approval** — Implementar carry_on(), withdraw_allowed(), abandon(), is_permanent(), public_approval(), validation_probe task-local, exclusão headless/Slack no approval gate
8. **zylos-task-durability** — Verificar e completar CAS SQL, database-as-clock, heartbeat lease_ms/3, guard/checkpoint/event trio, pg_notify inside transaction, select for update skip locked, 100-task active cap
9. **zylos-triage** — Completar Stage 1 rules-only safe negatives, Stage 2 model fallback + Zod parsing, eval measurement contra corpus rotulado
10. **zylos-sentinel** — Verificar output separado fora data dir, scheduling tokio interval 120s, issue upsert JSONL append-only, team incident rate limit 5/hour mute

### 🟢 Prioridade 3: Integração e Validação

11. **zylos-core** — Sair de 1 linha para tipos compartilhados entre crates
12. **Dupla auditoria** — Criar evals/audits/fase7b-audit-adversarial.md + fase7b-audit-isolated.md + fase7c equivalents
13. **Integração E2E** — Validar petgraph DAG da Fase 3 com resolver/proof/panel
14. **Integração SOAR gateway** — Validar zylos-soar com hitl/task-durability/triage

---

## Estimativa de Esforço Restante

| Fase | Horas Estimadas (Plano) | % Implementado | Horas Restantes |
|------|------------------------|----------------|-----------------|
| 7B Sprint 7B.1 | 96h | ~35% | ~62h |
| 7B Sprint 7B.2 | 72h | ~10% | ~65h |
| 7C Sprint 7C.1 | 100h | ~75% | ~25h |
| 7C Sprint 7C.2 | 76h | ~65% | ~27h |
| **Total** | **344h** | **~55%** | **~179h** |

---

## Recomendações Imediatas

1. **Verificar profundidade real** dos módulos em zylos-live-comm (equivalence_approval, approval, task_durability, triage, sentinel) — podem estar mais completos que os crates standalone
2. **zylos-core precisa de conteúdo** — atualmente vazio (1 linha), deveria conter tipos compartilhados
3. **Auditorias duplas obrigatórias** — cada fase exige dois audits (adversarial + isolated) antes de avançar
4. **Integração cross-crate** — muitos crates existem isoladamente mas não foram integrados entre si
5. **Testes insuficientes** em crates 7B — proof (17/30), resolver (5/?), panel (4/?) abaixo do mínimo exigido

---

*Gerado automaticamente por gap analysis comparativo entre zylos-next-master-plan.md (v6 expandido REA) e repositório zylos-next em 2026-11-03.*