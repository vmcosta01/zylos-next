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

## Correção dos filtros sobre eventos brutos (feat/f11-analytics-fix)

**Data:** 2026-11-17 · mesmo dataset (1.020.006 eventos / 30 d), mesma máquina (compartilhada com builds paralelos — há ruído), `--release`.

### Mudanças (`crates/zylos-analytics/src`)

| Filtro | Causa do custo | Correção |
|---|---|---|
| dimensão (`dim.channel = slack`, 30 d) | `json_extract(dims)` em varredura completa de `all_events` | nova tabela `rollup_dim_hourly` (dkey, dval, hora, agente, skill → n/ok/lat/custo/tokens), mantida em `insert_batch` e pela retenção; `Source::of` escolhe-a quando o filtro tem **exatamente uma** dimensão e é alinhado à hora (`Filter::dim_rollup_compatible`). Demais combinações seguem no caminho bruto (semântica idêntica). |
| latência (`min/max_latency_ms`) | `COUNT(*)` sobre a view (co-rotina, sem índice) | índices `idx_events_latency_ts` / `idx_archive_latency_ts` em `(latency_ms, ts)`; `count()` bruto conta por tabela (`(SELECT COUNT(*) FROM events …) + (SELECT COUNT(*) FROM events_archive …)`) para o planner usar índice cobrindo |
| predicado customizado (Rust) | `ORDER BY` na view + alocação de `String`/`BTreeMap` + `serde_json` por linha | `scan_events` interno: buffer `AnalyticsEvent` reaproveitado, parser rápido para o JSON plano de `dims` (fallback para `serde_json` em escapes/formatos não triviais), sem `ORDER BY` em `aggregate_with_predicate` (o `BTreeMap` de buckets já ordena) |

**Migração idempotente:** `SCHEMA` usa `CREATE … IF NOT EXISTS` (índices novos são criados ao abrir) e `migrate()` faz o backfill de `rollup_dim_hourly` a partir de `all_events` + `json_each` uma única vez, marcando `meta['schema.rollup_dim_hourly.v1']`. Medido num banco de 1,02 M eventos da versão anterior: 1º `open` = **10,9 s** (backfill + índices), aberturas seguintes ≈ 2–10 ms. API pública inalterada (só foi adicionado `Filter::dim_rollup_compatible`). Custo: arquivo 146,9 → 183,3 MiB.

Testes novos: `dim_rollup_matches_raw_scan` (rollup por dimensão = varredura bruta em count/summary/aggregate/breakdown/heatmap), `migration_backfills_dim_rollup_idempotently`, `parse_dims_matches_serde`, `rollup_compat` estendido.

### Comando

```bash
export CARGO_TARGET_DIR=E:/zylos-target/analytics
cargo build -j 4 --release -p zylos-analytics --example analytics_bench
NO_RETENTION=1 KEEP=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 fresh.db   # 2× (banco novo)
REUSE=1 $CARGO_TARGET_DIR/release/examples/analytics_bench 34000 fresh.db                 # 3× (processo novo)
```

### Antes × depois (ms)

| Operação | Antes (doc original) | Antes (reproduzido, fd3d13c) | Depois — execução completa a / b | Depois — `REUSE=1` (3×) |
|---|---:|---:|---:|---:|
| filtro por dimensão channel=slack (30 d) | 3328.3 | 3679.2 (REUSE: 494–734) | **68.0 / 54.5** | 16.2–17.8 |
| filtro latência > 5000 ms (30 d) | 685.2 | 216.4 (REUSE: 55–78) | **40.3 / 20.1** | 6.8–10.4 |
| predicado customizado (7 d) | 1575.6 | 1316.7 (REUSE: 197–200) | **498.8 / 254.5** | 93.9–116.8 |
| load do dashboard | 549.2 | 2290.0 | 480.2 / 331.1 | 79.0–91.4 |
| filtro agente+skill+7d (rollup) | 3.3 | 9.2 | 1.5 / 1.4 | 0.6–0.7 |
| export CSV 7 d (lote) | 7078.8 | 5350.4 | 2234.4 / 2559.5 | 704.9–834.3 |
| ingestão 1 M (50k/tx) | 46864.9 | 22937.9 | 29311.1 / 33621.2 | — |

Contagens idênticas antes/depois (30 buckets; 190.796 eventos; 7 buckets).

### Saída literal — execução completa "a" (pós-correção)

| Operação | Tempo (ms) | Observação |
|---|---:|---|
| gerar eventos sintéticos | 710.2 | 1020006 eventos |
| ingestão (insert_batch, 50k/tx) | 29311.1 |  |
| optimize (wal_checkpoint + ANALYZE) | 2295.9 |  |
| **load do dashboard** | 480.2 | parciais ms: open 7, count 10, summary 25, daily 85, breakdowns 238, heatmap 111, eventos 3 |
| aggregate Hourly (30d, rollup) | 78.6 | 720 buckets |
| aggregate Daily (30d, rollup) | 64.6 | 30 buckets |
| aggregate Weekly (30d, rollup) | 80.5 | 6 buckets |
| aggregate Monthly (30d, rollup) | 149.8 | 2 buckets |
| filtro agente+skill+7d (rollup) | 1.5 | 168 buckets |
| filtro por dimensão channel=slack (eventos brutos) | 68.0 | 30 buckets |
| filtro latência > 5000 ms (eventos brutos) | 40.3 | 190796 eventos |
| predicado customizado (in-memory, 7d) | 498.8 | 7 buckets |
| feedback loop (analyze) | 118.1 | 3 recomendações · <70%: ["pdf-extract"] |
| preset weekly_bot_performance (gerar + PDF) | 71.9 | 3 seções |
| preset monthly_cost_analysis (gerar + PDF) | 376.0 | 4 seções |
| preset skill_evolution_summary (gerar + PDF) | 331.8 | 3 seções |
| export CSV (7d, eventos brutos) | 2234.4 | 240567 linhas · 27896372 bytes |

### Status

| Critério | Meta | Medido | Status |
|---|---|---|---|
| Filtros por dimensão / latência / predicado sobre eventos brutos | < 500 ms | 54–68 / 20–40 / 255–499 ms (banco recém-ingerido); 16–18 / 7–10 / 94–117 ms (processo novo) | ✅ |

**Ressalvas honestas:** o predicado customizado logo após a ingestão (cache frio do SO, máquina com builds paralelos) ficou em 498.8 ms numa das duas execuções — dentro da meta, mas sem folga; ele continua sendo uma varredura dos 240 mil eventos de 7 d em Rust (não há como indexar um closure arbitrário). Filtros com **duas ou mais** dimensões, ou dimensão combinada com `success`/latência, não usam o rollup e continuam no caminho bruto com `json_extract` (~0,5–0,7 s em 30 d no banco quente); um índice de expressão por chave conhecida seria o próximo passo, se esse caso for relevante.
