# Regras do Agente — Zylos-Next v6.0

## Contexto do Projeto
Zylos-Next é um runtime soberano de sistemas multi-agente escrito em Rust. Este documento define regras **enforceáveis** que todo agente (humano ou IA) DEVE seguir ao contribuir para este projeto. Violação = trabalho inválido.

## Arquivos Obrigatórios Antes de Codar
Antes de escrever qualquer código Rust, o agente DEVE ler e entender:
1. `PRD.md` — O que estamos construindo e por quê
2. `ARCHITECTURE.md` — Como os 6 sistemas core se conectam
3. `DESIGN_SYSTEM.md` — Tokens e estados obrigatórios (para frontend)
4. `SECURITY.md` — Regras de segurança camada zero
5. `TESTING.md` — Checklist de testes por tipo
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