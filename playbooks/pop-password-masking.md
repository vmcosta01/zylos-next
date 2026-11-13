# POP-003: Password Masking no Transcript

**Version:** 1.0.0
**Playbook:** [pb-approval-gate-setup.md](./pb-approval-gate-setup.md)
**Last Updated:** 2026-11-11

## Purpose

Garantir que senhas e credenciais digitadas ou recebidas pelo agente nunca apareçam em plaintext no transcript de conversas, logs ou audit trail. O middleware `password_masking` do gateway intercepta payloads JSON antes da persistência e substitui valores de campos sensíveis por `"********"`.

## Trigger

- Usuário envia mensagem contendo senha/credencial via chat
- Bot recebe payload com campo `password`, `token`, `api_key`, etc.
- Integração externa retorna credenciais em response JSON
- Auditoria de segurança identifica plaintext em transcript histórico

## Steps

### 1. Verificar se masking está ativo

```bash
curl -s -X POST http://localhost:3000/api/v1/egress/check \
  -H "Content-Type: application/json" \
  -d '{"text": "{\"password\": \"test123\"}"}' | jq '.'
```

Se o gateway está com `password_masking` integrado, qualquer payload JSON persistido terá campos sensíveis mascarados automaticamente.

### 2. Campos mascarados automaticamente

| Campo | Case-Insensitive | Exemplo |
|-------|------------------|---------|
| password / passwd / senha | ✅ | `"senha": "********"` |
| secret / api_secret / client_secret | ✅ | `"secret": "********"` |
| token / access_token / refresh_token | ✅ | `"token": "********"` |
| api_key / apikey | ✅ | `"api_key": "********"` |
| private_key / signing_key / encryption_key | ✅ | `"private_key": "********"` |
| credentials / auth / authorization / bearer | ✅ | `"credentials": "********"` |
| connection_string / database_url / db_password | ✅ | `"db_password": "********"` |
| smtp_password | ✅ | `"smtp_password": "********"` |

### 3. Comportamento do masking

- **Apenas strings são mascaradas**: valores numéricos, booleanos ou null NÃO são alterados
- **Recursivo**: campos sensíveis em qualquer nível de aninhamento JSON são detectados
- **Arrays**: objetos dentro de arrays também têm campos mascarados
- **Case-insensitive**: `PASSWORD`, `Password`, `password` são todos detectados
- **Audit trail**: log registra QUANTOS campos foram mascarados (não os valores originais)

### 4. Se senha vazou em transcript histórico

1. Identificar mensagens afetadas via busca no audit log
2. Executar script de retroactive masking (ferramenta administrativa futura)
3. Notificar usuário sobre possível exposição
4. Rotacionar credenciais comprometidas imediatamente
5. Registrar incidente no SOAR

## Verification

- Enviar JSON com campo `password` via API → verificar transcript persistido mostra `"********"`
- Verificar logs do gateway: linha `masked N sensitive fields` sem valores originais
- Confirmar que campos não-sensíveis (`username`, `email`, `message`) permanecem intactos

## Rollback

Se masking causa problema funcional (ex: bot precisa de senha para autenticação):
1. O masking ocorre APÓS a lógica de negócio, apenas na persistência
2. O valor original ainda está disponível em memória durante o processamento
3. Se necessário desativar temporariamente: comentar chamada `mask_sensitive_fields()` no handler de persistência
4. ⚠️ **NUNCA desativar em produção sem approval do security team**

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Senha em plaintext encontrada em transcript | Security team + Incident Response | Imediatamente, severity critical |
| Novo campo sensível não coberto pela lista | Platform team | Via PR adicionando ao `SENSITIVE_FIELDS` |
| Masking quebrando funcionalidade legítima | Dev team + Security review | Após confirmação de que valor é necessário em plaintext |