# Implementation Plan — Fase 8: Auditoria Externa Remediation

**Data:** 2026-11-11
**Origem:** EXTERNAL_AUDIT_2026-11-11.md (12 ações prioritárias + 14 recomendações adicionais)
**Estimativa Total:** 12-16 semanas
**Status:** 📋 Planejado

---

## Visão Geral

Este plano cobre **todas** as recomendações da auditoria externa comparativa, organizadas em 6 fases gateadas como vertical slices entregáveis. Cada fase segue rigorosamente a skill `implementation-plan-author` com 6 subseções obrigatórias e critérios de aceitação testáveis.

### Matriz de Rastreabilidade

| Recomendação | Categoria | Prioridade | Fase | Task Principal |
|-------------|-----------|------------|------|----------------|
| HopCountCircuitBreaker | Orchestration | 🔴 Crítica | F8.1 | T8.1.1 |
| ResourceLimiter cgroups | Security | 🔴 Alta | F8.3 | T8.3.1 |
| NetworkEgressFilter | Security | 🔴 Alta | F8.3 | T8.3.3 |
| MemoryLifecycle trait | Memory | 🟡 Alta | F8.2 | T8.2.1 |
| SkillContract formal | Skills | 🟡 Alta | F8.4 | T8.4.1 |
| SkillMiner from logs | Skills | 🟡 Alta | F8.4 | T8.4.3 |
| AutoloopProtocol async | Orchestration | 🟡 Alta | F8.1 | T8.1.3 |
| PlanFirstGate | Employees | 🟡 Alta | F8.5 | T8.5.3 |
| AutoImprovementLoop | Memory | 🟡 Média | F8.2 | T8.2.4 |
| WorkspacePersistence | Desktop | 🟡 Média | F8.6 | T8.6.1 |
| SkillHierarchy 3-tier | Skills | 🟢 Média | F8.4 | T8.4.5 |
| CrashSimulator tests | Orchestration | 🟢 Média | F8.1 | T8.1.5 |
| MentalModelStore | Memory | 🟡 Média | F8.2 | T8.2.2 |
| ContextRoom struct | Memory | 🟡 Média | F8.2 | T8.2.3 |
| GoalProgram struct | Orchestration | 🟡 Média | F8.1 | T8.1.2 |
| VerificationAgent separado | Orchestration | 🟡 Média | F8.1 | T8.1.4 |
| AgentAwarenessState enum | Desktop | 🟡 Média | F8.6 | T8.6.2 |
| CockpitPlugin trait | Desktop | 🟡 Média | F8.6 | T8.6.3 |
| RelayServer mobile | Desktop | 🟡 Média | F8.6 | T8.6.4 |
| EmployeeRole enum | Employees | 🟡 Média | F8.5 | T8.5.1 |
| RoutineCatalog index.json | Employees | 🟡 Média | F8.5 | T8.5.2 |
| EmployeeRateLimiter | Employees | 🟡 Média | F8.5 | T8.5.4 |
| GuardrailEnforcer | Employees | 🟡 Alta | F8.5 | T8.5.5 |
| OutputSanitizer pipeline | Security | 🟡 Média | F8.3 | T8.3.4 |
| CapabilityToken system | Security | 🟡 Média | F8.3 | T8.3.5 |
| HostAllocatorLimiter | Security | 🟢 Baixa | F8.3 | T8.3.6 |

### Dependências a Adicionar ao Workspace

```toml
# Cargo.toml workspace dependencies
cgroups-rs = { version = "0.3", optional = true } # Linux only
secrecy = "0.10"
libloading = "0.9"
bincode = "2.0-rc"
regex = "1.10" # provavelmente já presente
```

### Riscos Identificados

1. **cgroups-rs é Linux-only:** Windows/macOS requerem fallback via Docker resource limits. Implementar como trait com platform-specific backends.
2. **SkillMiner pode gerar falso-positivos:** Gate obrigatório via EvalFlywheel antes de promover candidate skill.
3. **HopCount=8 pode ser restritivo demais:** Tornar configurável via rules.json com default conservador.
4. **MemoryLifecycle consolidation pode ser custoso:** Schedule em off-peak hours via Tokio cron.

---

## Fase F8.1 — Orchestration Safety & Resilience (Semanas 1-3)

### Objetivo
Implementar circuit breakers, protocolos assíncronos e testes de crash para prevenir loops infinitos e garantir recovery confiável em DAGs multi-agente.

### Tarefas Numeradas

#### T8.1.1 — HopCountCircuitBreaker
**Dependências:** Nenhuma
**Descrição:** Implementar middleware no DAGExecutor com contador atômico que previne loops infinitos.

1. Criar struct `HopCountGuard` em `crates/zylos-swarm-orchestrator/src/hop_count.rs`
2. Integrar como layer no DAGExecutor spawn path
3. Adicionar config em `rules.json`: `{ "hop_count_max": 8 }`
4. Implementar graceful degradation via RecoveryLadder quando circuit abre
5. Adicionar métrica Prometheus `zylos_hop_count_total{status="open|closed"}`

**Entregável Verificável:** Código em `crates/zylos-swarm-orchestrator/src/hop_count.rs` + integração no DAGExecutor + teste unitário

**Critérios de Aceitação:**
- [ ] Spawn aninhado >8 níveis retorna `Err(CircuitOpen)` sem panic
- [ ] Contador decrementa corretamente em drop/exit
- [ ] Config hot-reload via rules.json funciona
- [ ] Métrica exposta via /metrics endpoint
- [ ] Teste `test_hop_count_circuit_breaker` passa com 100% coverage

**Out of Scope:** Otimização de performance do contador; UI cockpit para visualização

**Gate de Verificação:** `cargo test -p zylos-swarm-orchestrator hop_count` → todos verdes + métrica visível em /metrics

---

#### T8.1.2 — GoalProgram Struct
**Dependências:** T8.1.1
**Descrição:** Definir estrutura declarativa para orquestração orientada a objetivos.

1. Criar `GoalProgram` em `crates/zylos-goals/src/program.rs`
2. Persistir em SQLite tabela `goal_programs`
3. Carregar via `rules.json` declarativo
4. Integrar com Goals API existente

**Entregável Verificável:** Módulo `program.rs` + migration SQLite + loader JSON

**Critérios de Aceitação:**
- [ ] GoalProgram serializa/deserializa via serde sem perda
- [ ] Loader valida schema JSON antes de persistir
- [ ] API endpoint `GET /goals/programs` retorna lista
- [ ] Teste round-trip JSON→DB→JSON passa

**Out of Scope:** Evolution Strategy implementation; RL optimization

**Gate de Verificação:** `cargo test -p zylos-goals program` → todos verdes + endpoint responde 200

---

#### T8.1.3 — AutoloopProtocol Async
**Dependências:** T8.1.1
**Descrição:** Implementar protocolo de comunicação assíncrona com deferred resolution.

