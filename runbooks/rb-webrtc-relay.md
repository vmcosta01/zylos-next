# RB-012: WebRTC Relay — Troubleshooting e Monitoramento

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-12
**Related Playbook:** [pb-webrtc-setup.md](../playbooks/pb-webrtc-setup.md)
**Related POP:** [pop-relay-emergency-stop.md](../playbooks/pop-relay-emergency-stop.md)
**Related SOAR:** [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md)

## Scope

Este runbook cobre diagnóstico, troubleshooting e monitoramento contínuo do sistema de WebRTC relay via WebSocket no zylos-gateway. Aplica-se quando o phone_relay_panel do cockpit falha em conectar, exibe frames corrompidos, tem latência excessiva, ou quando o gateway apresenta consumo anômalo de recursos relacionado ao relay.

## Architecture Overview

```
┌─────────────┐     WebSocket      ┌──────────────────┐    broadcast     ┌─────────────┐
│  Cockpit     │ ◄──────────────── │  zylos-gateway   │ ◄────────────── │  Agente      │
│  Canvas +    │   JSON frames     │  webrtc_relay.rs │   base64 PNG    │  Screenshot  │
│  Controls    │ ──────────────── ► │  WebRtcRelayState│                 │  Producer    │
└─────────────┘   click/touch/key  └──────────────────┘                 └─────────────┘
                                         │
                                    DashMap<session_id, broadcast::Sender>
```

### Componentes

| Componente | Arquivo | Responsabilidade |
|-----------|---------|-----------------|
| `webrtc_relay.rs` | `apps/zylos-gateway/src/webrtc_relay.rs` | Handler WS, broadcast, protocolo JSON |
| `WebRtcRelayState` | `apps/zylos-gateway/src/webrtc_relay.rs` | Estado compartilhado (DashMap de canais) |
| `phone_relay_panel.rs` | `apps/zylos-cockpit/src/components/phone_relay_panel.rs` | Canvas rendering, controle remoto, métricas |
| `computer_view_api.rs` | `apps/zylos-gateway/src/computer_view_api.rs` | REST API para sessões (subjacente ao relay) |

## Health Checks

### 1. Endpoint WebSocket responde a upgrade

```bash
curl -s -o /dev/null -w "%{http_code}" \
 -H "Connection: Upgrade" \
 -H "Upgrade: websocket" \
 -H "Sec-WebSocket-Version: 13" \
 -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
 "http://localhost:3000/api/v1/ws/computer-view/health-check"
# Esperado: 101 (Switching Protocols)
# Se 404: módulo não registrado no router
# Se 500: panic no handler — verificar logs
```

### 2. Broadcast channel ativo para sessão

```bash
# Verificar nos logs do gateway
grep "WebRTC relay channel created" /var/log/zylos-gateway.log | tail -5
grep "WebRTC relay channel cleaned up" /var/log/zylos-gateway.log | tail -5
```

Canais devem ser criados sob demanda e limpos quando não há consumidores.

### 3. Métricas de qualidade

Conectar via websocat e enviar ping:

```bash
echo '{"type":"ping","ts":'$(date +%s%3N)'}' | \
 websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id}
# Esperado: {"type":"quality","fps":N.N,"latency_ms":NN,"bitrate_kbps":NNN}
```

## Troubleshooting

### Sintoma: Cockpit não conecta ao relay (WS fecha imediatamente)

**Causas possíveis:**
1. Sessão não existe no computer-view REST API
2. CORS bloqueando upgrade WebSocket
3. Gateway sem feature `ws` do axum

**Diagnóstico:**
```bash
# 1. Verificar sessão existe
curl -s "http://localhost:3000/api/v1/computer-view/sessions/{session_id}" | jq '.session.state'
# Esperado: "active" ou "connecting"

# 2. Verificar CORS headers
curl -v -X OPTIONS "http://localhost:3000/api/v1/ws/computer-view/{session_id}" 2>&1 | grep -i "access-control"

# 3. Verificar compilação com ws
cargo check -p zylos-gateway 2>&1 | grep -i "websocket\|ws"
```

**Resolução:**
- Se sessão não existe: criar via `POST /api/v1/computer-view/sessions`
- Se CORS: verificar `local_guard::cors_origem_local` em `lib.rs`
- Se compilação: confirmar `axum = { features = ["ws"] }` no workspace Cargo.toml

### Sintoma: Frames chegam mas canvas não renderiza

**Causas possíveis:**
1. Base64 inválido ou formato não-PNG
2. Canvas dimensions não definidas
3. Erro JS no decode/render

**Diagnóstico:**
```bash
# Capturar frame raw via websocat
websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | head -1 | jq -r '.data' | base64 -d > /tmp/frame.png
file /tmp/frame.png
# Esperado: PNG image data

# Verificar console do browser (F12 → Console)
# Procurar por: "Failed to decode frame" ou "Canvas context null"
```

**Resolução:**
- Se base64 inválido: verificar producer no agente (deve usar `base64::engine::general_purpose::STANDARD`)
- Se canvas null: verificar que `<canvas>` tem `width` e `height` attributes setados
- Se erro JS: verificar `phone_relay_panel.rs` — função de renderização deve usar `create_image_bitmap` ou `draw_image`

