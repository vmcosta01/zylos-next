# RB-Sentinel-Threshold-Tuning — Runbook de Ajuste de Thresholds e Redução de Falsos Positivos

**Versão:** 1.1.0 (2026-11-17: §7 runtime do gateway)
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.7 T11.7.8
**Playbooks Relacionados:** [`pb-sentinel-rules-authoring.md`](../playbooks/pb-sentinel-rules-authoring.md)
**SOAR Workflow:** [`soar-production-alerts.md`](../docs/soar/soar-production-alerts.md)
**Público:** SRE, Dev Pleno, Tech Lead
**Tempo Estimado:** 30–60 minutos por ciclo de tuning

---

## 1. Visão Geral

Este runbook define o procedimento sistemático para ajustar thresholds de alertas do Sentinel, reduzindo falsos positivos sem comprometer a detecção de incidentes reais. Baseia-se em análise de dados históricos do Axiom, feedback de on-call e métricas de precisão/recall.

**Objetivo:** False positive rate <5% mantendo recall >95% para SEV-1/SEV-2.

---

## 2. Nível Senior (Tech Lead / SRE Senior)

### Critérios de Aceitação para Thresholds

| Métrica | Target | Medição |
|---------|--------|---------|
| False positive rate | <5% | (alertas não-confirmados / total alertas) × 100 |
| Recall SEV-1/2 | >95% | (incidentes detectados / incidentes reais) × 100 |
| Mean time to detect (MTTD) | <2 min SEV-1, <10 min SEV-2 | Tempo entre início do incidente e primeiro alerta |
| Alert fatigue score | <3 alertas/dia/on-call | Contagem média de alertas por engenheiro por dia |
| Cooldown effectiveness | >90% dedup | (alertas suprimidos por cooldown / total triggers) × 100 |

### Decisão Matrix: Quando Ajustar

| Situação | Ação | Risco |
|----------|------|-------|
| FP rate >10% em 7 dias | Aumentar threshold OU aumentar cooldown | Baixo (monitorar recall) |
| FP rate >20% em 24h | Desativar regra temporariamente + investigar | Médio (pode perder incidente real) |
| Recall <90% (incidente perdido) | Diminuir threshold OU adicionar nova regra | Alto (mais FPs esperados) |
| Alert fatigue >5/dia | Revisar severidades + consolidar regras | Baixo |
| Nova métrica disponível | Criar regra com threshold conservador + monitorar 7d | Baixo |

### Trade-offs Documentados

| Decisão | Racional | Alternativa Rejeitada |
|---------|----------|----------------------|
| Cooldown ≥ window_minutes | Previne re-triggers dentro da mesma janela de avaliação | Cooldown < window (duplicatas frequentes) |
| SEV-1 threshold conservador | Melhor falso positivo que falso negativo em crítico | Threshold agressivo (risco de miss) |
| `gated: true` para restart | Previne cascata de restarts automáticos | Auto-restart ungated (risco de loop) |
| Max 2 auto-executions | Limite seguro antes de escalar para humano | Sem limite (restart infinito possível) |

---

## 3. Nível Pleno (SRE / On-call Engineer)

### Ciclo de Tuning (Repetir Mensalmente ou Pós-Incidente)

#### Passo 1: Coletar Dados Históricos (15 min)

```bash
# Query Axiom: alertas disparados nos últimos 7 dias
# Dashboard → Explore → Query:
# _table="sentinel_alerts" | where _time > ago(7d) | summarize count() by rule_name, severity

# Query Axiom: falsos positivos marcados
# _table="sentinel_alerts" | where confirmed == false | summarize count() by rule_name
```

**Output esperado:** Tabela com contagem de alertas por regra e taxa de FP.

#### Passo 2: Identificar Regras Problemáticas (10 min)

Classificar cada regra em uma das categorias:

