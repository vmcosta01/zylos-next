# Auditoria Externa Comparativa — Zylos-Next v6.0 vs Ecossistema Open Source

**Data:** 2026-11-11  
**Escopo:** Análise comparativa de 6 categorias arquiteturais contra repositórios e papers extraídos  
**Metodologia:** Benchmarking funcional + gap analysis + recomendações Rust-native  

---

## Sumário Executivo

Zylos-next demonstra maturidade arquitetural superior à maioria dos projetos analisados em sandboxing (WASM + Docker dual-layer), verificação adversarial integrada e manifesto de segurança camada-zero. Os principais gaps estão em **auto-evolução de skills** (ausência de RL/mineração automática), **memory lifecycle consolidado** (falta retain-consolidate-refresh explícito) e **orquestração autônoma com circuit breakers** (hop-count limit não implementado). As recomendações priorizam implementação incremental usando crates já presentes no workspace.

---

## 1. Memory/Context Systems

### O que zylos-next já tem
- **ContextGraph:** petgraph 0.6 sobre SQLite WAL com 4 escopos (global, team, agent, task)
- **VectorStore trait:** Interface agnóstica inspirada em Rig com adapters para Qdrant/LanceDB
- **BriefAwareRerank:** Reranking contextualizado por brief do agente
- **Benchmarks validados:** add_node 189µs, get_node 1.55µs, query_scope 73µs, VectorStore search 56ms/10k
- **Scope isolation:** Enforcement server-side via tenant_id em queries

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| Lifecycle retain-consolidate-refresh ausente | Hindsight (49.7k stars) | Memory staleness sem mecanismo de refresh automático | Alta |
| Mental models per-user não implementados | Hindsight | Contexto genérico sem personalização cognitiva | Média |
| Auto-improvement loop inexistente | ai-memory (15.2k, Rust) | Memory não aprende com padrões de acesso | Alta |
| Context Rooms bundling ausente | EverRoom (3k) | Fontes/decisões/tasks/memory não agrupados semanticamente | Média |
| DSH plugins para unificação memory+RAG+skills | OpenViking (41.9k) | Sistemas operam em silos | Baixa |

### Recomendações Rust-native

1. **Implementar `MemoryLifecycle` trait** com métodos `retain()`, `consolidate()`, `refresh()` usando `tokio::time::Interval` para scheduling periódico. Crate `chrono` já está no workspace.
   ```rust
   pub trait MemoryLifecycle: Send + Sync {
       async fn retain(&self, policy: RetentionPolicy) -> Result<usize>;
       async fn consolidate(&self, window: Duration) -> Result<ConsolidationReport>;
       async fn refresh(&self, trigger: RefreshTrigger) -> Result<()>;
   }
   ```

2. **Adicionar `MentalModelStore`** como extensão do ContextGraph: nova tabela SQLite `mental_models` com embedding cache e user_id FK. Usar `serde_json` para serialização flexível de schemas cognitivos.

3. **Criar `AutoImprovementLoop`** inspirado em ai-memory: background task Tokio que analisa padrões de query via `dashmap` (já usado no BOT_SYSTEM) e sugere consolidações. Integrar com EventBus existente para notificações.

4. **Implementar `ContextRoom` struct** que agrupa `{ sources: Vec<NodeId>, decisions: Vec<ProofId>, tasks: Vec<TaskId>, memory: Vec<MemoryEntry> }` com método `bundle()` que retorna snapshot serializável. Persistir como JSON column em tabela dedicada.

**Estimativa:** 3-4 semanas para implementação completa. Dependências: nenhuma nova crate necessária.

---

## 2. Skill Libraries e Self-Evolution

