# ADR-013: Backend de Storage do Analytics (SQLite vs DuckDB vs TimescaleDB)

- **Status:** Aprovado
- **Data:** 2026-11-17
- **Fase:** 11.10 (Analytics Dashboard)

## Contexto

O Analytics Dashboard precisa armazenar eventos de execução (agente, skill, sucesso, latência, timestamp, custo, dimensões), agregar por hora/dia/semana/mês e carregar o dashboard em < 2 s com 30 dias (1M+ eventos). O workspace já usa `rusqlite 0.32` (bundled) em vários crates (ADR-005) e o Zylos é distribuído como binário local, sem serviços externos obrigatórios.

## Opções

| Opção | Prós | Contras |
|---|---|---|
| **SQLite (rusqlite bundled)** | já é dependência do workspace; zero ops; arquivo único; transações; WAL | row-store: agregações sobre eventos brutos são lentas em 1M+ linhas |
| DuckDB (`duckdb` crate) | colunar, agregações muito rápidas | dependência nativa grande (build C++ longo, binário maior); mais um engine para manter |
| TimescaleDB | hypertables, continuous aggregates, retenção nativa | exige PostgreSQL em execução — incompatível com o modelo local-first/binário único |

## Decisão

**SQLite** com **rollup horário materializado** (`rollup_hourly`, mantido na mesma transação da ingestão), tabela quente `events` + `events_archive` (view `all_events`), pragmas de cache/mmap e `optimize()` (`ANALYZE`) após carga em massa.

## Consequências

- Medido (`docs/fase11-reports/analytics-benchmark.md`): load do dashboard 549.2 ms (95.5–112.9 ms com processo novo) com 1.020.006 eventos; agregações via rollup 92–177 ms; presets < 1 s.
- Filtros que não cabem no rollup (dimensão, latência, sucesso, predicado) varrem eventos brutos: 0.7–3.3 s para 30 dias. Mitigação: estreitar período; evolução futura com rollups por dimensão ou índices de expressão.
- Reavaliar DuckDB se o volume passar de ~10M eventos/mês ou se filtros ad-hoc sobre eventos brutos virarem requisito interativo.

## Alternativas rejeitadas

- DuckDB: custo de build/binário desproporcional ao ganho no volume atual.
- TimescaleDB: exige servidor PostgreSQL.
