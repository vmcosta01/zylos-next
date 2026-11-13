# SOAR: Relay Latency Alert Response

**Trigger:** Latência do WebRTC relay excede threshold sustentado (>500ms p99 por >30s) ou FPS cai abaixo de 5 por >10s
**Severity:** Warning (degradação de UX; não compromete segurança)
**Auto-Remediation:** Partial (reduz qualidade automaticamente; não resolve causa raiz de rede/CPU)

## Detection

Sinais de degradação no relay:

1. **Latência p99 > 500ms:** Média móvel de `latency_ms` (via ping/pong) excede 500ms por 30 segundos consecutivos
2. **FPS < 5:** Contador de frames recebidos no cockpit < 5/s por 10 segundos
3. **Broadcast lagged:** `RecvError::Lagged` nos logs do gateway > 5/minuto
4. **Frame size spike:** Frames base64 > 2MB aparecem repetidamente (possível resolução excessiva)
5. **Memory pressure:** RSS do gateway cresce > 100MB/min durante sessão ativa

### Queries de detecção

```rust
// Métricas expostas via /api/v1/relay/metrics (futuro)
// Ou parsing de logs estruturados:
// grep '"type":"quality"' gateway.log | jq '.latency_ms' | datamash mean 1
// Se mean > 500 → alert
```

## Containment

1. **Reduzir qualidade automaticamente** (adaptive bitrate):
   ```rust
   // No producer (agente), ao receber quality feedback com latency_ms > 500:
   if quality.latency_ms > 500 {
       reduce_resolution(640, 360);
       reduce_fps(5);
   }
   ```

2. **Notificar cockpit** para exibir banner de degradação:
   ```json
   {"type":"quality","fps":3.2,"latency_ms":850,"bitrate_kbps":400,"degraded":true}
   ```

3. **Log warning estruturado** para correlação posterior:
   ```
   WARN relay_degraded session_id=X latency_ms=850 fps=3.2 action=quality_reduced
   ```

4. **Se latência > 2s sustentada:** acionar POP de emergency stop:
   - Ver [pop-relay-emergency-stop.md](../../playbooks/pop-relay-emergency-stop.md)

## Investigation

1. **Identificar bottleneck:**
   - CPU do gateway: `top -p $(pgrep zylos-gateway)` — se >80%, serialization/base64 é gargalo
   - Rede: `iperf3 -c localhost -p 3000` entre gateway e cockpit — se <10Mbps, rede é gargalo
   - Producer: verificar tempo de captura+encode no agente (`tracing::info!` com duração)

2. **Verificar tamanho de frames:**
   ```bash
   websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | \
     jq -r 'select(.type=="frame") | .data | length' | head -20
   # Se > 1_500_000 chars consistentemente: reduzir resolução no agente
   ```

3. **Correlacionar com outras métricas:**
   - Outros endpoints do gateway lentos? → problema sistêmico (CPU/memória)
   - Apenas esta sessão lenta? → problema isolado (rede do cliente, agente específico)
   - Pico coincide com deploy/restart? → possível regressão

4. **Classificar causa raiz:**
   - **Resolução excessiva:** Agente enviando 4K → configurar max 1280x720
   - **Encode lento:** Screenshot + base64 > 100ms/frame → usar JPEG em vez de PNG
   - **Broadcast capacity:** Lagged errors frequentes → aumentar channel size
   - **Rede congestionada:** Latência alta mas CPU baixa → QoS ou rede dedicada
   - **GC pressure:** Pausas de GC causando spikes → profile com `tracing-alloc`

## Remediation

### Automática
- Adaptive quality reduz resolução/FPS quando latência sobe
- Banner de degradação exibido no cockpit
- Warning log emitido para observabilidade

### Manual (após investigação)
1. **Se resolução excessiva:** Atualizar config do agente para max 1280x720, JPEG quality 70
2. **Se encode lento:** Implementar hardware-accelerated screenshot (DXGI on Windows, XShmGetImage on Linux)
3. **Se broadcast capacity:** Aumentar `broadcast::channel(128)` em `webrtc_relay.rs`
4. **Se rede:** Mover gateway para mesma rede/VPC do cockpit, ou usar CDN WebSocket
5. **Se GC:** Pool de buffers reutilizáveis para frames, evitar alocação por frame

## Recovery

1. Após causa raiz resolvida, restaurar qualidade original:
   ```json
   {"type":"quality","fps":15.0,"latency_ms":45,"bitrate_kbps":1200,"degraded":false}
   ```
2. Remover banner de degradação do cockpit
3. Monitorar por 15 min para confirmar estabilidade
4. Atualizar thresholds se falso positivo recorrente

## Post-Incident

| Item | Ação |
|------|------|
| Thresholds | Ajustar se alert foi falso positivo ou tardio |
| Adaptive quality | Implementar se não existia (feedback loop) |
| Metrics | Adicionar histogram de frame_size_bytes |
| Docs | Atualizar `pot-relay-quality-tuning.md` com novos parâmetros |
| Tests | Adicionar load test simulando N clientes simultâneos |

## Bound POPs

- [pop-relay-emergency-stop.md](../../playbooks/pop-relay-emergency-stop.md) — parada total quando degradação é crítica

## Bound Runbooks

- [rb-webrtc-relay.md](../../runbooks/rb-webrtc-relay.md) — troubleshooting detalhado

## Metrics

| Métrica | Target | Alert Threshold |
|---------|--------|-----------------|
| Latency p99 | < 200ms | > 500ms por 30s |
| FPS | > 10 | < 5 por 10s |
| Frame size p99 | < 500KB | > 2MB |
| Lagged clients | 0 | > 5/min |
| Degradation events | 0 | Qualquer ocorrência requer post-mortem |