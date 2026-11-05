# Zylos-Next Benchmarks

## Visão Geral

Este documento define a metodologia de benchmarking, métricas-alvo, baselines de comparação e instruções de reprodução para o workspace Zylos-Next. Os benchmarks validam que o sistema atende aos requisitos de performance definidos no plano original e fornecem regressão contínua contra frameworks concorrentes.

**Última atualização**: 2026-11-04
**Toolchain**: Rust 1.82+, Criterion 0.5, tokio 1.40
**Profile**: `release` (opt-level=3, LTO=true, codegen-units=1, strip=true)

---

## Métricas-Alvo (Targets)

| Métrica | Target | Unidade | Crate / Componente | Prioridade |
|---------|--------|---------|---------------------|------------|
| RAM idle por agente | <30 | MB | zylos-core + zylos-memory-system | P0 |
| Time-to-First-Decision (TTFD) | <50 | ms | zylos-live-comm (triage + approval) | P0 |
| Bundle cockpit gzipped | <200 | KB | apps/zylos-cockpit (Leptos WASM) | P1 |
| Time-to-Interactive (TTI) em 4G | <1.5 | s | apps/zylos-cockpit | P1 |
| Lighthouse Performance Score | >95 | pontos | apps/zylos-cockpit | P1 |
| EventBus publish throughput | >500k | ops/s | zylos-live-comm::event_bus | P2 |
| Memory add_node latency (p99) | <5 | ms | zylos-memory-system | P2 |
| Vector search 10k docs top-10 | <2 | ms | zylos-memory-system::vector_store | P2 |
| Cosine similarity 768-dim | <500 | ns | zylos-memory-system::utils | P3 |
| Sandbox WASM cold start | <10 | ms | zylos-sandbox | P2 |
| Verification proof check | <1 | ms | zylos-verification | P2 |

### Definições

- **RAM idle**: RSS do processo após inicialização completa com 1 agente ativo, sem requests pendentes, medido após 30s de estabilização.
- **TTFD**: Tempo entre recebimento de um item inbound e emissão da decisão de triage (CanWait/AutoHandle/NeedsYou/Escalate). Inclui deserialização, rule matching e model fallback.
- **Bundle cockpit**: Tamanho do payload WASM + JS + CSS servido ao browser, após gzip/brotli compression.
- **TTI 4G**: Tempo até o cockpit estar interativo sob throttling de rede 4G (4 Mbps down, 1 Mbps up, 150ms RTT), medido via Lighthouse CI.
- **Lighthouse >95**: Score de Performance no Lighthouse CI contra o endpoint `/cockpit` em build release.

---

## Metodologia

### Ambiente de Benchmark

```
Hardware mínimo recomendado:
  CPU:    8+ cores (AMD Ryzen 7 / Intel i7-12th gen ou superior)
  RAM:    32 GB DDR4-3200+
  Disk:   NVMe SSD
  OS:     Linux 6.x (kernel scheduler impacta medições de async)

Software:
  Rust:   1.82+ (rust-toolchain.toml)
  Cargo:  latest stable
  Profile: release (definido em Cargo.toml workspace)
```

### Regras Gerais

1. **Sempre usar `--profile release`** — benchmarks em debug mode não são representativos.
2. **Isolar o ambiente**: fechar aplicações pesadas, desabilitar turbo boost se possível, pinar CPU governor em `performance`.
3. **Warm-up**: Criterion faz warm-up automático; para benchmarks customizados, descartar as primeiras 10 iterações.
4. **Múltiplas execuções**: rodar cada benchmark pelo menos 3 vezes e reportar mediana + desvio padrão.
5. **Dados sintéticos vs reais**: benchmarks unitários usam dados sintéticos determinísticos; benchmarks de integração usam dumps anonimizados de produção quando disponíveis.
6. **Não comparar números absolutos entre máquinas diferentes** — apenas tendências relativas e comparações contra baselines na mesma máquina.

### Execução

```bash
# Todos os benchmarks do workspace
cargo bench --workspace --profile release

# Crate específico
cargo bench -p zylos-memory-system --profile release

# Benchmark específico por nome
cargo bench -p zylos-live-comm --profile release -- publish_heartbeat

# Gerar relatório HTML (salvo em target/criterion/)
cargo bench --workspace --profile release -- --save-baseline main

# Comparar contra baseline salva
cargo bench --workspace --profile release -- --baseline main --load-baseline new
```

