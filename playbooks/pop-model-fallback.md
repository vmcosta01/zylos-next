# POP-006: Fallback de Modelo Indisponível

**Version:** 1.0.0
**Playbook:** [pb-model-tier-strategy.md](./pb-model-tier-strategy.md)
**Last Updated:** 2026-11-11

## Purpose

Procedimento operacional para quando o modelo configurado para um bot está indisponível (quota esgotada, API error, timeout, deprecated). O sistema deve fazer fallback automático para o próximo tier disponível sem interromper o serviço.

## Trigger

- Bot retorna erro de API do provider (429 rate limit, 503 unavailable, 404 model not found)
- Latência do modelo excede threshold (>30s para resposta simples)
- Quota mensal/diária do provider esgotada
- Usuário reporta que bot não responde ou responde com erro de modelo

## Steps

### 1. Identificar causa da indisponibilidade

```bash
# Verificar config atual do bot
curl -s "http://localhost:3000/api/v1/models/config?bot_id={bot_id}" | jq '.'

# Verificar logs do gateway para erros de API
grep "{bot_id}" /var/log/zylos-gateway.log | grep -iE "error|timeout|429|503" | tail -20
```

Classificar causa:
- **Rate limit (429):** Temporário, retry com backoff
- **Model not found (404):** Modelo deprecated ou typo no model_id
- **Service unavailable (503):** Provider down, fallback imediato necessário
- **Quota exhausted:** Upgrade de plano ou troca de provider necessário
- **Timeout:** Modelo lento demais para use case, trocar tier

### 2. Aplicar fallback temporário

#### Opção A: Trocar para modelo alternativo no mesmo provider
```bash
curl -X PUT http://localhost:3000/api/v1/models/config/{bot_id} \
 -H "Content-Type: application/json" \
 -d '{
 "provider": "openai",
 "model_id": "gpt-4o-mini",
 "tier_strategy": "cheapest",
 "reasoning_effort": "low"
 }'
```

#### Opção B: Trocar para provider alternativo
```bash
curl -X PUT http://localhost:3000/api/v1/models/config/{bot_id} \
 -H "Content-Type: application/json" \
 -d '{
 "provider": "anthropic",
 "model_id": "claude-3-haiku",
 "tier_strategy": "balanced"
 }'
```

#### Opção C: Remover override e usar global
```bash
curl -X DELETE http://localhost:3000/api/v1/models/config/{bot_id}
# Bot volta a usar global automaticamente
```

### 3. Validar que fallback funciona

```bash
# Enviar mensagem teste ao bot e verificar resposta
# Ou verificar health check do endpoint de chat
curl -sf "http://localhost:3000/api/v1/bots/{bot_id}/health" | jq '.model_status'
# Esperado: "healthy" ou "available"
```

### 4. Documentar incidente

Adicionar entrada no `TRACKRECORD_FASE9.md`:
```markdown
### YYYY-MM-DD
- **Fixed:** Modelo {model_id} indisponível para bot {bot_id}
- **Cause:** {rate_limit/deprecated/quota/timeout}
- **Fallback:** {novo_model_id} via {opção A/B/C}
- **Duration:** {tempo de indisponibilidade}
- **Resolution:** {temporário/permanente}
```

## Verification

- Bot responde a mensagens teste sem erro de modelo
- Config efetiva (`GET /models/config?bot_id=X`) mostra novo modelo
- Logs do gateway não mostram mais erros de API para este bot
- Qualidade da resposta é aceitável (mesmo que inferior ao modelo original)

## Rollback

Quando modelo original voltar a funcionar:
1. Restaurar config original via `PUT /models/config/:bot_id`
2. Testar com mensagem simples
3. Se funcionar, remover nota de fallback do track record
4. Se não funcionar, manter fallback e investigar causa raiz

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Todos os modelos de um provider down | Platform team + Provider support | Imediatamente, afeta múltiplos bots |
| Fallback também falha | Dev team | Após 2 tentativas de fallback diferentes |
| Quota esgotada recorrentemente | Finance/Admin | Upgrade de plano ou redistribuição de carga |
| Modelo deprecated sem substituto claro | Architecture team | Planejar migração para novo modelo |