# Handoff: Fase 8 → Fase 9 (Cockpit v2)

**Date:** 2026-11-11
**Author:** Hermes Agent

## Executive Summary

Fase 8 foi concluída com sucesso: 29/29 tasks implementadas, 463 novos testes passando, 7 novos módulos Rust criados (swarm-orchestrator enhancements, bot-system, memory-system, agent-shield, skills, personalities, desktop), cockpit WASM compilando limpo, gateway servindo todos os endpoints na porta 3000. Fase 9 inicia com redesign completo do cockpit baseado em análise profunda de 12 dashboards open-source, priorizando segurança (approval gates, egress checker) e produtividade (morning brief, task triage, multi-model selector).

## Modules Delivered (Fase 8)

| Module | Status | Tests | Notes |
|--------|--------|-------|-------|
| HopCountCircuitBreaker | ✅ Complete | 29 | RAII guard com AtomicU8, CircuitOpen error |
| GoalProgram Struct | ✅ Complete | 10 | TargetSpec, EvaluationCriteria, EvolutionStrategy |
| AutoloopProtocol Async | ✅ Complete | 11 | Ask/Notice/Resolve/Later/Events enum |
| IndependentVerifier | ✅ Complete | 16 | Adversarial verification sem contexto compartilhado |
| CrashSimulator Tests | ✅ Complete | 5 | 5 cenários de falha (spawn, channel, timeout, panic, stress) |
| MemoryLifecycle Trait | ✅ Complete | 10 | RetentionPolicy, ConsolidationWindow, RefreshTrigger |
| MentalModelStore | ✅ Complete | 9 | SQLite + DashMap cache + cosine similarity |
| ContextRoom Struct | ✅ Complete | 9 | Semantic bundling, snapshot/versioning, tenant isolation |
| AutoImprovementLoop | ✅ Complete | 13 | AccessTracker lock-free, ImprovementEvent broadcast |
| ResourceLimiter + Platform | ✅ Complete | 14 | Enum dispatch (Noop/Cgroups/Docker) |
| NetworkEgressFilter | ✅ Complete | 15 | Whitelist per agent, subdomain wildcard, hot-reload |
| OutputSanitizer Pipeline | ✅ Complete | 18 | Regex pipeline, 10 built-in rules, 3 strategies |
| CapabilityToken System | ✅ Complete | 22 | Granular permissions, revocation, GC, prefix wildcard |
| HostAllocatorLimiter | ✅ Complete | 16 | CAS atomic allocation, peak memory monitoring |
| SkillContract Formal | ✅ Complete | 21 | (P,O,A,V,F) structure, Predicate, Assertion |
| SkillEvolutionGraph | ✅ Complete | 17 | petgraph DiGraph, merge/retire/add_validator |
| SkillMiner from Logs | ✅ Complete | 16 | Regex-based log pattern extraction |
| TrajectoryCompiler | ✅ Complete | 11 | HANDOFF + playbook YAML → SkillContract |
| SkillHierarchy 3-Tier | ✅ Complete | 23 | Planning/Functional/Atomic tier filtering |
| EmployeeRole Enum | ✅ Complete | 16 | 8 business roles mapped to presets |
| RoutineCatalog | ✅ Complete | 17 | Indexed catalog by role, validation on load |
| PlanFirstGate | ✅ Complete | 19 | Human approval gate, UUID tiebreaker |
| EmployeeRateLimiter | ✅ Complete | 14 | Token bucket per employee/role |
| GuardrailEnforcer | ✅ Complete | 17 | Declarative rules, priority ordering, async add_rule |
| WorkspacePersistence | ✅ Complete | TBD | JSON state file, debounce save, multi-tab |
| AgentAwarenessState | ✅ Complete | TBD | Enum states, CSS color mapping |
| RelayServer | ✅ Complete | TBD | HTTP/WebSocket relay port 9877, device pairing |
| MobileResponsive | ✅ Complete | TBD | Breakpoint detection, CSS injection |

## Known Issues (Carried Forward)

1. **sqlx SQLite multi-statement limitation:** Sempre separar CREATE TABLE/INDEX em statements individuais. Lição dura da Fase 8 — 4 endpoints retornavam 500 silenciosamente.
2. **chrono_now_stub second resolution:** Timestamps colidem em testes rápidos. Usar UUID lexicographic tiebreaker como padrão (estabelecido em PlanFirstGate).
3. **DashMap v6 API breaking change:** Usar `.key()`/`.value()` accessors, não tuple destructuring.
4. **Gateway binary lock no Windows:** Usar `cmd.exe /C "taskkill /F /IM zylos-gateway.exe"` antes de rebuild — git-bash traduz `//F` incorretamente.
5. **Trunk asset path:** CSS files referenciados em `index.html` devem estar na raiz do crate (não em `static/`), ou usar prefixo explícito.
6. **CORS config:** Gateway deve ter `allowed_origins = ["http://localhost:8080"]` para cockpit funcionar. Verificar após qualquer mudança de config.
7. **Port discovery:** Gateway roda na porta 3000 (não 9876 como documentado em alguns lugares). Cockpit roda na 8080 via trunk serve.

