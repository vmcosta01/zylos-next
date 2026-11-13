# PB-010: Configuração do WebRTC Relay para Phone Relay

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-12
**Related POPs:** [pop-relay-emergency-stop.md](./pop-relay-emergency-stop.md)
**Related Runbooks:** [rb-webrtc-relay.md](../runbooks/rb-webrtc-relay.md)
**Related SOAR:** [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md)

## Objective

Configurar o sistema de WebRTC relay via WebSocket para streaming de tela e controle remoto de dispositivos móveis pareados. Este playbook cobre a ativação do endpoint WebSocket no gateway, validação do protocolo de frames base64, configuração de indicadores de qualidade (FPS/latência/bitrate) e integração com o cockpit phone_relay_panel.

## Prerequisites

- Crate `zylos-gateway` compilado com feature `ws` do axum
- Dependência `futures-util` adicionada ao Cargo.toml do gateway
- Módulo `webrtc_relay` registrado em `lib.rs` e integrado ao router principal
- Cockpit servindo na porta 8080 com `phone_relay_panel` atualizado
- Sessão de computer-view ativa (endpoint REST `/api/v1/computer-view/sessions`)

## Steps

### 1. Verificar compilação do gateway com WebRTC relay

```bash
cargo check -p zylos-gateway
# Esperado: exit code 0, sem erros
```

Se falhar com erro de import `futures_util`, verificar que `futures-util = "0.3"` está em `apps/zylos-gateway/Cargo.toml`.

### 2. Validar endpoint WebSocket disponível

Iniciar o gateway e testar upgrade:

```bash
# Em background: cargo run -p zylos-gateway
# Testar upgrade WebSocket (deve retornar 101 Switching Protocols)
curl -i -N \
  -H "Connection: Upgrade" \
  -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  "http://localhost:3000/api/v1/ws/computer-view/test-session-001"
# Esperado: HTTP/1.1 101 Switching Protocols
```

### 3. Testar protocolo de mensagens

Conectar via `websocat` ou script Python e validar troca de mensagens:

```bash
# Enviar ping
echo '{"type":"ping","ts":1700000000000}' | websocat ws://localhost:3000/api/v1/ws/computer-view/test-session-001

# Esperado: resposta quality com latency_ms calculada
# {"type":"quality","fps":0.0,"latency_ms":42,"bitrate_kbps":0}
```

### 4. Validar broadcast multi-cliente

Abrir duas conexões WebSocket simultâneas para a mesma sessão. Enviar frame via API interna (ou producer mock) e verificar que ambas recebem o mesmo frame com `seq` incremental.

### 5. Testar controle remoto básico

```bash
# Enviar click via WebSocket
echo '{"type":"click","x":500,"y":300,"button":"left"}' | websocat ws://localhost:3000/api/v1/ws/computer-view/test-session-001

# Verificar nos logs do gateway:
# INFO Remote click received session_id=test-session-001 x=500 y=300 button=left
```

### 6. Integrar com cockpit phone_relay_panel

No cockpit, ao clicar "Iniciar Relay" em um dispositivo online:
1. WebSocket conecta a `ws://localhost:3000/api/v1/ws/computer-view/{device_id}`
2. Canvas renderiza frames base64 recebidos
3. Clicks/touches no canvas enviam eventos de controle via WS
4. Indicadores de FPS/latência atualizam em tempo real

Verificar no browser console que não há erros de conexão WS.

### 7. Configurar cleanup automático de sessões

O `WebRtcRelayState` remove canais quando não há consumidores. Validar:

```bash
# Conectar e desconectar rapidamente
websocat ws://localhost:3000/api/v1/ws/computer-view/cleanup-test
# Ctrl+C para desconectar

# Verificar nos logs:
# INFO WebRTC relay channel cleaned up session_id=cleanup-test
```

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Frame > 2MB base64 | Rejeitar com error frame | Prevenir OOM no gateway |
| Cliente lagged (>64 frames) | Log warning, continuar | Broadcast channel capacity = 64 |
| Latência > 500ms sustentada | Alerta SOAR | soar-relay-latency-alert.md |
| Múltiplos produtores para mesma sessão | Último writer vence | DashMap entry única por session_id |
| WebSocket fecha sem Close frame | Cleanup via select! timeout | Tokio select on send/recv tasks |

## Rollback Procedure

1. Comentar `.merge(webrtc_relay)` em `lib.rs` linha do `build_router`
2. Reiniciar gateway: `cargo run -p zylos-gateway`
3. Cockpit continua funcional sem relay (fallback para placeholder estático)
4. Investigar causa raiz antes de reativar

## SLA/RTO/RPO

- **SLA:** 95% disponibilidade (feature experimental Fase 10.3)
- **RTO:** 1 minuto (restart do gateway)
- **RPO:** 0 (sessões são efêmeras, sem dados persistentes)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §10.3
- [pb-computer-view-setup.md](./pb-computer-view-setup.md) — setup do endpoint REST subjacente
- [AXUM WebSocket docs](https://docs.rs/axum/latest/axum/extract/ws/index.html)