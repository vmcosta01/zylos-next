# SOAR — Production Alerts Workflow

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.7
**Runbooks Relacionados:** [`rb-production-incident-response.md`](../runbooks/rb-production-incident-response.md)
**Playbooks Relacionados:** [`pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md), [`pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md)
**Público:** SRE, Tech Lead, On-call Engineer
**Trigger:** Métricas do sentinel ou health check failures

---

## 1. Visão Geral

Este workflow SOAR define as regras de alerta automático para o gateway Zylos em produção. Cobre três categorias de alertas: error rate, latency e health check downtime. Cada alerta tem threshold, severidade, canal de notificação e ação de remediação automática (gated).

**Integrações:**
- **Fonte de métricas:** Axiom (logs estruturados) + `/gateway/health` endpoint
- **Notificação:** Slack webhook (configurável) + Sentry (error tracking)
- **Remediação:** Restart automático do gateway (gated, requer approval para produção)

---

## 2. Regras de Alerta

### Rule 1: Error Rate > 1%

```yaml
name: high-error-rate
description: "Taxa de erros HTTP 5xx excede 1% das requisições totais"
source: axiom
query: |
  status >= 500
  | summarize error_count = count() by bin(_time, 5m)
  | join (requests | summarize total = count() by bin(_time, 5m)) on _time
  | extend error_rate = error_count * 100.0 / total
  | where error_rate > 1.0
threshold: 1.0 (%)
window: 5 minutes
severity: SEV-2
notify:
 - slack: "#alerts-zylos"
 - sentry: capture_message("High error rate detected: {error_rate}%")
remediation:
 action: notify-oncall
 gated: true
 approval_required: true
cooldown: 15 minutes
```

### Rule 2: Latency P95 > 2s

```yaml
name: high-latency-p95
description: "Latência P95 das requisições excede 2 segundos"
source: axiom
query: |
  latency_ms > 0
  | summarize p95 = percentile(latency_ms, 95) by bin(_time, 5m)
  | where p95 > 2000
threshold: 2000 (ms)
window: 5 minutes
severity: SEV-3
notify:
 - slack: "#alerts-zylos"
remediation:
 action: none
 gated: false
cooldown: 30 minutes
```

### Rule 3: Health Check Down

```yaml
name: health-check-down
description: "Endpoint /api/v1/health não responde HTTP 200"
source: external-probe
probe:
 url: "https://prod-url/api/v1/health"
 method: GET
 expected_status: 200
 interval: 30s
 consecutive_failures: 3
severity: SEV-1
notify:
 - slack: "#alerts-critical"
 - pagerduty: tech-lead-oncall
 - sentry: capture_message("Health check DOWN for {consecutive_failures} consecutive probes")
remediation:
 action: restart-gateway
 gated: true
 approval_required: true
 max_auto_restarts: 2
 fallback: escalate-to-human
cooldown: 10 minutes
```

### Rule 4: WASM Bundle Size Regression

```yaml
name: wasm-size-regression
description: "WASM gzipped size excede 5MB no CI"
source: github-actions
trigger: job=wasm-size-check AND conclusion=failure
severity: SEV-3
notify:
 - slack: "#dev"
 - github-comment: "⚠️ WASM size regression detected. PR blocked."
remediation:
 action: block-pr-merge
 gated: false
cooldown: none
```

---

## 3. Matriz de Severidade

| Severidade | Response Time | Canal | Escalação Automática | Exemplo |
|-----------|--------------|-------|---------------------|---------|
| SEV-1 | <5 min | #alerts-critical + PagerDuty | Sim (após 15min sem ack) | Health check down, data loss |
| SEV-2 | <30 min | #alerts-zylos + Sentry | Sim (após 1h sem resolução) | Error rate >1%, partial outage |
| SEV-3 | <2h | #alerts-zylos | Não | Latency alta, size regression |
| SEV-4 | Next business day | #dev | Não | Warnings, deprecated deps |

---

## 4. Configuração de Notificação

### Slack Webhook (Manual)

1. Slack → App Directory → Incoming Webhooks → Add to Slack
2. Selecionar canal: `#alerts-zylos` (ou criar)
3. Copiar Webhook URL
4. Configurar no gateway:
 ```bash
 export SLACK_ALERT_WEBHOOK="https://hooks.slack.com/services/T.../B.../xxx"
 ```

### PagerDuty Integration (Manual)

1. PagerDuty → Services → New Service → `zylos-gateway`
2. Integration → Events API v2 → Copy Integration Key
3. Configurar:
 ```bash
 export PAGERDUTY_INTEGRATION_KEY="xxxxxxxxxxxx"
 ```

---

## 5. Remediação Automática (Gated)

### Restart Automático do Gateway

**Condições para execução automática:**
- ✅ Health check falhou ≥3 vezes consecutivas
- ✅ Menos de 2 restarts automáticos nas últimas 2 horas
- ✅ Approval pré-configurado no SOAR config (`auto_restart_approved: true`)

**Procedimento:**
```bash
# Executado pelo sentinel worker
systemctl restart zylos-gateway
# OU em Kubernetes:
kubectl rollout restart deployment/zylos-gateway
```

**Fallback se auto-restart falhar:**
1. Notificar on-call via PagerDuty
2. Criar incidente no Linear automaticamente
3. Documentar no Axiom como evento `remediation_failed`

---

## 6. Cooldown e Deduplicação

| Alerta | Cooldown | Dedup Key | Racional |
|--------|----------|-----------|----------|
| high-error-rate | 15 min | `error_rate:{window}` | Previne spam durante pico transitório |
| high-latency-p95 | 30 min | `latency_p95:{window}` | Latência pode ser sazonal |
| health-check-down | 10 min | `health:{endpoint}` | Crítico, cooldown curto |
| wasm-size-regression | none | `wasm_size:{pr_number}` | Único por PR |

---

## 7. Referências

- [`apps/zylos-gateway/src/sentinel_api.rs`](../apps/zylos-gateway/src/sentinel_api.rs) — API de métricas do sentinel
- [`playbooks/pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md) — Error tracking
- [`playbooks/pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md) — Logs estruturados
- [`runbooks/pop-health-check-validation.md`](../runbooks/pop-health-check-validation.md) — Validação de health checks
- [`runbooks/rb-production-incident-response.md`](../runbooks/rb-production-incident-response.md) — Resposta a incidentes

---

*Documento gerado conforme skill `soar-orchestration`. Próxima revisão: após primeiro alerta disparado em produção.*