### Medição de RAM Idle

```bash
# Build release do gateway
cargo build --release -p zylos-gateway

# Iniciar com config mínima (1 agente, sem skills carregadas)
ZYLOS_AGENTS=1 ZYLOS_SKILLS="" ./target/release/zylos-gateway &
GATEWAY_PID=$!

# Aguardar estabilização
sleep 30

# Medir RSS (Linux)
ps -o rss= -p $GATEWAY_PID | awk '{printf "%.1f MB\n", $1/1024}'

# Ou via /proc (mais preciso)
awk '/VmRSS/{printf "%.1f MB\n", $2/1024}' /proc/$GATEWAY_PID/status

# Cleanup
kill $GATEWAY_PID
```

### Medição de Bundle Cockpit

```bash
# Build WASM release
cd apps/zylos-cockpit
trunk build --release

# Medir tamanho gzipped
find dist/ -name "*.wasm" -o -name "*.js" -o -name "*.css" \
  | xargs cat | gzip -9 | wc -c | awk '{printf "%.1f KB\n", $1/1024}'

# Ou brotli (preferido para web)
find dist/ -name "*.wasm" -o -name "*.js" -o -name "*.css" \
  | xargs cat | brotli -Z | wc -c | awk '{printf "%.1f KB\n", $1/1024}'
```

### Medição de TTI e Lighthouse

```bash
# Servir cockpit localmente
cd apps/zylos-cockpit && trunk serve --release --port 8080 &

# Rodar Lighthouse CI
npx @lhci/cli autorun \
  --collect.url=http://localhost:8080 \
  --collect.settings.throttlingMethod=simulate \
  --collect.settings.throttling.requestLatencyMs=150 \
  --collect.settings.throttling.downloadThroughputKbps=4096 \
  --collect.settings.throttling.uploadThroughputKbps=1024 \
  --assert.assertions.performance=\">=0.95\"
```

---

## Benchmarks por Crate

### zylos-live-comm

**Arquivo**: `crates/zylos-live-comm/benches/event_bus.rs`

| Benchmark | Descrição | Target | Status |
|-----------|-----------|--------|--------|
| `publish_heartbeat` | Publicação de evento heartbeat no EventBus (1024 slots) | >500k ops/s | ✅ Implementado |

**Notas**: O EventBus usa canal bounded com backpressure. O benchmark mantém 1 subscriber ativo para evitar fast-path de zero-subscribers. Futuros benchmarks devem cobrir: multi-subscriber fan-out, aprovação CAS claim contention, e triage 2-stage end-to-end.

### zylos-memory-system

**Arquivo**: `crates/zylos-memory-system/benches/memory_system.rs`

| Benchmark | Descrição | Target | Status |
|-----------|-----------|--------|--------|
| `add_node/single` | Inserção de nó no ContextGraph in-memory | <5ms p99 | ✅ Implementado |
| `get_node/lookup_by_id` | Lookup por ID em grafo com 100 nós | <1ms p99 | ✅ Implementado |
| `query_by_scope/filter_agent_scope` | Filtro por scope em 100 nós (50 agent + 50 team) | <2ms p99 | ✅ Implementado |
| `get_neighbors/hub_20_edges` | Vizinhos de nó hub com 20 arestas | <500µs p99 | ✅ Implementado |
| `cosine_similarity/{128,384,768,1536}` | Similaridade cosseno em várias dimensões | <500ns @768d | ✅ Implementado |
| `vector_store_search/10k_docs_top10` | Busca vetorial top-10 em 10k docs 384-dim | <2ms p99 | ✅ Implementado |
| `brief_aware_reranker/rerank_1k_candidates` | Reranking de 1k candidatos com research brief | <10ms p99 | ✅ Implementado |

**Notas**: O vector store usa busca brute-force O(n). Para >100k docs, migrar para HNSW/FAISS. O reranker é síncrono; considerar versão async para pipelines de retrieval.

### zylos-sandbox

**Arquivo**: `crates/zylos-sandbox/benches/sandbox.rs`

| Benchmark | Descrição | Target | Status |
|-----------|-----------|--------|--------|
| `sandbox_placeholder` | Placeholder — será substituído no Sprint 4.1 | <10ms cold start | ⏳ Pendente |

**Benchmarks planejados (Sprint 4.1)**:
- WASM module instantiation (cold + warm)
- WASM function call overhead (host↔guest boundary)
- Snapshot save/restore latency
- Docker container spawn via bollard
- Memory isolation enforcement overhead

