# POP-Guardrail-Rule-Update — Atualização de Regras de Guardrail (T8.5.5)

**Versão:** 1.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `pop-author`
**Módulo:** `zylos-personalities::guardrails`

---

## 1. Objetivo

Procedimento operacional padrão para adicionar, modificar ou remover regras de guardrail no `GuardrailEnforcer` sem downtime do sistema.

## 2. Pré-requisitos

- Acesso ao gateway API (`http://localhost:9876`)
- Token de autenticação com permissão `admin:guardrails`
- Conhecimento do schema `GuardrailRule` (ver `crates/zylos-personalities/src/guardrails.rs`)

## 3. Procedimento

### 3.1 Adicionar Nova Regra

```bash
curl -X POST http://localhost:9876/api/v1/guardrails/rules \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "block-prod-deploy-weekend",
    "description": "Block production deploys on weekends",
    "routine_pattern": "deploy-production",
    "action": {"block": {"reason": "Production deploys blocked on weekends"}},
    "priority": 85,
    "enabled": true
  }'
```

### 3.2 Modificar Regra Existente

```bash
curl -X PATCH http://localhost:9876/api/v1/guardrails/rules/block-prod-deploy-weekend \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"priority": 90}'
```

### 3.3 Desativar Regra (Soft Delete)

```bash
curl -X PATCH http://localhost:9876/api/v1/guardrails/rules/block-prod-deploy-weekend \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'
```

### 3.4 Remover Regra (Hard Delete)

```bash
curl -X DELETE http://localhost:9876/api/v1/guardrails/rules/block-prod-deploy-weekend \
  -H "Authorization: Bearer $TOKEN"
```

## 4. Validação

Após qualquer alteração, verificar decisões recentes:

```bash
curl http://localhost:9876/api/v1/guardrails/decisions?limit=5 \
  -H "Authorization: Bearer $TOKEN"
```

Verificar stats do enforcer:

```bash
curl http://localhost:9876/api/v1/guardrails/stats \
  -H "Authorization: Bearer $TOKEN"
```

## 5. Rollback

Se a nova regra causar bloqueios indevidos:

1. Desativar imediatamente: `PATCH .../rules/{name} {"enabled": false}`
2. Verificar log de decisões para identificar impactos
3. Corrigir regra e reativar apenas após validação em staging

## 6. Cross-References

- **Runbook:** `runbooks/zylos-personalities.md`
- **Handoff:** `docs/handoffs/HANDOFF_FASE8.md` §2.7
- **Código:** `crates/zylos-personalities/src/guardrails.rs`
- **Playbook relacionado:** `playbooks/sop-incident-triage.md`

## 7. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-11 | Implementation Agent | Criação inicial |