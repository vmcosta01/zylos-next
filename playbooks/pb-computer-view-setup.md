# PB-005: Configuração do Computer View Panel

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-computer-control-emergency.md](./pop-computer-control-emergency.md)
**Related Runbooks:** [rb-zylos-computer-view.md](../runbooks/rb-zylos-computer-view.md)
**Related SOAR:** [soar-computer-control-breach.md](../docs/soar/soar-computer-control-breach.md)

## Objective

Configurar o sistema de computer view para que usuários possam visualizar e controlar remotamente o desktop/browser dos agentes em tempo real. Este playbook cobre provisionamento de sessões VNC, configuração de leases de controle, validação de fallback para screenshots e integração com o cockpit.

## Prerequisites

- Crate `zylos-computer-view` compilado e integrado ao gateway
- Endpoint `/api/v1/computer/sessions` acessível via localhost:3000
- Agente com desktop/browser disponível para conexão VNC (ou screenshot API)
- Cockpit servindo na porta 8080

## Steps

### 1. Verificar integridade do endpoint

```bash
curl -sf "http://localhost:3000/api/v1/computer/sessions" | jq 'length'
# Esperado: 0 ou array válido (não erro 500)
```

Se retornar erro, verificar se o router foi integrado no gateway `lib.rs`.

### 2. Criar sessão de computer view

```bash
curl -X POST http://localhost:3000/api/v1/computer/sessions \
 -H "Content-Type: application/json" \
 -d '{
 "bot_id": "bot-1",
 "vnc_endpoint": "ws://localhost:5900",
 "metadata": {"resolution": "1920x1080"}
 }'
```

Resposta esperada: objeto `ComputerSession` com `state: "connecting"` e `id` UUID.

### 3. Validar transição de estado

Após conexão VNC estabelecida (ou timeout):

```bash
# Atualizar estado para active (normalmente feito pelo proxy VNC)
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/state \
 -H "Content-Type: application/json" \
 -d '{"state": "active"}'

# Verificar
curl -s "http://localhost:3000/api/v1/computer/sessions" | jq '.[] | select(.id == "{session_id}") | .state'
# Esperado: "active"
```

### 4. Testar take/release control

```bash
# Usuário assume controle (lease de 10 min)
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/take-control \
 -H "Content-Type: application/json" \
 -d '{"user_id": "user-1", "duration_minutes": 10, "reason": "Debugging UI"}'

# Verificar estado mudou para user_control
curl -s "http://localhost:3000/api/v1/computer/sessions/{session_id}" | jq '.state'
# Esperado: "user_control"

# Usuário devolve controle
curl -X POST http://localhost:3000/api/v1/computer/sessions/{session_id}/release-control \
 -H "Content-Type: application/json" \
 -d '{"user_id": "user-1"}'

# Verificar estado voltou para active
curl -s "http://localhost:3000/api/v1/computer/sessions/{session_id}" | jq '.state'
# Esperado: "active"
```

### 5. Testar fallback screenshot

Quando VNC está indisponível:

```bash
curl -s "http://localhost:3000/api/v1/computer/sessions/{session_id}/screenshot" | jq '.format'
# Esperado: "png" (placeholder 1x1 pixel)
```

Em produção, o proxy VNC deve fornecer screenshots reais quando o stream falha.

### 6. Configurar expiry automático de leases

No gateway, adicionar background task periódica:

```rust
tokio::spawn(async move {
 loop {
 let expired = session_manager.expire_leases();
 if expired > 0 {
 tracing::info!(expired = expired, "Control leases expired");
 }
 tokio::time::sleep(Duration::from_secs(30)).await;
 }
});
```

Intervalo recomendado: 30 segundos. Lease padrão: 5 minutos.

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| VNC indisponível permanentemente | Usar screenshot polling (1-5 FPS) | Latência aceitável para monitoramento passivo |
| Lease expira durante operação crítica | Renovar automaticamente se usuário ainda ativo | Heartbeat do cockpit a cada 60s |
| Múltiplos usuários querem controle | Fila de espera com notificação | Primeiro a pedir tem prioridade após expiry |
| Sessão órfã (bot desconectado) | Cleanup automático após 5 min sem heartbeat | Background task periódica |

## Rollback Procedure

1. Encerrar todas as sessões ativas: iterar sobre `/computer/sessions` e `DELETE` cada uma
2. Desativar router no gateway se necessário (comentar `.merge(computer_router)`)
3. Reiniciar gateway
4. Investigar causa raiz antes de reativar

## SLA/RTO/RPO

- **SLA:** 99% disponibilidade (não crítico — fallback é screenshot estático)
- **RTO:** 2 minutos (restart do gateway + reconexão VNC)
- **RPO:** 0 (sessões são efêmeras, sem dados persistentes críticos)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.4
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenMausBot CanvasComputers.tsx](https://github.com/milind-soni/OpenMausBot/blob/main/src/components/CanvasComputers.tsx)
- [OpenDots ComputerPanel.tsx](https://github.com/CopilotKit/OpenDots/blob/main/src/client/ComputerPanel.tsx)