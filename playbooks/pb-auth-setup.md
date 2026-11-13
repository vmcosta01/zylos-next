# Playbook: Auth Setup (pb-auth-setup)

## Objetivo
Configurar autenticação multi-tenant JWT no zylos-gateway para um novo tenant.

## Pré-requisitos
- `ZYLOS_AUTH_SECRET` definido no ambiente (min 32 bytes)
- Gateway rodando com `zylos-auth` integrado
- Acesso admin ao cockpit ou CLI

## Passos

### 1. Verificar secret
```bash
echo $ZYLOS_AUTH_SECRET | wc -c
# Deve ser >= 32
```

### 2. Criar primeiro usuário admin via API
```bash
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "admin-001",
    "tenant_id": "tenant-default",
    "roles": ["admin"],
    "name": "Admin Inicial",
    "password_hash": "<bcrypt-hash>"
  }'
```

### 3. Validar token emitido
```bash
TOKEN=$(curl -s -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"user_id":"admin-001","password":"..."}' | jq -r .access_token)

curl -H "Authorization: Bearer $TOKEN" http://localhost:8080/api/v1/gateway/health
```

### 4. Adicionar membros ao tenant
Use o endpoint `/api/v1/auth/users` com role `editor` ou `viewer`.

## Rollback
- Remover variável `ZYLOS_AUTH_SECRET` → gateway rejeita todos os tokens
- Tokens já emitidos expiram naturalmente pelo TTL

## Referências
- `crates/zylos-auth/README.md`
- `pop-token-revoke`
- `soar-auth-breach`