# PB: Analytics Dashboard e Relatórios Preset

**Version:** 1.0.0
**Last Updated:** 2026-11-17
**Fase:** 11.10 (T11.10.1–T11.10.8)
**Owner:** Plataforma / Observabilidade

## Objetivo

Operar o Analytics Dashboard do Zylos: ingestão de eventos de execução de agentes, visualização no cockpit, relatórios preset, export e o feedback loop de auto-melhoria.

## Componentes

| Componente | Local |
|---|---|
| Modelo de evento, storage SQLite, agregações | `crates/zylos-analytics/src/{event,store,aggregations}.rs` |
| Filtros | `crates/zylos-analytics/src/filter.rs` → [POP filtros](./pop-analytics-filter-builder.md) |
| Export CSV/PDF | `crates/zylos-analytics/src/{export,pdf}.rs` → [POP export](./pop-analytics-export.md) |
| Feedback loop | `crates/zylos-analytics/src/feedback.rs` → [SOAR](../docs/soar/soar-analytics-auto-improvement.md) |
| Retenção | `crates/zylos-analytics/src/retention.rs` → [POP retenção](./pop-analytics-retention-policy.md) |
| Presets | `crates/zylos-analytics/src/reports.rs` |
| API | `apps/zylos-gateway/src/analytics_api.rs` (`/api/v1/analytics/*`) |
| UI | `apps/zylos-cockpit/src/components/analytics_panel.rs` + `analytics_charts.rs` (aba **Analytics**) |
| Benchmark | `crates/zylos-analytics/examples/analytics_bench.rs` → [POT](./pot-analytics-performance-benchmark.md) |
| Decisão de storage | [ADR-013](../docs/ADR-013-analytics-storage.md) |

## 1. Ingestão

Eventos (`agent`, `skill`, `success`, `latency_ms`, `ts`, opcionais `cost_usd`, `tokens`, `dimensions`):

```bash
curl -s -X POST http://localhost:3000/api/v1/analytics/events -H 'content-type: application/json' -d '[
  {"ts":1794873600,"agent":"atlas","skill":"web-search","success":true,"latency_ms":820,
   "cost_usd":0.002,"tokens":410,"dimensions":{"channel":"slack"}}
]'
# → 201 {"inserted":1}
```

Cada lote é uma transação que grava o evento e atualiza o rollup horário (`rollup_hourly`).

## 2. Relatórios preset

| Id | Título | Período | Conteúdo |
|---|---|---|---|
| `weekly_bot_performance` | Weekly Bot Performance | 7 dias | por agente (execuções, sucesso, latência, custo), barras de sucesso, volume diário |
| `monthly_cost_analysis` | Monthly Cost Analysis | 30 dias | custo por skill (top 10 + tabela), por agente, semanal, USD/execução |
| `skill_evolution_summary` | Skill Evolution Summary | 4 semanas | sucesso semanal por skill, tendência (pp), recomendações do feedback loop |

```bash
curl -s http://localhost:3000/api/v1/analytics/reports                                   # lista
curl -s "http://localhost:3000/api/v1/analytics/reports/weekly_bot_performance"          # JSON
curl -s "http://localhost:3000/api/v1/analytics/reports/monthly_cost_analysis?format=csv" -o custo.csv
curl -s "http://localhost:3000/api/v1/analytics/reports/skill_evolution_summary?format=pdf&skills=pdf-extract" -o evo.pdf
```

Filtros de agente/skill/dimensão restringem o relatório; o período é definido pelo preset (`now=<epoch>` permite gerar retroativamente). Tempos medidos com 1.02M eventos (gerar + PDF): weekly **163.6 ms**, monthly **842.9 ms**, skill evolution **701.8 ms** — todos abaixo da meta de 5 s.

## 3. Cadência sugerida

| Quando | Ação |
|---|---|
| Diário (madrugada) | `POST /analytics/retention` |
| Segunda-feira | PDF `weekly_bot_performance` para o time |
| Dia 1 do mês | PDF `monthly_cost_analysis` para finanças |
| Semanal | Revisar `/analytics/recommendations` e abrir evoluções de skill |

## 4. Troubleshooting

| Sintoma | Causa provável | Ação |
|---|---|---|
| Painel em "Erro ao carregar" | gateway fora / CORS | `pop-gateway-restart.md` |
| Painel "Nenhum evento" | período sem dados ou filtro restrito demais | ampliar período |
| Filtro lento (> 500 ms) | filtro por `success`/latência/`dim.*` varre eventos brutos | estreitar `since` |
| Banco grande | retenção não rodando | ver `pop-analytics-retention-policy.md` |
