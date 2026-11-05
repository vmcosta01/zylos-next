# Evaluation & Benchmarks — Zylos-Next v6.0

## Filosofia de Avaliação (Conceito #22)
Nenhum agente ou sistema é promovido sem benchmark versionado. Métricas são artefatos de primeira classe, não afterthoughts. Todo crate core tem targets mensuráveis definidos ANTES da implementação.

## Métricas por Crate

### zylos-bot-system
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| Agent spawn latency | <10ms p95 | criterion bench | CI + weekly |
| DAG execution (100 nodes) | <50ms p95 | criterion bench | CI + weekly |
| P2P message delivery | <1ms p99 | tokio::test + loom | CI |
| Recovery ladder activation | <100ms | integration test | CI |
| Shadow-git commit | <5ms | criterion bench | CI |
| RAM per idle agent | <30MB | /proc/self/status | CI + nightly |
| Concurrent agents (no lag) | 50+ | load test (k6) | Weekly |

### zylos-memory-system
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| Context graph write | <1ms p95 | criterion bench | CI |
| Context graph query | <1ms p95 | criterion bench | CI |
| VectorStore similarity search | <10ms p95 | criterion bench | CI |
| Brief-aware rerank | <5ms p95 | criterion bench | CI |
| Scope isolation overhead | <0.1ms | micro-bench | CI |
| SQLite WAL checkpoint | <50ms | integration test | Nightly |
| Memory bloat (1M entries) | <500MB | stress test | Weekly |

### zylos-live-comm
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| WebSocket connect time | <50ms | axum::test | CI |
| SSE event latency | <10ms p95 | integration test | CI |
| AG-UI streaming throughput | 1000 events/sec | load test | Weekly |
| Mobile MCP response | <100ms p95 | integration test | CI |
| Cockpit render (50 agents) | <16ms (60fps) | browser perf | Weekly |

### zylos-verification
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| Guardrail check | <1ms p95 | criterion bench | CI |
| Adversarial panel verdict | <500ms (excl LLM) | mock test | CI |
| Tree-of-Thoughts exploration | <2s (10 branches) | criterion bench | Weekly |
| False positive rate | <0.1% | eval dataset | Monthly |
| False negative rate | <1% | eval dataset | Monthly |

### zylos-sandbox
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| WASM cold start | <180ms | criterion bench | CI |
| WASM snapshot create | <100ms | criterion bench | CI |
| WASM snapshot resume | <50ms | criterion bench | CI |
| Docker container spawn | <500ms | integration test | CI |
| Sandbox escape attempts | 0 (must fail) | fuzzing | Nightly |

### zylos-skills
| Métrica | Target | Método | Frequência |
|---------|--------|--------|------------|
| index.json discovery | <100ms | criterion bench | CI |
| Schema validation | <1ms | criterion bench | CI |
| Skill load time | <10ms | criterion bench | CI |
| Eval flywheel judge | <2s (excl LLM) | mock test | Weekly |

## Datasets Versionados

### Eval Datasets
```
evals/datasets/
├── v1.0/
│   ├── context-graph-queries.json    # 10K queries com ground truth
│   ├── dag-workflows.json            # 500 DAGs reais
│   ├── verification-claims.json      # 2K claims com labels
│   ├── skills-index.json             # 1K skills metadata
│   └── README.md                     # Changelog + metodologia
└── latest -> v1.0/
```

### Benchmark Baselines
```
evals/baselines/
├── v1.0/
│   ├── bot-system.json               # criterion output
│   ├── memory-system.json
│   ├── live-comm.json
│   ├── verification.json
│   ├── sandbox.json
│   └── skills.json
└── latest -> v1.0/
```

## Protocolo de Promoção de Agente
1. **Benchmark verde**: todas as métricas do crate dentro do target
2. **Eval dataset**: accuracy > threshold definido em EVALUATION.md
3. **Adversarial panel**: juiz ≠ gerador aprova resultado
4. **Regression check**: nenhuma métrica degradou >5% vs baseline
5. **Sign-off**: approval manual em DECISIONS.md com link para relatório

## Ferramentas de Eval
| Ferramenta | Uso | Config |
|------------|-----|--------|
| `criterion` | Micro/macro benchmarks | HTML reports em `target/criterion/` |
| `proptest` | Property-based testing | 1000 cases default |
| `cargo-llvm-cov` | Code coverage | LCOV → Codecov |
| Custom eval runner | Dataset-driven evaluation | `evals/run-evals.sh` |
| LLM-as-judge | Adversarial verification | `evals/judges/` templates |

## Reporting
- **CI**: Badge no README com status do último benchmark
- **Weekly**: Relatório automático em `evals/reports/YYYY-WNN.md`
- **Monthly**: Trend analysis comparando últimas 4 semanas
- **Release**: Full benchmark report em CHANGELOG.md

## Referências
- [Manifesto Conceito #22](file:///C:/Users/Paulo/.claude/CONCEITOS-DESENVOLVIMENTO-IA.md)
- [TESTING.md](./TESTING.md) para estratégia de testes
- [ARCHITECTURE.md](./ARCHITECTURE.md) para métricas por sistema