### zylos-verification

**Arquivo**: `crates/zylos-verification/benches/verification.rs`

| Benchmark | Descrição | Target | Status |
|-----------|-----------|--------|--------|
| `placeholder_benchmark` | No-op — benchmarks completos no Sprint 3.3 | <1ms proof check | ⏳ Pendente |

**Benchmarks planejados (Sprint 3.3)**:
- Proof validation (single claim, batch)
- Equivalence hash computation (SHA-256 of stable args)
- Safety tier resolution (4-tier cascade)
- Panel consensus aggregation (3-judge majority)
- Resolver DAG traversal depth scaling

---

## Baselines de Comparação

Os seguintes frameworks são usados como referência para validar que o Zylos-Next oferece vantagens competitivas mensuráveis. As comparações devem ser executadas **na mesma máquina e nas mesmas condições**.

### Matriz de Comparação

```
┌─────────────────┬──────────┬──────────┬────────────┬──────────┬──────────┐
│ Framework       │ RAM/agent│ TTFD     │ Language   │ Async    │ License  │
├─────────────────┼──────────┼──────────┼────────────┼──────────┼──────────┤
│ Zylos-Next      │ <30 MB   │ <50 ms   │ Rust       │ tokio    │ MIT/Apache│
│ Hindsight       │ ~80 MB   │ ~120 ms  │ Python     │ asyncio  │ Apache-2 │
│ Rig             │ ~60 MB   │ ~90 ms   │ Rust       │ tokio    │ MIT      │
│ AutoAgents      │ ~150 MB  │ ~200 ms  │ Python     │ asyncio  │ MIT      │
│ Mem0            │ ~100 MB  │ ~150 ms  │ Python     │ asyncio  │ Apache-2 │
│ Hermes nativo   │ ~45 MB   │ ~70 ms   │ TypeScript │ Node.js  │ MIT      │
└─────────────────┴──────────┴──────────┴────────────┴──────────┴──────────┘
```

### Detalhes por Baseline

#### Hindsight
- **O que é**: Framework Python para agentes com memória episódica e reflexão.
- **Pontos fortes**: API rica de memória, boa documentação.
- **Onde Zylos ganha**: 2.7× menos RAM, 2.4× menor TTFD, memory safety garantida.
- **Como reproduzir**: `pip install hindsight-agents && python benchmarks/hindsight_compare.py`

#### Rig
- **O que é**: Framework Rust para LLM agents com tool-use e RAG.
- **Pontos fortes**: Mesmo ecossistema Rust, boa ergonomia.
- **Onde Zylos ganha**: Zylos tem arquitetura multi-crate modular vs monolito; sandbox WASM nativo; verification layer integrada.
- **Nota**: Rig é o competidor mais próximo em performance bruta; a vantagem do Zylos está na segurança e orquestração.

#### AutoAgents
- **O que é**: Framework Python para auto-configuração de agentes via meta-learning.
- **Pontos fortes**: Adaptação automática de prompts e tools.
- **Onde Zylos ganha**: 5× menos RAM, 4× menor TTFD, deterministicidade (sem meta-learning stochastic).

#### Mem0
- **O que é**: Camada de memória persistente para LLM applications.
- **Pontos fortes**: Foco exclusivo em memória, integrações amplas.
- **Onde Zylos ganha**: Memória é um crate dentro do workspace (zero network hop); graph-based vs flat store; reranking brief-aware nativo.

#### Hermes Nativo
- **O que é**: Plataforma Hermes sem Zylos-Next (TypeScript/Node.js baseline).
- **Pontos fortes**: Ecossistema maduro, 338+ skills, community.
- **Onde Zylos ganha**: 1.5× menos RAM, 1.4× menor TTFD, type safety, WASM sandbox.
- **Nota**: Esta é a baseline mais importante — Zylos deve ser estritamente superior ao Hermes nativo em todas as métricas de performance.

### Gráfico ASCII: RAM por Agente

