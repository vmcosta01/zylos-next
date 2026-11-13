# Regras do Agente — Zylos-Next v6.0

## Contexto do Projeto
Zylos-Next é um runtime soberano de sistemas multi-agente escrito em Rust. Este documento define regras **enforceáveis** que todo agente (humano ou IA) DEVE seguir ao contribuir para este projeto. Violação = trabalho inválido.

## Manifesto Inviolável (v3.1 — 10 Camadas Obrigatórias)
Todo agente DEVE cumprir integralmente o [Manifesto Conceitos-Desenvolvimento-IA v3.1](./docs/CONCEITOS-DESENVOLVIMENTO-IA.md). Este manifesto NÃO é apenas "22 conceitos" — é um sistema operacional completo composto por **10 camadas interdependentes**, todas obrigatórias e enforceáveis:

| # | Camada | Conteúdo |
|---|--------|----------|
| 1 | Conceitos Canônicos | 22 regras fundamentais (#1 PRD → #22 Evaluation) |
| 2 | Princípios Anti-Slop | 7 filtros de qualidade contra entregas medíocres |
| 3 | Loop Profissional | READ→UNDERSTAND→PLAN→IMPLEMENT→TEST→REVIEW→FIX→COMMIT→UPDATE DOCS |
| 4 | Penalidade | Violação = artefato descartado e refeito; sem exceções |
| 5 | Arquivos Obrigatórios | 15 documentos que todo projeto DEVE ter |
| 6 | Templates de Prompts | 6 prompts estruturados para revisão de artefatos |
| 7 | Matriz de Rastreabilidade | Fonte F1-F7 → Conceito → Diferencial CÓRTEX |
| 8 | Integração com Harness | Scripts, hooks, skills que enforce conceitos automaticamente |
| 9 | Skills Hermes Documentais | 14 skills (8 canônicas + 6 operacionais) |
| 10 | Protocolo de Evolução + Lições | Versionamento semântico, lições aprendidas |

**Regra de Leitura:** Agentes DEVEM ler o manifesto na ordem das 10 camadas. Parar na camada 1 e ignorar as camadas 2-10 = violação do manifesto. Cada camada adiciona constraints, automações e verificações que tornam as anteriores enforceáveis na prática.

Violação de qualquer parte do manifesto = trabalho inválido, independente de resultado funcional. A cópia canônica vive em `docs/CONCEITOS-DESENVOLVIMENTO-IA.md` dentro deste repositório.

## Skills Documentais — Invocação Automática e Obrigatória

**REGRA DE AUTO-INVOCAÇÃO:** As 28 skills abaixo são invocadas **AUTOMATICAMENTE** pelo agente sempre que o gatilho correspondente for detectado. O agente NUNCA deve esperar solicitação explícita do usuário para carregar uma skill. A detecção do gatilho é responsabilidade do agente; a omissão = violação do manifesto.

### Gatilhos de Auto-Invocação por Tipo de Trabalho

| Gatilho Detectado | Skill Auto-Carregada | Quando Invocar |
|-------------------|---------------------|----------------|
| Criar/editar `PRD.md` | `prd-author` | ANTES de escrever qualquer linha do PRD |
| Criar/editar `ARCHITECTURE.md` | `architecture-author` | ANTES de documentar arquitetura |
| Criar/editar `DESIGN_SYSTEM.md` ou `DESIGN.md` | `design-system-author` | ANTES de definir tokens ou componentes UI |
| Criar/editar `AGENTS.md` ou `RULES.md` | `agents-rules-author` | ANTES de definir regras de agente |
| Criar spec técnica para tarefa/feature | `spec-author` | ANTES de gerar código para a tarefa |
| Criar/editar `TESTING.md` ou escrever testes | `testing-checklist-author` | ANTES de escrever qualquer teste |
| Criar/editar `MEMORY.md`, `DECISIONS.md`, `TASKS.md` | `project-memory-author` | ANTES de atualizar documentos vivos |
| Completar fase ou transicionar entre agentes | `handoff-author` | ANTES de gerar mensagem de handoff |
| Configurar monitoramento diário/sentinela | `continuous-monitoring` | ANTES de configurar health checks |
| Definir roles/permissões/RBAC | `access-control-author` | ANTES de implementar controle de acesso |
| Criar dashboard ou painel de métricas | `dashboard-author` | ANTES de construir qualquer dashboard |
| Definir pipeline de deploy | `staged-deploy-author` | ANTES de configurar CI/CD ou deploy |
| Criar/editar `APP_FLOW.md` | `app-flow-author` | ANTES de mapear jornada do usuário |
| Criar/editar `IMPLEMENTATION_PLAN.md` | `implementation-plan-author` | ANTES de planejar fases de implementação |
| Definir DoD ou Out of Scope | `acceptance-criteria-author` | ANTES de declarar critérios de aceitação |
| Criar/editar `PROJECT_STRUCTURE.md` | `project-structure-author` | ANTES de definir estrutura de projeto |
| Criar/editar runbook (`runbooks/*.md`) | `runbook-builder` | ANTES de escrever runbook técnico |
| Criar/editar playbook (`playbooks/*.md`) | `playbook-author` | ANTES de escrever playbook estratégico |
| Criar/editar SOP (`playbooks/sop-*.md`) | `sop-author` | ANTES de escrever procedimento operacional |
| Criar/editar POP (`playbooks/pop-*.md`) | `pop-author` | ANTES de escrever procedimento de componente |
| Criar/editar workflow SOAR | `soar-orchestration` | ANTES de projetar orquestração SOAR |
| Configurar AIOps/métricas/alertas | `aiops-observability` | ANTES de configurar observabilidade |
| Entregar artefato para review/aprovação | `anti-slop-filter` | ANTES de promover qualquer entrega |
| Iniciar nova tarefa ou feature | `professional-loop` | NO INÍCIO de todo ciclo de trabalho |
| Revisar PRD/spec/fase/handoff/benchmark | `review-prompts` | ANTES de aprovar artefato crítico |
| Teste verde obtido, antes de declarar pronto | `mordida-protocol` | APÓS teste verde, ANTES de declarar completo |
| Final de fase ou declaração de "pronto" | `isolated-audit` | AO FINAL de cada fase, contexto-zero |
| Tarefa >15 turnos ou >5 arquivos | `subagent-driven-dev` | AO DETECTAR complexidade que excede janela |

### Regras Enforceáveis de Auto-Invocação

1. **Detecção proativa:** O agente DEVE detectar o gatilho ANTES de iniciar o trabalho correspondente. Não esperar o usuário pedir.
2. **Carregamento imediato:** Ao detectar gatilho, executar `skill_view(name)` IMEDIATAMENTE, antes de produzir qualquer output relacionado.
3. **Estrutura obrigatória:** Seguir EXATAMENTE a estrutura definida na skill (seções, ordem, conteúdo mínimo).
4. **Verificação pós-criação:** Executar o checklist de verificação da skill ANTES de entregar o artefato.
5. **Cross-linking:** Garantir que todos os links cruzados definidos na skill estejam funcionais.
6. **Penalidade:** Produzir documento sem carregar a skill correspondente = trabalho inválido, artefato descartado.

### Tabela Completa de Skills (28)

| Documento / Camada | Skill | Conceito / Camada |
|--------------------|-------|-------------------|
| `PRD.md` | `prd-author` | #1 PRD |
| `ARCHITECTURE.md` | `architecture-author` | #2 Arquitetura |
| `DESIGN_SYSTEM.md` / `DESIGN.md` | `design-system-author` | #3 Design System |
| `AGENTS.md` / `RULES.md` | `agents-rules-author` | #4 Regras do Agente |
| Specs técnicas por tarefa | `spec-author` | #5 Spec Antes de Código |
| `TESTING.md` | `testing-checklist-author` | #6 TDD |
| `MEMORY.md` / `DECISIONS.md` / `TASKS.md` | `project-memory-author` | #7 Documentos Vivos |
| `HANDOFF_[fase].md` | `handoff-author` | #8 Orquestração |
| Monitoramento diário | `continuous-monitoring` | #11 Monitoramento |
| RBAC / Permissões | `access-control-author` | #13 Multiusuário |
| Dashboard vivo | `dashboard-author` | #14 Dashboard |
| Pipeline de deploy | `staged-deploy-author` | #16 Deploy Estágios |
| `APP_FLOW.md` | `app-flow-author` | #18 User Journey |
| `IMPLEMENTATION_PLAN.md` | `implementation-plan-author` | #19 Impl Plan |
| DoD + Out of Scope | `acceptance-criteria-author` | #20 DoD/Out of Scope |
| `PROJECT_STRUCTURE.md` | `project-structure-author` | #21 Estrutura |
| Runbooks (`runbooks/*.md`) | `runbook-builder` | #5/#7 Docs |
| Playbooks (`playbooks/*.md`) | `playbook-author` | #1/#2 Strategy |
| SOPs (`playbooks/sop-*.md`) | `sop-author` | #5/#19 Procedures |
| POPs (`playbooks/pop-*.md`) | `pop-author` | #5/#12 Operations |
| SOAR Workflows | `soar-orchestration` | #12/#22 SOAR |
| AIOps/Métricas | `aiops-observability` | #12/#22 AIOps |
| Validação anti-slop | `anti-slop-filter` | Camada 2 |
| Loop profissional | `professional-loop` | Camada 3 |
| Templates de revisão | `review-prompts` | Camada 6 |
| Prova por mutação (mordida) | `mordida-protocol` | Orquestração Avançada |
| Auditoria isolada contexto-zero | `isolated-audit` | Orquestração Avançada |
| Decomposição em subagentes | `subagent-driven-dev` | Orquestração Avançada |
| Criar/editar approval gate ou regra HITL | `prd-author` + `access-control-author` | ANTES de implementar approval gates |
| Configurar regras de egress/network filter | `agents-rules-author` | ANTES de definir whitelist de rede |
| Exportar/auditar logs de ação | `pop-author` | ANTES de gerar relatório de auditoria |

## Arquivos Obrigatórios Antes de Codar
Antes de escrever qualquer código Rust, o agente DEVE ler e entender:
1. `PRD.md` — O que estamos construindo e por quê (skill: `prd-author`)
2. `ARCHITECTURE.md` — Como os sistemas se conectam (skill: `architecture-author`)
3. `DESIGN_SYSTEM.md` — Tokens e estados obrigatórios (skill: `design-system-author`)
4. `SECURITY.md` — Regras de segurança camada zero
5. `TESTING.md` — Checklist de testes por tipo (skill: `testing-checklist-author`)
6. `specs/DESIGN.md` — Typed contract (serde + jsonschema)
7. `specs/GOAL-CONTRACT.md` — verify:<cmd> obrigatório

## Convenções de Código Rust

### Naming
- **Crates:** `zylos-<system>` (kebab-case)
- **Modules:** snake_case (`bot_system`, `memory_graph`)
- **Types:** PascalCase (`LifecycleManager`, `ContextGraph`)
- **Functions:** snake_case (`spawn_agent`, `verify_claim`)
- **Constants:** SCREAMING_SNAKE_CASE (`MAX_AGENTS`, `WAL_CHECKPOINT_INTERVAL`)
- **Traits:** PascalCase com sufixo descritivo (`VectorStore`, `JudgeTrait`)

### Error Handling
- **NUNCA** usar `unwrap()` ou `expect()` em código de produção
- **SEMPRE** usar `thiserror` para erros de crate, `anyhow` apenas em apps/binários
- **SEMPRE** definir enum de erro específico por módulo
- **SEMPRE** propagar erros com contexto (`?` operator + `.context()`)
- Error types DEVEM implementar `std::error::Error` + `Display`

### Async Patterns
- **SEMPRE** usar Tokio como runtime async
- **SEMPRE** usar `JoinSet` para lifecycle de sub-agentes
- **SEMPRE** usar `CancellationToken` para graceful shutdown
- **NUNCA** usar `block_on()` dentro de async context
- **SEMPRE** documentar cancellation safety de funções async

### Safety
- **NUNCA** usar `unsafe` sem justificativa documentada em comentário SAFETY
- **SEMPRE** preferir abstractions safe (Arc, Mutex, RwLock)
- **SEMPRE** validar inputs externos (serde validation, jsonschema)
- **NUNCA** confiar em dados de rede/filesystem sem validação

### Testing
- **SEMPRE** escrever teste ANTES ou EM PARALELO ao código (TDD)
- **SEMPRE** cobrir: happy path, validation, error, permissions, performance
- **SEMPRE** usar `proptest` para property-based testing em grafos/DAGs
- **SEMPRE** usar `criterion` para benchmarks reproduzíveis
- Testes DEVEM ser independentes e idempotentes

### Documentation
- **SEMPRE** documentar tipos públicos com `///` doc comments
- **SEMPRE** incluir exemplos em doc comments (`# Examples`)
- **SEMPRE** manter README.md de cada crate atualizado
- **NUNCA** deixar TODO/FIXME sem ticket associado

## Regras de Segurança (Enforced por Hooks)

### Secrets
- **NUNCA** commitar secrets, tokens, passwords, API keys
- **SEMPRE** usar environment variables via `.env.example`
- Hook `scope-guard` bloqueia escrita de padrões de secret em código

### Authorization
- **SEMPRE** verificar authorization server-side (nunca só frontend)
- **SEMPRE** validar ownership de recursos antes de operação
- **NUNCA** confiar em claims de cliente sem verificação

### Sandboxing
- **SEMPRE** executar tools de terceiros em WASM sandbox
- **SEMPRE** isolar agentes via Docker (network + filesystem)
- **NUNCA** permitir acesso direto ao host sem approval explícito

## Boundaries (O Que NÃO Mudar Sem Approval)
- [ ] Estrutura de crates (adicionar/remover/rename)
- [ ] Dependências workspace-level (Cargo.toml root)
- [ ] Protocolos de comunicação (A2A/MCP/AG-UI)
- [ ] Schema de banco de dados (migrations exigem ADR)
- [ ] Configurações de CI/CD (.github/workflows/)
- [ ] Regras deste documento (AGENTS.md)

## Loop Profissional Obrigatório
```
READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS
```
Pular etapas = trabalho inválido. Cada etapa deve produzir artefato verificável.

## Auditoria Dupla Obrigatória ao Final de Cada Fase
**Nenhuma fase pode ser declarada concluída sem as DUAS auditorias abaixo, executadas sequencialmente:**

### 1. Auditoria Adversarial Completa (juiz ≠ gerador)
- **Agente:** `code-reviewer` (perfil separado do implementador)
- **Escopo:** Valida conformidade com Manifesto Conceitos-Desenvolvimento-IA v3.0 (22 conceitos + 7 princípios anti-slop + documentos obrigatórios), compilação limpa, testes passando, handoffs consistentes, verify:<cmd> executáveis
- **Output:** `evals/audits/faseN-audit.md` com veredito APPROVED ou CHANGES_REQUIRED
- **Se CHANGES_REQUIRED:** Implementador corrige todos os findings críticos e warnings antes de prosseguir

### 2. Auditoria Isolada de Verificação Pós-Correção
- **Agente:** `code-reviewer` (mesmo perfil, nova instância isolada)
- **Escopo:** Verifica que TODAS as correções exigidas pela auditoria #1 foram realmente aplicadas — re-executa cada verify:<cmd>, confirma compilação, testes, e consistência documental
- **Output:** `evals/audits/faseN-audit-v2.md` com veredito APPROVED ou CHANGES_REQUIRED
- **Se CHANGES_REQUIRED:** Loop volta para correção até APPROVED

### Regras Enforceáveis
- **NUNCA** declarar fase concluída sem ambos os relatórios APPROVED em `evals/audits/`
- **NUNCA** iniciar próxima fase sem APPROVED da auditoria isolada (v2)
- **SEMPRE** referenciar ambos os relatórios no HANDOFF_N.md do fase
- **SEMPRE** usar `delegate_task` com perfil `code-reviewer` para garantir isolamento juiz ≠ gerador
- Violação = trabalho inválido, fase deve ser re-auditada

## Comandos Úteis
```bash
# Build workspace
cargo build --workspace

# Check sem compilar
cargo check --workspace --all-targets

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Format check
cargo fmt --all -- --check

# Testes
cargo test --workspace --all-features

# Coverage
cargo llvm-cov --workspace --lcov --output-path lcov.info

# Security audit
cargo audit

# Benchmark
cargo bench --workspace
```

## Referências
- [Manifesto Conceitos-Desenvolvimento-IA](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [PRD.md](./PRD.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [SECURITY.md](./SECURITY.md)