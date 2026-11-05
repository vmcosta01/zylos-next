# Zylos-Next Roadmap v2.0 (Next 12 Months)

## Vision
Evolve Zylos-Next from a production-ready orchestration platform into an adaptive, self-improving multi-agent ecosystem with ML-driven optimization, cross-platform interoperability, and community-driven extensibility.

## Priority Matrix

| Priority | Feature | Effort | Impact | Quarter |
|----------|---------|--------|--------|---------|
| P0 | Branch coverage enforcement in CI | 2w | High | Q1 |
| P0 | Docker-in-Docker CI stage | 3w | High | Q1 |
| P1 | ML-driven task routing | 8w | Critical | Q1-Q2 |
| P1 | Cross-platform agent protocol (A2A) | 6w | Critical | Q2 |
| P1 | Live migration without drain | 4w | High | Q2 |
| P2 | Plugin marketplace | 10w | Medium | Q2-Q3 |
| P2 | Multi-tenant isolation | 6w | High | Q3 |
| P2 | Observability v2 (OpenTelemetry native) | 4w | Medium | Q3 |
| P3 | WASM component model support | 8w | Medium | Q3-Q4 |
| P3 | Federated memory across instances | 12w | High | Q4 |
| P3 | Natural language playbook authoring | 6w | Medium | Q4 |

## Q1: Quality & Foundation Hardening

### 1. Branch Coverage Enforcement (P0)
- **Goal:** Raise `zylos-live-comm` branch coverage from 68% → 85%
- **Approach:** Add `cargo-tarpaulin` to CI with branch-mode; fail PRs below threshold
- **Owner:** Core team
- **Success metric:** CI blocks merges below 85% branch coverage

### 2. Docker-in-Docker CI Stage (P0)
- **Goal:** Validate sandbox integration in CI, not just local dev
- **Approach:** Add DinD service container to GitHub Actions; run sandbox E2E tests
- **Owner:** DevOps
- **Success metric:** Sandbox tests pass in CI on every PR

### 3. ML-Driven Task Routing (P1)
- **Goal:** Replace static DAG scheduling with learned routing based on historical performance
- **Approach:** Train lightweight gradient-boosted model on task execution traces; integrate as `zylos-router` enhancement
- **Dependencies:** Task trace collection (already in shadow-git)
- **Risk:** Model drift → fallback to static routing with confidence threshold
- **Success metric:** 15% reduction in average task completion time vs static routing

## Q2: Interoperability & Scalability

### 4. Cross-Platform Agent Protocol (A2A) (P1)
- **Goal:** Enable Zylos agents to communicate with non-Zylos agents via A2A standard
- **Approach:** Implement A2A server/client in `zylos-live-comm`; bridge to AG-UI internally
- **Owner:** Integration team
- **Success metric:** Successful interop demo with 2 external agent frameworks

### 5. Live Migration Without Drain (P1)
- **Goal:** Zero-downtime upgrades without request draining pause
- **Approach:** Blue-green deployment with state transfer via shared SQLite WAL + EventBus replay
- **Dependencies:** Multi-instance coordination (see Roadmap v1.5)
- **Success metric:** Upgrade completes with <100ms p99 latency spike

### 6. Plugin Marketplace (P2)
- **Goal:** Community-contributed skills, detectors, and playbooks with verified provenance
- **Approach:** Signed plugin packages, sandboxed loading, reputation system
- **Owner:** Ecosystem team
- **Success metric:** 20+ community plugins published and verified

## Q3: Multi-Tenancy & Observability

### 7. Multi-Tenant Isolation (P2)
- **Goal:** Secure tenant boundaries for SaaS deployment
- **Approach:** Namespace-scoped memory, skills, and approvals; row-level security in SQLite
- **Owner:** Security team
- **Success metric:** Penetration test passes with zero cross-tenant data leaks

### 8. OpenTelemetry Native Integration (P2)
- **Goal:** Replace custom metrics with OTLP-native tracing, metrics, and logs
- **Approach:** Integrate `opentelemetry-rust` SDK; export to any OTLP-compatible backend
- **Owner:** Observability team
- **Success metric:** Full trace correlation from HTTP request → agent execution → outcome

## Q4: Advanced Capabilities

### 9. WASM Component Model Support (P3)
- **Goal:** Next-gen sandbox with typed interfaces and capability-based security
- **Approach:** Migrate from wasmtime modules to Wasmtime component model; maintain backward compat
- **Owner:** Sandbox team
- **Success metric:** Existing WASM skills run unmodified; new skills use component model

### 10. Federated Memory Across Instances (P3)
- **Goal:** Shared memory layer for horizontally scaled deployments
- **Approach:** CRDT-based memory replication with conflict resolution via Ground levels
- **Dependencies:** Multi-tenant isolation (Q3)
- **Success metric:** Memory consistency <500ms across 3 geographically distributed instances

### 11. Natural Language Playbook Authoring (P3)
- **Goal:** Non-engineers can create SOAR playbooks via natural language
- **Approach:** LLM-assisted playbook compiler with verification gate; human approval required
- **Owner:** Product team
- **Success metric:** 10+ playbooks authored by non-engineers and validated in staging

## Research Areas (Exploratory)

| Area | Question | Timeline | Output |
|------|----------|----------|--------|
| ML-driven orchestration | Can reinforcement learning optimize DAG scheduling beyond heuristics? | Q2-Q3 | Research paper + prototype |
| Adversarial robustness | How do equivalence-hashed approvals hold up against adversarial input crafting? | Q3 | Security audit + mitigations |
| Memory consolidation algorithms | Can temporal decay be tuned per-domain using feedback signals? | Q4 | Adaptive decay model |
| Cross-framework portability | What subset of Zylos patterns transfers to LangGraph/CrewAI/ADK? | Q4 | Portability guide + adapter library |

## Deprecation Plan

| Feature | Deprecated In | Removed In | Migration Path |
|---------|---------------|------------|----------------|
| Custom metrics endpoint (`/gateway/metrics`) | v6.1 | v7.0 | OpenTelemetry OTLP export |
| JSON config format | v6.0 | v7.0 | TOML + env vars (migration script provided) |
| Legacy approval format (no equivalence hash) | v6.0 | v6.2 | Auto-migration on first load |

## Success Metrics (12-Month Targets)

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| Active production deployments | 1 | 10+ | Self-reported + telemetry |
| Community contributors | 0 | 15+ | GitHub contributors |
| Plugin ecosystem size | 0 | 50+ | Marketplace listings |
| Average task completion time | Baseline | -20% | Shadow-git trace analysis |
| P0/P1 incidents per quarter | 0 | ≤2 | Incident tracker |
| Branch coverage (live-comm) | 68% | 85% | CI enforcement |
| Documentation completeness | 100% | 100% | Quarterly audit |

## Governance

- **Roadmap reviews:** Quarterly with all stakeholders
- **Feature prioritization:** Weighted scoring (impact × feasibility ÷ risk)
- **Breaking changes:** Require ADR + migration guide + 2-quarter deprecation window
- **Community input:** Monthly open roadmap session + GitHub discussions

---

*Approved: 2026-11-03 | Version: 2.0 | Next review: 2027-02-03*