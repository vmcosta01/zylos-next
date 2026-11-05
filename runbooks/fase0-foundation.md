# Runbook & Playbook: Fase 0 — FOUNDATION (Toolchain + Estrutura)

**Sistema:** FOUNDATION — Fase 0
**Crates:** `zylos-bot-system`, `zylos-memory-system`, `zylos-sandbox`, `zylos-search-mcp`
**Última Atualização:** 31 de outubro de 2026
**Status Operacional:** ✅ Toolchain validada, workspace Cargo configurado

---

## 🎯 Visão Geral (Todos os Níveis)

A Fase 0 estabelece a base técnica do Zylos-Next: toolchain Rust estável, workspace Cargo multi-crate, dependências críticas instaladas (wasm-pack, wasmtime, sqlite3), e estrutura de diretórios alinhada ao Master Plan v3.0. Sem esta fase correta, nenhuma outra fase compila ou executa.

---

## 👨‍💻 NÍVEL SENIOR — Arquitetura e Decisões

### Responsabilidades
- Validar escolhas de toolchain contra benchmarks de produção
- Garantir que o workspace Cargo escala para 10+ crates sem degradação
- Definir contratos de versão entre crates (semver interno)
- Auditar segurança da toolchain (supply chain, CVEs em dependências)

### Procedimentos

#### S0.1: Validar Toolchain Completa
```bash
# Verificar versões exatas contra ADR-001
rustc --version          # Esperado: 1.99.1+
cargo --version          # Esperado: 1.99.1+
wasm-pack --version      # Esperado: 0.15.0
wasmtime --version       # Esperado: 50.0.0
sqlite3 --version        # Esperado: 3.47.0+

# Verificar targets instalados
rustup target list --installed | grep wasm32
# Esperado: wasm32-unknown-unknown, wasm32-wasi
```

#### S0.2: Auditoria de Supply Chain
```bash
cd C:/Users/Paulo/zylos-next
cargo audit              # Requer: cargo install cargo-audit
cargo deny check         # Requer: cargo install cargo-deny
```
**Critério de Aceite Senior:** Zero advisories críticos. Warnings documentados em `DECISIONS.md` com justificativa.

#### S0.3: Contrato de Versão Entre Crates
Verificar que `Cargo.toml` do workspace usa path dependencies com versão explícita:
```toml
[workspace.dependencies]
zylos-bot-system = { path = "crates/zylos-bot-system", version = "0.1.0" }
zylos-memory-system = { path = "crates/zylos-memory-system", version = "0.1.0" }
```
**Por que importa:** Path-only dependencies quebram publicação futura no crates.io. Versão explícita permite migração gradual.

### Decisões Documentadas
| Decisão | Justificativa | Alternativa Rejeitada |
|---------|--------------|----------------------|
| Rust stable (não nightly) | Compatibilidade com wasmtime/wasm-pack; nightly quebra em updates | nightly por performance |
| Workspace Cargo (não monorepo tools) | Nativo Rust, zero overhead, IDE support | Bazel/Nx (overkill para Rust puro) |
| SQLite WAL mode | Concorrência leitor/escritor sem lock; compatível com petgraph | PostgreSQL (dependência externa pesada) |
| wasmtime (não wasmer) | Benchmark 180ms cold start vs 320ms; melhor WASI support | wasmer (API mais simples mas mais lenta) |

---

## 👷 NÍVEL PLENO — Implementação e Validação

### Responsabilidades
- Executar setup da toolchain seguindo guia passo-a-passo
- Configurar workspace Cargo com todos os crates
- Rodar testes de smoke para validar integração
- Documentar desvios do plano original

### Procedimentos

#### P0.1: Setup Inicial da Toolchain
```bash
# Instalar Rust (se não instalado)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Instalar versão específica
rustup install 1.99.1
rustup default 1.99.1

# Adicionar targets WASM
rustup target add wasm32-unknown-unknown
rustup target add wasm32-wasi

# Instalar ferramentas auxiliares
cargo install wasm-pack --version 0.15.0
cargo install wasmtime-cli --version 50.0.0
cargo install cargo-audit cargo-deny
```

#### P0.2: Criar Workspace Cargo
```bash
cd C:/Users/Paulo/zylos-next
cargo new --lib crates/zylos-bot-system
cargo new --lib crates/zylos-memory-system
cargo new --lib crates/zylos-sandbox
cargo new --lib crates/zylos-search-mcp
```

