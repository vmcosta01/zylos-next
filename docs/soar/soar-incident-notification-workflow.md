# SOAR — Incident → Notify → Ticket → Doc Workflow

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.8 T11.8.8
**Runbooks Relacionados:** [`rb-production-incident-response.md`](../../runbooks/rb-production-incident-response.md)
**Playbooks Relacionados:** [`pb-sentry-integration.md`](../../playbooks/pb-sentry-integration.md), [`pb-axiom-logging-setup.md`](../../playbooks/pb-axiom-logging-setup.md)
**Público:** SRE, Tech Lead, On-call Engineer
**Trigger:** Alerta SEV-1 ou SEV-2 disparado pelo Sentinel AlertRulesEngine

---

## 1. Visão Geral

Este workflow SOAR orquestra a resposta automatizada a incidentes críticos, conectando quatro sistemas em sequência:

```
Sentinel Alert → Slack Notification → Linear Ticket → Notion Post-Mortem Doc
```

Cada etapa é gated e idempotente. O workflow é acionado pelo `NotificationRouter` do gateway quando um `TriggeredAlert` do `AlertRulesEngine` atinge severidade SEV-1 ou SEV-2.

**Integrações utilizadas:**
- **Slack adapter** (`zylos-integrations::slack`) — notificação imediata
- **Linear adapter** (`zylos-integrations::linear`) — criação de ticket de incidente
- **Notion adapter** (`zylos-integrations::notion`) — documento de post-mortem inicial
- **Sentry** — captura de erro correlacionado
- **Axiom** — logs estruturados para timeline do incidente

---

## 2. Fluxo do Workflow

### Step 1: Alert Detection (Sentinel)

O `AlertRulesEngine` avalia métricas contra regras YAML. Quando uma regra dispara:

```rust
let alerts = engine.evaluate(&metrics);
for alert in &alerts {
    if matches!(alert.severity, AlertSeverity::Sev1 | AlertSeverity::Sev2) {
        router.dispatch(alert).await;
    }
}
```

**Gate:** Apenas SEV-1 e SEV-2 acionam este workflow. SEV-3/4 seguem rota simplificada (apenas Slack).

### Step 2: Slack Notification (Immediate)

O `NotificationRouter` formata a mensagem e envia via Slack adapter:

```yaml
# Routing rule example
- name: sev1-to-critical-channel
  alert_rule: "*"
  min_severity: SEV-1
  target_platform: slack
  target_recipient: "#alerts-critical"
  message_template: |
    🚨 *[{severity}] {rule_name}*
    {description}
    Value: {value} (threshold: {threshold})
    _Incident ticket being created automatically..._
```

**Idempotency:** Cooldown no `AlertRulesEngine` previne notificações duplicadas.

### Step 3: Linear Ticket Creation (Automated)

Após notificação Slack bem-sucedida, criar ticket no Linear:

```graphql
mutation IssueCreate($input: IssueCreateInput!) {
  issueCreate(input: $input) {
    success
    issue {
      id
      identifier
      url
    }
  }
}
```

**Issue fields:**
- **Title:** `[SEV-X] {rule_name}: {description}`
- **Description:** Alert details + link to Axiom query + Sentry event URL
- **Priority:** Urgent (SEV-1) / High (SEV-2)
- **Labels:** `incident`, `auto-created`, `{platform}`
- **Team:** Infrastructure / Platform (configurável)

**Gate:** Se Linear API falhar, log error no Axiom e não bloquear workflow. Ticket creation é best-effort.

### Step 4: Notion Post-Mortem Scaffold (Automated)

Criar página inicial de post-mortem no Notion com template pré-preenchido:

```json
{
  "parent": { "database_id": "{postmortem_db_id}" },
  "properties": {
    "Title": { "title": [{ "text": { "content": "[SEV-X] {rule_name} — {date}" } }] },
    "Status": { "select": { "name": "In Progress" } },
    "Severity": { "select": { "name": "SEV-1" } },
    "Date": { "date": { "start": "{iso_date}" } },
    "Linear Ticket": { "url": "{linear_issue_url}" }
  },
  "children": [
    { "type": "heading_2", "heading_2": { "rich_text": [{ "text": { "content": "Resumo" } }] } },
    { "type": "paragraph", "paragraph": { "rich_text": [{ "text": { "content": "_Preencher após resolução._" } }] } },
    { "type": "heading_2", "heading_2": { "rich_text": [{ "text": { "content": "Timeline" } }] } },
    { "type": "paragraph", "paragraph": { "rich_text": [{ "text": { "content": "| Hora | Evento |\n|------|--------|\n| {trigger_time} | Alerta disparado: {rule_name} |" } }] } },
    { "type": "heading_2", "heading_2": { "rich_text": [{ "text": { "content": "Causa Raiz" } }] } },
    { "type": "paragraph", "paragraph": { "rich_text": [{ "text": { "content": "_A determinar._" } }] } },
    { "type": "heading_2", "heading_2": { "rich_text": [{ "text": { "content": "Ações Preventivas" } }] } },
    { "type": "to_do_list", "to_do_list": { "rich_text": [{ "text": { "content": "_Adicionar ações pós-resolução._" } }] } }
  ]
}
```

