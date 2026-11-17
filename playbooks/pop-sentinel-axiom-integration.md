# POP — Integração Sentinel → Axiom (Ingestão de Métricas em Tempo Real)

**Versão:** 1.0.0
**Data:** 2026-11-17
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.7 T11.7.4
**Playbooks Relacionados:** [`pb-axiom-logging-setup.md`](./pb-axiom-logging-setup.md), [`pb-sentinel-rules-authoring.md`](./pb-sentinel-rules-authoring.md)
**Runbook Relacionado:** [`rb-sentinel-threshold-tuning.md`](../runbooks/rb-sentinel-threshold-tuning.md)
**SOAR:** [`soar-sentinel-auto-remediation.md`](../docs/soar/soar-sentinel-auto-remediation.md)
**Público:** SRE, DevOps, Dev Pleno
**Tempo Estimado:** 10–15 minutos

---

## 1. Objetivo

Habilitar e validar o envio contínuo das métricas, anomalias, alertas e remediações do Sentinel para um dataset Axiom, permitindo consultas históricas (tuning de thresholds, cálculo de taxa de falso positivo) e dashboards externos.

## 2. Arquitetura

```
requisição HTTP ─▶ middleware track_requests ─▶ MetricsCollector (latência, 5xx, fila)
                                                    │
                     tick (SENTINEL_TICK_SECS, 5 s) ▼
ProcfsProbe (CPU/mem) ─▶ MetricSnapshot ─▶ BaselineDetector (Z-score) ─▶ anomalias
                                       └─▶ AlertRulesEngine (YAML)    ─▶ alertas ─▶ RemediationGate
                                                    │
                          axiom_events() ───────────┘
                                │
                     AxiomMetricsSink (mpsc 256, fire-and-forget)
                                │
                 POST {AXIOM_ENDPOINT}/{dataset}/ingest  (Bearer AXIOM_TOKEN)
```

Código: `apps/zylos-gateway/src/sentinel_runtime.rs` (`AxiomMetricsSink`, `axiom_events`, `SentinelRuntime::tick`), reaproveitando `AxiomConfig::from_env()` de `apps/zylos-gateway/src/axiom_logging.rs`.

Garantias:
- O envio **nunca bloqueia** o tick nem requisições: canal com capacidade 256; se cheio, o lote é descartado com `warn!("buffer Axiom do Sentinel cheio; lote descartado")`.
- Sem `AXIOM_TOKEN` o sink é `NoopSink` — o Sentinel continua funcionando localmente (dashboard do cockpit e `/api/v1/sentinel/metrics`).

## 3. Variáveis de Ambiente

| Variável | Obrigatória | Padrão | Descrição |
|----------|-------------|--------|-----------|
| `AXIOM_TOKEN` | sim (para habilitar) | — | Token de ingestão (escopo *ingest* apenas) |
| `AXIOM_SENTINEL_DATASET` | não | `AXIOM_DATASET` | Dataset dedicado às métricas do Sentinel (recomendado: `zylos-sentinel`) |
| `AXIOM_DATASET` | não | ver `axiom_logging.rs` | Dataset dos logs do gateway (fallback) |
| `AXIOM_ENDPOINT` | não | ver `axiom_logging.rs` | Base da ingest API |
| `SENTINEL_TICK_SECS` | não | `5` (mín. 1) | Intervalo de coleta/envio |
| `SENTINEL_RULES_PATH` | não | regras embutidas (`DEFAULT_RULES_YAML`) | YAML de regras |
| `SENTINEL_REMEDIATION_MODE` | não | `dry_run` | `dry_run` \| `require_approval` \| `auto` |

## 4. Esquema dos Eventos

Todos os eventos têm `_time` (epoch s), `service = "zylos-gateway"` e `kind`:

