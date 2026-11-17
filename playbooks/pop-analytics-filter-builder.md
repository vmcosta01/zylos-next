# POP-ANALYTICS-001: Construir Filtros de Analytics

**Version:** 1.0.0
**Playbook:** [pb-analytics-preset-reports.md](./pb-analytics-preset-reports.md)
**Last Updated:** 2026-11-17
**Fase:** 11.10 (Analytics Dashboard)
**Crate:** `crates/zylos-analytics`

## Purpose

Montar filtros reproduzíveis sobre os eventos de execução (agente, skill, sucesso, latência, timestamp) para dashboards, exports e relatórios. Implementação: `crates/zylos-analytics/src/filter.rs` (`Filter`), exposto via query string em todos os endpoints `/api/v1/analytics/*` (`filter_from_params` em `apps/zylos-gateway/src/analytics_api.rs`).

## Trigger

- Investigar queda de taxa de sucesso de um agente/skill
- Preparar export CSV ou relatório para um recorte específico
- Configurar a visão padrão do painel **Analytics** do cockpit

## Parâmetros

| Parâmetro | Tipo | Semântica |
|---|---|---|
| `since` | epoch s | início **inclusivo** |
| `until` | epoch s | fim **exclusivo** |
| `agents` | lista `a,b` | OR entre agentes |
| `skills` | lista `a,b` | OR entre skills |
| `success` | `true`/`false` | só sucessos / só falhas |
| `min_latency_ms` / `max_latency_ms` | u64 | faixa de latência (inclusiva) |
| `dim.<chave>=<valor>` | string | dimensão custom (ex.: `dim.channel=slack`); chave `[A-Za-z0-9_.-]`, ≤64 chars |

Filtros diferentes são combinados com **AND**. Todos os valores viram parâmetros SQL (`?`), nunca SQL interpolado; a chave de dimensão é validada antes de entrar no `json_extract`.

## Steps

### 1. Descobrir agentes e skills disponíveis

```bash
curl -s "http://localhost:3000/api/v1/analytics/dimensions"
```

### 2. Montar o filtro e validar o resumo

```bash
SINCE=$(( $(date +%s) - 7*86400 ))
curl -s "http://localhost:3000/api/v1/analytics/summary?since=$SINCE&skills=pdf-extract&success=false"
```

A resposta traz `meta.description` (ex.: `desde 2026-11-10 00:00 UTC · skills [pdf-extract] · ...`): confira que é o recorte pretendido.

### 3. Aplicar o filtro às visões

```bash
curl -s "http://localhost:3000/api/v1/analytics/timeseries?since=$SINCE&skills=pdf-extract&granularity=hourly"
curl -s "http://localhost:3000/api/v1/analytics/breakdown?since=$SINCE&by=agent&skills=pdf-extract"
curl -s "http://localhost:3000/api/v1/analytics/heatmap?since=$SINCE&agents=atlas"
```

### 4. Predicados customizados (Rust)

Para regras que a query string não expressa, use `AnalyticsStore::query_events_with(&filter, |e| ..., limit)` ou `aggregate_with_predicate` — o `Filter` reduz o conjunto no SQLite e o predicado roda em memória sobre o resultado.

## Desempenho (medido — ver `docs/fase11-reports/analytics-benchmark.md`)

- Filtros só de **período/agente/skill** usam o rollup horário (`rollup_hourly`): agente+skill+7d em **3.3 ms** com 1.02M eventos.
- Filtros com **`success`, latência ou `dim.*`** obrigam varredura dos eventos brutos: `dim.channel=slack` em 30d levou **3328 ms**; latência>5000 ms **685 ms**. Para ficar <500 ms, estreite o período (`since`) ou combine com agente/skill.

## Verification

- [ ] `meta.description` corresponde ao recorte desejado
- [ ] `summary.count` > 0 (senão o painel mostra estado *Empty*)
- [ ] Filtro inválido (ex.: `dim.a'b=x`, `success=talvez`) retorna HTTP 400 com mensagem