| Categoria | Critério | Ação Prioritária |
|-----------|----------|-----------------|
| 🔴 Crítica | FP rate >20% OU incidente perdido | Ajuste imediato |
| 🟡 Atenção | FP rate 5-20% | Ajuste no próximo ciclo |
| 🟢 Saudável | FP rate <5% e recall >95% | Manter, revisar trimestralmente |
| ⚪ Inativa | Zero triggers em 30 dias | Avaliar se ainda necessária |

#### Passo 3: Analisar Causa Raiz dos FPs (20 min)

Para cada regra 🔴/🟡, identificar padrão:

| Padrão de FP | Causa Provável | Solução |
|-------------|---------------|---------|
| Triggers em horários fixos | Batch job/cron causando pico normal | Adicionar exceção temporal ou aumentar threshold |
| Triggers após deploy | Deploy causa transiente esperado | Aumentar cooldown para cobrir janela de deploy |
| Triggers em weekends/off-hours | Tráfego baixo amplifica variação percentual | Usar threshold absoluto ao invés de percentual |
| Triggers correlacionados | Múltiplas regras detectando mesmo evento | Consolidar em regra composta |
| Triggers aleatórios | Ruído na métrica | Suavizar métrica (moving average) ou aumentar window |

#### Passo 4: Propor Novo Threshold (10 min)

```yaml
# Antes
name: high-error-rate
threshold: 1.0
cooldown_minutes: 15

# Depois (exemplo: FP causado por batch job às 3am)
name: high-error-rate
threshold: 1.5 # Aumentado baseado em P99 histórico durante batch
cooldown_minutes: 30 # Cobrir janela de batch (20min) + margem
```

**Validação pré-aplicação:**
- Simular novo threshold contra dados históricos (Axiom query com `where value > new_threshold`)
- Verificar que incidentes reais ainda seriam detectados
- Estimar nova taxa de FP

#### Passo 5: Aplicar Mudança (5 min)

1. Editar arquivo YAML de regras (`sentinel-rules.yaml` ou config inline)
2. Validar sintaxe: `cargo test -p zylos-sentinel -- alert_rules`
3. Deploy via CI/CD ou hot-reload (se suportado)
4. Monitorar por 24h

#### Passo 6: Validar Pós-Mudança (24h depois)

```bash
# Comparar métricas antes/depois
# Axiom query: _table="sentinel_alerts" | where rule_name == "high-error-rate"
# | where _time > ago(1d) | summarize count(), avg(confirmed)
```

**Critérios de sucesso:**
- ✅ FP rate reduziu ≥50% vs baseline
- ✅ Zero incidentes perdidos no período
- ✅ Alert fatigue score não aumentou

Se falhar: reverter mudança e retornar ao Passo 3 com novos dados.

---

## 4. Nível Júnior (Dev Júnior / Estagiário)

### Checklist de Verificação Rápida

Antes de propor ajuste de threshold, verificar:

- [ ] Dados históricos cobrem ≥7 dias (inclui weekdays + weekend)
- [ ] Incidentes reais no período foram identificados e marcados
- [ ] Padrão de FP foi classificado (tabela §3 Passo 3)
- [ ] Novo threshold foi simulado contra dados históricos
- [ ] Impacto em recall foi estimado (quantos incidentes reais ainda seriam detectados?)
- [ ] Mudança foi documentada com racional no commit message
- [ ] Review solicitado a SRE/Tech Lead antes de aplicar

### Regras de Quando NÃO Ajustar Sozinho

- ❌ **NÃO** ajuste thresholds de regras SEV-1 sem approval do Tech Lead
- ❌ **NÃO** desative regras sem analisar impacto em recall
- ❌ **NÃO** aplique mudanças baseadas em <24h de dados
- ❌ **NÃO** ignore feedback de on-call ("esse alerta é sempre falso")
- ✅ **SEMPRE** documente racional da mudança no PR/commit
- ✅ **SEMPRE** monitore por 24h após aplicação
- ✅ **SEMPRE** reverta se recall cair abaixo de 95%

