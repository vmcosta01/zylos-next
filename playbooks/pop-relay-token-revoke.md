# POP-Relay-Token-Revoke — Revogação de Tokens do Relay Server (T8.6.3)

**Versão:** 1.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `pop-author`
**Módulo:** `zylos-desktop::relay_server`

---

## 1. Objetivo

Procedimento operacional para revogar tokens de dispositivos mobile pareados com o relay server, seja por perda de dispositivo, comprometimento ou descomissionamento.

## 2. Pré-requisitos

- Acesso ao desktop app ou gateway API (`http://localhost:9877`)
- Credenciais admin do relay server
- Identificação do dispositivo/token a revogar

## 3. Procedimento

### 3.1 Listar Dispositivos Ativos

```bash
curl http://localhost:9877/api/v1/relay/devices \
  -H "Authorization: Bearer $ADMIN_TOKEN" | jq '.[] | {device_name, created_at, last_used_at}'
```

### 3.2 Revogar Token Específico

```bash
curl -X DELETE http://localhost:9877/api/v1/relay/tokens/{token_id} \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### 3.3 Purgar Tokens Expirados

```bash
curl -X POST http://localhost:9877/api/v1/relay/tokens/purge \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

Resposta esperada: `{"purged_count": 3}`

### 3.4 Verificar Rate Limit Status

```bash
curl http://localhost:9877/api/v1/relay/rate-limits \
  -H "Authorization: Bearer $ADMIN_TOKEN" | jq '.[] | select(.blocked == true)'
```

## 4. Validação

Após revogação, confirmar que o token não é mais aceito:

```bash
curl http://localhost:9877/api/v1/relay/status \
  -H "Authorization: Bearer $REVOKED_TOKEN"
# Esperado: 401 Unauthorized
```

## 5. Rollback

Se token foi revogado acidentalmente:

1. Re-parear dispositivo: `POST /api/v1/relay/pair {"device_name": "..."}`
2. Distribuir novo token ao usuário do dispositivo
3. Logar incidente em `docs/incidents/` se revogação foi maliciosa

## 6. Segurança

- Tokens revogados **não podem ser reutilizados** — novo pairing gera token diferente
- Purge automático roda a cada 24h via background task
- Rate limit por IP previne brute-force de tokens

## 7. Cross-References

- **Runbook:** `runbooks/zylos-desktop.md` §Relay Server
- **Handoff:** `docs/handoffs/HANDOFF_FASE8.md` §2.8
- **Código:** `crates/zylos-desktop/src/relay_server.rs`
- **POP relacionado:** `playbooks/pop-guardrail-rule-update.md`

## 8. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-11 | Implementation Agent | Criação inicial |