### Sintoma: Latência > 500ms sustentada

**Causas possíveis:**
1. Producer enviando frames muito grandes (>2MB base64)
2. Broadcast channel lagged (capacity 64 insuficiente)
3. Rede entre gateway e cockpit congestionada
4. GC pressure no gateway (muitas alocações de String)

**Diagnóstico:**
```bash
# Verificar tamanho dos frames
websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | \
 jq -r '.data | length' | sort -n | tail -5
# Se > 2_000_000 chars: frames muito grandes

# Verificar warnings de lag nos logs
grep "Client lagged" /var/log/zylos-gateway.log | wc -l
# Se > 0: aumentar broadcast capacity ou reduzir FPS

# Verificar métricas de qualidade
echo '{"type":"ping","ts":'$(date +%s%3N)'}' | \
 websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | \
 jq '.latency_ms'
```

**Resolução:**
- Reduzir resolução do screenshot no agente (max 1280x720)
- Aumentar broadcast capacity: `broadcast::channel(128)` em `get_or_create_channel`
- Implementar adaptive quality: reduzir FPS quando latência sobe
- Verificar `pot-relay-quality-tuning.md` para parâmetros ótimos

### Sintoma: Controle remoto não funciona (clicks ignorados)

**Causas possíveis:**
1. Mensagem JSON malformada (campo faltando)
2. Coordenadas fora dos bounds do canvas
3. TODO não implementado no handler (encaminhamento para agente)

**Diagnóstico:**
```bash
# Enviar click válido e verificar log
echo '{"type":"click","x":100,"y":100,"button":"left"}' | \
 websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id}

# Verificar log
grep "Remote click received" /var/log/zylos-gateway.log | tail -1
# Se presente mas nada acontece: TODO não implementado
# Se ausente: JSON malformado ou WS não conectado
```

**Resolução:**
- Verificar schema `RelayClientMsg` em `webrtc_relay.rs`
- Implementar encaminhamento: chamar `computer_view_api::take_control` + enviar comando VNC/input
- Validar coordenadas no cockpit antes de enviar (clamp to canvas bounds)

### Sintoma: Memory leak no gateway (RSS cresce continuamente)

**Causas possíveis:**
1. Canais broadcast não sendo limpos após desconexão
2. Strings base64 acumuladas em memória
3. Tasks spawnadas não terminam na desconexão

**Diagnóstico:**
```bash
# Monitorar RSS do gateway
watch -n 5 'ps -o rss= -p $(pgrep -f zylos-gateway)'

# Verificar canais ativos
grep "relay channel" /var/log/zylos-gateway.log | grep -c "created"
grep "relay channel" /var/log/zylos-gateway.log | grep -c "cleaned up"
# Diferença deve ser ≈ número de sessões ativas

# Verificar tasks órfãs
grep "WebSocket relay connection closed" /var/log/zylos-gateway.log | wc -l
# Deve igualar número de conexões estabelecidas
```

**Resolução:**
- Verificar `cleanup_session` é chamado no fim de `handle_ws_socket`
- Usar `tokio::select!` para garantir cleanup em qualquer path de saída
- Considerar `Arc<str>` ou bytes pool para frames grandes

## Monitoring Dashboard

Métricas recomendadas para observabilidade:

| Métrica | Fonte | Alert Threshold |
|---------|-------|----------------|
| `relay_connections_active` | Gauge (DashMap len) | > 100 |
| `relay_frames_per_second` | Counter / window | < 5 ou > 60 |
| `relay_latency_ms_p99` | Histogram (ping RTT) | > 500ms |
| `relay_bytes_sent_total` | Counter | Rate > 50MB/s |
| `relay_lagged_clients_total` | Counter | > 0/min |
| `relay_errors_total` | Counter (serialize fail) | > 0/min |

## Maintenance

### Limpeza periódica de sessões órfãs

Adicionar background task no gateway:

```rust
tokio::spawn(async move {
    loop {
        // Iterar sobre sessões e remover as sem consumidores
        let orphaned: Vec<String> = state.sessions.iter()
            .filter(|entry| entry.value().receiver_count() == 0)
            .map(|entry| entry.key().clone())
            .collect();
        for session_id in orphaned {
            state.cleanup_session(&session_id);
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
});
```

### Rotação de logs

Logs de relay podem ser verbosos. Configurar rotação:

```toml
# tracing-subscriber config
[log_rotation]
max_size_mb = 100
max_files = 5
compress = true
```

## References

- [pb-webrtc-setup.md](../playbooks/pb-webrtc-setup.md) — setup inicial
- [pop-relay-emergency-stop.md](../playbooks/pop-relay-emergency-stop.md) — parada emergencial
- [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md) — alerta automático
- [pot-relay-quality-tuning.md](../playbooks/pot-relay-quality-tuning.md) — tuning de qualidade
- [AXUM WebSocket docs](https://docs.rs/axum/latest/axum/extract/ws/index.html)
- [tokio broadcast docs](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)