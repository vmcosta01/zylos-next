# Fase 0: Core Foundation — Runbook Zylos-Next

## Visão Geral

A Fase 0 estabelece a fundação do Zylos-Next v6: workspace Cargo funcional, tipos base compartilhados (`zylos-core`), error handling padronizado com `thiserror`, async runtime Tokio 1.40+ com `JoinSet` e `CancellationToken`, e os 15 documentos obrigatórios do Manifesto Conceitos-Desenvolvimento-IA v3.0. Esta fase garante que todas as fases subsequentes compartilhem contratos de tipo, erros e padrões async consistentes.

**Critério de Saída:** `cargo build --workspace` passa, CI verde, 15 documentos aprovados por auditoria adversarial, hook `verificar-gate` funcional.

## Pré-requisitos

- Rust 1.82+ (definido em `rust-toolchain.toml`)
- Cargo workspace virtual configurado em `Cargo.toml` raiz
- Dependências base: `tokio 1.40`, `serde`, `thiserror`, `anyhow`, `tracing`
- Ferramentas: `cargo-clippy`, `cargo-fmt`, `cargo-audit`, `cargo-tarpaulin`
- GitHub Actions configurado em `.github/workflows/ci.yml`
- 15 documentos obrigatórios criados (PRD.md, ARCHITECTURE.md, DESIGN_SYSTEM.md, AGENTS.md, APP_FLOW.md, TASKS.md, DECISIONS.md, MEMORY.md, TESTING.md, SECURITY.md, .env.example, EVALUATION.md, PROJECT_STRUCTURE.md, HANDOFF_0.md, IMPLEMENTATION_PLAN.md)

### Setup Inicial

```bash
# Verificar toolchain
rustup show active-toolchain  # Deve mostrar 1.82+

# Build completo
cargo build --workspace

# Lint e formatação
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check

# Security audit
cargo audit

# Testes
cargo test --workspace
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Workspace Virtual com Crates Soberanos**: Cada sistema core é um crate independente (`zylos-core`, `zylos-bot-system`, `zylos-memory-system`, etc.) sem dependência runtime de plugins Hermes/skills Python. A soberania é absoluta — FFI/MCP apenas para integração externa, nunca para lógica core.

2. **Error Handling com `thiserror` + `Result` Alias**: Cada crate define seu próprio `Error` enum com `#[derive(thiserror::Error)]` e exporta `pub type Result<T> = std::result::Result<T, Error>`. O `zylos-core` futuramente fornecerá traits de erro compartilhadas, mas atualmente é minimalista (`lib.rs` com 1 linha de doc).

3. **Async Runtime: Tokio JoinSet + CancellationToken**: Lifecycle de sub-agentes usa `JoinSet` para spawn concorrente e `CancellationToken` para graceful shutdown. Nunca use `tokio::spawn` fire-and-forget — sempre anexe o handle a um `JoinSet` ou armazene o `AbortHandle`.

4. **Performance Targets**: <30MB RAM por agente idle, <50ms TTFD (Time To First Decision), daemon compartilhado. Benchmarks com `criterion` são obrigatórios para qualquer função no hot path.

5. **Documentos Vivos como Contrato**: Os 15 documentos não são documentação passiva — são contratos enforceáveis. `AGENTS.md` define regras que hooks validam. `DECISIONS.md` contém ADRs versionadas. `TESTING.md` exige checklist verificável por tipo de teste.

### Extension Points

- **Novos crates**: Adicionar ao workspace em `Cargo.toml` raiz sob `[workspace] members`. Seguir convenção de nomenclatura `zylos-<system>`.
- **Traits compartilhadas**: Definir em `zylos-core` quando múltiplos crates precisam da mesma interface (ex: `VectorStore` trait foi definida em `zylos-memory-system` porque é específica daquele domínio).
- **Feature flags**: Usar para backends opcionais (ex: `#[cfg(feature = "qdrant")]` em `zylos-memory-system`).

### Performance Tuning

- Profile com `criterion` antes de otimizar. Target: search em 10k embeddings <10ms, traversal de 5 hops <50ms.
- Use `petgraph` com SQLite WAL para grafos — evita alocação excessiva em memória.
- Compactação de proveniência: 32 IDs/nó máximo (inspirado em AgentMemory).

## Para Dev Pleno

### Implementando um Novo Crate

1. Crie o crate no workspace:
```bash
cargo new crates/zylos-new-system --lib
```

2. Adicione ao `Cargo.toml` raiz:
```toml
[workspace]
members = [
    "crates/zylos-core",
    "crates/zylos-new-system",
    # ... outros crates
]
```

