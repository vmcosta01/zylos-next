# Playbook Estratégico — Zylos-Next v6.0

**Versão:** 1.0
**Data:** 31 de outubro de 2026
**Escopo:** Governança, tomada de decisão e fluxo estratégico para todas as fases (0–7)
**Público:** Tech Lead, Arquiteto, Product Owner, Agentes Orquestradores

---

## 1. Missão e Visão

### Missão
Construir um ecossistema de agentes IA **soberano em Rust**, com performance de nível sistema (<30MB RAM/agente, <50ms TTFD), segurança por design (sandboxing WASM + capabilities), e observabilidade nativa — sem dependência runtime de plugins externos para lógica core.

### Visão
Ser a referência em arquitetura multi-agente type-safe, onde cada componente é verificável, auditável e escalável horizontalmente via SQLite WAL + event bus.

### Valores Não-Negociáveis
1. **Soberania:** Zero dependência funcional de ecossistemas externos (Hermes, LangChain, CrewAI) para sistemas core
2. **Prova sobre Promessa:** Nenhum artefato é "pronto" sem benchmark, teste verde ou auditoria adversarial aprovada
3. **Segurança Camada Zero:** Threat model antes de código; capabilities antes de permissões
4. **Documentação Viva:** Docs atualizados ANTES do commit; nunca depois
5. **Loop Profissional Obrigatório:** READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS

---

## 2. Papéis e Responsabilidades (RACI)

| Papel | Responsabilidade Estratégica | Decisões que Pode Tomar | Escala Para |
|-------|------------------------------|-------------------------|-------------|
| **Tech Lead** | Arquitetura dos 6 sistemas core, aprovação de ADRs, gate de qualidade | Mudanças em traits públicas, tech stack, security model | Product Owner (mudanças de escopo) |
| **Arquiteto** | Design de contratos (specs/DESIGN.md), integração Hermes, threat model | Schemas JSON, envelope format, capability scope | Tech Lead (breaking changes) |
| **Product Owner** | Priorização de fases, definição de DoD, stakeholder alignment | Adiar/antecipar fases, ajustar métricas de sucesso | Steering Committee (orçamento >2 semanas) |
| **Agente Orquestrador** | Execução de runbooks, delegação para subagentes, verificação de gates | Retry de tasks falhas, seleção de perfil júnior/pleno/sênior | Tech Lead (blocker >4h) |
| **Auditor Adversarial** | Refutação de claims, validação anti-slop, dupla auditoria | Rejeitar entregáveis, exigir re-trabalho | Ninguém (independente por design) |
| **Memory Agent** | Consolidação noturna, detecção de contradições, decay político | Merge de memórias conflitantes, arquivamento automático | Auditor (contradições não-resolvidas) |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Gate de Entrada de Fase
Antes de iniciar qualquer fase (0–7), o Tech Lead deve validar:

```
[ ] Todos os documentos obrigatórios da fase anterior estão APPROVED
[ ] HANDOFF_N.md existe com 5 seções completas
[ ] Benchmarks da fase anterior passam nos targets definidos
[ ] Auditoria adversarial (dupla) está verde
[ ] Runbook da fase atual existe com 3 tiers (Sênior/Pleno/Júnior)
[ ] MEMORY.md reflete estado atual e lições aprendidas
```

**Se algum item falhar:** Fase bloqueada. Corrigir antes de prosseguir.

### 3.2 Critérios de Escalação

| Situação | Quem Decide | Tempo Máximo | Ação |
|----------|-------------|--------------|------|
| Blocker técnico <4h | Agente Orquestrador | 4h | Tentar workaround, logar em MEMORY.md |
| Blocker técnico >4h | Tech Lead | 8h | Reavaliar arquitetura, considerar pivot |
| Divergência de spec | Arquiteto + Tech Lead | 24h | ADR formal em DECISIONS.md |
| Mudança de escopo | Product Owner | 48h | Atualizar PRD.md + IMPLEMENTATION_PLAN.md |
| Falha recorrente em benchmark | Tech Lead | 72h | Spike de investigação, possível redesign |
| Contradição não-resolvida em memória | Auditor + Memory Agent | 24h | Arbitragem manual, documentar resolução |
| Security vulnerability crítica | Tech Lead + Security Reviewer | Imediato | Hotfix, rollback se necessário |

### 3.3 Protocolo de Pivot
Quando uma abordagem falha repetidamente (≥3 tentativas):

1. **Parar implementação** imediatamente
2. **Documentar falha** em MEMORY.md seção "Failed Approaches"
3. **Criar spike task** com timebox (max 8h)
4. **Avaliar alternativas** com matriz de trade-offs (performance vs complexidade vs soberania)
5. **Decidir via ADR** se mantém, adapta ou abandona
6. **Atualizar runbook** com lição aprendida

---

## 4. Governança de Qualidade

### 4.1 Princípios Anti-Slop (Filtros Obrigatórios)
Todo artefato passa por estes 7 filtros ANTES de ser considerado completo:

| # | Princípio | Como Verificar | Bloqueia Se |
|---|-----------|----------------|-------------|
| 1 | Nunca aceite "parece funcionar" | Benchmark/teste verde/screenshot/log | Declaração sem evidência |
| 2 | Nunca comece sem spec aprovada | Spec correspondente em specs/ | Código sem spec |
| 3 | Nunca pule estado de erro | 6 estados documentados por componente | Missing error handling |
| 4 | Nunca documente depois | Docs atualizados no mesmo commit | Commit sem docs |
| 5 | Nunca confie em memória de conversa | Artefatos em disco (MEMORY.md, DECISIONS.md) | Referência a conversa |
| 6 | Nunca promova sem benchmark | EVALUATION.md com métricas versionadas | Promoção sem dados |
| 7 | Nunca omita Out of Scope | Seção presente em PRD/specs | Ausência da seção |

