# POP-001: Decidir Approval Gate

**Version:** 1.0.0
**Playbook:** [pb-approval-gate-setup.md](./pb-approval-gate-setup.md)
**Last Updated:** 2026-11-11

## Purpose

Procedimento operacional para decidir uma requisição de aprovação pendente no Zylos Cockpit v2. Aplica-se quando um card de approval aparece inline no chat solicitando permissão para ação externa.

## Trigger

- Card de approval visível no chat com status "pending"
- Notificação de approval pendente via SSE ou polling
- Alerta SOAR de approval timeout (após 30 min sem decisão)

## Steps

### 1. Verificar contexto da ação

1. Ler o **summary** do card (descrição legível da ação)
2. Expandir **"Ver argumentos exatos"** para inspecionar o JSON completo
3. Confirmar que os argumentos correspondem à intenção declarada
4. Verificar o **bot_name** e **tool_name** para identificar origem

### 2. Avaliar risco

| Categoria | Risco | Ação Recomendada |
|-----------|-------|------------------|
| `send_message` | Alto (externo irreversível) | Revisar destinatário e conteúdo antes de aprovar |
| `deploy` | Crítico (infraestrutura) | Verificar ambiente (prod vs staging), changelog |
| `payment` | Crítico (financeiro) | Confirmar valor, destinatário, autorização |
| `sign_document` | Crítico (legal) | Ler documento completo antes de assinar |
| `delete_resource` | Médio-Alto | Confirmar recurso e backup existente |
| `read_sensitive` | Médio | Verificar se PII é necessário para a tarefa |
| `low_risk` | Baixo | Aprovar livremente, considerar allow-always |

### 3. Escolher decisão

- **Permitir uma vez** (`allow_once`): Ação segura mas não recorrente. Aprova apenas esta instância.
- **Sempre permitir** (`allow_always`): Ação segura e recorrente. Cria regra persistente para bot+tool+category. Use apenas após ≥3 aprovações idênticas.
- **Recusar** (`refuse`): Ação insegura, inesperada ou mal formada. Forneça motivo claro para o agente ajustar.

### 4. Executar decisão via UI

1. Clicar no botão correspondente no card de approval
2. Aguardar spinner de processamento (< 2s esperado)
3. Verificar card muda para estado "Aprovado" ✅ ou "Recusado" ❌
4. Se recusou, verificar mensagem de confirmação com motivo

### 5. Executar decisão via API (alternativo)

```bash
curl -X POST http://localhost:3000/api/v1/approvals/{id}/decide \
  -H "Content-Type: application/json" \
  -d '{"decision": "allow_once"}'
```

Respostas esperadas:
- `200`: Decisão registrada com sucesso
- `410`: Request expirado — ação foi cancelada automaticamente
- `409`: Request já decidido — verificar histórico

## Verification

- Card de approval mostra estado final (approved/refused/expired)
- Se `allow_once`: ação executa imediatamente após decisão
- Se `allow_always`: próxima ação idêntica não gera novo card
- Se `refuse`: ação não executa, motivo registrado em `refuse_reason`
- Audit log registra decisão em `/api/v1/audit/actions`

## Rollback

### Se aprovou por engano (allow_once):
1. Ação já executou — não há undo automático
2. Verificar resultado da ação e mitigar manualmente
3. Registrar incidente no audit log

### Se criou regra allow_always por engano:
```bash
# Listar regras do bot
curl -s http://localhost:3000/api/v1/approvals/rules?bot_id={bot_id} | jq '.[] | {id, tool_name}'

# Deletar regra específica
curl -X DELETE http://localhost:3000/api/v1/approvals/rules/{rule_id}
```

### Se recusou por engano:
1. Reenviar comando ao agente
2. Novo card de approval será gerado
3. Aprovar corretamente desta vez

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Approval expira repetidamente | Admin do sistema | >3 expiry em 24h para mesmo bot |
| Arguments suspeitos (PII leak) | Security team | Imediatamente, via SOAR |
| Bot gera approvals em loop | Platform team | >10 approvals/minuto para mesmo bot |
| UI não responde ao clique | Frontend team | Após 3 tentativas com erro de rede |