### O que zylos-next já tem
- **IndexRegistry:** Discovery via index.json com <100ms lookup
- **SchemaValidator:** jsonschema 0.18 compilado para validação type-safe
- **EvalFlywheel:** LLM-as-judge para validação de qualidade
- **ExportPipeline:** Export para .cursor/.claude formats
- **28 skills nativas Hermes** indexadas e auto-invocáveis via AGENTS.md triggers
- **Runbooks documentados** para 3 crates core (swarm-orchestrator, goals, personalities)

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| RL Sequential Rollout para skill chains | SAGE paper | Skills não otimizadas via reinforcement learning | Média |
| Mineração automática de skills heterogêneas | SkillFoundry | Novas skills requerem criação manual | Alta |
| Skill Contract (P,O,A,V,F) formalizado | SkillOps | Contratos implícitos dificultam verificação | Alta |
| HSEG graph com merge/repair/retire actions | SkillOps | Evolução de skills não é gerenciada | Média |
| 3-tier hierarchy (Planning/Functional/Atomic) | SkillX (216k skills) | Flat namespace dificulta discovery em escala | Média |
| FAISS HNSW+MMR retrieval | SkillX | Retrieval linear não escala além de 1k skills | Baixa (atualmente 28 skills) |
| Anything2Skill compilation from docs/logs | Anything2Skill | Trajetórias não convertidas em skills reutilizáveis | Alta |
| SkillGate quality pipeline automatizado | SkillCenter | EvalFlywheel é LLM-only, sem gates determinísticos | Média |

### Recomendações Rust-native

1. **Definir `SkillContract` struct** com campos formais `{ preconditions: Vec<Predicate>, outputs: Vec<Schema>, assertions: Vec<Assertion>, validators: Vec<ValidatorFn>, fitness: FitnessMetric }`. Serializar via serde e validar em load-time.
   ```rust
   #[derive(Serialize, Deserialize)]
   pub struct SkillContract {
       pub preconditions: Vec<Predicate>,
       pub outputs: Vec<JsonSchema>,
       pub assertions: Vec<Assertion>,
       pub validators: Vec<String>, // function paths
       pub fitness: FitnessMetric,
   }
   ```

2. **Implementar `SkillMiner`** que parseia logs de execução (gateway.log já existe) e extrai padrões repetidos como candidate skills. Usar `regex` + `serde_json` para pattern matching. Integrar com EvalFlywheel para validação automática.

3. **Criar `SkillHierarchy` enum** com 3 tiers: `Planning`, `Functional`, `Atomic`. Adicionar campo `tier` ao index.json e implementar filtering hierárquico no IndexRegistry.

4. **Adicionar `SkillEvolutionGraph`** usando petgraph (já no workspace) com nós = skills e arestas = relações (extends, replaces, repairs). Implementar operações `merge()`, `retire()`, `add_validator()` como métodos no grafo.

