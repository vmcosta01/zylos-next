# Development Guide — Zylos-Next v6.0

## Pré-requisitos

- Rust 1.82+ (stable)
- `rustup component add clippy rustfmt rust-analyzer`
- `cargo install cargo-watch cargo-tarpaulin cargo-audit`
- Git com hooks configurados

## Setup Inicial

```bash
git clone <repo> && cd zylos-next
cargo build --workspace
cargo clippy --workspace --all-targets
cargo fmt --all --check
```

## Dev Workflow

### Loop Profissional Obrigatório

```
READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS
```

1. **READ**: Ler docs relevantes (PRD.md, ARCHITECTURE.md, specs/) antes de começar
2. **UNDERSTAND**: Confirmar entendimento dos requisitos e constraints
3. **PLAN**: Escrever/atualizar spec técnica em `specs/` com Goal-Contract
4. **IMPLEMENT**: Codificar seguindo spec; TDD quando aplicável
5. **TEST**: Rodar testes unitários + integração; verificar gate
6. **REVIEW**: Auto-review ou peer review; juiz ≠ gerador
7. **FIX**: Corrigir issues encontradas na review
8. **COMMIT**: Commit atômico com mensagem descritiva
9. **UPDATE DOCS**: Atualizar TASKS.md, MEMORY.md, DECISIONS.md

### Comandos Úteis

```bash
# Build e check
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo test --workspace

# Watch mode
cargo watch -x 'test --workspace'
cargo watch -x 'clippy --workspace --all-targets'

# Benchmarks
cargo bench --workspace

# Security audit
cargo audit

# Coverage
cargo tarpaulin --workspace --out Html
```

## Política Git

- **Commits atômicos**: uma mudança lógica por commit
- **Branches por feature**: `feat/<nome>`, `fix/<nome>`, `refactor/<nome>`
- **Main protegida**: merge apenas via PR com CI verde + review
- **Mensagens descritivas**: `<type>(<scope>): <description>` (conventional commits)
- **Sign-off**: `git commit -s` para DCO

## Estrutura de Crates

Ver `PROJECT_STRUCTURE.md` para mapeamento completo crates ↔ conceitos.

## Debugging

```bash
# Logs estruturados
RUST_LOG=debug cargo run --bin zylos-gateway

# Flamegraph
cargo flamegraph --bin zylos-gateway

# Memory profiling
cargo valgrind --tool=massif --bin zylos-gateway
```

## CI/CD

Pipeline definido em `.github/workflows/ci.yml`:
- check → clippy → fmt → test → build → coverage → security-audit
- MSRV check (Rust 1.75+)
- Cache de `~/.cargo/registry` e `target/`
