# POP-012: Parada Emergencial do WebRTC Relay

**Version:** 1.0.0
**Playbook:** [pb-webrtc-setup.md](./pb-webrtc-setup.md)
**Last Updated:** 2026-11-12
**Related SOAR:** [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md)

## Purpose

Procedimento operacional para interromper imediatamente o relay de tela quando comportamento anômalo é detectado — seja por latência excessiva, conteúdo sensível exposto, controle remoto não autorizado, ou instabilidade do gateway. Este POP cobre desconexão forçada de sessões WebSocket, desativação temporária do endpoint e cleanup de estado.

## Trigger

- Latência sustentada > 2s (alerta SOAR soar-relay-latency-alert)
- Conteúdo sensível visível no stream de tela (PII, credenciais)
- Controle remoto sendo exercido por usuário não autorizado
- Gateway com consumo de memória/CPU anômalo causado pelo relay
- Usuário reporta que relay está travado ou exibindo artefatos corrompidos
- Alerta de segurança indicando possível session hijacking via WebSocket

## Steps

### 1. Desconectar sessão específica imediatamente

```bash
# Encerrar sessão via REST API (fecha WebSocket associado)
curl -X DELETE "http://localhost:3000/api/v1/computer-view/sessions/{session_id}"
# Esperado: 204 No Content
```

O `WebRtcRelayState::cleanup_session` remove o canal broadcast automaticamente quando a sessão REST é terminada.

### 2. Se endpoint WebSocket não responde

Forçar restart do gateway:

```bash
# Identificar PID do gateway
pgrep -f zylos-gateway

# Graceful shutdown via SIGTERM
kill -TERM $(pgrep -f zylos-gateway)

# Aguardar shutdown timeout (configurado em config.toml)
sleep 5

# Verificar que processo parou
! pgrep -f zylos-gateway && echo "stopped" || echo "still running"

# Se ainda rodando, SIGKILL
pkill -9 -f zylos-gateway || true
```

### 3. Desativar relay temporariamente sem restart

Se o problema é isolado ao relay mas o gateway precisa continuar servindo outras APIs:

```bash
# Editar lib.rs e comentar a linha .merge(webrtc_relay)
sed -i 's/\.merge(webrtc_relay)/\/\/ .merge(webrtc_relay) \/\/ DISABLED EMERGENCY/' \
 apps/zylos-gateway/src/lib.rs

# Recompilar e reiniciar
cargo run -p zylos-gateway
```

Alternativamente, usar feature flag se implementado:
```bash
ZYLOS_WEBRTC_RELAY_DISABLED=1 cargo run -p zylos-gateway
```

### 4. Limpar canais broadcast órfãos

Se sessões WebSocket ficaram presas após crash:

```bash
# O cleanup automático roda na desconexão, mas após crash pode haver leaks
# Reiniciar gateway limpa todo o estado in-memory
cargo run -p zylos-gateway

# Verificar nos logs que canais foram recriados apenas sob demanda
grep "WebRTC relay channel created" /var/log/zylos-gateway.log | tail -5
```

### 5. Notificar usuários afetados

Se o cockpit estava conectado à sessão interrompida:

```bash
# Publicar evento no EventBus para notificar cockpit
curl -X POST http://localhost:3000/api/v1/events \
 -H "Content-Type: application/json" \
 -d '{
 "type": "relay_emergency_stop",
 "session_id": "{session_id}",
 "reason": "latency_threshold_exceeded",
 "timestamp": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
 }'
```

O cockpit exibirá banner de reconexão automática ou mensagem de erro.

### 6. Documentar incidente

Adicionar entrada no log de incidentes:

```markdown
### YYYY-MM-DD HH:MM UTC
- **Incident:** Emergency stop do WebRTC relay
- **Session:** {session_id}
- **Trigger:** {latência/conteúdo_sensível/controle_não_autorizado/crash}
- **Duration:** {tempo de indisponibilidade}
- **Resolution:** {sessão encerrada/gateway reiniciado/relay desativado}
- **Follow-up:** {investigar causa raiz/atualizar thresholds/corriger bug}
```

## Verification

- Sessão WebSocket desconectada (client recebe Close frame ou connection reset)
- Canal broadcast removido do `WebRtcRelayState` (log: "channel cleaned up")
- Endpoint REST `/computer-view/sessions/{id}` retorna 404 após DELETE
- Cockpit exibe estado "desconectado" no phone_relay_panel
- Métricas de latência/FPS param de atualizar

## Rollback

Parada emergencial é reversível:

1. Recriar sessão via REST API se foi encerrada
2. Reativar `.merge(webrtc_relay)` se foi comentado
3. Reiniciar gateway se foi parado
4. Usuário reconecta via cockpit clicando "Iniciar Relay" novamente

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Relay não para após DELETE da sessão | Dev team | Imediatamente — possível leak de recursos |
| Gateway crash recursivo ao ativar relay | Platform team + SRE | Após 2º crash em <1h |
| Conteúdo sensível foi exposto a cliente não autorizado | Security team | Imediatamente, severity critical |
| Latência alta persiste após restart | Dev team | Investigar bottleneck de CPU/rede |
| Múltiplas sessões afetadas simultaneamente | Tech lead | Possível ataque ou falha sistêmica |

## References

- [pb-webrtc-setup.md](./pb-webrtc-setup.md) §Rollback Procedure
- [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md) — detecção automática
- [pop-computer-control-emergency.md](./pop-computer-control-emergency.md) — procedimento análogo para VNC