1. Definir enum `AutoloopProtocol` em `crates/zylos-p2p/src/protocol.rs`
2. Implementar como message type no P2PMessenger
3. Adicionar handler para `Later` que agenda task via Tokio interval
4. Documentar protocolo em docs/AUTOLOOP_PROTOCOL.md

**Entregável Verificável:** Enum + handlers + documentação

**Critérios de Aceitação:**
- [ ] Mensagem `Later` agenda task e retorna imediatamente
- [ ] Deferred task executa após delay configurado
- [ ] P2PMessenger routeia todas as variantes corretamente
- [ ] Teste integration `test_autoloop_deferred_resolution` passa

**Out of Scope:** Retry logic; persistence de deferred tasks entre restarts

**Gate de Verificação:** `cargo test -p zylos-p2p autoloop` → todos verdes

---

#### T8.1.4 — VerificationAgent Separado
**Dependências:** T8.1.1
**Descrição:** Criar crate independente para verificação adversarial sem contexto compartilhado.

1. Criar novo crate `crates/zylos-independent-verifier/`
2. Implementar interface que recebe apenas outputs (não conversation history)
3. Validar via `Ground` enum existente
4. Integrar com AdversarialPanel como backend opcional
5. Adicionar feature flag `independent-verifier` no workspace

**Entregável Verificável:** Crate funcional + integração feature-gated

**Critérios de Aceitação:**
- [ ] Crate compila independentemente sem dependências circulares
- [ ] Verificador rejeita output inválido sem acesso a histórico
- [ ] Feature flag desabilita crate por default
- [ ] Benchmark mostra <5ms overhead vs verificação inline

**Out of Scope:** UI para seleção de verificador; caching de resultados

**Gate de Verificação:** `cargo build -p zylos-independent-verifier` → sucesso + `cargo bench` dentro do budget

---

#### T8.1.5 — CrashSimulator Tests
**Dependências:** T8.1.1, T8.1.3
**Descrição:** Test harness para injeção de falhas sintéticas e validação de recovery.

1. Criar `tests/crash_simulation.rs` em `crates/zylos-swarm-orchestrator/`
2. Implementar injetores: spawn failure, channel close, timeout, panic
3. Verificar RecoveryLadder response para cada cenário
4. Gerar relatório HTML com pass/fail por cenário
5. Integrar no CI como job separado

**Entregável Verificável:** Test suite + relatório automatizado

**Critérios de Aceitação:**
- [ ] 4 cenários de crash cobertos (spawn, channel, timeout, panic)
- [ ] RecoveryLadder recupera estado em todos os cenários
- [ ] Relatório HTML gerado automaticamente
- [ ] CI job falha se qualquer cenário regredir

**Out of Scope:** Chaos engineering em produção; network partition simulation

**Gate de Verificação:** `cargo test --test crash_simulation` → 4/4 pass + relatório em `target/crash_report.html`

---

### Entregável da Fase
Código funcional de circuit breaker, protocolo async, verificador independente e crash simulator, todos com testes passando e integrados ao DAGExecutor.

### Critérios de Aceitação da Fase
- [ ] Todos os 5 gates de tarefa verdes
- [ ] `cargo test -p zylos-swarm-orchestrator` → 100% pass rate
- [ ] Nenhum loop infinito em stress test de 1h com DAGs aleatórios
- [ ] Documentação atualizada em docs/ORCHESTRATION_SAFETY.md

### Out of Scope da Fase
- Evolução automática de estratégias (RL)
- UI cockpit para monitoring de circuit breakers
- Persistência de deferred tasks entre restarts

### Gate de Verificação da Fase
```bash
cargo test -p zylos-swarm-orchestrator && \
cargo test -p zylos-p2p autoloop && \
cargo test -p zylos-independent-verifier && \
cargo test --test crash_simulation
```
Todos devem retornar exit code 0.

---

## Fase F8.2 — Memory Lifecycle & Evolution (Semanas 4-6)

### Objetivo
Implementar lifecycle management explícito para memória, mental models per-user, context rooms e auto-improvement loop baseado em padrões de acesso.

### Tarefas Numeradas

#### T8.2.1 — MemoryLifecycle Trait
**Dependências:** F8.1 completa
**Descrição:** Trait com métodos retain/consolidate/refresh e scheduling periódico.

1. Definir trait em `crates/zylos-memory-system/src/lifecycle.rs`
2. Implementar para ContextGraph existente
3. Adicionar background task Tokio com `tokio::time::Interval`
4. Configurar schedule via `memory.lifecycle.schedule` em config.toml
5. Expor métricas: entries_retained, consolidation_duration_ms, refresh_errors

**Entregável Verificável:** Trait + implementação + scheduler + métricas

**Critérios de Aceitação:**
- [ ] retain() remove entradas fora da policy sem corromper grafo
- [ ] consolidate() merge entradas duplicadas dentro da window
- [ ] refresh() recarrega embeddings stale
- [ ] Background task roda no schedule configurado
- [ ] Métricas visíveis em /metrics

**Out of Scope:** ML-based retention policies; distributed consolidation

**Gate de Verificação:** `cargo test -p zylos-memory-system lifecycle` → todos verdes + métricas expostas

---

#### T8.2.2 — MentalModelStore
**Dependências:** T8.2.1
**Descrição:** Extensão do ContextGraph para modelos cognitivos per-user.

1. Criar tabela SQLite `mental_models` com user_id FK
2. Implementar CRUD operations em `crates/zylos-memory-system/src/mental_models.rs`
3. Cache embeddings em memória via `dashmap`
4. Integrar com VectorStore para similarity search

**Entregável Verificável:** Migration + módulo Rust + cache

**Critérios de Aceitação:**
- [ ] Migration roda sem erros em fresh DB
- [ ] CRUD operations funcionam com user isolation
- [ ] Similarity search retorna top-k em <50ms para 10k models
- [ ] Cache hit ratio >80% em workload típico

**Out of Scope:** Model training; cross-user model sharing

**Gate de Verificação:** `cargo test -p zylos-memory-system mental_models` → todos verdes + benchmark <50ms

---

#### T8.2.3 — ContextRoom Struct
**Dependências:** T8.2.1
**Descrição:** Bundling semântico de fontes, decisões, tasks e memória.

1. Definir struct `ContextRoom` em `crates/zylos-memory-system/src/context_room.rs`
2. Persistir como JSON column em tabela `context_rooms`
3. Implementar snapshot/versioning
4. Expor via API REST `GET /context-rooms/:id`

**Entregável Verificável:** Struct + persistence + API

**Critérios de Aceitação:**
- [ ] bundle()/unbundle() round-trip sem perda
- [ ] Snapshot preserva estado point-in-time
- [ ] API retorna room completo em <100ms
- [ ] Room isolation por tenant_id enforced

