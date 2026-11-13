# SOAR: Audit Anomaly Detection Response

**Trigger:** Padrão suspeito detectado no audit log (spike de ações, refused rate alto, bot executando ações fora do padrão)
**Severity:** High (potencial comprometimento ou mau uso)
**Auto-Remediation:** Partial (alerta admin, pausa bot se crítico; não reverte ações)

## Detection

Anomalias são detectadas via monitoring contínuo do audit log:

### Sinais de anomalia
1. **Spike de volume:** >3x média diária de ações em 1h
2. **Refused rate alto:** >20% das ações recusadas em 24h
3. **Bot fora do padrão:** bot executando action_type nunca visto antes
4. **Horário incomum:** ações em horário não-trabalho sem schedule configurado
5. **Target suspeito:** mesmo target recebido por múltiplos bots em curto período
6. **Secret category block:** qualquer bloqueio por categoria `secret` no egress checker

### Queries de detecção
```sql
-- Spike de volume (última hora vs média 7d)
SELECT COUNT(*) as last_hour FROM action_audit_log 
WHERE created_at > datetime('now', '-1 hour');

SELECT AVG(daily_count) as avg_7d FROM (
 SELECT DATE(created_at) as day, COUNT(*) as daily_count 
 FROM action_audit_log 
 WHERE created_at > datetime('now', '-7 days') 
 GROUP BY DATE(created_at)
);

-- Refused rate
SELECT 
 COUNT(*) FILTER (WHERE approval_status='refused') * 100.0 / COUNT(*) as refused_pct
FROM action_audit_log 
WHERE created_at > datetime('now', '-24 hours');

-- Bot com action_type novo
SELECT bot_id, action_type, COUNT(*) as cnt
FROM action_audit_log 
WHERE created_at > datetime('now', '-24 hours')
 AND action_type NOT IN (
 SELECT DISTINCT action_type FROM action_audit_log 
 WHERE created_at < datetime('now', '-24 hours') AND bot_id = action_audit_log.bot_id
 )
GROUP BY bot_id, action_type;
```

## Containment

1. **Alertar admin imediatamente** via messaging connector (Slack/Telegram/email)
2. **Se severity critical** (secret leak, payment fraud):
 - Pausar bot afetado via API: `POST /api/v1/bots/{bot_id}/pause`
 - Bloquear tool específica via capability token revocation
3. **Preservar evidência:**
 - Export audit log do período suspeito: `GET /audit/export?since=X&bot_id=Y`
 - Snapshot do estado do bot (memory, goals, pending approvals)
4. **Publicar evento** `AuditAnomalyDetected { bot_id, anomaly_type, severity }` no EventBus

## Investigation

1. **Verificar contexto das ações suspeitas:**
 ```bash
 curl -s "http://localhost:3000/api/v1/audit/actions?bot_id={bot_id}&since={timestamp}&limit=100" | jq '.'
 ```
2. **Correlacionar com outros sinais:**
 - Approval requests pendentes/expirados no mesmo período
 - Egress blocks no mesmo período
 - Mudanças recentes em skills/rules do bot
 - Acessos ao cockpit/admin UI no mesmo período
3. **Classificar causa raiz:**
 - **Falso positivo:** spike legítimo (ex: campanha de emails agendada)
 - **Bug no bot:** loop infinito, instrução mal interpretada
 - **Prompt injection:** usuário malicioso manipulou bot via chat
 - **Credencial comprometida:** API key/token vazado, acesso não autorizado
 - **Configuração errada:** nova rule/approval gate mal configurada

## Remediation

### Automática
- Alerta enviado ao admin/security team
- Bot pausado se severity critical
- Evidence bundle gerado e armazenado

### Manual (após investigação)
1. **Se falso positivo:** ajustar threshold de alerta, documentar exceção
2. **Se bug no bot:** corrigir skill/instrução, adicionar teste de regressão
3. **Se prompt injection:** revisar guardrails, adicionar filtro de input, rotacionar credenciais
4. **Se credencial comprometida:** revogar token, gerar novo, auditar todas as ações desde comprometimento
5. **Se config errada:** reverter mudança, adicionar validação pré-deploy

## Recovery

1. Após causa raiz resolvida, despausar bot: `POST /api/v1/bots/{bot_id}/resume`
2. Monitorar por 24h para confirmar que anomalia não recurs
3. Atualizar thresholds/regras de detecção se necessário
4. Documentar lições aprendidas no post-incident

## Post-Incident

| Item | Ação |
|------|------|
| Documentação | Atualizar `rb-zylos-audit-log.md` §Monitoring com novo sinal de anomalia |
| Rules | Ajustar thresholds de alerta se falso positivo |
| Guardrails | Adicionar nova regra de proteção se prompt injection confirmado |
| Training | Atualizar instruções do bot para evitar comportamento anômalo |
| Metrics | Adicionar nova métrica de anomalia ao dashboard |
| Retrospective | Agendar se severity critical ou impacto financeiro/legal |

## Bound POPs

- [pop-audit-log-export.md](../../playbooks/pop-audit-log-export.md) — para gerar evidence bundle
- [pop-approval-decide.md](../../playbooks/pop-approval-decide.md) — se anomalia envolve approvals

## Bound Runbooks

- [rb-zylos-audit-log.md](../../runbooks/rb-zylos-audit-log.md) — queries e troubleshooting
- [rb-zylos-approval.md](../../runbooks/rb-zylos-approval.md) — se anomalia envolve approval system

## Metrics

| Métrica | Target | Alert Threshold |
|---------|--------|-----------------|
| Time to detect (TTD) | <5 min | >30 min |
| Time to contain (TTC) | <15 min | >1h |
| False positive rate | <10% | >30% |
| Mean time to resolve (MTTR) | <2h | >24h |
| Recurrence rate (30d) | 0 | Qualquer recorrência requer root cause review |