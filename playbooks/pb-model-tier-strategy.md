# PB-004: Configuração de Tier Strategy e Modelos

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-model-fallback.md](./pop-model-fallback.md)
**Related Runbooks:** [rb-zylos-model-config.md](../runbooks/rb-zylos-model-config.md)

## Objective

Configurar o sistema de seleção de modelos para que cada bot tenha a configuração ideal de modelo LLM baseada em tier strategy (custo vs qualidade vs velocidade), reasoning effort e hierarquia de herança. Este playbook cobre definição de defaults globais, overrides por bot e validação da resolução hierárquica.

## Prerequisites

- Crate `zylos-model-config` compilado e integrado ao gateway
- Migração da tabela `model_configs` aplicada via `ModelConfigStore::run_migrations()`
- Gateway servindo na porta 3000

## Steps

### 1. Definir config global default

```bash
curl -X PUT http://localhost:3000/api/v1/models/config/__global__ \
 -H "Content-Type: application/json" \
 -d '{
 "provider": "openai",
 "model_id": "gpt-4o-mini",
 "tier_strategy": "balanced",
 "reasoning_effort": "medium",
 "max_tokens": 4096,
 "temperature": 0.7,
 "is_default": true
 }'
```

Esta config será usada por qualquer bot que não tenha override específico.

### 2. Configurar overrides por bot

Para bots que precisam de modelos diferentes:

```bash
# Bot de reasoning complexo → modelo mais inteligente
curl -X PUT http://localhost:3000/api/v1/models/config/bot-analyst \
 -H "Content-Type: application/json" \
 -d '{
 "provider": "anthropic",
 "model_id": "claude-3.5-sonnet",
 "tier_strategy": "smartest",
 "reasoning_effort": "high"
 }'

# Bot de triagem rápida → modelo mais barato
curl -X PUT http://localhost:3000/api/v1/models/config/bot-triage \
 -H "Content-Type: application/json" \
 -d '{
 "provider": "openai",
 "model_id": "gpt-4o-mini",
 "tier_strategy": "cheapest",
 "reasoning_effort": "low"
 }'
```

### 3. Validar resolução hierárquica

```bash
# Bot com override → deve retornar source: "bot"
curl -s "http://localhost:3000/api/v1/models/config?bot_id=bot-analyst" | jq '.source'
# Esperado: "bot"

# Bot sem override → deve retornar source: "global"
curl -s "http://localhost:3000/api/v1/models/config?bot_id=bot-unknown" | jq '.source'
# Esperado: "global"

# Sem global config → deve retornar source: "default"
# (apenas se global foi deletado)
```

### 4. Testar fallback após remoção de override

```bash
# Remover override do bot
curl -X DELETE http://localhost:3000/api/v1/models/config/bot-analyst

# Verificar que agora usa global
curl -s "http://localhost:3000/api/v1/models/config?bot_id=bot-analyst" | jq '.source'
# Esperado: "global"
```

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Novo bot sem requisitos específicos | Usar global default | Não criar override desnecessário |
| Bot de análise/reasoning | tier_strategy=smartest, effort=high | Qualidade > custo |
| Bot de triagem/resposta rápida | tier_strategy=cheapest, effort=low | Velocidade > qualidade |
| Bot de produção crítica | tier_strategy=balanced, effort=medium | Equilíbrio |
| Modelo indisponível | Fallback automático para próximo tier | Verificar POP de fallback |

## Rollback Procedure

1. Deletar override problemático: `DELETE /api/v1/models/config/:bot_id`
2. Bot volta a usar global automaticamente
3. Se global também problemático: atualizar global para modelo estável conhecido
4. Verificar logs do gateway para confirmação de resolução

## SLA/RTO/RPO

- **SLA:** 99.9% disponibilidade (crítico para funcionamento dos bots)
- **RTO:** 1 minuto (update via API é imediato)
- **RPO:** 0 (configs persistidas em SQLite)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.3
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [AnyModelBot model-picker.js](https://github.com/modelflare-ai/AnyModelBot/blob/master/ui/components/model-picker.js)