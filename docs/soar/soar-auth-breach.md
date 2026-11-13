# SOAR: Auth Breach Response (soar-auth-breach)

## Trigger
- Detecção de uso anômalo de tokens (múltiplos tenants, geolocalização impossível)
- Relatório de credenciais vazadas (dark web, paste sites)
- Falha massiva de autenticação (>50% 401 em 5 min)
- Acesso não autorizado a recursos admin por role viewer/editor

## Severity
**CRITICAL** — acionar imediatamente

## Playbook Automatizado

### Fase 1: Contenção (< 2 min)
1. **Revogar tokens do usuário/tenant afetado**
   ```bash
   curl -X POST http://localhost:8080/api/v1/auth/revoke \
     -H "Authorization: Bearer $ADMIN_TOKEN" \
     -d '{"scope":"user","target":"<user_id>","reason":"breach_detected"}'
   ```
2. **Bloquear IP suspeito** (se aplicável)
   ```bash
   curl -X POST http://localhost:8080/api/v1/auth/blocklist/ip \
     -d '{"ip":"x.x.x.x","ttl_secs":3600,"reason":"breach"}'
   ```
3. **Desconectar sessões ativas via WebSocket**
   - PresenceTracker: forçar leave de todos os client_ids do user_id afetado

### Fase 2: Investigação (< 15 min)
1. Consultar audit log para identificar escopo do acesso
   ```bash
   curl "http://localhost:8080/api/v1/audit?user_id=<user_id>&since=1h"
   ```
2. Verificar se outros tenants foram acessados
3. Identificar dados/skills modificados ou exfiltrados

### Fase 3: Recuperação (< 30 min)
1. Rotacionar `ZYLOS_AUTH_SECRET` se comprometimento sistêmico
2. Forçar re-login de todos os usuários do tenant
3. Restaurar skills/documentos de snapshot pré-breach
4. Notificar usuários afetados com instruções de troca de senha

### Fase 4: Post-mortem (< 24h)
1. Documentar timeline do incidente
2. Atualizar regras de detecção no sentinel
3. Revisar permissões e TTLs
4. Publicar relatório interno

## Escalação
- Se múltiplos tenants afetados → CISO + Legal
- Se dados sensíveis exfiltrados → DPO + notificação regulatória
- Se gateway indisponível → DevOps on-call

## Referências
- `pop-token-revoke`
- `rb-zylos-auth`
- `crates/zylos-auth/src/middleware.rs`