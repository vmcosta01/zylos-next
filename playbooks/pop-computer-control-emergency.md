# POP-007: Controle Emergencial de Computer View

**Version:** 1.0.0
**Playbook:** [pb-computer-view-setup.md](./pb-computer-view-setup.md)
**Last Updated:** 2026-11-11
**Related SOAR:** [soar-computer-control-breach.md](../docs/soar/soar-computer-control-breach.md)

## Purpose

Procedimento operacional para quando o usuário precisa assumir controle emergencial do desktop do agente — seja para interromper ação perigosa, corrigir erro visual, ou investigar comportamento suspeito. Este POP cobre take control imediato, extensão de lease e release forçado.

## Trigger

- Agente executando ação não autorizada no desktop (navegando para site suspeito, preenchendo formulário incorreto)
- Usuário precisa interagir manualmente com UI que o agente não consegue operar
- Investigação de bug requer inspeção visual do estado do desktop
- Alerta SOAR de comportamento anômalo do agente no computer view

## Steps

### 1. Assumir controle imediatamente

```bash
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/take-control \
 -H "Content-Type: application/json" \
 -d '{
 "user_id": "admin",
 "duration_minutes": 30,
 "reason": "EMERGENCY: Agent performing unauthorized action"
 }'
```

Respostas esperadas:
- `200`: Controle assumido, `state` mudou para `user_control`
- `409`: Outro usuário já tem controle ativo → verificar `control_lease.user_id` e `expires_at`
- `400`: Sessão não está ativa → verificar estado atual

### 2. Se outro usuário tem controle e não responde

Forçar release via API administrativa (requer permissão admin):

```bash
# Verificar quem tem controle
curl -s "http://localhost:3000/api/v1/computer/sessions/{session_id}" | jq '.control_lease'

# Release forçado (admin only)
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/release-control \
 -H "Content-Type: application/json" \
 -d '{"user_id": "admin", "force": true}'
```

### 3. Estender lease se necessário

Se a operação emergencial requer mais tempo que o lease original:

```bash
# Renovar por mais 30 minutos
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/take-control \
 -H "Content-Type: application/json" \
 -d '{
 "user_id": "admin",
 "duration_minutes": 30,
 "reason": "Extension: still investigating"
 }'
```

O sistema substitui o lease anterior pelo novo com duração estendida.

### 4. Devolver controle ao agente

Após resolver a situação:

```bash
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/release-control \
 -H "Content-Type: application/json" \
 -d '{"user_id": "admin"}'
```

Verificar que `state` voltou para `active` e `control_lease` é `null`.

### 5. Se sessão está travada ou irresponsiva

Encerrar e recriar:

```bash
# Encerrar sessão problemática
curl -X DELETE http://localhost:3000/api/v1/computer/sessions/{session_id}

# Criar nova sessão
curl -X POST http://localhost:3000/api/v1/computer/sessions \
 -H "Content-Type: application/json" \
 -d '{"bot_id": "bot-1", "vnc_endpoint": "ws://localhost:5900"}'
```

### 6. Documentar incidente

Adicionar entrada no `TRACKRECORD_FASE9.md`:
```markdown
### YYYY-MM-DD
- **Incident:** Controle emergencial de computer view para bot {bot_id}
- **Cause:** {ação não autorizada / bug / investigação}
- **Duration:** {tempo de controle humano}
- **Resolution:** {agente corrigido / sessão recriada / regra adicionada}
```

## Verification

- Estado da sessão mostra `user_control` após take-control
- Cockpit exibe stream VNC com indicador de controle humano
- Após release, estado volta para `active` e agente retoma operação
- Lease expiry automático funciona (testar com duration_minutes=1)

## Rollback

Controle emergencial é reversível por design:
- Release devolve controle ao agente imediatamente
- Se agente não retoma operação após release, encerrar sessão e recriar
- Se problema persiste, desativar computer view temporariamente via config do gateway

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Agente executa ação perigosa apesar do controle humano | Security team + Incident Response | Imediatamente, severity critical |
| Sessão não responde a take/release control | Platform team | Após 3 tentativas falhas |
| VNC stream congelado mas sessão mostra "active" | Dev team | Bug no proxy VNC |
| Lease não expira automaticamente | Dev team | Background task pode ter parado |