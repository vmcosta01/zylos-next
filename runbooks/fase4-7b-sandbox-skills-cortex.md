# Runbook: Fases 4-7B — Sandbox, Skills, Search, CORTEX Patterns

## Visão Geral
Este runbook cobre a operação dos módulos das Fases 4-7B do Zylos-Next:
- **Fase 4:** Sandbox Isolation (WASM + Docker + Browser anti-detect + WebMCP + Parallel search)
- **Fase 5:** Skills Registry (338 skills migradas)
- **Fase 6:** Live Communication avançado (AG-UI completo, SOAR bridge)
- **Fase 7B:** Padrões CORTEX (proof, resolver, panel, router, cerebelo, learn)

---

## Nível Junior: Operações Básicas

### Verificar saúde dos módulos
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-sandbox       # WASM + Docker sandbox
cargo test -p zylos-search-mcp    # Parallel search MCP
cargo test -p zylos-proof         # CORTEX proof system
cargo test -p zylos-resolver      # DAG resolver
cargo test -p zylos-panel         # Adversarial panel
cargo test -p zylos-router        # Grounded routing
cargo test -p zylos-cerebelo      # Shadow-git audit
cargo test -p zylos-learn         # Feedback loops
```

### Compilar e lint
```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

### Erros comuns
| Erro | Causa | Correção |
|------|-------|----------|
| `wasmtime` link error | Missing system deps | Instalar cmake, clang no Windows |
| `bollard` connection refused | Docker daemon not running | Iniciar Docker Desktop |
| `async_trait` dyn-compat | Trait object safety | Usar `#[async_trait]` em todos os impls |
| `f64` in HashMap key | f64 não implementa Hash/Eq | Wrap em `OrderedFloat<f64>` ou usar string key |
| `NetworkMode` missing Default | Enum sem default | Adicionar `#[derive(Default)]` + `#[default]` no variant |

---

## Nível Pleno: Arquitetura e Configuração

### Fase 4: Sandbox Isolation

#### WASM Sandbox (wasmtime 25.0)
- **Isolation:** Guest code roda em sandbox com memory limits
- **Resource limits:** CPU time, memory cap, filesystem access restrito
- **Host functions:** Interface segura para callbacks controlados
- **Tests:** 5 testes cobrindo instantiation, execution, resource limits

#### Docker Sandbox (bollard 0.17)
- **Container lifecycle:** Create → Start → Exec → Stop → Remove
- **Network modes:** Bridge, Host, None (com `#[derive(Default)]`)
- **Volume mounts:** Read-only por default, writable opt-in
- **Security:** Non-root user, capability dropping, seccomp profiles
- **Tests:** 8 testes cobrindo lifecycle, network, volumes, security

#### Browser Anti-Detect
- **Fingerprint rotation:** User-Agent, viewport, timezone, language
- **Headless detection bypass:** WebDriver flag removal, chrome properties
- **Proxy support:** SOCKS5/HTTP proxy chain com failover
- **Session persistence:** Cookie/state reuse across runs

#### WebMCP Spec Compliance
- **Protocol version:** MCP 2024-11-05
- **Tool registration:** JSON Schema validation
- **Streaming:** SSE transport com reconnection
- **Error handling:** Standardized error codes

#### Parallel Search MCP
- **Concurrent queries:** Fan-out para múltiplos providers
- **Result merging:** Dedup por URL, relevance scoring
- **Timeout handling:** Per-provider timeouts, partial results
- **Tests:** 8 testes cobrindo concurrency, dedup, timeout, scoring

### Fase 5: Skills Registry
- **Migration target:** 338 skills do Hermes para Zylos format
- **Format:** SKILL.md com YAML frontmatter + markdown body
- **Categories:** devops, research, creative, productivity, software-development
- **Quality gate:** Zero perda de funcionalidade na migração
- **Indexing:** Busca por name, category, trigger keywords

