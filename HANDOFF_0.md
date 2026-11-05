# Handoff da Fase 0 — Zylos-Next v6.0

## Contexto Aprovado
A Fase 0 (Fundação) foi executada conforme Master Plan v3.0 integrado ao Manifesto Conceitos-Desenvolvimento-IA v3.0. Todos os 15 documentos obrigatórios foram criados, o workspace Cargo compila com sucesso (`cargo check --workspace` verde), e o hook `verificar-gate` está implementado como script executável.

**Data de Conclusão:** 2026-10-30
**Versão do Plano:** 3.0
**Status:** ✅ Aprovado para Fase 1

## Artefatos Gerados

### Workspace & Tooling
| Artefato | Caminho | Status |
|----------|---------|--------|
| Workspace Cargo | `Cargo.toml` (8 crates + 3 apps) | ✅ Compilando |
| Rust Toolchain | `rust-toolchain.toml` (stable 1.99.1) | ✅ Funcional |
| CI/CD Pipeline | `.github/workflows/ci.yml` | ✅ Configurado |
| Hook verificar-gate | `scripts/verificar-gate.sh` | ✅ Implementado |
| Git Ignore | `.gitignore` | ✅ Criado |

### Documentos Obrigatórios do Manifesto (15/15)
| # | Documento | Conceito | Caminho | Status |
|---|-----------|----------|---------|--------|
| 1 | PRD.md | #1 PRD Antes de Código | `PRD.md` | ✅ Criado |
| 2 | ARCHITECTURE.md | #2 Arquitetura Definida | `ARCHITECTURE.md` | ✅ Criado |
| 3 | DESIGN_SYSTEM.md | #3 Design System | `DESIGN_SYSTEM.md` | ✅ Criado |
| 4 | AGENTS.md | #4 Regras do Agente | `AGENTS.md` | ✅ Criado |
| 5 | APP_FLOW.md | #18 User Journey | `APP_FLOW.md` | ✅ Criado |
| 6 | IMPLEMENTATION_PLAN.md | #19 Plano por Fases | Master Plan v3.0 (externo) | ✅ Referência |
| 7 | TASKS.md | #7 Documentos Vivos | `TASKS.md` | ✅ Criado |
| 8 | DECISIONS.md | #7 Documentos Vivos | `DECISIONS.md` | ✅ 7 ADRs |
| 9 | MEMORY.md | #7 Documentos Vivos | `MEMORY.md` | ✅ Criado |
| 10 | TESTING.md | #6 TDD com IA | `TESTING.md` | ✅ Criado |
| 11 | SECURITY.md | #12 Segurança Camada Zero | `SECURITY.md` | ✅ Criado |
| 12 | .env.example | #12 Segurança Camada Zero | `.env.example` | ✅ Criado |
| 13 | EVALUATION.md | #22 Evaluation/Benchmark | `EVALUATION.md` | ✅ Criado |
| 14 | PROJECT_STRUCTURE.md | #21 Estrutura Padronizada | `PROJECT_STRUCTURE.md` | ✅ Criado |
| 15 | HANDOFF_0.md | #8 Handoff Estruturado | Este documento | ✅ Gerado |

### Specs Técnicas
| Documento | Conteúdo | Status |
|-----------|----------|--------|
| `specs/DESIGN.md` | Typed contracts (serde + jsonschema) para AgentConfig, WorkflowDAG, VerificationClaim, MemoryContextEntry | ✅ Schema válido |
| `specs/GOAL-CONTRACT.md` | Template verify:<cmd> + exemplos + anti-patterns | ✅ Documentado |

## Blockers Resolvidos
| Blocker | Causa Raiz | Resolução |
|---------|-----------|-----------|
| `rusqlite` feature `wal` inválida | WAL é pragma SQL, não feature do crate | Removido do Cargo.toml; ativado via `PRAGMA journal_mode=WAL` em runtime |
| Conflito `cargo-clippy.exe` na toolchain | Binários residuais de instalação anterior | `rustup toolchain uninstall stable && rustup toolchain install stable` |
| PowerShell curl alias | `curl` → `Invoke-WebRequest` sem `-L` | Usado Python urllib ou curl.exe explícito |
| Strings triple-quoted longas no kernel | SyntaxError em strings >400 linhas | Dividido em múltiplas chamadas `write_file` |