Configurar `Cargo.toml` raiz:
```toml
[workspace]
members = [
    "crates/zylos-bot-system",
    "crates/zylos-memory-system",
    "crates/zylos-sandbox",
    "crates/zylos-search-mcp",
]
resolver = "2"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

#### P0.3: Smoke Test do Workspace
```bash
cd C:/Users/Paulo/zylos-next
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```
**Critério de Aceite Pleno:** Todos os comandos retornam exit code 0. Qualquer warning de clippy é corrigido ou justificado.

### Checklist de Validação
- [ ] `rustc --version` retorna 1.99.1+
- [ ] `wasm-pack --version` retorna 0.15.0
- [ ] `wasmtime --version` retorna 50.0.0
- [ ] `cargo build --workspace` compila sem erros
- [ ] `cargo test --workspace` passa todos os testes
- [ ] `cargo clippy --workspace` zero warnings
- [ ] Targets wasm32 instalados (`rustup target list --installed`)

---

## 🌱 NÍVEL JÚNIOR — Execução Guiada

### Responsabilidades
- Seguir procedimentos passo-a-passo sem desvios
- Reportar erros exatos (mensagem + comando + output)
- Não modificar configurações sem aprovação Pleno/Senior
- Validar cada etapa antes de prosseguir

### Procedimentos Passo-a-Passo

#### J0.1: Verificar Instalação do Rust
Abra o terminal e execute:
```bash
rustc --version
```
✅ **Se retornar** `rustc 1.99.1 (...)` → prossiga para J0.2
❌ **Se retornar** "command not found" → instale Rust seguindo P0.1
❌ **Se retornar** versão diferente → execute `rustup default 1.99.1`

#### J0.2: Verificar Ferramentas Auxiliares
Execute cada comando e anote o resultado:
```bash
wasm-pack --version    # Anote: _______
wasmtime --version     # Anote: _______
sqlite3 --version      # Anote: _______
```
Compare com os valores esperados na seção S0.1. Se algum divergar, reporte ao Pleno com a mensagem exata.

#### J0.3: Compilar o Projeto
```bash
cd C:/Users/Paulo/zylos-next
cargo build --workspace 2>&1 | tee build.log
```
✅ **Se terminar com** `Finished` → sucesso, prossiga
❌ **Se houver erro** → copie a mensagem de erro completa e reporte

#### J0.4: Rodar Testes
```bash
cargo test --workspace 2>&1 | tee test.log
```
✅ **Esperado:** `test result: ok. X passed; 0 failed`
❌ **Se houver falha** → anote qual teste falhou e a mensagem de erro

### Glossário para Júniores
| Termo | Significado |
|-------|------------|
| **Workspace Cargo** | Configuração que agrupa múltiplos crates Rust em um único projeto compartilhando dependências |
| **Crate** | Unidade de código Rust (biblioteca ou executável); equivalente a "pacote" em outras linguagens |
| **wasm-pack** | Ferramenta que compila Rust para WebAssembly compatível com navegadores e Node.js |
| **wasmtime** | Runtime que executa módulos WebAssembly fora do navegador (servidor/sandbox) |
| **WAL mode** | Write-Ahead Logging do SQLite; permite leituras concorrentes sem bloquear escritas |
| **clippy** | Linter oficial do Rust que detecta padrões problemáticos e sugere correções |
| **target** | Plataforma de compilação (ex: `wasm32-unknown-unknown` = WebAssembly genérico) |

### Quando Escalar
- Erro de compilação que não entende após ler a mensagem → **Pleno**
- Divergência de versão de ferramenta → **Pleno**
- Falha em teste que já passou anteriormente → **Senior**
- Aviso de segurança (`cargo audit`) → **Senior imediatamente**

---

## 🔗 Referências Cruzadas
- **Master Plan:** `zylos-next-master-plan.md` linhas 1-100 (Fase 0)
- **ADRs:** `specs/DESIGN.md` (ADR-001: Toolchain, ADR-002: Workspace Structure)
- **Próxima Fase:** `runbooks/fase1-bot-system.md`
- **Manifesto:** `CONCEITOS-DESENVOLVIMENTO-IA.md` conceitos #1 (Rust First), #5 (Zero Trust Toolchain)