3. Configure dependências base no `Cargo.toml` do crate:
```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
tokio = { version = "1.40", features = ["full"] }
tracing = "0.1"
```

4. Defina o módulo de erros (`src/error.rs`):
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NewSystemError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    #[error("invalid input: {0}")]
    InvalidInput(String),
    
    #[error("not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, NewSystemError>;
```

5. Estruture o `lib.rs` com módulos públicos e re-exports:
```rust
//! zylos-new-system — Descrição do sistema

pub mod error;
pub mod types;
// ... outros módulos

pub use error::{NewSystemError, Result};
pub use types::{/* tipos públicos */};
```

### Padrão Async com JoinSet

```rust
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

pub async fn run_agents(agents: Vec<AgentConfig>, token: CancellationToken) -> Result<()> {
    let mut set = JoinSet::new();
    
    for config in agents {
        let token = token.clone();
        set.spawn(async move {
            // Agent loop com cancellation check
            loop {
                if token.is_cancelled() {
                    break;
                }
                // ... trabalho do agente
            }
        });
    }
    
    // Aguardar todos completarem ou cancelarem
    while let Some(result) = set.join_next().await {
        match result {
            Ok(_) => {}
            Err(e) if e.is_cancelled() => {}
            Err(e) => tracing::error!("agent panicked: {e}"),
        }
    }
    
    Ok(())
}
```

### Loop Profissional Obrigatório

Todo task segue: `READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS`. Documentar cada passo. Nunca pular estados de erro. Nunca documentar depois.

## Para Dev Junior

### Passo a Passo: Primeiro Contributo

1. **Clone e setup**: `git clone`, `rustup update`, `cargo build --workspace`
2. **Leia AGENTS.md**: Entenda as regras enforceáveis antes de escrever código
3. **Escolha uma task do TASKS.md**: Seção "Next" tem prioridade
4. **Leia a spec correspondente**: Toda task tem spec em `specs/` ou referência em docs
5. **Implemente com TDD**: Escreva teste primeiro, implemente, faça passar
6. **Rode o checklist completo**:
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
7. **Atualize documentos vivos**: TASKS.md (mova para Done), MEMORY.md (lições aprendidas), DECISIONS.md (se tomou decisão arquitetural)
8. **Commit atômico**: Uma mudança lógica por commit, mensagem descritiva

### Pitfalls Comuns

- **Não use `unwrap()` em produção**: Sempre trate erros explicitamente com `?` ou `match`
- **Não ignore warnings do clippy**: Eles existem por razão. Fix ou documente `#[allow]` com justificativa
- **Não faça `tokio::spawn` sem handle**: Sempre use `JoinSet` ou armazene o handle
- **Não misture `Utc::now()` com SQL timestamps**: Use database-as-clock (SQL `now()`) para leases
- **Não adicione dependências sem justificar**: Cada dep nova precisa de rationale em DECISIONS.md

### Debugging Tips

- Use `tracing` com níveis: `debug!` para fluxo, `warn!` para recuperações, `error!` para falhas
- `RUST_LOG=zylos_core=debug,zylos_memory_system=trace` para filtrar logs
- `cargo test -- --nocapture` para ver output de testes
- Use `dbg!()` temporariamente para inspeção rápida (remova antes de commit)

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `cargo build` falha com dep resolution | Workspace mal configurado | Verifique `[workspace] members` no Cargo.toml raiz |
| Clippy warnings bloqueiam CI | Código não conforme | Rode `cargo clippy --fix` e revise mudanças |
| Testes flaky | Race condition em async | Use `tokio::time::pause()` em testes, evite `sleep` real |
| `cargo audit` reporta vulnerabilidade | Dep desatualizada | `cargo update -p <crate>` ou substitua dep |
| MSRV check falha | Feature muito nova | Verifique `rust-version` no Cargo.toml, ajuste ou feature-gate |
| Hook `verificar-gate` bloqueia | Declaração sem artefato | Produza artefato verificável (teste verde, screenshot, log) antes de declarar pronto |

## Referências

- **Master Plan**: `C:/Users/Paulo/AppData/Local/hermes/cache/scratch/zylos-next-master-plan.md` (linhas 1-358: Fase 0 detalhada)
- **zylos-core source**: `crates/zylos-core/src/lib.rs`
- **Manifesto**: `C:\Users\Paulo\.claude\CONCEITOS-DESENVOLVIMENTO-IA.md`
- **Workspace root**: `Cargo.toml`
- **CI config**: `.github/workflows/ci.yml`
- **ADRs**: `DECISIONS.md`
- **Specs**: `specs/DESIGN.md`, `specs/GOAL-CONTRACT.md`