### 4.2 Dupla Auditoria Adversarial
Todo entregável crítico (HANDOFF_*.md, specs, benchmarks) passa por:

1. **Auditor Gerador:** Cria o artefato seguindo loop profissional
2. **Auditor Refutador:** Busca ativamente REFUTE (não confirmar)
3. **Correção:** Gerador ajusta baseado nas refutações
4. **Re-auditoria:** Refutador valida correções
5. **APPROVED:** Ambos assinam em evals/audits/

**Regra:** Auditor ≠ Gerador SEMPRE. Mesmo agente não pode auditar próprio trabalho.

### 4.3 Gates Automáticos (Hooks)
Implementados via `verificar-gate` e CI/CD:

- **Pre-commit:** `cargo clippy --workspace`, `cargo fmt --check`, `cargo test --workspace`
- **Pre-merge:** Coverage >80%, security audit limpo, benchmarks dentro do target
- **Post-deploy:** Health check, smoke test, métricas de latência/RAM

---

## 5. Integração com Ecossistema Hermes

### 5.1 Filosofia de Integração
Zylos-Next é **soberano mas interoperável**. Hermes serve como:
- **Orquestrador externo:** Kanban dispatch, cron scheduling, connector management
- **Fonte de dados:** Memories, skills, agent configs via adapters
- **Canal de entrega:** Cockpit web, messaging bots, notifications

**Nunca:** Copiar código Hermes, depender de plugins Hermes para lógica core, modificar source Hermes.

### 5.2 Contrato de Comunicação
Definido em `specs/COMMUNICATION-CONTRACT.md`:
- Envelope unificado com correlation_id para tracing
- Event taxonomy por namespace de sistema
- REST bridge para Hermes plugins (/api/v1/hermes/{system}/{action})
- JSON Schema validation em runtime

### 5.3 Migração Gradual
Fases de coexistência:
1. **Fase 0-2:** Zylos como sidecar, Hermes como orquestrador primário
2. **Fase 3-5:** Zylos assume sistemas core, Hermes como adapter layer
3. **Fase 6-7:** Zylos como runtime principal, Hermes como connector opcional

---

## 6. Métricas de Sucesso por Fase

| Fase | Métrica Primária | Target | Métrica Secundária | Target |
|------|------------------|--------|--------------------|--------|
| 0 | Documentos aprovados | 15/15 | CI verde | <10min |
| 1 | Context Graph traversal | <50ms (5 hops) | Vector search | <10ms (10k embeddings) |
| 2 | DAG execution (100 nós) | <1s | Rollback automático | 100% success |
| 3 | Guardrail latency | <1ms | Falso-positivo rate | <5% |
| 4 | Sandbox cold start | <100ms | Memory isolation | Zero leaks |
| 5 | Skill discovery | <100ms | Eval flywheel coverage | >90% skills |
| 6 | AG-UI streaming | <10ms latency | 50+ agents concurrent | No lag |
| 7 | End-to-end workflow | <500ms | Production stability | 99.9% uptime |

---

## 7. Ferramentas e Ambientes

| Categoria | Ferramenta | Uso | Acesso |
|-----------|-----------|-----|--------|
| Linguagem | Rust 1.82+ | Todos os crates | rustup |
| Runtime | Tokio 1.40 | Async, lifecycle | Cargo.toml |
| Database | SQLite WAL | Graph, vectors, audit | rusqlite |
| Sandbox | Wasmtime 25.0 | Code isolation | Cargo.toml |
| Frontend | Leptos 0.7 | Cockpit WASM | Cargo.toml |
| CI/CD | GitHub Actions | Build, test, deploy | .github/workflows/ |
| Docs | Markdown + mdBook | Documentação viva | docs/ |
| Audit | Minimax/GPT-4o | Adversarial review | API keys (keyring) |
| Profiling | criterion + flamegraph | Benchmarks | cargo-criterion |

---

## 8. Referências Cruzadas

| Documento | Conteúdo | Quando Consultar |
|-----------|----------|------------------|
| [PRD.md](./PRD.md) | Requisitos de produto, DoD, Out of Scope | Início de cada fase |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Topologia dos 6 sistemas, fluxo de dados | Design de novos componentes |
| [DECISIONS.md](./DECISIONS.md) | ADRs versionadas | Antes de mudar tech stack |
| [MEMORY.md](./MEMORY.md) | Estado atual, lições, failed approaches | Início de cada sessão |
| [TASKS.md](./TASKS.md) | Board estruturado (Next/Progress/Done) | Daily planning |
| [SECURITY.md](./SECURITY.md) | Threat model, capability model | Qualquer mudança de segurança |
| [EVALUATION.md](./EVALUATION.md) | Métricas e benchmarks por crate | Antes de promover agente/fase |
| [runbooks/](./runbooks/) | Guias operacionais 3-tier | Execução de tarefas específicas |
| [specs/](./specs/) | Contratos tipados, schemas JSON | Implementação de interfaces |

---

## 9. Atualização Contínua

Este playbook é um **documento vivo**. Deve ser revisado e atualizado:
- Ao final de cada fase (durante handoff)
- Quando houver pivot estratégico
- Quando nova lição aprendida invalidar regra existente
- Mensalmente pelo Tech Lead

**Responsável pela manutenção:** Tech Lead
**Última revisão:** 31 de outubro de 2026 (v1.0 — criação inicial)