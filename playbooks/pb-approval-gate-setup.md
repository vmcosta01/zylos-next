# PB-001: Configuração de Approval Gates HITL

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-approval-decide.md](./pop-approval-decide.md)
**Related Runbooks:** [rb-zylos-approval.md](../runbooks/rb-zylos-approval.md)
**Related SOAR:** [soar-approval-timeout.md](../docs/soar/soar-approval-timeout.md)

## Objective

Configurar o sistema de approval gates para que ações externas dos agentes (envio de mensagens, deploys, pagamentos, assinaturas) exijam confirmação humana antes da execução. Este playbook cobre a configuração inicial, definição de categorias de risco e ajuste de regras persistentes.

## Prerequisites

- Crate `zylos-approval` compilado e integrado ao gateway
- Migração `001_initial.sql` aplicada (ou `run_migrations()` executada)
- Cockpit v2 servindo na porta 8080
- Gateway servindo na porta 3000 com CORS configurado para localhost:8080

## Steps

### 1. Verificar integridade do schema

```bash
curl -s http://localhost:3000/api/v1/approvals/pending | jq '. | length'
# Esperado: 0 ou array válido (não erro 500)
```

Se retornar erro, executar `ApprovalStore::run_migrations()` via endpoint administrativo ou reiniciar o gateway.

### 2. Definir categorias de ação por bot

No arquivo de configuração do bot (`bots/<bot-id>/config.yaml`), mapear ferramentas para categorias:

```yaml
approval_categories:
  send_email: send_message      # Sempre pede approval
  send_slack: send_message      # Sempre pede approval
  deploy_prod: deploy           # Sempre pede approval
  deploy_staging: low_risk      # Não pede por padrão
  create_invoice: payment       # Sempre pede approval
  sign_contract: sign_document  # Sempre pede approval
  delete_user: delete_resource  # Pede se produção
  read_pii: read_sensitive      # Pede se PII detectado
  generate_report: low_risk     # Não pede
```

### 3. Configurar expiry padrão

No gateway config (`apps/zylos-gateway/config.yaml`):

```yaml
approval:
  default_expiry_seconds: 3600    # 1 hora
  max_expiry_seconds: 86400       # 24 horas máximo
  cleanup_interval_seconds: 300   # Limpar expirados a cada 5 min
```

### 4. Testar fluxo end-to-end

1. Abrir cockpit em http://localhost:8080
2. Enviar mensagem que trigger ação externa (ex: "enviar email para teste@exemplo.com")
3. Verificar card de approval aparece inline no chat
4. Clicar "Permitir uma vez" → verificar ação executa
5. Repetir → clicar "Sempre permitir" → verificar próxima execução é automática
6. Repetir → clicar "Recusar" → verificar ação não executa e motivo é registrado

### 5. Configurar alertas de timeout

No SOAR workflow `soar-approval-timeout.md`, definir:
- Alerta após 30 minutos sem decisão
- Cancelamento automático após expiry
- Notificação ao usuário via cockpit + messaging connector

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Nova ferramenta sem categoria | Classificar como `low_risk` inicialmente, monitorar 1 semana, reclassificar se necessário | Princípio da precaução progressiva |
| Falso positivo frequente | Criar regra allow-always via UI ou API | >5 aprovações idênticas em 24h |
| Ação crítica sem approval | Adicionar à categoria `send_message`/`deploy`/`payment` imediatamente | Qualquer ação com impacto externo irreversível |
| Expiry muito curto | Aumentar para 2h se usuários reportam perda de contexto | Feedback de usabilidade |

## Rollback Procedure

1. Desativar approval gates: setar `approval.enabled = false` no gateway config
2. Reiniciar gateway: `hermes gateway restart` (fora da sessão ativa)
3. Verificar ações executam sem approval
4. Investigar causa raiz antes de reativar

## SLA/RTO/RPO

- **SLA:** 99.9% disponibilidade do approval service (crítico para segurança)
- **RTO:** 5 minutos (restart do gateway + reaplicação de migrations)
- **RPO:** 0 (decisões persistidas em SQLite WAL, sem perda em crash)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.1
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDots PageReviewCard](https://github.com/CopilotKit/OpenDots/blob/main/src/client/PageReviewCard.tsx)
- [OpenDuet approvals/service.ts](https://github.com/CopilotKit/OpenDuet/blob/main/src/approvals/service.ts)
- [AnyModelBot cu-permission.js](https://github.com/modelflare-ai/AnyModelBot/blob/master/ui/components/cu-permission.js)