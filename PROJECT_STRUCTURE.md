# Estrutura do Projeto — Zylos-Next v6.0

## Mapeamento Crates ↔ Conceitos do Manifesto (Conceito #21)

| Crate / App | Conceito(s) | Responsabilidade |
|-------------|-------------|------------------|
| `zylos-core` | #2, #5 | Tipos compartilhados, traits base, error types |
| `zylos-bot-system` | #2, #6, #22 | Lifecycle, DAG executor, P2P messaging, shadow-git |
| `zylos-memory-system` | #2, #7, #22 | Context Graph SQLite WAL, VectorStore trait, rerank |
| `zylos-live-comm` | #11, #14, #18 | AG-UI streaming, event bus SSE/WS, cockpit API |
| `zylos-verification` | #9, #10, #20 | Ground enum, adversarial panel, guardrail <1ms |
| `zylos-sandbox` | #12, #13 | WASM snapshot/resume, Docker per-agent, anti-detect |
| `zylos-skills` | #7, #21, #22 | index.json discovery, schema validation, eval flywheel |
| `zylos-mobile` | #18 | Mobile MCP server para apps externos |
| `zylos-gateway` | #2, #16 | Axum server unificando todos os sistemas |
| `zylos-cli` | #19 | CLI administrativa |
| `zylos-daemon` | #2 | Daemon compartilhado para lifecycle de agentes |

## Estrutura de Diretórios

```
zylos-next/
├── Cargo.toml                 # Workspace virtual (resolver = "2")
├── rust-toolchain.toml        # stable + clippy + rustfmt + wasm32
├── .gitignore
├── .env.example               # Variáveis de ambiente (sem valores reais)
│
├── PRD.md                     # Conceito #1: Product Requirements Document
├── ARCHITECTURE.md            # Conceito #2: Arquitetura com UML
├── DESIGN_SYSTEM.md           # Conceito #3: Tokens e estados obrigatórios
├── AGENTS.md                  # Conceito #4: Regras enforceáveis
├── APP_FLOW.md                # Conceito #18: User Journey
├── IMPLEMENTATION_PLAN.md     # Conceito #19: Master Plan v3.0 (referência externa)
├── TASKS.md                   # Conceito #7: Board estruturado
├── DECISIONS.md               # Conceito #7: ADRs versionadas
├── MEMORY.md                  # Conceito #7: Estado atual e lições
├── TESTING.md                 # Conceito #6: Checklist de testes
├── SECURITY.md                # Conceito #12: Segurança camada zero
├── EVALUATION.md              # Conceito #22: Métricas e benchmarks
├── PROJECT_STRUCTURE.md       # Conceito #21: Este documento
│
├── crates/
│   ├── zylos-core/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-bot-system/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-memory-system/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-live-comm/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-verification/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-sandbox/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── zylos-skills/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── zylos-mobile/
│       ├── Cargo.toml
│       └── src/lib.rs
│
├── apps/
│   ├── zylos-gateway/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── zylos-cli/
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   └── zylos-daemon/
│       ├── Cargo.toml
│       └── src/main.rs
│
├── specs/
│   ├── DESIGN.md              # Typed contract (serde + jsonschema)
│   ├── GOAL-CONTRACT.md       # verify:<cmd> obrigatório
│   └── ADRs/                  # Architecture Decision Records (backup)
│
├── skills/
│   ├── index.json             # Skill registry (vazio inicialmente)
│   ├── schemas/
│   │   └── skill.schema.json  # JSON Schema para validação de skills
│   └── GLOSSARY.md            # Termos base
│
├── evals/
│   ├── benchmarks/            # Criterion benchmarks
│   ├── judges/                # LLM-as-judge templates
│   ├── audits/                # Relatórios de auditoria adversarial
│   ├── datasets/              # Datasets versionados para eval
│   └── baselines/             # Baselines de performance
│
├── docs/                      # Documentação complementar
├── examples/                  # Exemplos de uso
├── tests/                     # Integration + E2E tests
│
└── .github/
    └── workflows/
        └── ci.yml             # GitHub Actions CI/CD
```

## Convenções de Organização

- **Crates:** Cada sistema core é um crate separado em `crates/zylos-<system>/`
- **Apps:** Binários executáveis em `apps/zylos-<name>/`
- **Specs:** Contratos técnicos e schemas em `specs/`
- **Skills:** Registry e schemas de skills em `skills/`
- **Evals:** Benchmarks, datasets e auditorias em `evals/`
- **Docs raiz:** 14 arquivos obrigatórios do manifesto na raiz do projeto

## Dependências entre Crates

```
zylos-core ← (todos os outros crates dependem dele)
zylos-bot-system ← zylos-gateway, zylos-daemon
zylos-memory-system ← zylos-gateway
zylos-live-comm ← zylos-gateway
zylos-verification ← zylos-gateway
zylos-sandbox ← zylos-gateway
zylos-skills ← zylos-gateway
zylos-mobile ← zylos-gateway
```

## Referências
- [Manifesto Conceito #21](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md) — Estrutura Padronizada
- [ARCHITECTURE.md](./ARCHITECTURE.md) para detalhes dos 6 sistemas core
- [DECISIONS.md](./DECISIONS.md) para ADRs que justificam esta estrutura