## Decisions Made (Fase 8 → 9 Transition)

1. **Design system baseado em 12 dashboards reais:** Análise profunda de código-fonte (não READMEs) de OpenBot, OpenDots, OpenMuse, Paperclip, Rerun, Ruflo, ECC, AnyModelBot, OpenDuet, OpenMausBot, agency-agents, awesome-grok-bot. Repositórios mantidos em `C:/Users/Paulo/dashboard-analysis/`.
2. **Approval-first como princípio de design:** Toda ação externa exige confirmação humana. Padrão extraído de 5/12 dashboards analisados.
3. **Inline approval cards (não modal):** Mantém contexto conversacional, reduz fricção. ADR-001 documentado.
4. **Egress checker síncrono no frontend:** Defesa em profundidade, latência <5ms aceitável. ADR-002 documentado.
5. **SQLite para Spaces/Documents:** Consistente com stack Zylos, busca full-text futura via FTS5. ADR-004 documentado.
6. **Multi-model per-bot com herança global:** Flexibilidade sem configuração excessiva. ADR-005 documentado.

## Risks for Fase 9

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| VNC proxy complexo demais | Alta | Alto | Fallback para static screenshots + polling |
| Leptos 0.7 breaking changes | Média | Médio | Pin versão exata, testar antes de upgrade |
| sqlx migrations falham | Baixa | Alto | Statements individuais (lição Fase 8) |
| Context window overflow | Alta | Médio | Subagentes para tasks independentes |
| Build times >15min | Média | Médio | Incremental builds, sccache |
| Document compliance drift | Média | Alto | DOCUMENT_COMPLIANCE_FASE9.md como checklist obrigatório |

## Environment State

- **Rust:** 1.82+
- **Workspace:** 41 crates (40 original + zylos-approval novo)
- **Gateway:** Porta 3000, ZYLOS_HERMES_TOKEN=[REDACTED]
- **Cockpit:** Porta 8080 via trunk serve, WASM compila limpo
- **Design tokens:** `apps/zylos-cockpit/design-tokens.css` (669 linhas)
- **Design system doc:** `docs/DESIGN_SYSTEM.md` (286 linhas)
- **Analysis report:** `C:/Users/Paulo/dashboard-analysis/ANALYSIS_REPORT.md`
- **Implementation plan:** `docs/IMPLEMENTATION_PLAN_DETAILED.md` (~1900 linhas)
- **Document compliance:** `docs/DOCUMENT_COMPLIANCE_FASE9.md` (47 artefatos mapeados)
- **Decisions log:** `docs/DECISIONS_FASE9.md` (6 ADRs iniciais)
- **Track record:** `docs/TRACKRECORD_FASE9.md` (iniciado)

## Documentation Index (Created/Updated for Fase 9)

### Planejamentos
- [IMPLEMENTATION_PLAN_DETAILED.md](../IMPLEMENTATION_PLAN_DETAILED.md) — Plano completo com 4 fases, 29 tasks, specs técnicas
- [DOCUMENT_COMPLIANCE_FASE9.md](../DOCUMENT_COMPLIANCE_FASE9.md) — 47 artefatos obrigatórios mapeados
- [ANALYSIS_REPORT.md](../../dashboard-analysis/ANALYSIS_REPORT.md) — Análise de 12 dashboards, 32 gaps identificados

### Decisões e Track Record
- [DECISIONS_FASE9.md](../DECISIONS_FASE9.md) — 6 ADRs iniciais
- [TRACKRECORD_FASE9.md](../TRACKRECORD_FASE9.md) — Log cronológico

### Crate zylos-approval (Task 1.1)
- [README.md](../../crates/zylos-approval/README.md) — Arquitetura, API, schema
- [pb-approval-gate-setup.md](../../playbooks/pb-approval-gate-setup.md) — Playbook de configuração
- [pop-approval-decide.md](../../playbooks/pop-approval-decide.md) — POP operacional
- [rb-zylos-approval.md](../../runbooks/rb-zylos-approval.md) — Runbook técnico
- [soar-approval-timeout.md](../soar/soar-approval-timeout.md) — SOAR workflow

### Referência Externa
- 12 repositórios clonados em `C:/Users/Paulo/dashboard-analysis/`