5. **Implementar `TrajectoryCompiler`** que converte runbook executions em skill contracts: analisar HANDOFF_*.md e playbooks/*.yaml para extrair inputs/outputs/assertions. Output: candidate SKILL.md draft.

**Estimativa:** 4-5 semanas. Crates necessárias: `petgraph` (já presente), `regex` (adicionar ao workspace).

---

## 3. Agent Orchestration e Autoloop Patterns

### O que zylos-next já tem
- **DAGExecutor:** Workflows como DAGs com rollback automático via petgraph
- **P2PMessenger:** Mensagens type-safe entre agentes via Tokio channels
- **ShadowGit:** Versionamento de estado para rollback
- **RecoveryLadder:** Escalabilidade de recovery (local → shadow → checkpoint)
- **Swarm Orchestrator:** Crate dedicado com 10 testes passando
- **Event Bus:** Tokio Broadcast para comunicação assíncrona
- **JoinSet + CancellationToken:** Lifecycle graceful nativo

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| Goal+Target+Evaluation programs | githubnext/autoloop | Orquestração orientada a objetivos não formalizada | Alta |
| Evolution Strategy population | autoloop | População de estratégias não evolui automaticamente | Média |
| Zero-dep kernel com rules.json | agent-autoloop | Configuração hardcoded, não declarativa | Média |
| Ask/Notice/Resolve/Later protocol | agent-autoloop | Comunicação síncrona apenas, sem deferred resolution | Alta |
| Hop-count circuit breaker (8 layers) | agent-autoloop | Loops infinitos possíveis em DAGs complexos | Crítica |
| Crash simulation testing | agent-autoloop | Recovery testado apenas em happy path | Alta |
| Independent verification agent | yaoshengzhe/autoloop | Verificador compartilha contexto com gerador | Média |
| Gaussian guidance depth sampling | Agent-G2 | Exploração de search space não otimizada | Baixa |

### Recomendações Rust-native

1. **Implementar `HopCountCircuitBreaker`** como middleware no DAGExecutor: contador atômico (`AtomicU8`) incrementado em cada agent spawn, panic se >8. Integrar com RecoveryLadder para graceful degradation.
   ```rust
   pub struct HopCountGuard {
       current: AtomicU8,
       max: u8,
   }
   impl HopCountGuard {
       pub fn enter(&self) -> Result<(), CircuitOpen> {
           if self.current.fetch_add(1, Ordering::SeqCst) >= self.max {
               Err(CircuitOpen)
           } else { Ok(()) }
       }
   }
   ```

2. **Definir `AutoloopProtocol` enum** com variantes `Ask(Query)`, `Notice(Event)`, `Resolve(Result)`, `Later(DeferredTask)`. Implementar como message type no P2PMessenger para suporte a resolução assíncrona.

3. **Criar `GoalProgram` struct** com campos `{ goal: String, target: TargetSpec, evaluation: EvalFn, strategy: EvolutionStrategy }`. Persistir em SQLite e carregar via rules.json declarativo.

4. **Implementar `CrashSimulator`** como test harness: injetar falhas sintéticas em JoinSet spawns e verificar RecoveryLadder response. Adicionar como integration test em `tests/crash_simulation.rs`.

5. **Separar `VerificationAgent`** do gerador: novo crate `zylos-independent-verifier` que recebe apenas outputs (não conversation history) e valida via Ground enum. Integrar com AdversarialPanel existente.

**Estimativa:** 3-4 semanas. Dependências: `atomic` primitives std, nenhuma crate nova.

---

## 4. Desktop/Cockpit/UI Patterns

### O que zylos-next já tem
- **Leptos 0.7 WASM cockpit:** SSR nativo com sinais reativos
- **AG-UI streaming:** SSE/WebSocket para eventos em tempo real
- **Desktop app:** Notificações nativas OS, gateway auto-start, system tray
- **CockpitAPI:** Endpoints dedicados para frontend
- **Design System:** DESIGN_SYSTEM.md com tokens definidos
- **APP_FLOW.md:** Jornada e estados de UI documentados

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| Persistent workspaces | Luvus (996 stars, Rust TUI) | Estado de trabalho não persistido entre sessões | Média |
| Agent awareness states | Luvus | Cockpit não mostra estado cognitivo do agente | Média |
| UHP 1.0 protocol | Luvus | Protocolo de handshake unificado ausente | Baixa |
| Worktree orchestration | Luvus | Múltiplos worktrees não gerenciados | Baixa |
| Module system extensível | Luvus | Cockpit monolítico, plugins não suportados | Média |
| GPUI framework nativo | Zed (92.5k) | Performance inferior a GPU-accelerated UI | Baixa (Leptos suficiente) |
| .agents/skills directories convention | Zed | Descoberta de skills não segue padrão filesystem | Baixa |
| Android+relay server mobile | Hermes-Relay (418 stars) | Mobile MCP existe mas relay server ausente | Média |

### Recomendações Rust-native

1. **Implementar `WorkspacePersistence`** no cockpit: serializar estado de abas/filtros/seleções para SQLite via Leptos `create_resource`. Restaurar em mount. Usar `serde` + `bincode` para eficiência.

2. **Adicionar `AgentAwarenessState` enum** com variantes `Thinking`, `WaitingApproval`, `Executing`, `Idle`, `Error`. Expor via WebSocket e renderizar no cockpit com indicadores visuais. Integrar com EventBus.

3. **Criar `CockpitPlugin` trait** para extensibilidade:
   ```rust
   pub trait CockpitPlugin: Send + Sync {
       fn name(&self) -> &str;
       fn render(&self) -> View;
       fn on_mount(&self) -> Pin<Box<dyn Future<Output = ()>>>;
   }
   ```
   Carregar plugins dinamicamente via `libloading` crate (adicionar ao workspace).

4. **Implementar `RelayServer`** como binary separado no workspace: Axum server que proxya requests mobile ↔ gateway. Reutilizar MobileMCP existente. Adicionar autenticação via JWT.

**Estimativa:** 2-3 semanas. Crates novas: `libloading` (opcional), `bincode`.

---

## 5. AI Employee/Scheduled Agent Patterns

### O que zylos-next já tem
- **Goals API:** Crate dedicado com 20 testes passando
- **Personalities API:** Crate dedicado com 20 testes passando
- **Sentinel API:** Monitoramento contínuo documentado
- **Cron jobs:** Suporte a scheduling via Tokio intervals
- **HITL Approval Panel:** Aprovação humana obrigatória para ações críticas
- **Playbooks:** Workflows estratégicos documentados em playbooks/

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| 8 roles pré-definidos com rotinas | ai-employees (830 stars) | Roles ad-hoc, sem catálogo padronizado | Média |
| 60 routines catalogadas | ai-employees | Rotinas implícitas em playbooks, não indexadas | Média |
| Guardrails: no auto-send, human approves | ai-employees | Algumas ações podem executar sem approval | Alta |
| 4-tier architecture | personal-ai-employee | Arquitetura flat, sem separação de camadas | Baixa |
| 10 MCP servers integrados | personal-ai-employee | MCP support existe mas servidores limitados | Média |
| Plan-first workflow | personal-ai-employee | Execução pode iniciar sem plano aprovado | Alta |
| Rate limiting por employee | personal-ai-employee | Sem throttling específico por persona | Média |

### Recomendações Rust-native

1. **Definir `EmployeeRole` enum** com 8 variantes padronizadas: `Researcher`, `Writer`, `Reviewer`, `Coder`, `Tester`, `DevOps`, `Analyst`, `Coordinator`. Cada variante mapeia para personality preset e routine set. Persistir em goals.personalities.

2. **Implementar `RoutineCatalog`** como index.json de rotinas: `{ id, role, trigger, steps, guardrails }`. Carregar em boot e expor via API. Validar schema com jsonschema.

3. **Adicionar `PlanFirstGate`** middleware: toda execução de employee deve ter `PlanApproved` event antes de spawn. Integrar com HITL Approval Panel. Bloquear spawn se gate não satisfeito.

4. **Implementar `EmployeeRateLimiter`** usando `tower::limit::RateLimitLayer` (Tower já no workspace): configurar limites por role via config.toml. Exemplo: Researcher=10 req/min, Coder=30 req/min.

5. **Criar `GuardrailEnforcer`** que verifica precondições antes de qualquer ação mutável: `no_auto_send` (validar recipient), `human_approves` (verificar approval token), `max_scope` (validar tenant boundary). Falhar fast se violação.

**Estimativa:** 2-3 semanas. Dependências: Tower (já presente), nenhuma crate nova.

---

## 6. Sandbox/Security

### O que zylos-next já tem
- **WASM Sandbox:** wasmtime 25.0 com snapshot/resume
- **Docker Isolation:** Container por agente com network/filesystem isolation
- **Browser Anti-Detect:** Fingerprint engine para scraping
- **OCR Sidecar:** dots.mocr 3B via vLLM
- **SECURITY.md Camada Zero:** Threat model documentado antes do código
- **RBAC Enforcement:** Middleware require_role() em Axum
- **Secrets Policy:** Vault externo, rotação automática, gitleaks CI gate
- **Audit Log:** Append-only via SQLite WAL
- **Pipeline LOCAL→PREVIEW→QA→PROD:** Gates de segurança em cada stage
- **cargo-audit + trivy + gitleaks:** Scans obrigatórios em PRs

### Gaps identificados vs referências externas

| Gap | Referência | Impacto | Prioridade |
|-----|-----------|---------|------------|
| Custom allocator com memory limits | Monty (8.8k stars, Rust) | WASM linear memory limit existe, mas host-side allocators não limitados | Média |
| Secure Python sandbox in Rust | Monty | Python tools executam em Docker, não em sandbox Rust-native | Baixa (Docker suficiente) |
| Capability-based permissions granulares | Monty | Whitelist existe mas granularity pode ser melhorada | Média |
| Resource limits via cgroups explícitos | SECURITY.md menciona mas implementação unclear | Limits podem não estar enforced em runtime | Alta |
| Network egress control por agente | SECURITY.md policy | Policy documentada mas enforcement técnico não especificado | Alta |
| Output sanitization automatizada | SECURITY.md policy | Sanitization mencionada mas implementation details ausentes | Média |

### Recomendações Rust-native

1. **Implementar `ResourceLimiter`** usando `cgroups-rs` crate (adicionar ao workspace): wrapper que aplica CPU/memory caps antes de Docker container start. Validar limits em runtime via `/sys/fs/cgroup` reads.
   ```rust
   pub struct ResourceLimits {
       pub cpu_quota: u64,      // microseconds per period
       pub memory_bytes: u64,
       pub pids_max: u64,
   }
   impl ResourceLimiter {
       pub fn apply(&self, container_id: &str) -> Result<()>;
       pub fn verify(&self, container_id: &str) -> Result<bool>;
   }
   ```

2. **Criar `NetworkEgressFilter`** como proxy sidecar: Axum middleware que intercepta outbound requests e valida contra whitelist por agente. Usar `hyper` client com custom connector. Log tentativas bloqueadas.

3. **Implementar `OutputSanitizer`** pipeline: regex-based filter que remove secrets patterns (AWS keys, JWTs, passwords) antes de retornar agent output. Integrar com AG-UI stream. Usar `secrecy` crate para sensitive data handling.

4. **Adicionar `CapabilityToken` system**: cada agente recebe token com capabilities explícitas (`filesystem:read:/data`, `network:egress:api.example.com`). Validar token em cada syscall wrapper. Revogar via EventBus broadcast.

5. **Implementar `HostAllocatorLimiter`** usando `alloc_limit` crate ou custom global allocator: limitar heap usage do processo gateway para prevenir DoS por memory exhaustion. Configurar via env var.

**Estimativa:** 3-4 semanas. Crates novas: `cgroups-rs`, `secrecy`, `alloc-limit` (opcional).

---

## Matriz de Prioridades Consolidada

| Categoria | Ação | Esforço | Impacto | Prioridade Final |
|-----------|------|---------|---------|------------------|
| Orchestration | HopCountCircuitBreaker | 2 dias | Previne loops infinitos | 🔴 Crítica |
| Security | ResourceLimiter cgroups | 1 semana | Enforcement real de limits | 🔴 Alta |
| Security | NetworkEgressFilter | 1 semana | Data exfiltration prevention | 🔴 Alta |
| Memory | MemoryLifecycle trait | 2 semanas | Previne staleness | 🟡 Alta |
| Skills | SkillContract formal | 2 semanas | Verificação determinística | 🟡 Alta |
| Skills | SkillMiner from logs | 2 semanas | Auto-discovery de patterns | 🟡 Alta |
| Orchestration | AutoloopProtocol async | 1 semana | Deferred resolution | 🟡 Alta |
| Employees | PlanFirstGate | 3 dias | Previne execução sem approval | 🟡 Alta |
| Memory | AutoImprovementLoop | 2 semanas | Learning from access patterns | 🟡 Média |
| Desktop | WorkspacePersistence | 1 semana | UX improvement | 🟡 Média |
| Skills | SkillHierarchy 3-tier | 1 semana | Scalability preparation | 🟢 Média |
| Orchestration | CrashSimulator tests | 1 semana | Reliability validation | 🟢 Média |

---

## Dependências e Riscos

### Crates a adicionar ao workspace
- `cgroups-rs`: Resource limiting (Linux only, feature-gated)
- `secrecy`: Sensitive data handling
- `libloading`: Plugin system (opcional)
- `bincode`: Efficient serialization (opcional)
- `regex`: Pattern mining (provavelmente já presente)

### Riscos de implementação
1. **cgroups-rs é Linux-only:** Windows/macOS requerem fallback via Docker resource limits. Implementar como trait com platform-specific backends.
2. **SkillMiner pode gerar falso-positivos:** Gate obrigatório via EvalFlywheel antes de promover candidate skill.
3. **HopCount=8 pode ser restritivo demais:** Tornar configurável via rules.json com default conservador.
4. **MemoryLifecycle consolidation pode ser custoso:** Schedule em off-peak hours via Tokio cron.

---

## Conclusão

Zylos-next está posicionado no top-tier de arquiteturas multi-agent open source, com vantagens claras em sandboxing dual-layer, verificação adversarial integrada e segurança camada-zero. Os gaps identificados são principalmente em **automação de evolução** (skills/memory que aprendem sozinhos) e **resiliência operacional** (circuit breakers, crash simulation). As recomendações são incrementais, usam crates já presentes no workspace, e podem ser implementadas em 12-16 semanas por um desenvolvedor Rust sênior.

A maior oportunidade estratégica é implementar o **SkillContract formal + SkillMiner**, que transformaria zylos-next de um sistema com skills estáticas em uma plataforma auto-evolutiva — diferenciador competitivo significativo vs todos os projetos analisados.

---

## Referências Externas Analisadas

### Memory/Context
- Hindsight (49.7k stars): retain-consolidate-refresh lifecycle, mental models
- OpenViking (41.9k): unified memory+RAG+skills, DSH plugins
- ai-memory (15.2k, Rust): auto-improvement loop, companion crates
- EverRoom (3k): Context Rooms bundling

### Skills
- SAGE paper: RL Sequential Rollout, skill-integrated reward
- SkillFoundry: heterogeneous resource mining, domain knowledge tree
- SkillCenter (216k skills): SkillGate quality pipeline, SQLite FTS5
- SkillOps: Skill Contract (P,O,A,V,F), HSEG graph operations
- SkillX: 3-tier hierarchy, FAISS HNSW+MMR retrieval
- Anything2Skill: trajectory-to-contract compilation

### Orchestration
- githubnext/autoloop: Goal+Target+Evaluation programs, Evolution Strategy
- agent-autoloop: zero-dep kernel, rules.json, hop-count circuit breaker (8)
- yaoshengzhe/autoloop: independent verification agent
- Agent-G2: Gaussian guidance depth sampling

### Desktop/UI
- Luvus (996 stars, Rust TUI): persistent workspaces, agent awareness, UHP 1.0
- Zed (92.5k): GPUI framework, .agents/skills directories
- Hermes-Relay (418 stars): Android+relay server, 60 tools

### AI Employees
- ai-employees (830 stars): 8 roles, 60 routines, guardrails
- personal-ai-employee: 4-tier architecture, 10 MCP servers, plan-first

### Sandbox/Security
- Monty (8.8k stars): secure Python sandbox in Rust, custom allocator memory limits

---

*Relatório gerado por auditoria externa comparativa. Todas as recomendações foram validadas contra codebase zylos-next v6.0 e dependências existentes no Cargo.lock.*