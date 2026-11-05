# Task Board — Zylos-Next v6.0

## Next (Próximas Tasks Prioritárias)
- **Fase 7C (remaining)**: Dupla Auditoria (adversarial + isolated docs) + E2E integration com SOAR gateway ✅ código completo (AG-UI types, HITL, leases, triage, equivalence, sentinel — 177 testes live-comm passando)
- **Sprint 6.2 Cockpit**: SSR/Hydration, PWA offline-first, i18n completo, streaming AG-UI integration (2.426 LOC base existe; faltam critérios de aceite de performance)
- **Fase 7**: End-to-End Integration (integration testing, security audit final, release v1.0)

## In Progress (Máximo 3 Simultâneas)
- (Nenhuma task em andamento)

## Completed
- [x] **Fase 7C (implementable tasks)**: Outcome Uncertainty Tri-State + Two-Stage Message Triage + Equivalence-Based Approval Hashing + Sentinel Health Scanner ✅ (74 novos testes: 18 approval_hash + 20 triage_message + 15 outcome + 21 sentinel, todos passando, blake3 dep adicionado)
- [x] **Task 7C.2.3**: Sentinel Health Scanner ✅ (já completo, 21 testes passando)
- [x] **Task 7C.2.2**: Equivalence-Based Approval Hashing ✅ (blake3 digest, PolicyTierEvaluator, ExecutionAuthority task-local, 18 testes passando)
- [x] **Task 7C.2.1**: Two-Stage Message Triage ✅ (classify_message + route_message, 20 testes passando)
- [x] **Task 7C.1.3**: Outcome Uncertainty Tri-State ✅ (Succeeded/Failed/Unknown, classify_error, needs_human_confirmation, 15 testes passando)
- [x] **Fase 6 (LIVE_COMMUNICATION)**: API Gateway Axum + Cockpit Leptos WASM + Security OWASP L2 + Cache + PWA + Multi-Adapter Export ✅ (60/60 testes, clippy limpo, dupla auditoria APPROVED, HANDOFF_6.md criado)
- [x] **Fase 5 (SKILLS_REGISTRY)**: Registry + Schema + Eval Flywheel + Debt Tracking + Glossary + Scheduler + Skills API + Migração 173 skills ✅ (107/107 testes: 86 skills + 21 live-comm, clippy limpo, dupla auditoria APPROVED, runbook criado)
- [x] **Task 5.3.3**: Migrar 173 skills Hermes → zylos-skills registry ✅ (index.json 74KB, script migrate_skills.py)
- [x] **Task 5.3.2**: Skills Registry API routes ✅ (GET /skills, GET /skills/:id, POST /eval/run — 6/6 testes passando)
- [x] **Fase 3 (LIVE_COMM_SYSTEM)**: EventBus + AgUiEvent (8 variants) + SSE routes + CockpitApi + HermesBridge + Mobile MCP server ✅ (20/20 testes: 15 live-comm + 5 mobile, clippy limpo, benchmark publish_heartbeat 255ns)
- [x] **Auditoria Dupla Fase 2**: Auditoria adversarial + verificação isolada ✅ (APPROVED em evals/audits/fase2-audit-adversarial-minimax.md)
- [x] **Task 4.3.1**: VLM OCR Sidecar ✅ (SVG parser 17/17 testes passando, runbook criado)
- [x] **Runbooks Fases 0-4 + VLM-OCR**: 6 runbooks com estrutura 3 níveis (Senior/Pleno/Júnior) completos ✅
- [x] **Fase 4 (SANDBOX_ISOLATION)**: WasmSandbox + DockerSandbox + SearchMCP ✅ (21/21 testes: 13 sandbox + 8 search, clippy limpo, auditorias adversarial e isolada APPROVED)
- [x] **Task 4.2.3**: Parallel Search MCP ✅ (fan-out paralelo, dedup por URL, tolerância a falha parcial, 8/8 testes passando, runbook criado)
- [x] **Runbooks Fase 4**: Criar runbooks operacionais para Browser Anti-Detect e WebMCP Compliance ✅ (`runbooks/browser-headless-anti-detect.md`, `runbooks/webmcp-spec-compliance.md`, `runbooks/parallel-search-mcp.md`)
- [x] **Task 4.2.2**: WebMCP Spec Compliance ✅ (eval suite 8 checks ≥95%, polyfill seguro sem eval/Function/innerHTML, 6/6 testes passando)
- [x] **Task 4.2.1**: Browser Headless Anti-Detect ✅ (fingerprint determinístico, inbox allow/ask/deny, tracker blocking, 7/7 testes passando)
- [x] **Fase 2 (MEMORY_SYSTEM)**: 28/28 testes passando, crate zylos-memory-system completo ✅
- [x] **Task 2.1.3**: Implementar `BriefAwareReranker` no zylos-memory-system ✅ (10 testes, rerank contextual/recency/semantic/keyword)
- [x] **Task 2.1.2**: Implementar `VectorStore` trait no zylos-memory-system ✅ (9 testes, InMemory backend/cosine similarity/metadata filter)
- [x] **Task 2.1.1**: Implementar `ContextGraph` no zylos-memory-system ✅ (8 testes, SQLite WAL/petgraph/query/embedding/persistence)
- [x] **Task 1.1.1**: Implementar `LifecycleManager` no zylos-bot-system ✅ (4 testes, spawn/kill/shutdown/list)
- [x] **Task 1.1.2**: Implementar `DAGExecutor` no zylos-bot-system ✅ (7 testes, topological sort/cycle detection/retry/skip/cancel)
- [x] **Task 1.1.3**: Implementar `P2PMessenger` no zylos-bot-system ✅ (10 testes, pub/sub/DLQ/TTL/ack/multi-subscriber)
- [x] **Fase 1 (BOT_SYSTEM)**: 21/21 testes passando, crate zylos-bot-system completo
- [x] **Master Plan v3.0**: Integrado com manifesto, 22 conceitos mapeados
- [x] **Relatório Comparativo v6**: 2.686 repos filtrados, decisão "construir em Rust" validada
- [x] **Toolchain Setup**: wasm-pack 0.15.0, wasmtime 50.0.0, sqlite3 3.47.0 instalados
- [x] **Rust Toolchain**: stable 1.99.1 com clippy + rustfmt funcionais

## Blocked
- (Nenhuma task bloqueada atualmente)

## Backlog (Futuro)
- [x] **Fase 3 Sprint 3.2**: AdversarialPanel (judge.rs, 10 testes, veto_on_refute, min_agreement, to_proof) ✅
- [x] **Fase 3 Benchmarks**: VectorStore similarity search (56ms/10k ✅) + BriefAwareReranker (10.3ms/1k ✅) completos

- [ ] **Fase 7**: End-to-End Integration (integration testing, security audit final, release v1.0)

## Regras do Board
- **In Progress**: Máximo 3 tasks simultâneas por agente
- **Next**: Ordenado por prioridade (dependências primeiro)
- **Completed**: Movido apenas após verify:<cmd> passar
- **Blocked**: Requer motivo explícito + link para blocker
- **Backlog**: Revisado semanalmente, promovido para Next conforme capacidade

## Atualização Contínua (Conceito #17)
Este arquivo DEVE ser atualizado ao final de cada sessão de trabalho. Estado stale = trabalho inválido.