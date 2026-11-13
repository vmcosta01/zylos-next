# RULES.md — Zylos-Next Project Conventions & Boundaries

**Versão:** 1.0.0
**Data:** 2026-11-12
**Manifesto:** [CONCEITOS-DESENVOLVIMENTO-IA.md](./docs/CONCEITOS-DESENVOLVIMENTO-IA.md) v3.1

---

## 1. Contexto do Projeto

- **Nome:** Zylos-Next
- **Linguagem:** Rust 1.82+ (stable)
- **Frameworks:** Axum 0.7 (gateway), Leptos 0.7 (cockpit WASM), Tauri v2 (desktop)
- **Propósito:** Sistema multi-agente com cockpit de operação, orquestração SOAR, governança de AI employees e persistência em SQLite
- **Documentos base:**
  - [PRD.md](./PRD.md) — Requisitos funcionais
  - [ARCHITECTURE.md](./docs/ARCHITECTURE.md) — Diagramas C4 e decisões
  - [SECURITY.md](./SECURITY.md) — Threat model e mitigações
  - [AGENTS.md](./AGENTS.md) — Regras de auto-invocação e comportamento de agentes
  - [DESIGN_SYSTEM.md](./docs/DESIGN_SYSTEM.md) — Tokens visuais e estados de UI

---

## 2. Arquivos Obrigatórios Antes de Codar

Todo agente DEVE ler nesta ordem antes de iniciar qualquer tarefa:

1. `AGENTS.md` — Regras de comportamento e auto-invocação
2. `RULES.md` — Este documento (convenções e boundaries)
3. `docs/CONCEITOS-DESENVOLVIMENTO-IA.md` — Manifesto v3.1 (10 camadas)
4. `IMPLEMENTATION_PLAN.md` ou plano da fase corrente — Tasks e gates
5. `docs/DESIGN_SYSTEM.md` — Se tarefa envolve UI
6. `SECURITY.md` — Se tarefa envolve segurança/auth/rede

---

## 3. Convenções de Código

### Naming
- **Crates:** `zylos-{domínio}` (kebab-case): `zylos-channels`, `zylos-auth`, `zylos-analytics`
- **Módulos:** snake_case: `channels_api`, `webrtc_relay`, `auth_login`
- **Types:** PascalCase: `ChannelMessage`, `TopologyNode`, `SkillDef`
- **Functions:** snake_case: `fetch_json`, `generate_yaml_preview`, `validate_yaml_preview`
- **Constants:** SCREAMING_SNAKE_CASE: `LS_KEY`, `DEFAULT_LEASE_MINUTES`
- **CSS classes:** kebab-case com prefixo de componente: `channel-panel`, `skill-editor-toolbar`, `topology-node`

### Error Handling
- **Crates:** Usar `thiserror` para erros de biblioteca, `anyhow` apenas em binários/tests
- **Proibido:** `unwrap()` em código de produção (apenas em testes com comentário justificativo)
- **Gateway handlers:** Retornar `(StatusCode, Json<ErrorResponse>)` padronizado
- **Cockpit components:** Sempre tratar `Err` em `fetch_json()` com estado de erro visível na UI + fallback documentado com `// FALLBACK: <justificativa>`

### Async Patterns
- **Runtime:** Tokio (gateway), wasm-bindgen-futures (cockpit)
- **Cockpit:** `spawn_local` de `leptos::task` para async no browser
- **Blocking:** Nunca bloquear thread async; usar `tokio::task::spawn_blocking` para I/O síncrono
- **Cancellation:** Todos os loops infinitos devem respeitar `CancellationToken` ou `select!`

### Safety
- **`unsafe`:** Proibido exceto em wrappers FFI documentados com `// SAFETY:` comment explicando invariantes
- **Input validation:** Todo input externo DEVE ser validado via serde deserialize ou schema explícito
- **SQL:** Apenas queries parametrizadas via sqlx; string interpolation em SQL é violação crítica

