# Runbook: Notificações Slack (rb-notify-slack)

**Versão:** 1.0.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `runbook-builder`
**Playbook Pai:** [`pb-legacy-sync`](../playbooks/pb-legacy-sync.md)
**SOAR Workflow:** [`soar-guardrail-enforcement`](../playbooks/soar-guardrail-enforcement.md)

---

## 1. Visão Geral

### Propósito
Enviar notificações estruturadas via Slack para canais específicos durante todas as fases de operações SOAR, incluindo início de sync, alertas de severidade, aprovações HITL pendentes e resumos pós-execução.

### Módulos Envolvidos
- `zylos-live-comm` — HermesBridge para publicação de eventos
- `zylos-soar` — Orquestrador SOAR que dispara notificações como side-effect
- `zylos-sentinel` — AIOps que gera alertas convertidos em notificações Slack

### Integração
Este runbook é acionado por:
- **Playbook:** [`pb-legacy-sync`](../playbooks/pb-legacy-sync.md) (fases pré-sync, pós-sync, SEV-2+)
- **SOAR Workflow:** [`soar-guardrail-enforcement`](../playbooks/soar-guardrail-enforcement.md) (Step 2: Contain → notify)
- **Sentinel:** Alertas automáticos de drift, rate limit e guardrail violations

---

## 2. Nível Senior (Tech Lead / Arquiteto / SRE Senior)

### Decisões de Arquitetura

| Decisão | Alternativa Rejeitada | Racional |
|---------|----------------------|----------|
| Slack Webhook via HermesBridge | Integração direta com Slack API | Centralização: todos os eventos passam pelo EventBus, auditáveis no EvidenceLedger |
| Templates JSON estruturados | Texto livre em cada chamada | Consistência visual + parseável por bots downstream |
| Rate limit 30 msg/min/canal | Sem limite | Prevenção de flood em incidentes cascata |
| Fallback para email se Slack falhar | Retry infinito | Resiliência: notificação crítica não pode ser perdida |

### Modelo de Segurança
- Webhook URLs armazenados como secrets no `zylos-vault`, nunca em código ou env vars plaintext
- Tokens rotacionados a cada 90 dias via POP de rotação de credenciais
- Logs de envio sanitizados: payload sensível redigido antes de persistir no EvidenceLedger

### Trade-offs

| Aspecto | Escolha | Impacto |
|---------|---------|---------|
| Latência | Async via EventBus (+200ms) | Aceitável: notificação não é critical path |
| Confiabilidade | At-least-once delivery | Duplicatas possíveis; templates são idempotentes por design |
| Formatação | Block Kit JSON | Mais complexo que texto simples, mas permite interatividade (botões de aprovação) |

---

## 3. Nível Pleno (Dev Pleno / On-call Engineer)

### Configuração

| Variável | Descrição | Default | Obrigatório |
|----------|-----------|---------|-------------|
| `ZYLOS_SLACK_WEBHOOK_URL` | Webhook URL do canal principal | — | Sim (via Vault) |
| `ZYLOS_SLACK_INCIDENT_WEBHOOK` | Webhook do canal #incident | — | Sim (via Vault) |
| `ZYLOS_SLACK_RATE_LIMIT` | Max mensagens/minuto/canal | 30 | Não |
| `ZYLOS_SLACK_FALLBACK_EMAIL` | Email fallback se Slack falhar | oncall@zylos.dev | Não |

### Comandos Operacionais

#### Enviar notificação manual via API
```bash
curl -X POST http://localhost:3000/api/v1/hermes/live-comm/slack-notify \
  -H "Authorization: Bearer $ZYLOS_HERMES_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "channel": "#engineering",
    "template": "sync_complete",
    "context": {
      "batch_id": "batch-20261111-001",
      "records_synced": 48523,
      "duration_secs": 5420,
      "drift_pct": 0.02
    }
  }'
# Esperado: {"status":"sent","message_id":"msg_abc123"}
```

#### Testar conectividade Slack
```bash
curl -X POST http://localhost:3000/api/v1/hermes/live-comm/slack-ping \
  -H "Authorization: Bearer $ZYLOS_HERMES_TOKEN"
# Esperado: {"status":"ok","latency_ms":142}
```

#### Verificar fila de notificações pendentes
```bash
curl http://localhost:3000/api/v1/hermes/live-comm/slack-queue \
  -H "Authorization: Bearer $ZYLOS_HERMES_TOKEN"
# Esperado: {"pending":0,"failed":0,"rate_limited":0}
```

### Troubleshooting

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| HTTP 403 no webhook | Token expirado ou webhook revogado | Rotacionar webhook via Vault, atualizar secret |
| HTTP 429 | Rate limit excedido | Aguardar 60s, verificar se há loop de retry |
| Timeout > 10s | Slack API instável | Fallback para email automático, verificar status.slack.com |
| Mensagem duplicada | Retry após timeout sem confirmação | Idempotente por template+batch_id; seguro ignorar |
| Notificação não chega | Canal arquivado ou bot removido | Verificar channel ID, re-adicionar bot ao workspace |