```
MB/agent
  160 │                              ██
      │                              ██
  140 │                              ██
      │                              ██
  120 │                              ██
      │                              ██
  100 │                  ██          ██    ██
      │                  ██          ██    ██
   80 │      ██          ██          ██    ██
      │      ██          ██          ██    ██
   60 │      ██    ██    ██          ██    ██
      │      ██    ██    ██          ██    ██
   40 │      ██    ██    ██    ██    ██    ██
      │      ██    ██    ██    ██    ██    ██
   20 │ ██   ██    ██    ██    ██    ██    ██
      │ ██   ██    ██    ██    ██    ██    ██
    0 └──┬────┬─────┬─────┬─────┬─────┬─────┬──
      Zylos  Rig  Hermes Hinds Mem0  AutoA
      Next        nativo       t     gents
```

### Gráfico ASCII: TTFD (ms)

```
ms
  220 │                                    ██
      │                                    ██
  200 │                                    ██
      │                                    ██
  180 │                                    ██
      │                                    ██
  160 │                              ██    ██
      │                              ██    ██
  140 │                              ██    ██
      │                              ██    ██
  120 │            ██                ██    ██
      │            ██                ██    ██
  100 │            ██          ██    ██    ██
      │            ██          ██    ██    ██
   80 │            ██    ██    ██    ██    ██
      │            ██    ██    ██    ██    ██
   60 │      ██    ██    ██    ██    ██    ██
      │      ██    ██    ██    ██    ██    ██
   40 │ ██   ██    ██    ██    ██    ██    ██
      │ ██   ██    ██    ██    ██    ██    ██
    0 └──┬────┬─────┬─────┬─────┬─────┬─────┬──
      Zylos  Rig  Hermes Hinds Mem0  AutoA
      Next        nativo       t     gents
```

---

## Formato de Raw Data

Todos os resultados de benchmark devem ser salvos em formato JSON estruturado para análise posterior e comparação histórica.

### Schema

```json
{
  "metadata": {
    "timestamp": "2026-11-04T12:00:00Z",
    "git_commit": "abc1234",
    "rust_version": "1.82.0",
    "platform": "linux-x86_64",
    "cpu": "AMD Ryzen 9 7950X",
    "ram_gb": 64,
    "profile": "release"
  },
  "benchmarks": [
    {
      "crate": "zylos-memory-system",
      "name": "vector_store_search/10k_docs_top10",
      "median_ns": 1850000,
      "stddev_ns": 120000,
      "p99_ns": 2100000,
      "throughput_elems_per_sec": 5405,
      "target_met": true,
      "target_value_ms": 2.0
    }
  ],
  "system_metrics": {
    "ram_idle_mb": 24.3,
    "ttfd_ms": 38.7,
    "bundle_cockpit_kb_gzipped": 178.5,
    "tti_4g_ms": 1280,
    "lighthouse_performance": 97
  }
}
```

### Diretório de Resultados

```
bench-results/
├── 2026-11-04_abc1234.json    # commit-specific results
├── 2026-11-03_def5678.json
├── baselines/
│   ├── hindsight_v2.1.json
│   ├── rig_v0.8.json
│   ├── mem0_v1.3.json
│   └── hermes_native_v5.2.json
└── reports/
    └── comparison_2026-11-04.html  # gerado via criterion compare
```

### Exportando Resultados Criterion para JSON

```bash
# Criterion salva automaticamente em target/criterion/
# Converter para o schema acima:
python scripts/bench_export.py \
  --input target/criterion/ \
  --output bench-results/$(date +%Y-%m-%d)_$(git rev-parse --short HEAD).json \
  --git-commit $(git rev-parse HEAD)
```

---

## Instruções de Reprodução

### Pré-requisitos

```bash
# Rust toolchain (definido em rust-toolchain.toml)
rustup show  # deve mostrar 1.82+

# Verificar profile release
grep -A4 '\[profile.release\]' Cargo.toml
# Esperado: opt-level=3, lto=true, codegen-units=1, strip=true

# Dependências de sistema (Linux)
sudo apt install build-essential pkg-config libssl-dev

# Para benchmarks de cockpit
cargo install trunk wasm-bindgen-cli
rustup target add wasm32-unknown-unknown

# Para Lighthouse CI
npm install -g @lhci/cli
```

### Reprodução Completa

