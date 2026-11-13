# POP-005: Regenerar Morning Brief

**Version:** 1.0.0
**Playbook:** [pb-morning-brief-config.md](./pb-morning-brief-config.md)
**Last Updated:** 2026-11-11

## Purpose

Forçar regeneração do morning brief quando o conteúdo está desatualizado, incorreto ou quando novas fontes de dados foram integradas. O brief é normalmente gerado uma vez por dia; este POP sobrescreve essa restrição.

## Trigger

- Usuário reporta que brief não reflete estado atual (approvals novos não aparecem)
- Nova integração foi adicionada e precisa ser refletida no brief
- Bug corrigido no generator e brief existente contém dados errados
- Teste de nova funcionalidade requer brief fresco

## Steps

### 1. Confirmar necessidade de regeneração

Verificar se o brief atual realmente está desatualizado:

```bash
curl -s "http://localhost:3000/api/v1/brief/today?user_id=default" | jq '.generated_at'
# Comparar com horário atual — se <5min atrás, provavelmente já está atualizado
```

Se brief foi gerado recentemente mas ainda parece errado, o problema pode estar nas fontes de dados (approval_requests/action_audit_log), não no generator.

### 2. Executar regeneração forçada

```bash
curl -X POST http://localhost:3000/api/v1/brief/regenerate \
 -H "Content-Type: application/json" \
 -d '{"user_id": "default", "force": true}'
```

Respostas esperadas:
- `200`: Brief regenerado com sucesso, novo content_json retornado
- `500`: Erro interno — verificar logs do gateway

### 3. Validar novo conteúdo

```bash
curl -s "http://localhost:3000/api/v1/brief/today?user_id=default" | jq '{
 generated_at: .generated_at,
 total_items: .total_items,
 needs_you: .needs_you_count,
 sections: [.sections[] | {name: .name, count: (.items | length)}]
}'
```

Verificar:
- `generated_at` é timestamp recente (<1 min atrás)
- Contagens refletem estado atual das tabelas fonte
- Seções populadas conforme esperado

### 4. Se brief continua incorreto após regeneração

O problema está nas fontes de dados, não no generator:

1. Verificar approvals pendentes diretamente:
 ```bash
 curl -s "http://localhost:3000/api/v1/approvals/pending" | jq 'length'
 ```
2. Verificar audit log diretamente:
 ```bash
 curl -s "http://localhost:3000/api/v1/audit/actions?limit=5" | jq '.entries | length'
 ```
3. Se fontes têm dados mas brief não os inclui → bug no generator → abrir issue
4. Se fontes estão vazias → brief vazio é comportamento correto

## Verification

- Novo brief tem `generated_at` recente
- Contagens batem com queries diretas nas tabelas fonte
- Cockpit exibe brief atualizado após refresh (Ctrl+Shift+R)
- Histórico mostra entrada duplicada para mesma data (upsert funcionou)

## Rollback

Regeneração é operação idempotente e segura:
- Brief anterior é substituído via upsert (não há perda de histórico de dias anteriores)
- Se regeneração produziu resultado pior, simplesmente aguarde o próximo trigger automático (06:00) ou regenere novamente após corrigir causa raiz
- Não há undo direto — o brief antigo não é preservado separadamente

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Regeneração retorna 500 consistentemente | Platform team | Após 3 tentativas com intervalo de 1 min |
| Brief ignora dados que existem nas fontes | Dev team | Bug no generator, abrir issue com exemplos |
| Brief demora >10s para gerar | Dev team | Performance issue, verificar queries SQL |
| Usuário não vê brief atualizado no cockpit | Frontend team | Possível cache do browser ou SSE não atualizando |