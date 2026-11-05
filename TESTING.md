# Estratégia de Testes — Zylos-Next v6.0

## Filosofia TDD com IA
Testes são escritos **ANTES ou EM PARALELO** ao código, nunca depois. O ciclo é:
```
Understand → Plan → Implement → Test → Review → Fix → Commit
```
Teste verde do caminho feliz **NÃO basta** — ataque bordas, round-trip, isca real.

## Checklist Estruturado por Tipo de Teste

### Unit Tests (por crate)
| Checklist | Status | Ferramenta |
|-----------|--------|------------|
| Happy Path: função retorna valor esperado | ☐ | `#[test]` |
| Validação: inputs inválidos retornam Error | ☐ | `assert!(matches!(...))` |
| Erro: falhas recuperáveis tratadas | ☐ | `thiserror` assertions |
| Edge Cases: empty, null, overflow, unicode | ☐ | `proptest` |
| Concorrência: race conditions, deadlocks | ☐ | `tokio::test` + `loom` |
| Performance: hot path < threshold | ☐ | `criterion` |
| Regressão: bugs fixados têm teste | ☐ | Regression test suite |

### Integration Tests (cross-crate)
| Checklist | Status | Ferramenta |
|-----------|--------|------------|
| BOT_SYSTEM: spawn → execute → result | ☐ | `tests/bot_integration.rs` |
| MEMORY_SYSTEM: write → query → rerank | ☐ | SQLite in-memory |
| LIVE_COMMUNICATION: WS connect → stream → disconnect | ☐ | `axum::test` |
| VERIFICATION_PROOF: claim → guardrail → ground | ☐ | Mock LLM |
| SANDBOX_ISOLATION: WASM load → exec → snapshot | ☐ | `wasmtime` test harness |
| SKILLS_REGISTRY: discover → validate → load | ☐ | Temp dir fixtures |
| End-to-End: request → all systems → response | ☐ | Docker compose test |

### Property-Based Tests (grafos/DAGs)
| Checklist | Status | Ferramenta |
|-----------|--------|------------|
| DAG acíclico após qualquer mutação | ☐ | `proptest` + petgraph |
| Context Graph consistency após insert/delete | ☐ | State machine testing |
| VectorStore similarity monotonicidade | ☐ | Float comparison epsilon |
| Shadow-Git rollback idempotência | ☐ | Random operation sequences |
| P2P message delivery guarantee | ☐ | Channel stress test |

### Benchmarks (performance regression)
| Metric | Target | Tool |
|--------|--------|------|
| Cold start time | <180ms | `criterion` |
| Agent spawn latency | <10ms | `criterion` |
| Memory query p95 | <1ms | `criterion` |
| WASM snapshot time | <100ms | `criterion` |
| Guardrail check p95 | <1ms | `criterion` |
| Throughput (tasks/sec) | >2400 | Custom benchmark |
| RAM per idle agent | <30MB | `/proc/self/status` |

### Security Tests
| Checklist | Status | Tool |
|-----------|--------|------|
| SQL injection prevention | ☐ | rusqlite prepared statements |
| Path traversal blocked | ☐ | Canonical path validation |
| Secret detection in code | ☐ | `gitleaks` pre-commit |
| Dependency vulnerabilities | ☐ | `cargo audit` CI job |
| WASM sandbox escape | ☐ | wasmtime fuzzing |
| Authorization bypass | ☐ | Role-based test matrix |

## Checklist de Testes de Permissões e Autorização

| Cenário | Status | Método de Verificação |
|---------|--------|----------------------|
| Usuário sem role acessa endpoint protegido | ☐ | HTTP 403 em integração test |
| Role `viewer` tenta write operation | ☐ | HTTP 403 + audit log entry |
| Role `admin` executa operação privilegiada | ☐ | HTTP 200 + success audit |
| Token expirado rejeitado | ☐ | HTTP 401 com mensagem clara |
| Cross-tenant access bloqueado | ☐ | Tenant isolation test suite |
| API key revogada não autentica | ☐ | Revocation propagation <5s |
| Rate limit por role respeitado | ☐ | Load test com múltiplos roles |
| RBAC matrix cobre todos os endpoints | ☐ | Coverage report ≥95% |
| Secrets nunca aparecem em logs/respostas | ☐ | Log scrubbing validation |
| Permission cache invalidation correta | ☐ | Cache bust após role change |

## Estrutura de Diretórios de Testes
```
tests/
├── unit/               # Unit tests por crate (no próprio crate/src/)
├── integration/        # Cross-crate integration tests
│   ├── bot_system.rs
│   ├── memory_system.rs
│   ├── live_comm.rs
│   ├── verification.rs
│   ├── sandbox.rs
│   └── skills_registry.rs
├── e2e/                # End-to-end com Docker
│   ├── docker-compose.test.yml
│   └── scenarios/
├── fixtures/           # Dados de teste versionados
│   ├── skills/
│   ├── graphs/
│   └── workflows/
└── benchmarks/         # Criterion benchmarks
    ├── bot_bench.rs
    ├── memory_bench.rs
    └── sandbox_bench.rs
```

## Convenções de Nomenclatura
- **Unit:** `test_<function>_<scenario>` (ex: `test_spawn_agent_with_invalid_config`)
- **Integration:** `test_<system>_<flow>` (ex: `test_memory_write_query_rerank`)
- **Benchmark:** `bench_<operation>` (ex: `bench_agent_spawn_latency`)
- **Property:** `prop_<invariant>` (ex: `prop_dag_acyclic_after_mutation`)

## Cobertura Mínima
- **Código de produção:** >80% line coverage
- **Funções públicas:** 100% devem ter pelo menos 1 teste
- **Error paths:** Todos os variants de erro devem ser exercitados
- **Hot paths:** Benchmark obrigatório com baseline versionado

## Ferramentas Obrigatórias
| Ferramenta | Uso | Config |
|------------|-----|--------|
| `cargo test` | Runner padrão | `--all-features --workspace` |
| `proptest` | Property-based | 1000 cases default |
| `criterion` | Benchmarks | HTML reports em `target/criterion/` |
| `cargo-llvm-cov` | Coverage | LCOV output para Codecov |
| `cargo-audit` | Security | Fail on critical vulnerabilities |
| `loom` | Concurrency model checking | Apenas para crates async core |

## Referências
- [Manifesto Conceito #6](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [AGENTS.md](./AGENTS.md) para convenções de código
- [EVALUATION.md](./EVALUATION.md) para métricas de benchmark