### Checklist Operacional

- [ ] Webhook URLs configurados no Vault
- [ ] Bot `zylos-notifier` adicionado aos canais #engineering e #incident
- [ ] Rate limit configurado (default: 30/min)
- [ ] Fallback email configurado para SEV-1/SEV-2
- [ ] Template `sync_complete` testado em staging
- [ ] Template `sev_alert` testado com payload real

---

## 4. Nível Júnior (Dev Júnior / Estagiário / Agente Automatizado)

### Passo a Passo: Enviar Notificação de Sync Completo

1. **Verificar se o gateway está ativo:**
   ```bash
   curl -s http://localhost:3000/gateway/health | jq .status
   # Esperado: "healthy"
   # Se != "healthy" → ESCALAR para SRE On-call, NÃO prosseguir
   ```

2. **Montar payload da notificação:**
   ```json
   {
     "channel": "#engineering",
     "template": "sync_complete",
     "context": {
       "batch_id": "<ID_DO_BATCH>",
       "records_synced": <NUMERO>,
       "duration_secs": <SEGUNDOS>,
       "drift_pct": <PERCENTUAL>
     }
   }
   ```

3. **Enviar via API:**
   ```bash
   curl -X POST http://localhost:3000/api/v1/hermes/live-comm/slack-notify \
     -H "Authorization: Bearer $ZYLOS_HERMES_TOKEN" \
     -H "Content-Type: application/json" \
     -d @payload.json
   ```

4. **Verificar resposta:**
   - `{"status":"sent",...}` → Sucesso ✅
   - `{"status":"queued",...}` → Fila cheia, será enviado em breve ⏳
   - `{"status":"failed","error":"..."}` → Falha ❌ → Verificar troubleshooting acima

### Quando NÃO Enviar e Escalar

- ❌ Gateway retorna erro 5xx → Escalar SRE On-call
- ❌ Webhook URL não configurado → Criar ticket de configuração, NÃO hardcode URL
- ❌ Payload contém dados sensíveis (PII, tokens, passwords) → Sanitizar ANTES de enviar
- ❌ Canal destino é desconhecido → Perguntar Tech Lead antes de enviar
- ❌ Rate limit atingido repetidamente → Possível loop; parar e investigar causa raiz

### Templates Disponíveis

| Template | Uso | Canal Default |
|----------|-----|---------------|
| `sync_start` | Início de batch sync | #engineering |
| `sync_complete` | Fim de batch sync com métricas | #engineering |
| `sync_failed` | Falha em batch sync | #incident |
| `sev_alert` | Alerta SEV-1/SEV-2 com contexto | #incident |
| `hitl_approval_pending` | Aprovação humana pendente (>15min) | #engineering + DM ao aprovador |
| `guardrail_blocked` | Guardrail bloqueou ação | #security |
| `daily_summary` | Resumo diário de operações | #engineering |

### Glossário

| Termo | Significado |
|-------|-------------|
| **Webhook** | URL única do Slack que aceita POST JSON para enviar mensagens ao canal |
| **Block Kit** | Framework do Slack para mensagens ricas com blocos formatados e interativos |
| **Idempotente** | Operação que produz mesmo resultado independente de quantas vezes é executada |
| **Rate Limit** | Limite de requisições por unidade de tempo para prevenir abuso |
| **Fallback** | Mecanismo alternativo quando o canal primário falha |

---

## 5. Referências Cruzadas

- **Playbook Pai:** [`pb-legacy-sync`](../playbooks/pb-legacy-sync.md) — Este runbook é acionado nas fases de comunicação do playbook
- **SOAR Workflow:** [`soar-guardrail-enforcement`](../playbooks/soar-guardrail-enforcement.md) — Step 2 (Contain) usa este runbook para notificar admins
- **POPs Relacionados:** [`pop-guardrail-rule-update`](../playbooks/pop-guardrail-rule-update.md), [`pop-relay-token-revoke`](../playbooks/pop-relay-token-revoke.md)
- **Código:** `crates/zylos-live-comm/src/hermes_bridge.rs`, `apps/zylos-gateway/src/hermes_api.rs`
- **Monitoramento:** Grafana Dashboard "Slack Notifications" (taxa de envio, falhas, latência)
- **Canais Slack:** #engineering, #incident, #security
- **Última Revisão:** 2026-11-11
- **Próximo Review Obrigatório:** 2026-12-11

---

## Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0.0 | 2026-11-11 | Implementation Agent | Criação inicial com binding para pb-legacy-sync e soar-guardrail-enforcement |