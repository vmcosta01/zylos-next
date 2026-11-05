# Runbook: Fases 0-3 — Foundation, Core Orchestration, Verification, Live Communication

## Visão Geral
Este runbook cobre a operação dos módulos fundamentais do Zylos-Next (Fases 0-3):
- **Fase 0:** Project scaffolding, workspace config, CI/CD
- **Fase 1:** Core orchestration (zylos-core, zylos-memory, zylos-orchestrator)
- **Fase 2:** Verification engine (zylos-proof, zylos-resolver, zylos-panel, zylos-router, zylos-cerebelo, zylos-learn)
- **Fase 3:** Live communication (zylos-live-comm base modules)

---

## Nível Junior: Operações Básicas

### Verificar saúde do workspace
```bash
cd C:/Users/Paulo/zylos-next
cargo check --workspace          # Compilação
cargo test --workspace           # Todos os testes
cargo clippy --workspace -- -D warnings  # Lint estrito
```

### Testar crate específico
```bash
cargo test -p zylos-core         # Orquestração central
cargo test -p zylos-memory       # Memória e consolidação
cargo test -p zylos-proof        # Proof system (5 ground levels)
cargo test -p zylos-resolver     # DAG resolver com BFS frontier
cargo test -p zylos-panel        # Adversarial panel (judge ≠ generator)
cargo test -p zylos-router       # Grounded routing + leash
cargo test -p zylos-cerebelo     # Shadow-git + audit chain
cargo test -p zylos-learn        # Feedback + reward shaping
cargo test -p zylos-live-comm    # AG-UI, EventBus, auth, cache
```

### Erros comuns
| Erro | Causa | Correção |
|------|-------|----------|
| `sqlx` not found | Workspace usa `rusqlite` | Trocar dep para `rusqlite = { workspace = true }` |
| `tokio-test` missing | Não está em workspace deps | Remover de dev-dependencies ou adicionar ao workspace |
| `ready_nodes` inclui completed | Bug no resolver filter | Filtrar `NodeStatus::Completed` explicitamente |
| `write_file` refuses overwrite | Tool requires prior read | Usar `patch` ou ler arquivo antes de escrever |

---

## Nível Pleno: Arquitetura e Configuração

### Fase 0: Workspace Structure
```
zylos-next/
├── Cargo.toml              # Workspace root com todos os members
├── crates/
│   ├── zylos-core/         # Tipos centrais, traits, errors
│   ├── zylos-memory/       # Memory store, consolidação, decay
│   ├── zylos-orchestrator/ # DAG execution, checkpointing
│   ├── zylos-proof/        # 5-level Ground enum, ProofBuilder
│   ├── zylos-resolver/     # BFS frontier, wave scheduling
│   ├── zylos-panel/        # Adversarial judging, refutation
│   ├── zylos-router/       # Grounded routing, sacred targets
│   ├── zylos-cerebelo/     # Shadow-git, audit chain SHA256
│   ├── zylos-learn/        # Feedback loops, recurrence gate
│   ├── zylos-live-comm/    # AG-UI, EventBus, approval, triage, sentinel
│   ├── zylos-hitl/         # HITL standalone
│   ├── zylos-triage/       # Triage standalone
│   └── zylos-sentinel/     # Sentinel standalone
├── apps/
│   └── zylos-cockpit/      # Leptos 0.7 WASM frontend
├── runbooks/               # Este diretório
├── playbooks/              # Procedimentos operacionais
├── docs/handoffs/          # Handoff entre fases
└── evals/audits/           # Auditorias adversariais e isoladas
```

### Fase 1: Core Orchestration
- **zylos-core:** Tipos compartilhados (AgentId, TaskId, Ground, ProofResult)
- **zylos-memory:** Consolidação noturna, contradição detection, decay temporal
- **zylos-orchestrator:** DAG com petgraph, checkpoint por wave, rollback

### Fase 2: Verification Engine (CORTEX-derived)
- **zylos-proof:** Ground enum (Assumed/Observed/Confirmed/Verified/Proven), ProofBuilder com chain of evidence, VerificationGate com min_ground threshold
- **zylos-resolver:** BFS frontier sobre DAG, checkpoint por wave, ready_nodes exclui Completed
- **zylos-panel:** JudgeVote, PanelConfig, adversarial aggregation (1 refutation defeats majority)
- **zylos-router:** RouteTarget, leash bounds, sacred targets imutáveis, proposeChange com proof
- **zylos-cerebelo:** AuditEntry, shadow-git commits, test-ratchet (coverage nunca diminui)
- **zylos-learn:** Feedback, Policy, reward shaping, snapshot, recurrence gate ≥3× = mature lesson

### Fase 3: Live Communication Base
- **agui:** AG-UI event types (TextDelta, StateSnapshot, ToolCall, Heartbeat)
- **event_bus:** Pub/sub com filtering por event type
- **auth:** Token creation/validation com HMAC-SHA256
- **cache:** HTTP response caching com TTL, eviction, stats
- **cockpit:** Health endpoint, streaming SSE com backoff determinístico
- **hermes_bridge:** Forward events between Hermes and Zylos
- **soar_bridge:** SOAR event publishing, playbook lifecycle
- **security:** XSS/SQLi/path traversal detection, input sanitization, audit log ring buffer
- **rate_limit:** Token bucket rate limiting
- **openapi:** OpenAPI spec generation (JSON/YAML)
- **skills_api:** Skill registry CRUD, eval runs

---

## Nível Senior: Debugging e Extensão

### Debug: DAG resolution stuck
1. Verificar `ready_nodes()` — deve excluir `NodeStatus::Completed`
2. Checkpoint pode estar stale — forçar recompute via `resolver.checkpoint(dag)`
3. Circular dependency? `petgraph::algo::is_cyclic_directed(&dag)`

### Debug: Proof ground level mismatch
- ProofBuilder exige evidence ground ≥ proof ground
- Se evidence é `Observed` mas proof claims `Verified`, verify() retorna `Pending`
- Use `ProofBuilder::with_evidence()` para adicionar evidence correta

### Debug: Panel refutation não funciona
- Panel requer judge ≠ generator (adversarial)
- 1 refutation defeats majority — verificar `JudgeVote::Refute` está sendo contado
- PanelConfig.min_judges deve ser ≥ 3 para resultados significativos

### Debug: Router leash violation
- Leash define bounds máximos para mudanças
- `proposeChange()` retorna Err se mudança excede leash
- Sacred targets são imutáveis — qualquer change attempt falha

### Extending verification pipeline
```rust
// Adicionar novo ground level ao ProofBuilder
use zylos_proof::{Ground, ProofBuilder};

let proof = ProofBuilder::new("claim", Ground::Confirmed)
    .with_evidence(Ground::Confirmed, "test output")
    .with_evidence(Ground::Verified, "integration test passed")
    .build();

// Gate verifica min_ground
let gate = VerificationGate::new(Ground::Confirmed);
assert!(gate.check(&proof).is_ok());
```

### Métricas chave
- **Proof acceptance rate:** proofs Valid / total verified
- **Panel agreement:** % de decisions com unanimous votes
- **Resolver throughput:** nodes resolved / second
- **Memory consolidation ratio:** entries before / after nightly consolidation
- **Cache hit rate:** hits / (hits + misses)

---

## Referências
- Master plan: `C:\Users\Paulo\AppData\Local\hermes\cache\scratch\zylos-next-master-plan.md`
- CORTEX architecture: `C:/Users/Paulo/Documents/manter/CORTEX-ARQUITETURA.md`
- Audits: `evals/audits/fase{0,1,2,3}-audit-{adversarial,isolated}.md`