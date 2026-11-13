# Runbook: zylos-auth (rb-zylos-auth)

## Visão Geral
Operações diárias do subsistema de autenticação multi-tenant e CRDT collaboration.

## Health Checks

### Auth subsystem
```bash
curl -s http://localhost:8080/api/v1/auth/health | jq .
# Esperado: {"status":"ok","token_service":"ready","crdt_server":"ready","presence_tracker":"ready"}
```

### CRDT sync server
```bash
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8080/api/v1/crdt/documents | jq .
# Retorna lista de documentos ativos e contagem de subscribers
```

### Presence tracker
```bash
curl -s -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8080/api/v1/presence/active?doc_id=skill-abc | jq .
# Retorna usuários conectados ao documento
```

## Operações Comuns

### Listar tokens ativos por tenant
```bash
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  "http://localhost:8080/api/v1/auth/tokens?tenant_id=tenant-default"
```

### Adicionar usuário a tenant
```bash
curl -X POST http://localhost:8080/api/v1/auth/users \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"user_id":"new-user","tenant_id":"tenant-default","roles":["editor"],"name":"Novo Editor"}'
```

### Remover usuário
```bash
curl -X DELETE http://localhost:8080/api/v1/auth/users/new-user \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### Purgar presença stale
```bash
curl -X POST http://localhost:8080/api/v1/presence/prune \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"timeout_ms": 30000}'
```

## Alertas e Respostas

| Sintoma | Causa Provável | Ação |
|---------|---------------|------|
| 401 em todas as rotas | Secret ausente ou alterada | Verificar `ZYLOS_AUTH_SECRET`, restart gateway |
| Tokens expiram muito rápido | TTL configurado baixo | Ajustar `TokenConfig::default_ttl_secs` |
| CRDT updates não chegam | WebSocket desconectado | Verificar CORS WS, reconectar cliente |
| Presence não atualiza | Stale timeout muito curto | Aumentar timeout ou verificar heartbeat |
| Conflitos frequentes | Muitos editores simultâneos | Ver `pop-crdt-conflict-resolve` |

## Escalação
- Se auth falhar para todos os tenants → acionar `soar-auth-breach`
- Se CRDT perder dados → acionar `pop-crdt-conflict-resolve` + restaurar snapshot
- Se performance degradar com >50 documentos ativos → ver `pot-crdt-sync-tuning`

## Referências
- `crates/zylos-auth/README.md`
- `pb-auth-setup`
- `pop-token-revoke`
- `soar-auth-breach`