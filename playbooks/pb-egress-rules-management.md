# PB-002: Gerenciamento de Regras de Egress

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-egress-false-positive.md](./pop-egress-false-positive.md)
**Related Runbooks:** [rb-zylos-egress.md](../runbooks/rb-zylos-egress.md)
**Related SOAR:** [soar-egress-block-response.md](../docs/soar/soar-egress-block-response.md)

## Objective

Gerenciar regras customizadas de detecção de dados sensíveis no verificador de egress. Este playbook cobre criação, teste, ativação e remoção de regras customizadas para detectar padrões específicos do domínio (IDs internos, códigos de projeto, formatos proprietários).

## Prerequisites

- Crate `zylos-egress` compilado e integrado ao gateway
- Endpoint `/api/v1/egress/rules` acessível via localhost:3000
- Permissão administrativa para modificar regras de segurança

## Steps

### 1. Listar regras existentes

```bash
curl -s http://localhost:3000/api/v1/egress/rules | jq '.'
```

### 2. Criar nova regra customizada

```bash
curl -X POST http://localhost:3000/api/v1/egress/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "internal_project_code",
    "pattern": "PROJ-[A-Z]{2,4}-\\d{4,6}",
    "category": "custom",
    "blocks": false,
    "enabled": true
  }'
```

Campos obrigatórios:
- `name`: identificador legível da regra
- `pattern`: regex válida (testar antes de deploy)
- `category`: `custom`, `email`, `phone`, `secret`, etc.
- `blocks`: `true` = bloqueia envio; `false` = apenas avisa
- `enabled`: `true` = ativa; `false` = desativada (mantida para referência)

### 3. Testar regra contra corpus de amostra

Antes de ativar em produção, testar com textos reais:

```bash
curl -X POST http://localhost:3000/api/v1/egress/check \
  -H "Content-Type: application/json" \
  -d '{"text": "Referência ao projeto PROJ-ENG-2024-001 no documento"}'
```

Verificar:
- Finding detectado com categoria correta
- Redacted text gerado apropriadamente
- `passed` reflete configuração `blocks` da regra

### 4. Ajustar categorias bloqueantes

Se uma categoria deve bloquear/enviar apenas aviso:

```rust
// No gateway config ou via API admin
let checker = EgressChecker::new()
    .with_blocked_categories(vec![
        SensitiveCategory::Secret,
        SensitiveCategory::CreditCard,
        SensitiveCategory::Custom, // Agora custom também bloqueia
    ]);
```

### 5. Desativar regra problemática

```bash
curl -X DELETE http://localhost:3000/api/v1/egress/rules/{rule_id}
```

Ou desativar sem deletar (mantém para auditoria):
```bash
curl -X PATCH http://localhost:3000/api/v1/egress/rules/{rule_id} \
  -H "Content-Type: application/json" \
  -d '{"enabled": false}'
```

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Novo padrão de dado sensível identificado | Criar regra custom com `blocks=false` inicialmente | Monitorar 1 semana, promover para `blocks=true` se confirmado |
| Falso positivo recorrente (>5/dia) | Desativar regra ou ajustar regex | Verificar POP de false-positive |
| Regex muito ampla (match >10% dos textos) | Refinar pattern ou adicionar contexto | Precision > recall para regras bloqueantes |
| Dado sensível novo não coberto por built-ins | Priorizar adição como built-in se universal | Custom rules são para padrões específicos do domínio |

## Rollback Procedure

1. Desativar todas as regras custom: iterar sobre `/egress/rules` e setar `enabled=false`
2. Reiniciar gateway se necessário para recarregar checker
3. Verificar que apenas built-ins permanecem ativos
4. Investigar causa raiz antes de reativar

## SLA/RTO/RPO

- **SLA:** 99.9% disponibilidade do egress checker (crítico para segurança)
- **RTO:** 2 minutos (reload de regras é hot-reload, sem restart)
- **RPO:** 0 (regras persistidas em SQLite/config, sem perda)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.2
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDuet egress.ts](https://github.com/CopilotKit/OpenDuet/blob/main/src/approvals/egress.ts)