**Gate:** Notion page creation é best-effort. Se falhar, log no Axiom e adicionar link do Linear ticket na thread do Slack como fallback.

### Step 5: Correlação Sentry + Axiom (Automated)

Adicionar ao ticket Linear e página Notion:
- Link para evento Sentry mais recente com stack trace correspondente
- Query URL do Axiom filtrando logs no window do incidente
- Métricas relevantes (error rate, latency p95) no momento do alerta

---

## 3. Configuração

### Env Vars Necessárias

```bash
# Slack
SLACK_BOT_TOKEN=xoxb-...
SLACK_ALERTS_CRITICAL_CHANNEL=C0123456789

# Linear
LINEAR_API_KEY=lin_api_...
LINEAR_INCIDENT_TEAM_ID=team-uuid

# Notion
NOTION_INTEGRATION_TOKEN=ntn_...
NOTION_POSTMORTEM_DB_ID=db-uuid

# Sentry (optional correlation)
SENTRY_DSN=https://...
SENTRY_ORG=zylos
SENTRY_PROJECT=zylos-gateway
```

### Notification Router Rules (YAML)

```yaml
rules:
  - name: sev1-full-workflow
    alert_rule: "*"
    min_severity: SEV-1
    target_platform: slack
    target_recipient: "#alerts-critical"
    message_template: "🚨 *[{severity}] {rule_name}*: {description}\nValue: {value} (threshold: {threshold})\n_Auto-creating incident ticket..._"

  - name: sev2-full-workflow
    alert_rule: "*"
    min_severity: SEV-2
    target_platform: slack
    target_recipient: "#alerts-zylos"
    message_template: "⚠️ *[{severity}] {rule_name}*: {description}\nValue: {value} (threshold: {threshold})\n_Auto-creating incident ticket..._"
```

---

## 4. Error Handling e Fallbacks

| Step | Failure Mode | Fallback | Impact |
|------|-------------|----------|--------|
| Slack notify | API error / timeout | Log to Axiom + retry 1x after 30s | Delayed notification |
| Linear ticket | Auth expired / rate limit | Add note to Slack thread with alert details | Manual ticket creation needed |
| Notion page | Integration not connected | Skip silently, link Linear ticket in Slack | Manual doc creation needed |
| Sentry correlation | No matching event | Skip correlation section | Less context in ticket |
| Axiom query | Dataset unavailable | Use static time range link | May need manual query adjustment |

**Princípio:** Cada step é independente. Falha em um step NÃO bloqueia os subsequentes. O workflow é best-effort end-to-end.

---

## 5. Validação

### Teste Local (sem tokens reais)

```bash
# Validar que o NotificationRouter carrega rules corretamente
cargo test -p zylos-gateway --lib notification_router

# Validar que adapters compilam e health_check retorna false sem tokens
cargo test -p zylos-integrations --lib slack
cargo test -p zylos-integrations --lib notion
cargo test -p zylos-integrations --lib linear

# Validar que AlertRulesEngine dispara alerts corretamente
cargo test -p zylos-sentinel --lib alert_rules
```

### Teste End-to-End (requer tokens reais — T11.8.7)

1. Configurar env vars com tokens de sandbox/staging
2. Iniciar gateway: `SENTRY_DSN=... SLACK_BOT_TOKEN=... cargo run -p zylos-gateway`
3. Simular métrica que dispara alerta SEV-2
4. Verificar: Slack message received → Linear ticket created → Notion page scaffolded

---

## 6. Referências

- [`crates/zylos-sentinel/src/alert_rules.rs`](../../crates/zylos-sentinel/src/alert_rules.rs) — AlertRulesEngine
- [`apps/zylos-gateway/src/notification_router.rs`](../../apps/zylos-gateway/src/notification_router.rs) — NotificationRouter
- [`crates/zylos-integrations/src/slack.rs`](../../crates/zylos-integrations/src/slack.rs) — Slack adapter
- [`crates/zylos-integrations/src/notion.rs`](../../crates/zylos-integrations/src/notion.rs) — Notion adapter
- [`crates/zylos-integrations/src/linear.rs`](../../crates/zylos-integrations/src/linear.rs) — Linear adapter
- [`docs/soar/soar-production-alerts.md`](./soar-production-alerts.md) — Regras de alerta
- [`runbooks/rb-production-incident-response.md`](../../runbooks/rb-production-incident-response.md) — Resposta manual a incidentes
- [`playbooks/pb-sentinel-rules-authoring.md`](../../playbooks/pb-sentinel-rules-authoring.md) — Criação de regras YAML

---

*Documento gerado conforme skill `soar-orchestration`. Próxima revisão: após primeiro incidente real processado pelo workflow.*