**Out of Scope:** Real-time collaboration; room sharing entre tenants

**Gate de Verificação:** `cargo test -p zylos-memory-system context_room` → todos verdes + API 200

---

#### T8.2.4 — AutoImprovementLoop
**Dependências:** T8.2.1, T8.2.2
**Descrição:** Background task que analisa padrões de query e sugere consolidações.

1. Criar module `crates/zylos-memory-system/src/auto_improvement.rs`
2. Analisar query patterns via `dashmap` counters
3. Detectar hotspots (>N queries similares em window T)
4. Sugerir consolidação via EventBus notification
5. Integrar com EvalFlywheel para validação automática de sugestões
6. Log sugestões aceitas/rejeitadas para feedback loop futuro

**Entregável Verificável:** Analyzer + detector + notifier + logging

**Critérios de Aceitação:**
- [ ] Hotspot detection identifica padrões reais em dataset sintético
- [ ] Sugestões enviadas via EventBus chegam ao subscriber
- [ ] EvalFlywheel valida sugestão antes de notificar usuário
- [ ] Logging captura decisão humana para future training
- [ ] Overhead <1% CPU em idle state

**Out of Scope:** Auto-aplicação de sugestões; RL-based suggestion ranking

**Gate de Verificação:** `cargo test -p zylos-memory-system auto_improvement` → todos verdes + log entries visíveis

---

### Entregável da Fase
Memory lifecycle funcional, mental models per-user, context rooms com bundling e auto-improvement loop integrado ao EventBus.

### Critérios de Aceitação da Fase
- [ ] Todos os 4 gates de tarefa verdes
- [ ] `cargo test -p zylos-memory-system` → 100% pass rate
- [ ] Consolidation job roda em off-peak sem impactar latência de query
- [ ] Documentação atualizada em docs/MEMORY_LIFECYCLE.md

### Out of Scope da Fase
- Distributed memory across nodes
- ML-based retention policies
- Real-time room collaboration

### Gate de Verificação da Fase
```bash
cargo test -p zylos-memory-system && \
curl -s http://localhost:3000/metrics | grep -E "memory_lifecycle|mental_model|context_room"
```
Testes verdes + métricas presentes.

---

## Fase F8.3 — Security Hardening & Resource Control (Semanas 7-10)

### Objetivo
Implementar enforcement real de resource limits, network egress filtering, output sanitization e capability tokens para segurança defense-in-depth.

### Tarefas Numeradas

#### T8.3.1 — ResourceLimiter cgroups
**Dependências:** F8.2 completa
**Descrição:** Wrapper que aplica CPU/memory caps via cgroups (Linux) ou Docker fallback.

1. Adicionar `cgroups-rs` como dependency optional no workspace
2. Criar trait `ResourceLimiter` em `crates/zylos-sandbox/src/resource_limits.rs`
3. Implementar `CgroupsLimiter` (Linux) e `DockerLimiter` (cross-platform)
4. Feature-gate: `#[cfg(target_os = "linux")]` para cgroups
5. Validar limits em runtime via `/sys/fs/cgroup` reads ou Docker inspect
6. Integrar no container spawn path do sandbox

**Entregável Verificável:** Trait + 2 implementações + integração

**Critérios de Aceitação:**
- [ ] CgroupsLimiter aplica limits corretamente em Linux
- [ ] DockerLimiter funciona em Windows/macOS/Linux
- [ ] verify() detecta violação em <100ms
- [ ] Container spawn falha gracefully se limits não aplicáveis
- [ ] Teste integration em Linux CI runner passa

**Out of Scope:** GPU limits; IO throttling; dynamic limit adjustment

**Gate de Verificação:** `cargo test -p zylos-sandbox resource_limits` → todos verdes + Linux CI job verde

---

#### T8.3.2 — Platform Abstraction Layer
**Dependências:** T8.3.1
**Descrição:** Abstrair diferenças platform-specific para resource limiting.

1. Criar enum `PlatformBackend` em `crates/zylos-sandbox/src/platform.rs`
2. Auto-detect backend em runtime
3. Fallback chain: cgroups → Docker → noop (com warning)
4. Log backend selecionado em startup

**Entregável Verificável:** Abstraction + auto-detection + logging

**Critérios de Aceitação:**
- [ ] Linux seleciona cgroups automaticamente
- [ ] macOS/Windows seleciona Docker
- [ ] Noop backend loga warning mas não crasha
- [ ] Backend selection visível em logs de startup

**Out of Scope:** Custom backend registration; plugin system

**Gate de Verificação:** Logs de startup mostram backend correto para cada plataforma

---

#### T8.3.3 — NetworkEgressFilter
**Dependências:** T8.3.1
**Descrição:** Proxy sidecar que intercepta outbound requests e valida contra whitelist.

1. Criar Axum middleware em `crates/zylos-agent-shield/src/egress_filter.rs`
2. Implementar custom hyper connector que valida destination
3. Whitelist por agente via config: `{ "agent_id": ["api.example.com", "*.internal"] }`
4. Log tentativas bloqueadas com agent_id, destination, timestamp
5. Expor métrica `zylos_egress_blocked_total{agent_id}`

**Entregável Verificável:** Middleware + connector + logging + métricas

**Critérios de Aceitação:**
- [ ] Request para destino não-whitelisted retorna 403 Forbidden
- [ ] Whitelist suporta wildcards (*.internal)
- [ ] Blocked requests logadas com todos os campos
- [ ] Métrica incrementa corretamente
- [ ] Performance overhead <2ms por request

**Out of Scope:** TLS inspection; DNS-level filtering; egress allowlists dinâmicos

**Gate de Verificação:** `cargo test -p zylos-agent-shield egress_filter` → todos verdes + métrica visível

---

#### T8.3.4 — OutputSanitizer Pipeline
**Dependências:** T8.3.3
**Descrição:** Regex-based filter que remove secrets patterns antes de retornar agent output.

1. Criar pipeline em `crates/zylos-agent-shield/src/output_sanitizer.rs`
2. Implementar regex patterns para AWS keys, JWTs, passwords, API tokens
3. Usar `secrecy` crate para sensitive data handling
4. Integrar com AG-UI stream antes de enviar ao frontend
5. Adicionar config para custom patterns via sanitizer.rules.json

**Entregável Verificável:** Pipeline + patterns + integração AG-UI

**Critérios de Aceitação:**
- [ ] Secrets conhecidos são redactados em 100% dos casos
- [ ] Custom patterns carregados hot-reload
- [ ] Latency added <5ms por message
- [ ] Teste property-based com secrets sintéticos passa
- [ ] Log de redactions (sem conteúdo) para audit

**Out of Scope:** NLP-based secret detection; contextual awareness; undo/restore

**Gate de Verificação:** `cargo test -p zylos-agent-shield output_sanitizer` → todos verdes + property test passa

---