### Testing
- **TDD:** Testes escritos ANTES ou EM PARALELO ao código (Conceito #6)
- **Cobertura:** ≥80% para crates de negócio, ≥90% para security-critical
- **Property-based:** Obrigatório para sanitizers, parsers e validadores (`proptest`)
- **Naming:** `test_{funcionalidade}_{cenário}`: `test_hop_count_circuit_breaker`, `test_validate_yaml_missing_name`

### Documentation
- **Doc comments:** Obrigatórios em todo `pub fn`, `pub struct`, `pub enum`, `pub trait`
- **README:** Todo crate DEVE ter README.md com: propósito, API pública, exemplos, dependências
- **Inline comments:** Apenas para explicar "porquê", nunca "o quê"

---

## 4. Regras de Segurança

- **Secrets:** NUNCA commitar tokens, passwords ou keys. Usar env vars (`ZYLOS_HERMES_TOKEN`, `JWT_SECRET`)
- **Authorization:** Server-side ALWAYS. Frontend checks são UX-only, nunca security boundary
- **Sandboxing:** Tools terceiros (LLM calls, code execution) DEVEM rodar em WASM sandbox ou Docker container
- **Input validation:** Serde + jsonschema para todos os endpoints REST. Rejeitar early com 400 Bad Request
- **CORS:** Whitelist explícita em `local_guard.rs`. Nunca `allow_origin("*")` em produção
- **JWT:** HS256 mínimo, expiry obrigatório, refresh token rotation, revocation list
- **Egress:** NetworkEgressFilter whitelist-only. Default deny. Hot-reload via rules.json

---

## 5. Boundaries (O Que NÃO Mudar Sem Approval)

As seguintes mudanças requerem aprovação explícita do Tech Lead + registro em DECISIONS.md:

- Estrutura de crates/diretórios em `crates/` e `apps/`
- Dependências workspace-level no `Cargo.toml` raiz
- Protocolos de comunicação (WebSocket message format, REST API contracts)
- Schema de banco de dados (migrations em `crates/*/migrations/`)
- Configurações de CI/CD (`.github/workflows/`)
- Este arquivo (`RULES.md`) e `AGENTS.md`
- Design tokens em `DESIGN_SYSTEM.md` (cores, tipografia, spacing)
- Portas e endpoints padrão do gateway (3000, /api/v1/*)

---

## 6. Loop Profissional Obrigatório

```
READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS
```

Cada etapa DEVE produzir artefato verificável:
- **READ:** Lista de arquivos lidos + resumo do contexto
- **UNDERSTAND:** Spec técnica ou clarificação de requisitos
- **PLAN:** Tarefas atômicas com dependências e estimativas
- **IMPLEMENT:** Código + testes unitários
- **TEST:** Gate verde (`cargo test`, `cargo clippy -- -D warnings`)
- **REVIEW:** Auditoria adversarial ou peer review
- **FIX:** Correções aplicadas + re-teste
- **COMMIT:** Mensagem descritiva, branch por feature
- **UPDATE DOCS:** TrackRecord, Handoff, README, MODULE_DOCUMENTATION_STATUS atualizados

Pular etapas = trabalho inválido (Conceito #3, Penalidade #4).

---

## 7. Auditoria Dupla Obrigatória

Ao final de cada fase:

1. **Auditoria Adversarial:** Juiz ≠ gerador. Tenta REFUTAR a implementação. Relatório em `evals/audits/` ou `docs/audits/`
2. **Auditoria Isolada:** Contexto-zero. Verifica se correções foram realmente aplicadas. Sem acesso ao histórico da implementação
3. **Gate:** Ambas as auditorias devem retornar APPROVED ou CHANGES_REQUIRED com findings resolvidos antes de avançar

Skills: `isolated-audit`, `adversarial-panel`, `finding-validator`

---

## 8. Comandos Úteis

### Build & Check
```bash
cargo check --workspace                    # Verifica compilação
cargo clippy --workspace -- -D warnings     # Lint estrito (gate obrigatório)
cargo build --workspace                     # Build dev
cargo build --release --workspace           # Build release
```

### Test
```bash
cargo test --workspace                      # Todos os testes
cargo test -p zylos-channels                # Crate específico
cargo test --workspace -- --nocapture       # Com output visível
wasm-pack test --headless --chrome          # E2E cockpit WASM
```

### Cockpit WASM
```bash
cd apps/zylos-cockpit
trunk build                                 # Dev build
trunk build --release                       # Release build
trunk serve                                 # Dev server com hot reload
```

### Gateway
```bash
cargo run -p zylos-gateway                  # Run dev server
ZYGOS_HERMES_TOKEN=secret cargo run -p zylos-gateway  # Com auth
```

### Quality
```bash
cargo audit                                 # Vulnerabilidades em deps
cargo deny check                            # License + ban check
trivy fs .                                  # Filesystem scan
```

### Docs
```bash
cargo doc --workspace --no-deps --open      # Gerar e abrir docs
```

---

*Documento gerado via skill `agents-rules-author` v1.0.0. Revisão obrigatória após mudança de stack ou processo.*