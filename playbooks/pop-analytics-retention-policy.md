# POP-ANALYTICS-003: Política de Retenção de Analytics

**Version:** 1.0.0
**Playbook:** [pb-analytics-preset-reports.md](./pb-analytics-preset-reports.md)
**Last Updated:** 2026-11-17
**Fase:** 11.10 (Analytics Dashboard)
**Crate:** `crates/zylos-analytics`

## Purpose

Limitar crescimento do banco de analytics (`data/analytics.db`) mantendo histórico útil. Implementação: `crates/zylos-analytics/src/retention.rs` (`RetentionPolicy`, `AnalyticsStore::apply_retention`).

## Política padrão

| Campo | Padrão | Semântica |
|---|---|---|
| `archive_after_days` | 30 | eventos mais antigos saem de `events` (quente) para `events_archive` (frio); `null` desativa |
| `delete_after_days` | **90** | eventos e linhas do rollup horário mais antigos são **apagados** |

Regra: `archive_after_days < delete_after_days` (validado; senão HTTP 400). Cortes alinhados à hora cheia para manter o rollup consistente. Consultas leem a view `all_events` (quente + arquivo), então arquivar não muda resultados — só deixa a tabela quente pequena para ingestão e índices.

Configuração por ambiente: `ZYLOS_ANALYTICS_DB` (caminho do banco) e `ZYLOS_ANALYTICS_RETENTION_DAYS` (sobrescreve `delete_after_days`).

## Trigger

- Rotina diária (cron/scheduler) — recomendado fora do horário comercial
- Banco acima do orçamento de disco
- Mudança de requisito de compliance (reter mais/menos)

## Steps

### 1. Ver política atual e última execução

```bash
curl -s "http://localhost:3000/api/v1/analytics/retention"
# → {"policy":{"archive_after_days":30,"delete_after_days":90},"last_run":{...},"hot_events":N,"archived_events":M}
```

### 2. Aplicar a política vigente

```bash
curl -s -X POST "http://localhost:3000/api/v1/analytics/retention"
```

### 3. Alterar a política (persistida no processo do gateway)

```bash
curl -s -X POST "http://localhost:3000/api/v1/analytics/retention" -H 'content-type: application/json' \
  -d '{"archive_after_days":14,"delete_after_days":60}'
```

O retorno traz `RetentionReport` (`archived`, `deleted`, `rollup_rows_deleted`, cortes). A última execução fica em `meta['retention.last_run']`.

### 4. Backup antes de reduzir retenção

Apagar é irreversível. Antes de baixar `delete_after_days`, faça backup (`playbooks/pop-backup-restore.md`) ou exporte CSV do período (`pop-analytics-export.md`).

## Custo medido

Benchmark (1.02M eventos, arquivar >7d e apagar >29d): 763.398 arquivados, 16.041 apagados, 1.439 linhas de rollup removidas, em **26.9 s** (a outra execução, com a máquina sob carga de builds paralelos, levou 79.7 s). Execute fora de pico: a operação segura o lock do banco.

## Verification

- [ ] `cargo test -p zylos-analytics retention` passa (inclui caso >90d apagado)
- [ ] `GET /analytics/retention` mostra `last_run` atualizado
- [ ] Nenhum evento com `ts < now - delete_after_days` em `/analytics/events?until=<corte>`