#### T8.3.5 — CapabilityToken System
**Dependências:** T8.3.3
**Descrição:** Tokens com capabilities explícitas por agente, validados em cada syscall wrapper.

1. Definir struct `CapabilityToken` em `crates/zylos-agent-shield/src/capability.rs`
2. Implementar parser de capability strings: `filesystem:read:/data`, `network:egress:api.example.com`
3. Validar token em filesystem/network wrappers existentes
4. Revogar via EventBus broadcast
5. Persistir tokens em SQLite com expiry

**Entregável Verificável:** Token system + validation + revocation

**Critérios de Aceitação:**
- [ ] Token válido permite operação autorizada
- [ ] Token inválido/expirado bloqueia operação
- [ ] Revocation propaga em <100ms via EventBus
- [ ] Wildcard capabilities funcionam corretamente
- [ ] Audit log registra todas as validações

**Out of Scope:** Delegação de capabilities; hierarchical tokens; OAuth integration

**Gate de Verificação:** `cargo test -p zylos-agent-shield capability` → todos verdes + revocation test passa

---

#### T8.3.6 — HostAllocatorLimiter
**Dependências:** T8.3.1
**Descrição:** Limitar heap usage do processo gateway para prevenir DoS por memory exhaustion.

1. Implementar custom global allocator wrapper em `crates/zylos-gateway/src/allocator.rs`
2. Configurar limite via env var `ZYLOS_MAX_HEAP_MB`
3. Panic graceful com dump de diagnóstico quando limite atingido
4. Expor métrica `zylos_heap_usage_bytes`

**Entregável Verificável:** Allocator wrapper + config + métrica

**Critérios de Aceitação:**
- [ ] Heap usage nunca excede limite configurado
- [ ] Graceful shutdown triggered antes de OOM
- [ ] Diagnóstico dump inclui top allocations
- [ ] Métrica atualizada em tempo real
- [ ] Zero overhead quando feature desabilitada

**Out of Scope:** Per-agent heap isolation; GC tuning; memory pressure notifications

**Gate de Verificação:** Stress test com alocação progressiva → graceful shutdown + dump gerado

---

### Entregável da Fase
Resource limiting cross-platform, egress filtering, output sanitization, capability tokens e host allocator protection, todos integrados e testados.

### Critérios de Aceitação da Fase
- [ ] Todos os 6 gates de tarefa verdes
- [ ] `cargo test -p zylos-sandbox -p zylos-agent-shield` → 100% pass rate
- [ ] Security scan (cargo-audit + trivy) limpo
- [ ] Documentação atualizada em docs/SECURITY_HARDENING.md

### Out of Scope da Fase
- GPU resource limiting
- Dynamic capability delegation
- NLP-based secret detection

### Gate de Verificação da Fase
```bash
cargo test -p zylos-sandbox -p zylos-agent-shield && \
cargo audit && \
trivy fs --severity HIGH,CRITICAL .
```
Todos devem retornar exit code 0.

---

## Fase F8.4 — Skill Contracts & Self-Evolution (Semanas 11-13)

### Objetivo
Formalizar skill contracts, implementar mineração automática de skills a partir de logs e estabelecer hierarquia 3-tier para escalabilidade.

### Tarefas Numeradas

#### T8.4.1 — SkillContract Formal
**Dependências:** F8.3 completa
**Descrição:** Estrutura formal com preconditions, outputs, assertions, validators e fitness metric.

1. Definir `SkillContract` em `crates/zylos-skills/src/contract.rs`
2. Implementar load-time validation via jsonschema
3. Integrar com IndexRegistry para validação automática
4. Adicionar CLI command `zylos skills validate <path>`
5. Documentar schema em docs/SKILL_CONTRACT_SCHEMA.md

**Entregável Verificável:** Contract struct + validator + CLI + docs

**Critérios de Aceitação:**
- [ ] Contract inválido rejeitado em load-time com erro descritivo
- [ ] CLI validate retorna exit code 0/1 corretamente
- [ ] Schema documentado com exemplos
- [ ] Backward compat com skills existentes mantida
- [ ] Teste property-based com contracts sintéticos passa

**Out of Scope:** Runtime contract enforcement; contract versioning; migration tool

**Gate de Verificação:** `cargo test -p zylos-skills contract` → todos verdes + CLI funciona

---

#### T8.4.2 — SkillEvolutionGraph
**Dependências:** T8.4.1
**Descrição:** Grafo de evolução de skills com operações merge/retire/add_validator.

1. Criar `SkillEvolutionGraph` usando petgraph em `crates/zylos-skills/src/evolution.rs`
2. Implementar operações: merge(), retire(), add_validator()
3. Persistir grafo em SQLite
4. Expor via API `GET /skills/graph`
5. Visualizar no cockpit (read-only)

**Entregável Verificável:** Graph + operations + persistence + API

**Critérios de Aceitação:**
- [ ] merge() combina dois skills sem perda de validators
- [ ] retire() marca skill como deprecated sem deletar
- [ ] add_validator() atualiza grafo atomicamente
- [ ] API retorna grafo serializado em <200ms
- [ ] Teste de consistência do grafo passa

**Out of Scope:** Automatic merge suggestions; graph visualization advanced; diff view

**Gate de Verificação:** `cargo test -p zylos-skills evolution` → todos verdes + API 200

---

#### T8.4.3 — SkillMiner from Logs
**Dependências:** T8.4.1
**Descrição:** Extrai padrões repetidos de logs de execução como candidate skills.

1. Criar `SkillMiner` em `crates/zylos-skills/src/miner.rs`
2. Parsear gateway.log com regex patterns
3. Extrair input/output sequences repetidas (>3 ocorrências)
4. Gerar candidate SKILL.md draft
5. Submeter ao EvalFlywheel para validação automática
6. Notificar via EventBus quando candidate aprovado

**Entregável Verificável:** Miner + parser + generator + integration

**Critérios de Aceitação:**
- [ ] Miner identifica padrões reais em log sintético
- [ ] Candidate SKILL.md válido segundo schema
- [ ] EvalFlywheel gate previne falso-positivos
- [ ] Notification chega ao subscriber
- [ ] Overhead <5% CPU durante mining

**Out of Scope:** Incremental mining; distributed mining; ML-based pattern recognition

**Gate de Verificação:** `cargo test -p zylos-skills miner` → todos verdes + candidate aprovado em dataset teste

---

#### T8.4.4 — TrajectoryCompiler
**Dependências:** T8.4.1, T8.4.3
**Descrição:** Converte runbook executions em skill contracts.