### Fase 6: Live Communication Avançado
- **AG-UI events:** TextDelta, StateSnapshot, ToolCall, Heartbeat, ApprovalCard
- **EventBus:** Pub/sub async com filtering e backpressure
- **Auth:** HMAC-SHA256 tokens com expiry e revocation
- **Cache:** HTTP response caching com TTL, LRU eviction
- **Cockpit:** Leptos 0.7 WASM frontend com SSE streaming
- **SOAR bridge:** Playbook lifecycle, incident publishing

### Fase 7B: Padrões CORTEX
- **zylos-proof:** 5 ground levels (Assumed→Proven), ProofBuilder, VerificationGate
- **zylos-resolver:** BFS frontier sobre petgraph DAG, checkpoint por wave
- **zylos-panel:** JudgeVote adversarial, 1 refutation defeats majority
- **zylos-router:** Grounded routing + leash bounds + sacred targets
- **zylos-cerebelo:** Shadow-git commits, audit chain SHA256, test-ratchet
- **zylos-learn:** Feedback → Policy → reward shaping, recurrence gate ≥3×

---

## Nível Senior: Debugging e Extensão

### Debug: WASM sandbox crash
1. Verificar wasmtime version compatibilidade (25.0)
2. Memory limit muito baixo? Aumentar `MemoryType::new(min, max)`
3. Host function signature mismatch? Verificar `Func::wrap` types
4. Guest panic? Habilitar `wasm_backtrace` para stack trace

### Debug: Docker container não inicia
1. `docker ps -a` para verificar containers parados
2. Logs: `docker logs <container_id>`
3. Network mode conflict? Verificar port bindings
4. Volume mount permission? Usar `:ro` ou ajustar ownership

### Debug: Search MCP partial results
1. Verificar timeout per-provider (default 5s)
2. Provider retornando erro? Check `SearchResult.error` field
3. Dedup removendo resultados válidos? Ajustar similarity threshold
4. Relevance scoring errado? Verificar weights em `ScoreConfig`

### Debug: CORTEX proof stuck em Pending
1. Evidence ground level < proof ground level? Adicionar evidence mais forte
2. VerificationGate min_ground muito alto? Baixar threshold ou adicionar evidence
3. Proof refuted? Verificar contradictory evidence, remover ou resolver
4. Chain of evidence quebrada? Verificar `composed_from` references

### Debug: Panel disagreement não resolve
1. Judges usando mesmo model/prompt? Diversificar configurations
2. Refutation não sendo detectada? Verificar `JudgeVote::Refute` parsing
3. Majority threshold errado? Ajustar `PanelConfig.min_agreement`
4. Judge timeout? Aumentar `PanelConfig.judge_timeout_ms`

### Extending sandbox providers
```rust
// Adicionar novo sandbox backend
use zylos_sandbox::SandboxProvider;

pub struct FirecrackerSandbox { /* ... */ }

#[async_trait]
impl SandboxProvider for FirecrackerSandbox {
    async fn create(&self, config: SandboxConfig) -> Result<SandboxInstance>;
    async fn execute(&self, instance: &SandboxInstance, cmd: &str) -> Result<Output>;
    async fn destroy(&self, instance: SandboxInstance) -> Result<()>;
}
```

### Métricas chave
- **Sandbox startup time:** WASM <100ms, Docker <2s
- **Search latency p99:** <3s com 5 providers paralelos
- **Proof verification rate:** >95% first-pass acceptance
- **Panel agreement rate:** >80% unanimous decisions
- **Skills migration completeness:** 338/338 = 100%

---

## Referências
- Master plan: `C:\Users\Paulo\AppData\Local\hermes\cache\scratch\zylos-next-master-plan.md`
- CORTEX architecture: `C:/Users/Paulo/Documents/manter/CORTEX-ARQUITETURA.md`
- Audits: `evals/audits/fase{4,5,6,7b}-audit-{adversarial,isolated}.md`