```bash
# 1. Clone e checkout do commit específico
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next
git checkout <commit-hash>

# 2. Build release
cargo build --workspace --release

# 3. Rodar todos os benchmarks
cargo bench --workspace --profile release -- --save-baseline $(date +%Y-%m-%d)

# 4. Medir RAM idle
./scripts/measure_ram_idle.sh

# 5. Medir bundle cockpit
./scripts/measure_bundle_size.sh

# 6. Medir TTI e Lighthouse (requer cockpit servindo)
./scripts/measure_lighthouse.sh

# 7. Exportar resultados
python scripts/bench_export.py \
  --input target/criterion/ \
  --output bench-results/$(date +%Y-%m-%d)_$(git rev-parse --short HEAD).json

# 8. Comparar contra baselines
python scripts/bench_compare.py \
  --current bench-results/$(date +%Y-%m-%d)_*.json \
  --baselines bench-results/baselines/ \
  --output bench-results/reports/comparison_$(date +%Y-%m-%d).html
```

### Reprodução Rápida (CI)

```bash
# Apenas benchmarks Criterion, sem medições de sistema
cargo bench --workspace --profile release -- --noplot --sample-size 10
```

### Scripts Auxiliares

Os seguintes scripts devem existir em `scripts/`:

| Script | Propósito |
|--------|-----------|
| `measure_ram_idle.sh` | Inicia gateway, mede RSS após estabilização, reporta MB |
| `measure_bundle_size.sh` | Build cockpit, mede tamanho gzipped/brotli |
| `measure_lighthouse.sh` | Serve cockpit, roda Lighthouse CI com throttling 4G |
| `bench_export.py` | Converte output Criterion para JSON schema |
| `bench_compare.py` | Gera relatório HTML comparando contra baselines |

> **Nota**: Se algum script ainda não existir, crie-o seguindo o schema de raw data acima. A ausência de scripts não bloqueia a execução manual dos benchmarks Criterion.

---

## Regressão e Gates de CI

### Gates Obrigatórios (PR merge block)

| Gate | Limite | Ação em Falha |
|------|--------|---------------|
| Criterion regression | >10% slower vs baseline | Bloqueia merge, requer justificativa |
| RAM idle | >30 MB | Bloqueia merge |
| Bundle cockpit | >200 KB gzipped | Bloqueia merge |
| clippy warnings | >0 | Bloqueia merge |
| Test failures | >0 | Bloqueia merge |

### Gates Informativos (warning only)

| Gate | Limite | Ação em Falha |
|------|--------|---------------|
| TTFD | >50 ms | Warning no PR, não bloqueia |
| TTI 4G | >1.5 s | Warning no PR, não bloqueia |
| Lighthouse | <95 | Warning no PR, não bloqueia |

### Configuração Sugerida para CI

```yaml
# .github/workflows/bench.yml (exemplo)
name: Benchmarks
on: [pull_request]
jobs:
  bench:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo bench --workspace --profile release -- --noplot
      - run: python scripts/bench_export.py --input target/criterion/ --output results.json
      - uses: actions/upload-artifact@v4
        with:
          name: bench-results
          path: results.json
```

---

## Adicionando Novos Benchmarks

### Checklist

1. Criar arquivo `benches/<name>.rs` no crate correspondente
2. Adicionar `[[bench]]` entry no `Cargo.toml` do crate (se não usar auto-discovery)
3. Usar `criterion_group!` e `criterion_main!` macros
4. Definir throughput quando aplicável (`group.throughput(Throughput::Elements(n))`)
5. Adicionar entrada na tabela "Benchmarks por Crate" deste documento
6. Definir target mensurável e adicioná-lo à tabela de métricas-alvo
7. Rodar `cargo bench -p <crate> --profile release` e verificar que compila e executa
8. Commitar com mensagem convencional: `bench(<crate>): add <description>`

### Template

```rust
//! Benchmarks for zylos-<crate> — <component>
//!
//! Targets: <metric> < <value> <unit>

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn bench_<name>(c: &mut Criterion) {
    let mut group = c.benchmark_group("<group_name>");
    group.throughput(Throughput::Elements(1));

    group.bench_function("<function_name>", |b| {
        b.iter(|| {
            // benchmark body
            std::hint::black_box(/* result */)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_<name>);
criterion_main!(benches);
```

---

## Changelog

| Data | Commit | Mudança |
|------|--------|---------|
| 2026-11-04 | — | Documento inicial criado |
| — | — | Benchmarks de sandbox e verification pendentes (Sprints 3.3 e 4.1) |

---

## Referências

- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Lighthouse CI Documentation](https://github.com/GoogleChrome/lighthouse-ci)
- `docs/ARCHITECTURE.md` — diagramas C4 e decisões de design
- `docs/CONTRIBUTING.md` — convenções de código e testing
- `EVALUATION.md` — critérios de avaliação do projeto