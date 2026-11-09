# Runbook: Zylos Sentinel — AIOps e Detecção de Anomalias

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Alerta de anomalia detectado pelo Sentinel ou degradação de métricas
**Público-alvo:** SRE / On-call Engineer / Agente Automatizado
**Pré-requisitos:** Crate `zylos-sentinel` compilado, Prometheus/Grafana acessível

---

## 1. Visão Geral

O Sentinel é o módulo de AIOps do Zylos responsável por:
- Coleta contínua de métricas de agentes e gateway
- Detecção de anomalias via modelos estatísticos (Z-score, IQR, isolation forest)
- Geração de alertas contextualizados com severity automática
- Correlação de eventos cross-componente
- Integração com SOAR para resposta automatizada

**Playbook associado:** [`playbooks/aiops-anomaly-response.md`](../playbooks/aiops-anomaly-response.md)

---

## 2. Nível Senior

### Arquitetura de Decisão
| Decisão | Alternativa Rejeitada | Racional |
|---------|----------------------|----------|
| Z-score como detector primário | LSTM/Transformer | Latência <10ms vs >500ms; falso positivo aceitável em troca de velocidade |
| Alertas push via webhook | Polling periódico | Reduz MTTR em 40% (benchmark interno) |
| Modelo por componente vs global | Global único | Componentes têm distribuições diferentes; modelo único perde sensibilidade |

### Thresholds de Severidade
| Severity | Z-Score | Ação Automática |
|----------|---------|-----------------|
| SEV-1 (Crítico) | > 4.0 | SOAR auto-contenção + page on-call |
| SEV-2 (Alto) | 3.0–4.0 | Alerta Slack + ticket automático |
| SEV-3 (Médio) | 2.5–3.0 | Log warning + dashboard highlight |
| SEV-4 (Baixo) | 2.0–2.5 | Métrica anotada, sem alerta |

### Trade-offs
- Falso positivo rate alvo: ≤5% (ajustável via `sentinel.false_positive_budget`)
- Recall mínimo: ≥90% para SEV-1/SEV-2
- Janela de detecção: 5min rolling (configurável)

---

## 3. Nível Pleno

### Comandos Operacionais
```bash
# Status do sentinel
zylos sentinel status

# Verificar thresholds ativos
zylos sentinel config show

# Testar detector com dados históricos
zylos sentinel backtest --from 2026-11-01 --to 2026-11-05

# Silenciar alerta por maintenance window
zylos sentinel silence --duration 2h --reason "deploy v2.6"

# Exportar métricas para análise offline
zylos sentinel export --format parquet --output /tmp/sentinel_metrics.parquet
```

### Tabela de Troubleshooting
| Sintoma | Causa Provável | Solução |
|---------|---------------|---------|
| Zero alertas apesar de degradação | Threshold muito alto ou janela muito curta | Ajustar `sentinel.zscore_threshold` ou `window_minutes` |
| Excesso de falsos positivos | Modelo não recalibrado após mudança de carga | Executar `zylos sentinel recalibrate --last 7d` |
| Alerta atrasado (>5min) | Backpressure no pipeline de métricas | Verificar queue depth: `zylos queue status` |
| Métricas ausentes | Scraper falhou ou endpoint mudou | Checar `zylos sentinel scrapers health` |

### Checklist de Validação Pós-Incidente
- [ ] Alerta foi gerado dentro do SLA (<2min da anomalia)
- [ ] Severidade atribuída corretamente
- [ ] SOAR workflow foi acionado (se aplicável)
- [ ] Post-mortem documentou se threshold precisa de ajuste
- [ ] Modelo foi recalibrado se houve mudança de baseline

---

## 4. Nível Júnior

### Passo a Passo: Investigar Alerta do Sentinel

1. **Verificar o alerta**
   ```bash
   zylos sentinel alerts --severity SEV-1 --last 1h
   ```
   Output esperado: JSON com `alert_id`, `component`, `metric`, `zscore`, `timestamp`

2. **Correlacionar com outros componentes**
   ```bash
   zylos sentinel correlate --alert-id <ID> --window 30m
   ```
   Output esperado: Lista de eventos relacionados em outros componentes

3. **Verificar métricas brutas no Grafana**
   - Acesse dashboard "Sentinel Anomaly Detection"
   - Filtre pelo componente e janela temporal do alerta
   - Confirme visualmente a anomalia

4. **Se for falso positivo:**
   ```bash
   zylos sentinel feedback --alert-id <ID> --label false_positive
   ```
   Isso alimenta o pipeline de recalibração automática.

5. **Se for verdadeiro:**
   - Acione o playbook correspondente ao componente afetado
   - Documente no post-mortem

### Regras de Escalação
- SEV-1 sem resolução em 15min → PagerDuty on-call
- SEV-2 sem resolução em 1h → Tech Lead
- SEV-3 recorrente (>3x/dia) → Abrir ticket de tuning

---

## 5. Referências Cruzadas

- **Playbook:** [`playbooks/aiops-anomaly-response.md`](../playbooks/aiops-anomaly-response.md)
- **SOAR Workflow:** [`docs/soar/runbooks/sentinel-auto-triage.runbook.yaml`](../docs/soar/runbooks/sentinel-auto-triage.runbook.yaml)
- **Crate:** `crates/zylos-sentinel/src/lib.rs`
- **Dashboard:** Grafana → "Sentinel Anomaly Detection"
- **Runbook relacionado:** [`monitoring-observability.md`](monitoring-observability.md)