### Glossário

| Termo | Definição |
|-------|-----------|
| False Positive (FP) | Alerta disparado sem incidente real correspondente |
| True Positive (TP) | Alerta disparado corretamente durante incidente real |
| Recall | TP / (TP + FN) — fração de incidentes reais detectados |
| Precision | TP / (TP + FP) — fração de alertas que são reais |
| Cooldown | Período mínimo entre alertas duplicados da mesma regra |
| Window | Janela temporal de avaliação da métrica |
| Alert Fatigue | Exaustão cognitiva causada por excesso de alertas irrelevantes |

---

## 5. Procedimento de Emergência: Desativar Regra

**Trigger:** Regra causando >20 FPs/hora OU bloqueando operações normais.

1. Comentar/remover regra do arquivo YAML
2. Reload do sentinel (restart gateway ou hot-reload)
3. Criar ticket Linear com tag `sentinel-tuning` e dados do FP
4. Notificar #dev: "⚠️ Regra `{rule_name}` desativada temporariamente. Ticket: LINEAR-XXX"
5. Reavaliar em ≤48h com dados completos

**Rollback:** Se desativação causar incidente perdido, reativar imediatamente com threshold original e priorizar tuning.

---

## 6. Métricas de Monitoramento Contínuo

Adicionar ao dashboard Sentinel (T11.7.6):

| Métrica | Target | Alerta se |
|---------|--------|-----------|
| FP rate por regra | <5% | >10% por 3 dias consecutivos |
| Total alertas/dia | <10 | >20 por 2 dias consecutivos |
| Tempo médio entre alertas | >30min | <10min (possível storm) |
| Regras sem triggers em 30d | 0 | >0 (regra órfã?) |
| Incidentes perdidos | 0 | Qualquer valor >0 |

---

## 7. Tuning no Runtime do Gateway (T11.7.2 / T11.7.4 / T11.7.6)

> Esta seção complementa os passos 1–6 com os controles **reais** do runtime (`apps/zylos-gateway/src/sentinel_runtime.rs`). As consultas acima usam `_table="sentinel_alerts"` como notação genérica; no dataset real todos os eventos ficam no mesmo dataset (`AXIOM_SENTINEL_DATASET`, ex.: `zylos-sentinel`) separados por `kind` — ver [`pop-sentinel-axiom-integration.md`](../playbooks/pop-sentinel-axiom-integration.md).

### 7.1 Duas camadas de detecção — ajuste cada uma separadamente

| Camada | Onde | Parâmetros | Efeito no FP |
|--------|------|------------|--------------|
| **Regras YAML** (threshold fixo) | `SENTINEL_RULES_PATH` ou `DEFAULT_RULES_YAML` | `threshold`, `operator`, `cooldown_minutes`, `severity`, `remediation.gated`, `max_auto_executions` | Gera alertas e propostas de remediação |
| **Baseline Z-score** | `BaselineDetector::default()` = `new(window=60, min_samples=10, z_threshold=3.0)`, `min_stddev=0.5` | janela (amostras), amostras mínimas, limiar Z | Só marca anomalias (ponto vermelho no cockpit, `kind=sentinel_anomaly`) — **não** dispara remediação |

Com `SENTINEL_TICK_SECS=5`, janela 60 = 5 min de baseline e o detector só alerta após 10 ticks (~50 s) — por isso o critério "CPU >80% em <30 s" é coberto pela **regra** `cpu-spike` (1 tick), não pelo Z-score.

### 7.2 Regras padrão embutidas