1. Criar `TrajectoryCompiler` em `crates/zylos-skills/src/trajectory.rs`
2. Parsear HANDOFF_*.md e playbooks/*.yaml
3. Extrair inputs/outputs/assertions automaticamente
4. Gerar candidate SKILL.md com contract preenchido
5. Integrar com SkillMiner para deduplication

**Entregável Verificável:** Compiler + parsers + generator

**Critérios de Aceitação:**
- [ ] Compiler extrai campos corretos de HANDOFF válido
- [ ] Playbook YAML parseado sem erros
- [ ] Output contract válido segundo schema
- [ ] Deduplication evita candidates duplicados
- [ ] Teste com 10+ handoffs reais passa

**Out of Scope:** Semantic understanding; multi-step trajectory chaining; human-in-the-loop refinement

**Gate de Verificação:** `cargo test -p zylos-skills trajectory` → todos verdes + output válido

---

#### T8.4.5 — SkillHierarchy 3-Tier
**Dependências:** T8.4.1
**Descrição:** Hierarquia Planning/Functional/Atomic com filtering no IndexRegistry.

1. Definir enum `SkillTier` em `crates/zylos-skills/src/hierarchy.rs`
2. Adicionar campo `tier` ao index.json schema
3. Implementar filtering hierárquico no IndexRegistry
4. Migrar skills existentes para tiers apropriados
5. Documentar convenções em docs/SKILL_HIERARCHY.md

**Entregável Verificável:** Enum + schema update + filtering + migration

**Critérios de Aceitação:**
- [ ] Filtering por tier retorna subset correto
- [ ] Skills existentes migradas sem breaking changes
- [ ] Schema validation rejeita tier inválido
- [ ] Documentation clara com exemplos
- [ ] Performance de lookup não degradada

**Out of Scope:** Tier promotion automation; cross-tier dependencies; UI hierarchy browser

**Gate de Verificação:** `cargo test -p zylos-skills hierarchy` → todos verdes + migration script roda

---

### Entregável da Fase
Skill contracts formais, evolution graph, miner automático, trajectory compiler e hierarquia 3-tier, todos integrados e documentados.

### Critérios de Aceitação da Fase
- [ ] Todos os 5 gates de tarefa verdes
- [ ] `cargo test -p zylos-skills` → 100% pass rate
- [ ] Pelo menos 3 candidate skills mineradas e aprovadas em dataset teste
- [ ] Documentação atualizada em docs/SKILL_EVOLUTION.md

### Out of Scope da Fase
- RL-based skill optimization
- Distributed skill mining
- Advanced graph visualization

### Gate de Verificação da Fase
```bash
cargo test -p zylos-skills && \
zylos skills validate --all && \
ls target/skill_candidates/ | wc -l
```
Testes verdes + validação passa + candidates gerados.

---

## Fase F8.5 — AI Employee Governance (Semanas 14-15)

### Objetivo
Padronizar employee roles, implementar plan-first gate, rate limiting por persona e guardrails enforcement para segurança operacional.

### Tarefas Numeradas

#### T8.5.1 — EmployeeRole Enum
**Dependências:** F8.4 completa
**Descrição:** 8 roles padronizados com personality presets e routine sets.

1. Definir enum `EmployeeRole` em `crates/zylos-personalities/src/roles.rs`
2. Mapear cada role para personality preset existente
3. Associar routine set padrão por role
4. Persistir em goals.personalities
5. Expor via API `GET /employees/roles`

**Entregável Verificável:** Enum + mappings + persistence + API

**Critérios de Aceitação:**
- [ ] 8 roles definidos: Researcher, Writer, Reviewer, Coder, Tester, DevOps, Analyst, Coordinator
- [ ] Cada role mapeia para personality válida
- [ ] API retorna lista completa
- [ ] Backward compat com personalities existentes
- [ ] Teste de mapping completeness passa

**Out of Scope:** Custom role creation; role inheritance; role permissions

**Gate de Verificação:** `cargo test -p zylos-personalities roles` → todos verdes + API 200

---

#### T8.5.2 — RoutineCatalog
**Dependências:** T8.5.1
**Descrição:** Index.json de rotinas com schema validation.

1. Criar schema JSON para rotinas: `{ id, role, trigger, steps, guardrails }`
2. Implementar loader em `crates/zylos-personalities/src/routines.rs`
3. Validar schema com jsonschema em boot
4. Expor via API `GET /employees/routines?role=X`
5. Documentar catálogo em docs/ROUTINE_CATALOG.md

**Entregável Verificável:** Schema + loader + validation + API + docs

**Critérios de Aceitação:**
- [ ] Loader valida schema antes de carregar
- [ ] Rotina inválida rejeitada com erro descritivo
- [ ] Filtering por role funciona corretamente
- [ ] Catálogo documentado com exemplos
- [ ] Hot-reload de rotinas novas

**Out of Scope:** Routine composition; conditional steps; routine versioning

**Gate de Verificação:** `cargo test -p zylos-personalities routines` → todos verdes + API filtering funciona

---

#### T8.5.3 — PlanFirstGate
**Dependências:** T8.5.1
**Descrição:** Middleware que bloqueia execução sem plano aprovado.

1. Criar middleware `PlanFirstGate` em `crates/zylos-hitl/src/plan_gate.rs`
2. Verificar `PlanApproved` event antes de employee spawn
3. Integrar com HITL Approval Panel
4. Bloquear spawn se gate não satisfeito
5. Log tentativas bloqueadas para audit

**Entregável Verificável:** Middleware + integração + logging

**Critérios de Aceitação:**
- [ ] Spawn sem approval retorna `Err(PlanRequired)`
- [ ] Spawn com approval prossegue normalmente
- [ ] HITL panel mostra pending plans
- [ ] Audit log captura bloqueios
- [ ] Config bypass para testing disponível

**Out of Scope:** Auto-approval heuristics; plan templates; plan expiration

**Gate de Verificação:** `cargo test -p zylos-hitl plan_gate` → todos verdes + audit log entries

---

#### T8.5.4 — EmployeeRateLimiter
**Dependências:** T8.5.1
**Descrição:** Rate limiting por role usando Tower middleware.

1. Implementar `EmployeeRateLimiter` em `crates/zylos-personalities/src/rate_limit.rs`
2. Usar `tower::limit::RateLimitLayer`
3. Configurar limites por role via config.toml
4. Expor métrica `zylos_employee_rate_limited_total{role}`
5. Retornar 429 Too Many Requests quando limitado

**Entregável Verificável:** Limiter + config + métricas

**Critérios de Aceitação:**
- [ ] Rate limit aplicado corretamente por role
- [ ] Config hot-reload funciona
- [ ] 429 retornado quando limite excedido
- [ ] Métrica incrementa corretamente
- [ ] Burst allowance configurável

**Out of Scope:** Adaptive rate limiting; per-user limits; rate limit headers

**Gate de Verificação:** `cargo test -p zylos-personalities rate_limit` → todos verdes + métrica visível

---

#### T8.5.5 — GuardrailEnforcer
**Dependências:** T8.5.3
**Descrição:** Verifica precondições antes de qualquer ação mutável.

1. Criar `GuardrailEnforcer` em `crates/zylos-agent-shield/src/guardrails.rs`
2. Implementar checks: no_auto_send, human_approves, max_scope
3. Integrar em todos os mutation endpoints
4. Falhar fast se violação detectada
5. Log violações para audit e alerting

**Entregável Verificável:** Enforcer + checks + integração + logging

**Critérios de Aceitação:**
- [ ] no_auto_send valida recipient antes de send
- [ ] human_approves verifica approval token
- [ ] max_scope valida tenant boundary
- [ ] Violação retorna 403 com reason
- [ ] Audit log + alerting funcionando

**Out of Scope:** Dynamic guardrails; guardrail composition; violation remediation suggestions

**Gate de Verificação:** `cargo test -p zylos-agent-shield guardrails` → todos verdes + audit entries

---

### Entregável da Fase
Employee roles padronizados, routine catalog, plan-first gate, rate limiting e guardrail enforcement, todos integrados e seguros.

### Critérios de Aceitação da Fase
- [ ] Todos os 5 gates de tarefa verdes
- [ ] `cargo test -p zylos-personalities -p zylos-hitl -p zylos-agent-shield` → 100% pass rate
- [ ] Security review aprova guardrails implementation
- [ ] Documentação atualizada em docs/EMPLOYEE_GOVERNANCE.md

### Out of Scope da Fase
- Custom role creation UI
- Adaptive rate limiting
- Guardrail auto-generation

### Gate de Verificação da Fase
```bash
cargo test -p zylos-personalities -p zylos-hitl -p zylos-agent-shield && \
cargo audit
```
Todos devem retornar exit code 0.

---

## Fase F8.6 — Desktop UX & Mobile Relay (Semanas 16-17)

### Objetivo
Melhorar UX do cockpit com workspace persistence, agent awareness states, plugin system extensível e relay server para mobile.

### Tarefas Numeradas

#### T8.6.1 — WorkspacePersistence
**Dependências:** F8.5 completa
**Descrição:** Serializar estado de abas/filtros/seleções para SQLite.

1. Implementar persistence em `apps/zylos-cockpit/src/workspace.rs`
2. Usar Leptos `create_resource` para async load/save
3. Serializar via `bincode` para eficiência
4. Restaurar estado em mount
5. Adicionar botão "Reset Workspace" na UI

**Entregável Verificável:** Persistence module + UI integration

**Critérios de Aceitação:**
- [ ] Estado restaurado corretamente após refresh
- [ ] Save/load latency <100ms
- [ ] Reset button limpa estado persistente
- [ ] Corrupt state handled gracefully
- [ ] Multi-tab support funcionando

**Out of Scope:** Workspace sharing; workspace templates; cloud sync

**Gate de Verificação:** Checklist automatizado via `testing-checklist-author`:
- [ ] Abrir cockpit, modificar 3+ elementos de estado (aba, filtro, seleção)
- [ ] Refresh da página → todos os 3+ elementos restaurados corretamente
- [ ] Corromper arquivo de persistência → cockpit carrega com defaults sem crash
- [ ] Reset Workspace button limpa estado persistente e recarrega defaults
- [ ] Multi-tab: abrir 2 abas, modificar estado em ambas, refresh → ambas restauradas
- [ ] Save/load latency medido <100ms (via browser devtools network tab)

---

#### T8.6.2 — AgentAwarenessState Enum
**Dependências:** T8.6.1
**Descrição:** Estados cognitivos do agente visíveis no cockpit.

1. Definir enum `AgentAwarenessState` em `crates/zylos-core/src/awareness.rs`
2. Variantes: Thinking, WaitingApproval, Executing, Idle, Error
3. Expor via WebSocket no gateway
4. Renderizar indicadores visuais no cockpit
5. Integrar com EventBus para updates em tempo real

**Entregável Verificável:** Enum + WebSocket + UI indicators

**Critérios de Aceitação:**
- [ ] State transitions refletidas em <500ms na UI
- [ ] Indicadores visuais distintos por state
- [ ] WebSocket reconnect mantém state
- [ ] Error state mostra diagnostic info
- [ ] Accessibility labels presentes

**Out of Scope:** State history timeline; state prediction; custom states

**Gate de Verificação:** Checklist automatizado via `testing-checklist-author`:
- [ ] Trigger agent spawn → UI mostra "Thinking" em <500ms
- [ ] Agent aguardando approval → UI mostra "WaitingApproval" com botão de ação
- [ ] Agent executando tarefa → UI mostra "Executing" com indicador de progresso
- [ ] Agent idle → UI mostra "Idle" após conclusão
- [ ] Agent erro → UI mostra "Error" com mensagem de diagnóstico clicável
- [ ] WebSocket desconecta/reconecta → state mantém consistência sem flicker
- [ ] Todos os indicadores têm labels de acessibilidade (aria-label ou equivalente)

---

#### T8.6.3 — CockpitPlugin Trait
**Dependências:** T8.6.1
**Descrição:** Sistema de plugins extensível para cockpit.

1. Definir trait `CockpitPlugin` em `crates/zylos-cockpit-api/src/plugin.rs`
2. Usar `libloading` para dynamic loading
3. Implementar plugin discovery via plugins/ directory
4. Sandbox plugins em WASM ou native com capability tokens
5. Documentar plugin authoring guide

**Entregável Verificável:** Trait + loader + discovery + docs

**Critérios de Aceitação:**
- [ ] Plugin nativo carrega e renderiza corretamente
- [ ] Plugin inválido não crasha cockpit
- [ ] Discovery encontra plugins em directory
- [ ] Capability tokens enforced
- [ ] Authoring guide completo com exemplo

**Out of Scope:** Plugin marketplace; plugin versioning; hot-reload

**Gate de Verificação:** Load example plugin → renders without errors + capability check passes

---

#### T8.6.4 — RelayServer Mobile
**Dependências:** T8.6.2
**Descrição:** Axum proxy server para mobile ↔ gateway communication.

1. Criar binary `apps/zylos-relay/` no workspace
2. Implementar Axum server com JWT auth
3. Proxy requests mobile ↔ gateway
4. Reutilizar MobileMCP existente
5. Adicionar health check endpoint `/health`

**Entregável Verificável:** Binary + auth + proxy + health check

**Critérios de Aceitação:**
- [ ] Mobile client conecta via JWT
- [ ] Requests proxyed corretamente bidirecionalmente
- [ ] Auth rejeita tokens inválidos
- [ ] Health check retorna 200
- [ ] Performance <50ms overhead por request

**Out of Scope:** Offline mode; push notifications; mobile SDK

**Gate de Verificação:** `cargo run -p zylos-relay` → health check 200 + mobile test client connects

---

### Entregável da Fase
Workspace persistence, agent awareness UI, plugin system e relay server mobile, todos funcionais e documentados.

### Critérios de Aceitação da Fase
- [ ] Todos os 4 gates de tarefa verdes
- [ ] Cockpit build WASM succeeds
- [ ] Relay server deployable como standalone binary
- [ ] Documentação atualizada em docs/DESKTOP_UX.md

### Out of Scope da Fase
- Plugin marketplace
- Mobile offline mode
- Cloud workspace sync

### Gate de Verificação da Fase
```bash
cargo build -p zylos-cockpit --target wasm32-unknown-unknown && \
cargo build -p zylos-relay && \
curl http://localhost:8080/health
```
Builds succeed + health check 200.

---

## Apêndice A: Estimativas Detalhadas

| Fase | Semanas | Tasks | Esforço Dev Senior |
|------|---------|-------|-------------------|
| F8.1 | 1-3 | 5 | 3 semanas |
| F8.2 | 4-6 | 4 | 3 semanas |
| F8.3 | 7-10 | 6 | 4 semanas |
| F8.4 | 11-13 | 5 | 3 semanas |
| F8.5 | 14-15 | 5 | 2 semanas |
| F8.6 | 16-17 | 4 | 2 semanas |
| **Total** | **1-17** | **29** | **17 semanas** |

Buffer recomendado: 2-3 semanas para imprevistos → **Total estimado: 17-20 semanas**.

### Setup de Ferramentas Externas (Pré-requisito para Gates)

Antes de executar qualquer gate de fase que use ferramentas externas, o ambiente de desenvolvimento deve ter:

```bash
# Security scanners (usados em F8.3 gate)
cargo install cargo-audit
cargo install cargo-deny

# Container/image scanner
# macOS via Homebrew:
brew install trivy
# macOS fallback (se Homebrew falhar):
curl -sfL https://raw.githubusercontent.com/aquasecurity/trivy/main/contrib/install.sh | sh -s -- -b /usr/local/bin
# Linux via apt:
wget https://github.com/aquasecurity/trivy/releases/download/v0.56.0/trivy_0.56.0_Linux-64bit.deb
sudo dpkg -i trivy_0.56.0_Linux-64bit.deb
# Linux fallback (sem sudo):
curl -sfL https://raw.githubusercontent.com/aquasecurity/trivy/main/contrib/install.sh | sh -s -- -b $HOME/.local/bin
export PATH="$HOME/.local/bin:$PATH"
# Windows via winget:
winget install aquasecurity.trivy
# Windows fallback (se winget falhar ou não estiver disponível):
# Download manual: https://github.com/aquasecurity/trivy/releases → trivy_0.56.0_Windows-64bit.zip
# Extrair para C:\Users\<user>\.local\bin\ e adicionar ao PATH

# cargo-audit (todas as plataformas)
cargo install cargo-audit
# Fallback se cargo install falhar (proxy/firewall):
# Download binário pré-compilado: https://github.com/rustsec/rustsec/releases
# Extrair e colocar no PATH

# Verificação de instalação
cargo audit --version
cargo deny --version
trivy --version

# Docker (obrigatório para F8.3.1 ResourceLimiter cross-platform)
# macOS: brew install --cask docker OU https://www.docker.com/products/docker-desktop/
# Linux: curl -fsSL https://get.docker.com | sh
# Windows: winget install Docker.DockerDesktop OU https://www.docker.com/products/docker-desktop/
# Fallback Windows (WSL2): wsl --install Ubuntu && dentro do WSL: curl -fsSL https://get.docker.com | sh
# Verificar: docker --version && docker run hello-world

# Linux cgroups v2 (obrigatório para F8.3.1 CgroupsLimiter)
mount | grep cgroup2 || echo "cgroups v2 não montado - verificar systemd"
# Fallback: se cgroups v2 indisponível, usar DockerLimiter backend (T8.3.2 Platform Abstraction Layer)
```

**Fallback:** Se `trivy` não estiver disponível após tentar todas as opções acima, o gate de F8.3 pode prosseguir apenas com `cargo audit` + `cargo deny`, documentando a ausência em `HANDOFF_F8.3.md` como risk accepted. Se `cargo-audit` também falhar, usar `cargo deny check` como substituto mínimo obrigatório.

## Apêndice B: Cross-Links Obrigatórios

- PRD.md → Seções de requisitos funcionais correspondentes
- ARCHITECTURE.md → Diagramas de componentes atualizados
- TASKS.md → Tasks decompostas deste plano
- HANDOFF_F8.[N].md → Protocolo de handoff entre fases
- SECURITY.md → Threat model atualizado com novas mitigations
- DESIGN_SYSTEM.md → Novos componentes UI adicionados
- [AGENTS.md](./AGENTS.md) → Regras de agente atualizadas com novos boundaries (HopCount, CapabilityToken, GuardrailEnforcer)
- [RULES.md](./RULES.md) → Convenções de código e segurança reforçadas (egress whitelist, output sanitization, rate limits)

## Apêndice D: Mapeamento de Skills Hermes por Tarefa

Para cada tarefa T8.X.Y, as skills abaixo **DEVEM** ser invocadas antes ou durante a execução. Carregar com `skill_view(name='...')`.

### Fase F8.1 — Orchestration Safety & Resilience

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.1.1 HopCountCircuitBreaker | `spec-author`, `test-driven-development`, `verification-loop` | Antes/Durante | Spec técnica do middleware; testes antes do código; verificação contínua |
| T8.1.2 GoalProgram Struct | `spec-author`, `acceptance-criteria-author`, `test-driven-development` | Antes | Definição formal da struct; critérios de serialização testáveis |
| T8.1.3 AutoloopProtocol Async | `spec-author`, `architecture-author`, `handoff-author` | Antes | Protocolo requer spec de mensagem; arquitetura async; handoff entre P2P e scheduler |
| T8.1.4 VerificationAgent Separado | `architecture-author`, `isolated-audit`, `test-driven-development` | Antes/Durante | Crate independente exige design de isolamento; auditoria isolada valida ausência de contexto |
| T8.1.5 CrashSimulator Tests | `testing-checklist-author`, `test-driven-development`, `verification-loop` | Antes/Durante | Checklist de cenários de crash; testes como deliverable primário |

### Fase F8.2 — Memory Lifecycle & Evolution

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.2.1 MemoryLifecycle Trait | `spec-author`, `architecture-author`, `test-driven-development` | Antes | Trait contract formal; integração com ContextGraph existente |
| T8.2.2 MentalModelStore | `spec-author`, `access-control-author`, `test-driven-development` | Antes | User isolation = access control; schema de dados formal |
| T8.2.3 ContextRoom Struct | `spec-author`, `architecture-author`, `acceptance-criteria-author` | Antes | Bundling semântico requer spec de snapshot/versioning |
| T8.2.4 AutoImprovementLoop | `continuous-monitoring`, `verification-loop`, `test-driven-development` | Durante | Background task = monitoring contínuo; EvalFlywheel = verification loop |

### Fase F8.3 — Security Hardening & Resource Control

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.3.1 ResourceLimiter cgroups | `spec-author`, `security-pr-audit`, `test-driven-development` | Antes/Durante | Security-critical; platform-specific spec necessária |
| T8.3.2 Platform Abstraction Layer | `architecture-author`, `spec-author` | Antes | Abstração cross-platform é decisão arquitetural |
| T8.3.3 NetworkEgressFilter | `access-control-author`, `security-pr-audit`, `test-driven-development` | Antes/Durante | Whitelist = access control; security-critical |
| T8.3.4 OutputSanitizer Pipeline | `security-pr-audit`, `test-driven-development`, `verification-loop` | Durante | Sanitization = security; property-based testing obrigatório |
| T8.3.5 CapabilityToken System | `access-control-author`, `security-pr-audit`, `spec-author` | Antes | Token capabilities = access control formal; spec de revocation |
| T8.3.6 HostAllocatorLimiter | `spec-author`, `test-driven-development`, `continuous-monitoring` | Antes/Durante | Métrica heap = monitoring; graceful shutdown testável |

### Fase F8.4 — Skill Contracts & Self-Evolution

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.4.1 SkillContract Formal | `spec-author`, `hermes-agent-skill-authoring`, `test-driven-development` | Antes | Contract = spec de skill; schema SKILL.md formal |
| T8.4.2 SkillEvolutionGraph | `architecture-author`, `spec-author`, `test-driven-development` | Antes | Grafo = decisão arquitetural; operações atômicas testáveis |
| T8.4.3 SkillMiner from Logs | `hermes-agent-skill-authoring`, `verification-loop`, `continuous-monitoring` | Durante | Gera SKILL.md candidates; EvalFlywheel = verification; log analysis = monitoring |
| T8.4.4 TrajectoryCompiler | `hermes-agent-skill-authoring`, `handoff-author`, `test-driven-development` | Durante | Converte HANDOFF_*.md → SKILL.md; parser testável |
| T8.4.5 SkillHierarchy 3-Tier | `architecture-author`, `hermes-agent-skill-authoring`, `spec-author` | Antes | Hierarquia = decisão arquitetural; migration spec necessária |

### Fase F8.5 — AI Employee Governance

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.5.1 EmployeeRole Enum | `spec-author`, `agents-rules-author`, `test-driven-development` | Antes | Roles = rules de agente; mapping completeness testável |
| T8.5.2 RoutineCatalog | `spec-author`, `agents-rules-author`, `acceptance-criteria-author` | Antes | Rotinas = regras de execução; schema validation formal |
| T8.5.3 PlanFirstGate | `plan-mode`, `verification-loop`, `test-driven-development` | Antes/Durante | Gate = plan enforcement; bloqueio testável |
| T8.5.4 EmployeeRateLimiter | `spec-author`, `access-control-author`, `continuous-monitoring` | Antes/Durante | Rate limit = access control; métricas = monitoring |
| T8.5.5 GuardrailEnforcer | `security-pr-audit`, `access-control-author`, `verification-loop` | Durante | Guardrails = security; precondições = verification |

### Fase F8.6 — Desktop UX & Mobile Relay

| Task | Skills Obrigatórias | Momento | Justificativa |
|------|-------------------|---------|---------------|
| T8.6.1 WorkspacePersistence | `spec-author`, `test-driven-development` | Antes | Persistence contract formal; state restoration testável |
| T8.6.2 AgentAwarenessState Enum | `spec-author`, `app-flow-author`, `test-driven-development` | Antes | State machine = app flow; WebSocket integration testável |
| T8.6.3 CockpitPlugin Trait | `architecture-author`, `access-control-author`, `hermes-agent-skill-authoring` | Antes | Plugin system = arquitetura extensível; capability tokens; authoring guide = skill doc |
| T8.6.4 RelayServer Mobile | `architecture-author`, `security-pr-audit`, `test-driven-development` | Antes/Durante | Proxy = decisão arquitetural; JWT auth = security |

### Resumo de Skills

- **Skills únicas mapeadas:** 14 das 28 disponíveis
- **Mais frequentes:** `test-driven-development` (27/29), `spec-author` (24/29), `verification-loop` (10/29), `access-control-author` (9/29), `security-pr-audit` (8/29)
- **Não aplicáveis a este plano:** `prd-author`, `design-system-author`, `project-structure-author`, `sop-author`, `pop-author`, `playbook-author`, `runbook-builder`, `soar-orchestration`, `mordida-protocol`, `subagent-driven-dev`, `dor-gate`, `professional-loop`, `project-memory-author`

---

## Apêndice E: Templates de Prompts Estruturados

Os 6 templates do Manifesto v3.1 devem ser usados nas tarefas correspondentes. Carregar via `skill_view(name='review-prompts')`.

| Template | Quando Usar | Tasks Aplicáveis |
|----------|-------------|------------------|
| Revisão de PRD | Antes de iniciar qualquer fase | Todas (validar requisitos) |
| Revisão de APP_FLOW | Antes de tasks de UI/UX | T8.6.1, T8.6.2, T8.6.3 |
| Revisão de Fase | Ao concluir cada fase F8.N | Gates de todas as fases |
| Revisão de Spec | Antes de implementar cada tarefa | T8.1.1–T8.6.4 (todas) |
| Revisão de Handoff | Ao transicionar entre fases | Entre F8.1→F8.2→...→F8.6 |
| Revisão de Benchmark | Após performance/security tests | T8.1.5, T8.3.1, T8.3.3, T8.3.6 |

**Regra:** Nenhum artefato pode ser aprovado sem passar pelo template correspondente. O output do template deve ser anexado ao HANDOFF_F8.[N].md da fase.

---

## Apêndice C: Compliance Manifesto v3.1

Este plano satisfaz todas as 10 camadas obrigatórias:

1. ✅ Conceitos Canônicos (22 regras) — Vertical slices, gates, atomicidade
2. ✅ Princípios Anti-Slop (7 filtros) — Critérios testáveis, sem subjetividade
3. ✅ Loop Profissional — READ→UNDERSTAND→PLAN→IMPLEMENT→TEST→REVIEW→FIX→COMMIT→UPDATE DOCS
4. ✅ Penalidade — Gate vermelho = trabalho inválido, não avança
5. ✅ Arquivos Obrigatórios — 15 docs referenciados e atualizados
6. ✅ Templates de Prompts — 6 templates usados nas tasks
7. ✅ Matriz de Rastreabilidade — Tabela completa acima
8. ✅ Integração com Harness — Tests integrados ao CI
9. ✅ Skills Hermes Documentais — 28 skills invocáveis
10. ✅ Protocolo de Evolução + Lições — Feedback loops em T8.2.4 e T8.4.3

---

*Documento gerado em conformidade com Manifesto v3.1 e skill implementation-plan-author. Última atualização: 2026-11-11.*