| `kind` | Campos |
|--------|--------|
| `sentinel_metric` | `cpu_percent`, `memory_percent`, `latency_p95_ms`, `error_rate_percent`, `queue_depth`, `request_count` |
| `sentinel_anomaly` | `metric`, `value`, `z_score`, `baseline_mean`, `baseline_stddev` |
| `sentinel_alert` | `rule_name`, `severity`, `value`, `threshold`, `confirmed` (null — preenchido na triagem de FP) |
| `sentinel_remediation` | `remediation_id`, `rule_name`, `action`, `status` |

## 5. Procedimento

### Passo 1: Criar dataset e token (Axiom UI)
1. Settings → Datasets → **New dataset** `zylos-sentinel`.
2. Settings → API tokens → **New token** com permissão *Ingest* restrita ao dataset.
3. Guardar o token no cofre de segredos (nunca em `.env` versionado).

### Passo 2: Configurar o gateway
```bash
export AXIOM_TOKEN="<do cofre>"
export AXIOM_SENTINEL_DATASET="zylos-sentinel"
export SENTINEL_TICK_SECS=5
```
Reiniciar o gateway conforme [`pop-gateway-restart.md`](./pop-gateway-restart.md).

### Passo 3: Verificar inicialização
Nos logs do gateway devem aparecer:
- `Sentinel → Axiom ingest habilitado dataset=zylos-sentinel`
- `Sentinel runtime inicializado mode=DryRun rules=5`

### Passo 4: Verificar dados locais
```bash
curl -s http://localhost:3000/api/v1/sentinel/metrics | jq '.current, .rules, .remediation_mode'
```
`current` deve ser não-nulo após o primeiro tick (≤ `SENTINEL_TICK_SECS`).

### Passo 5: Verificar no Axiom (APL)
```
['zylos-sentinel']
| where kind == "sentinel_metric"
| where _time > ago(5m)
| summarize count(), avg(cpu_percent), max(latency_p95_ms) by bin(_time, 1m)
```
Esperado: ~12 eventos/min com tick de 5 s.

### Passo 6: Consultas de operação
```
// Anomalias por métrica (24h)
['zylos-sentinel'] | where kind == "sentinel_anomaly" | where _time > ago(1d)
| summarize count() by metric

// Alertas por regra/severidade (7d) — base do tuning
['zylos-sentinel'] | where kind == "sentinel_alert" | where _time > ago(7d)
| summarize count() by rule_name, severity

// Remediações e seus estados
['zylos-sentinel'] | where kind == "sentinel_remediation"
| summarize count() by rule_name, action, status
```

## 6. Troubleshooting

| Sintoma | Causa provável | Ação |
|---------|----------------|------|
| Sem log "ingest habilitado" | `AXIOM_TOKEN` ausente/vazio | Conferir env do processo |
| `Axiom sentinel ingest falhou status=401/403` | Token inválido ou sem escopo no dataset | Regerar token com *Ingest* no dataset correto |
| `status=404` | Dataset inexistente / nome divergente | Criar dataset ou corrigir `AXIOM_SENTINEL_DATASET` |
| `buffer Axiom do Sentinel cheio` | Rede lenta / Axiom indisponível | Verificar conectividade; aumentar `SENTINEL_TICK_SECS` temporariamente |
| `current` nulo em `/sentinel/metrics` | Loop ainda não rodou | Aguardar 1 tick; checar se gateway subiu sem panic |
| CPU/mem sempre 0 | Host sem `/proc` (Windows/macOS) — `ProcfsProbe` retorna `None` | Esperado fora de Linux; métricas HTTP continuam válidas |

## 7. Rollback

Remover `AXIOM_TOKEN` (ou `AXIOM_SENTINEL_DATASET`) e reiniciar: o Sentinel volta ao `NoopSink`, sem impacto em detecção, alertas ou dashboard local.

## 8. Verificação Automatizada

```bash
cargo test -p zylos-gateway --lib sentinel_runtime   # inclui tick_sends_axiom_events_and_soar
```
