# PB-003: Configuração do Morning Brief Generator

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-brief-regenerate.md](./pop-brief-regenerate.md)
**Related Runbooks:** [rb-zylos-brief.md](../runbooks/rb-zylos-brief.md)

## Objective

Configurar o gerador de morning brief para que cada usuário receba um resumo matinal personalizado agregando aprovações pendentes, tarefas em andamento, eventos de calendário e falhas de integração. Este playbook cobre configuração inicial, ajuste de fontes de dados e validação do conteúdo gerado.

## Prerequisites

- Crate `zylos-brief` compilado e integrado ao gateway
- Migração da tabela `morning_briefs` aplicada via `BriefStore::run_migrations()`
- Tabelas `approval_requests` e `action_audit_log` existentes (Fase 1)
- Gateway servindo na porta 3000

## Steps

### 1. Verificar integridade do schema

```bash
curl -sf "http://localhost:3000/api/v1/brief/today?user_id=default" | jq '.sections | length'
# Esperado: 4 (needs_you, can_wait, calendar, unchecked)
```

Se retornar erro 500, verificar se migrations foram aplicadas. Reiniciar gateway se necessário.

### 2. Validar conteúdo das seções

Verificar se cada seção popula corretamente:

| Seção | Fonte Esperada | Como Testar |
|-------|----------------|-------------|
| needs_you | approval_requests (status=pending) | Criar approval pendente via API, regenerar brief |
| can_wait | action_audit_log (result_status=null) | Registrar ação sem resultado, regenerar brief |
| calendar | Placeholder (integração futura) | Sempre vazia por enquanto |
| unchecked | action_audit_log (result_status=failure) | Registrar ação com falha, regenerar brief |

### 3. Configurar trigger de geração

No gateway config (`apps/zylos-gateway/config.yaml`):

```yaml
brief:
  auto_generate_hour: 6 # Hora local para geração automática (0-23)
  timezone: "America/Sao_Paulo" # Fuso horário do usuário
  retention_days: 30 # Retenção de histórico
  max_items_per_section: 10 # Limite por seção para evitar briefs gigantes
```

### 4. Testar regeneração forçada

```bash
curl -X POST http://localhost:3000/api/v1/brief/regenerate \
 -H "Content-Type: application/json" \
 -d '{"user_id": "default", "force": true}'
```

Verificar que novo brief é gerado mesmo se já existir um para o dia.

### 5. Validar persistência (não regenera no mesmo dia)

1. Chamar `GET /api/v1/brief/today?user_id=default` duas vezes
2. Verificar que `generated_at` é idêntico nas duas respostas
3. Confirmar que apenas 1 row existe na tabela para user+date

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Brief vazio quando deveria ter itens | Verificar conectividade com tabelas fonte | Query direta em approval_requests/action_audit_log |
| Seção calendar sempre vazia | Normal — integração futura via MCP connector | Priorizar após Fase 2 completa |
| Muitos itens em needs_you (>10) | Aumentar max_items_per_section ou priorizar por criticidade | Feedback de usabilidade |
| Brief não gera automaticamente às 06:00 | Verificar cron job no gateway | Logs do scheduler |

## Rollback Procedure

1. Desativar auto-generate: setar `brief.auto_generate = false` no config
2. Briefs existentes permanecem acessíveis via `/brief/history`
3. Regeneração manual ainda funciona via `/brief/regenerate`
4. Investigar causa raiz antes de reativar

## SLA/RTO/RPO

- **SLA:** 99% disponibilidade (não crítico — fallback é brief vazio)
- **RTO:** 5 minutos (restart do gateway)
- **RPO:** 0 (briefs persistidos em SQLite, sem perda)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.1
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDuet morning-brief.ts](https://github.com/CopilotKit/OpenDuet/blob/main/src/shared/morning-brief.ts)