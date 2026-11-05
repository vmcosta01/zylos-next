# zylos-live-comm

Comunicação em tempo real para orquestração multi-agente: API Gateway multi-tenant,
Event Bus pub/sub, AG-UI streaming, Cockpit API, Hermes/SOAR bridges, approval workflows,
task durability, outcome tracking, triage, equivalence approval, sentinel monitoring.

**Fase no Master Plan:** Fase 6 (LIVE_COMM_SYSTEM) — Sprints 6.1, 6.2, 6.3

## Features Implementadas

### API Gateway (`gateway.rs` — 758 linhas)
- Multi-tenant middleware (X-Tenant-ID header + JWT claims fallback)
- RBAC 5 níveis (Viewer/Operator/Developer/Admin/Owner) com LRU cache TTL 60s
- Rate limiting multi-tier (global/tenant/user) via `governor` token bucket
- Correlation ID middleware (UUID v4 propagation)
- OpenTelemetry tracing layer via `tower-http`
- Health/readiness probes
- Graceful shutdown via `CancellationToken`
- Axum Router builder (`build_gateway`)

### Event Bus (`event_bus.rs` — 582 linhas)
- Topic-based pub/sub routing
- Dual transport: SSE + WebSocket
- Backpressure strategies
- Persistence com replay from offset
- Filter engine (CEL-like expressions)
- Dead letter queue
- Schema registry

### Event Types (`event_types.rs` — 216 linhas)
- Event type definitions para AG-UI streaming
- RunStarted, RunFinished, ToolCallStart/End, TextMessageContent, StateSnapshot, CustomEvent

### Event Persistence (`event_persistence.rs` — 276 linhas)
- Append-only event storage
- Replay from arbitrary offset
- TTL-based expiration

### Event Filter (`event_filter.rs` — 387 linhas)
- CEL-like expression evaluation
- Server-side filtering para reduzir bandwidth

### AG-UI Streaming (`agui.rs` — 157 linhas)
- Session Events SDK types
- Incremental rendering support
- Sequence numbering para gap detection

### Auth (`auth.rs` — 168 linhas)
- JWT RS256 validation
- Claims extraction (sub, tenant_id, roles)

### Security (`security.rs` — 334 linhas)
- CSRF double-submit cookie
- CSP headers
- Input validation centralized

### Approval Workflow (`approval.rs` — 615 linhas)
- HITL approval cards inline no stream AG-UI
- ApprovalRequest/ApprovalResponse events
- Timeout configurável

### Task Durability (`task_durability.rs` — 724 linhas)
- Lease-based task execution
- Heartbeat mechanism
- Automatic retry on failure

### Outcome Tracking (`outcome.rs` — 472 linhas)
- Structured outcome recording
- Success/failure/partial classification

### Triage (`triage.rs` — 867 linhas)
- Automated task routing
- Priority classification
- Agent capability matching

### Equivalence Approval (`equivalence_approval.rs` — 958 linhas)
- Semantic equivalence verification
- Multi-judge consensus
- Evidence capsule attachment

### Sentinel (`sentinel.rs` — 816 linhas)
- Anomaly detection
- Performance monitoring
- Alert generation

### Bridges
- **SOAR Bridge** (`soar_bridge.rs` — 202 linhas): Integração com SOAR orchestrator
- **Hermes Bridge** (`hermes_bridge.rs` — 183 linhas): Integração com Hermes plugins

### Outros Módulos
- **Routes** (`routes.rs` — 135 linhas): Axum route definitions
- **WebSocket** (`ws.rs` — 117 linhas): WS upgrade handler
- **Cockpit** (`cockpit.rs` — 59 linhas): Dashboard API stubs
- **Rate Limit** (`rate_limit.rs` — 102 linhas): Standalone rate limiter
- **OpenAPI** (`openapi.rs` — 142 linhas): OpenAPI 3.1 spec generation
- **Cache** (`cache.rs` — 347 linhas): Multi-layer caching (LRU + TTL)
- **Skills API** (`skills_api.rs` — 320 linhas): Skill registry endpoints

## API Pública (resumo por módulo)

| Módulo | Pub Items | Destaques |
|--------|-----------|-----------|
| gateway.rs | 39 | TenantContext, RbacLevel, MultiTierRateLimiter, build_gateway |
| event_bus.rs | 21 | EventBus pub/sub core |
| event_types.rs | 15 | AG-UI event type definitions |
| approval.rs | 18 | HITL approval workflow |
| task_durability.rs | 20 | Lease-based execution |
| outcome.rs | 29 | Outcome classification |
| triage.rs | 37 | Task routing/priority |
| equivalence_approval.rs | 34 | Semantic equivalence |
| sentinel.rs | 28 | Anomaly detection |
| security.rs | 12 | CSRF/CSP/input validation |

## Status vs Master Plan

| Task | Status | Notas |
|------|--------|-------|
| 6.1.1 API Gateway Multi-Tenant | ✅ Implementado | Multi-tenant, RBAC 5-tier, rate limiting, tracing, health probes, graceful shutdown |
| 6.1.2 Event Bus Reliable Pub/Sub | ✅ Implementado | Dual transport, backpressure, persistence/replay, filter engine, DLQ, schema registry |
| 6.1.3 Integração Plugins Hermes | ✅ Implementado | hermes_bridge.rs + soar_bridge.rs |
| 6.2.1 WASM Frontend (Leptos) | ❌ Não implementado | Frontend é crate separado ou externo |
| 6.2.2 Lazy-Roster 50+ Agentes | ❌ Não implementado | Depende do frontend Leptos |
| 6.2.3 AG-UI Streaming | ✅ Implementado | agui.rs + event_types.rs + ws.rs |
| 6.3.1 Benchmark Suite | ⚠️ Parcial | benches/event_bus.rs existe; suite consolidado incompleto |
| 6.3.2 Security Hardening | ⚠️ Parcial | CSRF/CSP/input validation ✅; MFA/WebAuthn ❌; Merkle audit log ❌; adaptive rate limiting ❌; secret management ❌; continuous pentest ❌; STRIDE threat model ❌ |
| 6.3.3 Otimização/Documentação | ⚠️ Parcial | Cache multi-layer ✅; profiling/flamegraph ❌; docs/COCKPIT.md ❌; docs/SECURITY.md ❌ |

## Gaps Críticos

1. **WASM Frontend (Leptos 0.7)** — Sprint 6.2 inteiro não está neste crate; verificar se existe crate separado
2. **Lazy-Roster** — Virtualização de lista para 500+ agentes não implementada
3. **MFA/WebAuthn/FIDO2** — Autenticação multi-factor ausente (apenas JWT RS256)
4. **ABAC hybrid** — Policy engine OPA-compatible ausente (apenas RBAC 5-tier)
5. **Merkle tree audit log** — Audit log imutável com Merkle root não implementado
6. **Adaptive rate limiting** — Dynamic adjustment baseado em error rate/load ausente
7. **Secret management** — Vault integration / encrypted env vars ausente
8. **Continuous pentest** — OWASP ZAP/Burp/Semgrep/gitleaks CI pipeline não implementado
9. **STRIDE threat model** — Documento de threat modeling ausente
10. **Performance budget CI gate** — Bundle size/TTI enforcement não implementado
11. **PWA offline-first** — Service worker/offline queue não aplicável (backend crate)
12. **Benchmark suite consolidado** — cargo bench + playwright + k6 automation incompleto