## Próximo Passo: Fase 1 — BOT_SYSTEM
Iniciar implementação do `LifecycleManager` no crate `zylos-bot-system`:

1. **Task 1.1.1**: Implementar `LifecycleManager` com Tokio JoinSet + CancellationToken
   - Spawn/kill/suspend agents
   - Graceful shutdown hierárquico
   - Métricas de lifecycle (spawn latency, active count)
   - **verify:** `cargo test -p zylos-bot-system lifecycle`

2. **Task 1.1.2**: Implementar `DAGExecutor` com petgraph
   - Execução topológica de workflows
   - Rollback automático via shadow-git
   - Timeout por nó configurável
   - **verify:** `cargo test -p zylos-bot-system dag`

3. **Task 1.1.3**: Implementar `P2PMessenger` type-safe
   - Channels Tokio com schema validation
   - Message delivery guarantee (ack/nack)
   - Dead letter queue para falhas
   - **verify:** `cargo test -p zylos-bot-system p2p`

**Dependências da Fase 1:** Nenhuma externa; workspace e specs já disponíveis.
**Estimativa:** 40h (2 semanas)

## Perguntas Abertas
1. **Vector Store Backend:** SQLite-only para MVP ou integrar Qdrant/LanceDB desde a Fase 2? (Atual: SQLite via rusqlite)
2. **LLM Provider para Adversarial Panel:** OpenAI GPT-4o-mini como default ou suportar Anthropic/local desde o início? (Atual: OpenAI via .env)
3. **Cockpit Frontend:** Iniciar Leptos WASM na Fase 1 (paralelo ao BOT_SYSTEM) ou aguardar Fase 6? (Master Plan diz Fase 6)
4. **Shadow-Git Storage:** SQLite table ou filesystem separado? (Atual: planejado como SQLite table)

## Critérios de Saída da Fase 0 — Verificados
- [x] `cargo build --workspace` passa (exit 0)
- [x] CI/CD configurado (.github/workflows/ci.yml)
- [x] 15 documentos obrigatórios criados e válidos
- [x] Hook `verificar-gate` funcional (`scripts/verificar-gate.sh`)
- [x] Loop profissional documentado em AGENTS.md
- [x] Princípios anti-slop mapeados em seção específica do Master Plan
- [x] Matriz de rastreabilidade 22 conceitos → tasks/artefatos completa
- [x] ADRs iniciais aprovados (7 decisões arquiteturais)
- [x] Security rules definidas ANTES de qualquer código (SECURITY.md)
- [x] Eval metrics definidas por crate (EVALUATION.md)

## Auditoria Adversarial
**Status:** ✅ COMPLETA — Fase 0 aprovada após dupla auditoria adversarial

**Relatórios:**
- `evals/audits/fase0-fase1-audit.md` (v1) — CHANGES_REQUIRED → correções aplicadas
- `evals/audits/fase0-fase1-audit-v2.md` (v2) — APPROVED (verificação pós-correção)
- `evals/audits/fase0-fase1-audit-v3-glm.md` (v3 GLM) — CHANGES_REQUIRED → correções aplicadas nesta revisão
- `evals/audits/fase0-fase1-audit-v3-minimax.md` (v3 MiniMax) — CHANGES_REQUIRED → correções aplicadas nesta revisão

**Checklist de auditoria (todos verificados):**
- [x] Nenhum documento viola princípios anti-slop
- [x] Todos os docs têm seções "Out of Scope" completas
- [x] Specs técnicas têm verify:<cmd> executável
- [x] SECURITY.md define regras antes de implementação
- [x] EVALUATION.md tem métricas mensuráveis por crate
- [x] AGENTS.md tem regras enforceáveis (não genéricas)

## Referências
- [Master Plan v3.0](file:///C:/Users/Paulo/AppData/Local/hermes/cache/scratch/zylos-next-master-plan.md)
- [Manifesto Conceitos-Desenvolvimento-IA](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [DECISIONS.md](./DECISIONS.md) para ADRs completos
- [MEMORY.md](./MEMORY.md) para estado atual e lições aprendidas