# Memory — Zylos-Next v6.0

## Estado Atual (2026-11-10 — Pós-Sessão Skills + Manifesto v3.1)
- **Fase:** 7 (END_TO_END_INTEGRATION) — Fases 0-7C ✅ COMPLETAS | Dupla Auditoria ✅ APPROVED | HANDOFF_7.md criado
- **Manifesto:** v3.1 com 10 camadas obrigatórias, copiado para `docs/CONCEITOS-DESENVOLVIMENTO-IA.md` (independente do CÓRTEX)
- **Skills Hermes:** 28 skills nativas criadas e indexadas (16 canônicas + 6 operacionais + 3 estruturais + 3 orquestração avançada)
- **Auto-Invocação:** AGENTS.md atualizado com tabela de gatilhos — agentes carregam skills automaticamente sem solicitação do usuário
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` → ✅ CLEAN (verificado antes do cargo clean)
- **Fases 7B/7C:** Todas as 18 funções confirmadas IMPLEMENTADAS; GAP_ANALYSIS atualizado — 0 gaps restantes
- **Correções de clippy:** ~50+ fixes aplicados em todo o workspace
- **Auditorias:** Fase 7B (adversarial + isolada ✅) + Fase 7C (adversarial + isolada ✅)
- **Testes:** 1.087 testes passando (1.037 core + 50 dos 3 crates: swarm-orchestrator 10, goals 20, personalities 20)
- **Runbooks dos 3 crates:** zylos-swarm-orchestrator.md, zylos-goals.md, zylos-personalities.md criados com estrutura 3-tier
- **Documentação operacional:** 7 documentos (3 SOPs + 4 POPs) + índice atualizado em playbooks/README.md
- **Desktop app:** Notificações nativas OS, gateway auto-start, system tray implementados
- **Stubs substituídos:** chrono_now_stub → timestamp real, DockerBackend/WasmBackend → execução real, ApprovalWorkflow → auto-approve
- **⚠️ BLOCKER TEMPORÁRIO:** `cargo clean` corrompeu cache do registry; `STATUS_STACK_BUFFER_OVERRUN` ao recompilar. Requer reinício do sistema para restaurar ambiente de compilação. Código e testes estavam 100% funcionais antes do clean.

## Decisões Tomadas
| Data | Decisão | Racional | ADR |
|------|---------|----------|-----|
| 2026-10-30 | Rust como linguagem única | <30MB RAM por agente, memory safety, performance 5x vs Python | ADR-001 |
| 2026-10-30 | SQLite WAL como storage | ACID + zero-config + portabilidade; suficiente para 50 agentes | ADR-002 |
| 2026-10-30 | WASM (wasmtime) para sandbox | Isolamento forte obrigatório para tools de terceiros | ADR-003 |
| 2026-10-30 | Leptos para cockpit web | SSR nativo, sinais reativos, mesma linguagem do backend | ADR-004 |
| 2026-10-30 | petgraph + SQLite para Context Graph | Grafo persistente com queries <1ms p95 | ADR-005 |
| 2026-10-30 | Tokio JoinSet + CancellationToken | Lifecycle graceful sem leaks | ADR-006 |
| 2026-10-30 | Axum + Tower para API Gateway | Type-safe routing + middleware composável | ADR-007 |

## Convenções de Processo
### Dupla Auditoria Obrigatória ao Final de Cada Fase
- **Regra:** Nenhuma fase é concluída sem DUAS auditorias sequenciais por `code-reviewer` (juiz ≠ gerador)
- **Auditoria #1 (Adversarial Completa):** Valida conformidade com manifesto, compilação, testes, handoffs → gera `evals/audits/faseN-audit.md`
- **Auditoria #2 (Isolada Pós-Correção):** Verifica que TODAS as correções da #1 foram aplicadas → gera `evals/audits/faseN-audit-v2.md`
- **Gate:** Próxima fase só inicia após ambos os relatórios APPROVED
- **Registrado em:** AGENTS.md (seção "Auditoria Dupla Obrigatória"), Master Plan v3.0 (critérios de saída de todas as 8 fases), MEMORY.md (esta seção)

## Lições Aprendidas
### Fase 1 (BOT_SYSTEM)
- **Lifetime em closures async:** Referências `&str` não podem ser movidas para `async move` blocks dentro de `JoinSet::spawn`. Solução: converter para `String` antes do move (`name.to_string()`).
- **tokio-util é crate separado:** `CancellationToken` vive em `tokio_util::sync`, não em `tokio`. Requer dependência explícita `tokio-util = { version = "0.7", features = ["rt"] }`.
- **DashMap entry API:** `entry().or_insert_with()` retorna `RefMut` que precisa ser declarado como `mut` para modificar campos internos.
- **Broadcast channel sem subscribers:** `broadcast::send()` retorna `Ok(0)` quando não há receivers ativos, não erro. Dead letter queue captura esse caso.
- **petgraph toposort:** Retorna `Err(Cycle)` se grafo tiver ciclos; sempre verificar antes de executar DAG.
- **serde_json como dependência explícita:** Mesmo com `serde` no workspace, `serde_json` precisa ser adicionado separadamente ao Cargo.toml do crate.

### Fase 0 (Fundação)
- **rusqlite não tem feature `wal`:** WAL é modo ativado via pragma SQL, não feature do crate. Removido do Cargo.toml.
- **Conflito de toolchain Rust no Windows:** `rustup component add clippy` falha se binários residuais existirem. Solução: `rustup toolchain uninstall stable && rustup toolchain install stable`.
- **Strings triple-quoted longas no kernel Python:** Causam SyntaxError. Dividir documentos grandes em múltiplas chamadas de `write_file`.
- **PowerShell curl é alias para Invoke-WebRequest:** Não suporta `-L`. Usar Python urllib ou curl.exe explícito.

## Abordagens que Falharam
| Abordagem | Por que falhou | Alternativa adotada |
|-----------|---------------|-------------------|
| Reativar ~12 perfis Rust antigos | Profiles deletados do disco; recriar causaria crash do bot-forge | 16 skills Rust substituem perfis |
| Usar better-sqlite3 | Node.js apenas, incompatível com arquitetura Rust | sqlx / rusqlite |
| OMH plugin para orquestração | `omh_agent_board_core_unavailable`; pacote Python não instalado | Crew plugin como alternativa |
| Feature `wal` no rusqlite | Feature não existe no crate | Pragma WAL via SQL |

## Comandos Úteis
```bash
# Build workspace
cd /c/Users/Paulo/zylos-next && cargo build --workspace

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

# Toolchain info
rustc --version && cargo --version && wasm-pack --version && wasmtime --version
```

## Issues Conhecidos
- [x] Benchmarks de performance executados para ContextGraph (add_node 189µs, get_node 1.55µs, query_scope 73µs, neighbors 39µs) — VectorStore search (56ms/10k) e Rerank (10.3ms/1k) completos ✅
- [ ] Task 0.1.5 (Contrato de comunicação A2A/MCP) adiada para Fase 5 (LIVE_COMMUNICATION)

## Próximo Passo Imediato
Iniciar Fase 7 (END_TO_END_INTEGRATION): Integration testing, security audit final, release v1.0. Gate de entrada aprovado via HANDOFF_6.md.

## Referências
- [Manifesto Conceitos-Desenvolvimento-IA](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [Master Plan v3.0](./IMPLEMENTATION_PLAN.md)
- [DECISIONS.md](./DECISIONS.md) para ADRs completos