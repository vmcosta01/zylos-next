# SOAR: Approval Timeout Response

**Trigger:** Approval request expira sem decisão humana após `expires_at`
**Severity:** Medium
**Auto-Remediation:** Partial (cancela ação, notifica usuário; não re-executa)

## Detection

O gateway detecta approvals expirados via background task periódica (intervalo configurável, default 5 min):

```sql
SELECT id, bot_id, tool_name, summary, created_at, expires_at 
FROM approval_requests 
WHERE status = 'pending' AND expires_at IS NOT NULL AND expires_at < datetime('now')
```

Alternativamente, `get_pending()` retorna `ApprovalError::Expired` quando cockpit tenta renderizar card expirado.

## Containment

1. Atualizar status para `expired`:
   ```sql
   UPDATE approval_requests SET status = 'expired' WHERE id = ?
   ```
2. Invalidar receipt JSON (se existir) para prevenir replay stale
3. Publicar evento `ApprovalExpired { request_id, bot_id }` no EventBus

## Investigation

1. Verificar quantos approvals expiraram nas últimas 24h:
   ```sql
   SELECT COUNT(*) FROM approval_requests 
   WHERE status = 'expired' AND created_at > datetime('now', '-24 hours')
   ```
2. Se >10 expiry/24h para mesmo bot → possível UX issue (cards não visíveis, notificações falhando)
3. Se expiry rate >20% do total → verificar `default_expiry_seconds` no config
4. Verificar logs do cockpit para erros de renderização de approval cards

## Remediation

### Automática
- Ação original é cancelada (não executa)
- Bot recebe notificação via EventBus: `ApprovalExpired`
- Bot deve decidir: retry com novo approval ou abortar tarefa

### Manual (se padrão recorrente)
1. Ajustar `default_expiry_seconds` no gateway config se muito curto
2. Verificar se cockpit SSE/polling está funcionando (cards aparecem?)
3. Verificar se notificações push estão habilitadas para approvals pendentes
4. Considerar adicionar som/alerta visual para novos approvals

## Recovery

1. Usuário reenvia comando ao agente
2. Novo approval request é gerado com expiry fresco
3. Monitorar se novo request é decidido dentro do prazo

## Post-Incident

| Item | Ação |
|------|------|
| Documentação | Atualizar `rb-zylos-approval.md` §Troubleshooting se nova causa raiz identificada |
| Config | Ajustar `default_expiry_seconds` se necessário |
| Alerting | Adicionar métrica `approval_expired_rate` ao dashboard de monitoring |
| UX | Se cards não visíveis foram a causa, priorizar fix no cockpit |

## Bound POPs

- [pop-approval-decide.md](../playbooks/pop-approval-decide.md) — quando usuário decide antes do timeout

## Bound Runbooks

- [rb-zylos-approval.md](../runbooks/rb-zylos-approval.md) — troubleshooting de approval system

## Metrics

| Métrica | Target | Alert Threshold |
|---------|--------|-----------------|
| Expired rate (24h) | <5% | >20% |
| Avg time to decide | <10 min | >30 min |
| Pending queue depth | <10 | >50 |
| Retry-after-expiry rate | <10% | >30% (indica ação importante sendo perdida) |