| Regra | Métrica | Limite | Severidade | Cooldown | Remediação |
|-------|---------|--------|------------|----------|------------|
| `cpu-spike` | `cpu_percent` | > 80 | SEV-2 | 5 min | `restart-gateway` (gated) |
| `memory-critical` | `memory_percent` | > 90 | SEV-2 | 10 min | `notify-oncall` (máx. 2 auto) |
| `error-rate-high` | `error_rate_percent` | > 1 | SEV-1 | 5 min | `notify-oncall:pagerduty` (máx. 2 auto) |
| `latency-degraded` | `latency_p95_ms` | > 1500 | SEV-3 | 15 min | `scale:gateway:3` (gated) |
| `queue-backlog` | `queue_depth` | > 200 | SEV-3 | 15 min | `scale:gateway:4` (gated) |

Para customizar: copiar o bloco `DEFAULT_RULES_YAML` para um arquivo, editar e apontar `SENTINEL_RULES_PATH`. YAML inválido → o gateway registra `regras do Sentinel inválidas; usando padrão` e segue com as regras embutidas (fail-safe — confira o log após cada mudança).

### 7.3 Consultas APL para medir FP

```
// Alertas por regra (7d)
['zylos-sentinel'] | where kind == "sentinel_alert" and _time > ago(7d)
| summarize alerts = count() by rule_name, severity

// Taxa de FP: remediações rejeitadas pelo on-call ≈ falsos positivos
['zylos-sentinel'] | where kind == "sentinel_remediation" and _time > ago(7d)
| summarize total = count(), rejeitadas = countif(status == "rejected") by rule_name
| extend fp_rate = rejeitadas * 100.0 / total

// Distribuição real da métrica para escolher o threshold (p95/p99)
['zylos-sentinel'] | where kind == "sentinel_metric" and _time > ago(7d)
| summarize percentiles_array(latency_p95_ms, 50, 95, 99), percentiles_array(cpu_percent, 95, 99)
```

Regra prática: threshold ≥ p99 da métrica em período saudável, e cooldown ≥ 3× o tick.

### 7.4 Verificação visual (cockpit)

Aba **Sentinel** → *Anomalias em tempo real*: cada sparkline mostra a série (até 120 pontos), a **linha tracejada vermelha** no limite padrão e **pontos vermelhos** nas anomalias Z-score. Atualização a cada 3 s (`POLL_SECS`). Se uma regra dispara mas a série nunca cruza a linha, o threshold do YAML diverge do exibido — o painel usa os limites padrão de `CHART_METRICS` (`apps/zylos-cockpit/src/components/sentinel_panel.rs`); atualize ambos.

### 7.5 Remediação durante o tuning

Mantenha `SENTINEL_REMEDIATION_MODE=dry_run` (padrão) durante qualquer ciclo de tuning: as propostas ficam registradas como `dry_run` no Axiom e permitem medir quantas remediações *teriam* sido disparadas sem executar nada. Promoção de modo: ver [`soar-sentinel-auto-remediation.md`](../docs/soar/soar-sentinel-auto-remediation.md) §7.

### 7.6 Validação após mudança

```bash
cargo test -p zylos-sentinel                        # parser YAML, regras, gate
cargo test -p zylos-gateway --lib sentinel_runtime  # DEFAULT_RULES_YAML parseia (5 regras)
curl -s http://localhost:3000/api/v1/sentinel/metrics | jq '.rules, .remediation_mode'
```

---

## 8. Referências

- [`crates/zylos-sentinel/src/alert_rules.rs`](../crates/zylos-sentinel/src/alert_rules.rs) — Implementação do engine
- [`playbooks/pb-sentinel-rules-authoring.md`](../playbooks/pb-sentinel-rules-authoring.md) — Formato YAML e boas práticas
- [`docs/soar/soar-production-alerts.md`](../docs/soar/soar-production-alerts.md) — Regras de produção pré-definidas
- [`playbooks/pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md) — Fonte de dados históricos
- [`runbooks/rb-production-incident-response.md`](./rb-production-incident-response.md) — Resposta a incidentes

---

*Documento gerado conforme skill `runbook-builder`. Próxima revisão: após primeiro ciclo de tuning completo em produção.*