# Analytics — Benchmark de Performance (T11.10.8)

**Data:** 2026-11-17
**Fase:** 11.10 (Analytics Dashboard)
**Procedimento:** [pot-analytics-performance-benchmark.md](../../playbooks/pot-analytics-performance-benchmark.md)
**Código:** `crates/zylos-analytics/examples/analytics_bench.rs`

## Ambiente

| Item | Valor |
|---|---|
| CPU | 13th Gen Intel Core i5-13420H (12 threads) |
| RAM | 23.7 GB |
| SO | Windows 11 Pro |
| Toolchain | rustc 1.99.1, perfil `release` |
| Storage | SQLite (rusqlite 0.32 bundled), arquivo em disco, WAL |
| Observação | máquina compartilhada com builds `cargo` paralelos de outros agentes durante a medição — números têm ruído |

## Comando

```bash
export CARGO_TARGET_DIR=E:/zylos-target/analytics
cargo build --release -p zylos-analytics --example analytics_bench
NO_RETENTION=1 KEEP=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 bench.db
REUSE=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 bench.db   # 3× (processo novo)
```

Dataset: 30 dias sintéticos (`synthetic::generate`, seed fixa, padrão diurno), 34.000 eventos/dia → **1.020.006 eventos**, 10 skills, 6 agentes, 2 dimensões (`channel`, `model`). "Agora" fixo = 2026-11-17 00:00 UTC.

## Resultado — execução completa (saída literal)

| Operação | Tempo (ms) | Observação |
|---|---:|---|
| gerar eventos sintéticos | 1679.6 | 1020006 eventos |
| ingestão (insert_batch, 50k/tx) | 46864.9 |  |
| optimize (wal_checkpoint + ANALYZE) | 2735.5 |  |
| **load do dashboard** (open + count + summary + daily + 2 breakdowns + heatmap + 100 eventos) | 549.2 | 1020006 linhas · 30 dias · 10 skills · 6 agentes · 168 células · 100 eventos · parciais ms: open 23, count 27, summary 35, daily 143, breakdowns 201, heatmap 100, eventos 4 |
| aggregate Hourly (30d, rollup) | 92.1 | 720 buckets |
| aggregate Daily (30d, rollup) | 102.1 | 30 buckets |
| aggregate Weekly (30d, rollup) | 91.6 | 6 buckets |
| aggregate Monthly (30d, rollup) | 176.7 | 2 buckets |
| filtro agente+skill+7d (rollup) | 3.3 | 168 buckets |
| filtro por dimensão channel=slack (eventos brutos) | 3328.3 | 30 buckets |
| filtro latência > 5000 ms (eventos brutos) | 685.2 | 190796 eventos |
| predicado customizado (in-memory, 7d) | 1575.6 | 7 buckets |
| feedback loop (analyze) | 189.1 | 3 recomendações · <70%: ["pdf-extract"] |
| preset weekly_bot_performance (gerar + PDF) | 163.6 | 3 seções · PDF 6685 bytes |
| preset monthly_cost_analysis (gerar + PDF) | 842.9 | 4 seções · PDF 9677 bytes |
| preset skill_evolution_summary (gerar + PDF) | 701.8 | 3 seções · PDF 8274 bytes |
| export CSV (7d, eventos brutos) | 7078.8 | 240567 linhas · 27896372 bytes |

linhas=1020006 · sucesso médio=87.7% · arquivo=146.9 MiB
RESULTADO: load 549.2 ms → PASS (<2s)

## Load com processo novo sobre o mesmo banco (`REUSE=1`)

| Execução | Load do dashboard (ms) |
|---|---:|
| 1 | 95.5 |
| 2 | 104.0 |
| 3 | 112.9 |

## Critérios de aceite

| Critério (plano §11.10) | Meta | Medido | Status |
|---|---|---|---|
| Load do dashboard com 30 dias (1M+ linhas) | < 2 s | 549.2 ms (frio, após ingestão); 95.5–112.9 ms (processo novo) | ✅ |
| Filtros por período/agente/skill | < 500 ms | 3.3 ms (agente+skill+7d) | ✅ |
| Filtros por dimensão / latência / predicado sobre eventos brutos (30d) | < 500 ms | 3328.3 / 685.2 / 1575.6 ms | ⚠️ acima da meta |
| 3 relatórios preset | < 5 s cada | 163.6 / 842.9 / 701.8 ms | ✅ |
| Feedback loop detecta skill < 70% | ≥ 1 | 1 (`pdf-extract`), 3 recomendações | ✅ |

## Observações honestas

- **Variância:** em execuções anteriores, sob carga de compilação paralela e antes do `optimize()` pós-ingestão, o mesmo load mediu 1195.6 ms e 1983.7 ms (passou raspando), e uma primeira versão sem os pragmas de cache/mmap e sem `ANALYZE` mediu **5845.8 ms (FAIL)**. As correções que levaram ao resultado atual: `cache_size`/`mmap_size`/`temp_store=MEMORY`, `optimize()` (checkpoint do WAL + `ANALYZE`) após carga em massa e `query_events` com `LIMIT` por tabela antes do `UNION ALL`.
- **Rollup x eventos brutos:** agregações por período/agente/skill usam `rollup_hourly` (~7 mil linhas para 30 dias) e ficam em dezenas/centenas de ms. Filtros por `success`, latência, `dim.*` ou predicado customizado precisam varrer eventos brutos; com 30 dias e 1M eventos passam de 500 ms. Com recorte de 7 dias o custo cai proporcionalmente; um índice de expressão por dimensão ou rollup por dimensão é o próximo passo, se necessário.
- **Export CSV 7d** (240.567 linhas, 27.9 MB) levou 7.1 s — export é operação de lote, não interativa.
- **Ingestão** de 1M eventos em lotes de 50k: 46.9 s nesta execução (25.0 s, 29.2 s e 111 s em execuções anteriores) ≈ 21–41 mil eventos/s, incluindo upsert do rollup.
- **Retenção** (execução separada; arquivar >7d e apagar >29d sobre 1.02M eventos): 763.398 arquivados, 16.041 apagados, 1.439 linhas de rollup removidas em 26.9 s (79.7 s em outra execução sob carga).
