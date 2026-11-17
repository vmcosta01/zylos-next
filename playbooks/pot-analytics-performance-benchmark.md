# POT: Benchmark de Performance do Analytics

**Versão:** 1.0.0
**Data:** 2026-11-17
**Fase:** 11.10 (T11.10.8)
**Crate:** `crates/zylos-analytics`
**Resultados:** [docs/fase11-reports/analytics-benchmark.md](../docs/fase11-reports/analytics-benchmark.md)

## Propósito

Medir de forma reproduzível o desempenho do Analytics Dashboard com 30 dias de dados sintéticos (≥ 1M eventos) e verificar as metas do plano: load < 2 s, filtros < 500 ms, presets < 5 s.

## Quando executar

- Antes de merge que altere `store.rs`, `aggregations.rs`, `filter.rs` ou o schema
- Ao mudar o backend de storage (ver ADR-013)
- Mensalmente, para detectar regressão

## Procedimento

### 1. Build release (nunca medir em debug)

```bash
export CARGO_TARGET_DIR=E:/zylos-target/analytics
cargo build --release -p zylos-analytics --example analytics_bench
```

### 2. Execução completa (gera, ingere, otimiza, mede)

```bash
NO_RETENTION=1 KEEP=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 E:/zylos-target/analytics/benchdb/bench.db > run_full.md
```

Argumentos: `[eventos_por_dia=34000] [caminho_do_banco]`. 34.000/dia × 30 dias ≈ 1.02M eventos.

Variáveis:
- `KEEP=1` — mantém o banco/diretório temporário.
- `REUSE=1` — reaproveita o banco existente (pula geração, ingestão e retenção).
- `NO_RETENTION=1` — pula a etapa de retenção, que altera o banco (para depois medir com `REUSE=1`).

### 3. Load com processo novo (3 repetições)

```bash
for i in 1 2 3; do REUSE=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 E:/zylos-target/analytics/benchdb/bench.db | grep 'load do'; done
```

### 4. Retenção (opcional, destrutivo)

Rode sem `NO_RETENTION` para medir `apply_retention` (arquivar > 7d, apagar > 29d).

### 5. Registrar

Cole a saída literal em `docs/fase11-reports/analytics-benchmark.md` com CPU, RAM, SO, toolchain e a carga da máquina no momento. **Nunca** arredonde ou estime números que não foram medidos.

## O que é medido

| Linha | Inclui |
|---|---|
| load do dashboard | abrir conexão nova + count + summary + série diária + breakdown por agente e skill + heatmap 7×24 + 100 eventos mais recentes |
| aggregate Hourly/Daily/Weekly/Monthly | série de 30d via rollup |
| filtros | rollup (agente+skill+7d), dimensão, latência, predicado customizado |
| feedback loop | `feedback::analyze` com config padrão |
| presets | gerar + renderizar PDF de cada preset |
| export CSV | 7 dias de eventos brutos |

## Critério de PASS/FAIL

O binário imprime `RESULTADO: load X ms → PASS (<2s)` ou `FAIL`. Filtros e presets são comparados manualmente às metas (500 ms / 5 s).

## Dicas de interpretação

- Ruído: builds paralelos e antivírus distorcem números — repita e reporte a faixa.
- Primeira leitura após ingestão é "fria" no SQLite, mas o arquivo pode estar no cache do SO; `REUSE=1` em processo novo mede a conexão nova com cache quente do SO.
- Rode `AnalyticsStore::optimize()` após carga em massa: sem `ANALYZE` o planner escolheu planos ruins (load medido 5.8 s antes da otimização).
