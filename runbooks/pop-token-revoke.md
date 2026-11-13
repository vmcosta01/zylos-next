# POP: Token Revoke (pop-token-revoke)

## Escopo
Revogar tokens JWT ativos para um usuário ou tenant comprometido.

## Gatilho
- Usuário reporta credenciais vazadas
- Auditoria detecta atividade suspeita
- Desligamento de funcionário
- Rotação de secret solicitada

## Procedimento

### 1. Identificar escopo da revogação
```bash
# Listar tokens ativos por usuário (se houver token store/audit log)
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8080/api/v1/auth/tokens?user_id=user-comprometido
```

### 2. Revogação imediata (blacklist)
Adicionar JTI ou user_id à blacklist no gateway:
```bash
curl -X POST http://localhost:8080/api/v1/auth/revoke \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id": "user-comprometido", "reason": "credential_leak"}'
```

### 3. Forçar logout no cockpit
Invalidar sessões ativas via WebSocket disconnect ou flag no presence tracker.

### 4. Rotacionar secret (se necessário)
```bash
export ZYLOS_AUTH_SECRET="nova-chave-secreta-min-32-bytes"
# Reiniciar gateway — todos os tokens antigos são invalidados automaticamente
```

### 5. Notificar usuários afetados
Enviar email/notificação solicitando re-login e troca de senha.

## Verificação
- Tentar usar token antigo → deve retornar 401
- Novo login gera token válido
- Audit log registra evento de revogação

## SLA
- Revogação blacklist: < 1 minuto
- Rotação de secret: < 5 minutos (restart do gateway)

## Referências
- `crates/zylos-auth/src/tokens.rs`
- `soar-auth-breach`
- `pb-auth-setup`