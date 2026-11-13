# POT-005: WebRTC Relay Quality Tuning

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-12
**Related Playbook:** [pb-webrtc-setup.md](./pb-webrtc-setup.md)
**Related Runbook:** [rb-webrtc-relay.md](../runbooks/rb-webrtc-relay.md)
**Related SOAR:** [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md)

## Purpose

Guia de tuning para otimizar qualidade, latência e consumo de recursos do WebRTC relay. Cobre parâmetros de captura de tela no agente, configuração do broadcast channel no gateway, e adaptive quality no cockpit. Use este documento quando o relay funciona mas não atinge os targets de FPS/latência definidos no SOAR.

## Targets de Qualidade

| Parâmetro | Target | Acceptable | Critical |
|-----------|--------|------------|----------|
| Latência p99 | < 200ms | < 500ms | > 2s |
| FPS | 10-15 | 5-10 | < 5 |
| Frame size p99 | < 500KB | < 2MB | > 5MB |
| Memory/sessão | < 50MB | < 200MB | > 1GB |
| CPU overhead | < 10% | < 30% | > 80% |

## Parâmetros de Captura (Agente)

### Resolução

```toml
# Config do agente (zylos-computer-view ou equivalente)
[relay.capture]
max_width = 1280
max_height = 720
# Reduzir para 640x360 se latência > 500ms sustentada
```

**Impacto:**
- 1920x1080 PNG ≈ 2-4MB base64 → latência alta, bandwidth alto
- 1280x720 JPEG q70 ≈ 200-400KB base64 → bom balanço
- 640x360 JPEG q50 ≈ 50-100KB base64 → baixa latência, qualidade reduzida

### Formato de Imagem

| Formato | Tamanho típico | Encode time | Qualidade | Recomendação |
|---------|---------------|-------------|-----------|--------------|
| PNG | 2-4MB | 50-100ms | Lossless | ❌ Muito grande para relay |
| JPEG q90 | 500KB-1MB | 20-40ms | Alta | ⚠️ Apenas se rede > 100Mbps |
| JPEG q70 | 200-400KB | 15-30ms | Boa | ✅ Default recomendado |
| JPEG q50 | 50-150KB | 10-20ms | Aceitável | ✅ Para mobile/low-bandwidth |
| WebP q70 | 150-300KB | 30-60ms | Boa | ⚠️ Decode mais lento no browser |

### Frame Rate

```rust
// No producer loop do agente
const TARGET_FPS: u32 = 10;
const MIN_FRAME_INTERVAL_MS: u64 = 1000 / TARGET_FPS as u64;

loop {
    let start = Instant::now();
    let frame = capture_screenshot()?;
    let encoded = encode_jpeg(&frame, 70)?;
    let base64 = BASE64_STANDARD.encode(&encoded);
    
    tx.send(RelayServerMsg::Frame {
        data: base64,
        ts: now_millis(),
        seq: next_seq(),
    })?;
    
    // Throttle to target FPS
    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed < MIN_FRAME_INTERVAL_MS {
        tokio::time::sleep(Duration::from_millis(
            MIN_FRAME_INTERVAL_MS - elapsed
        )).await;
    }
}
```

**Adaptive FPS:** Reduzir automaticamente quando feedback de qualidade indica degradação:

```rust
if last_quality.latency_ms > 500 {
    target_fps = max(3, target_fps - 2);
} else if last_quality.latency_ms < 200 && target_fps < 15 {
    target_fps += 1;
}
```

## Parâmetros do Gateway

### Broadcast Channel Capacity

```rust
// Em webrtc_relay.rs, get_or_create_channel()
// Default: 64 frames buffer
// Aumentar se "Client lagged" warnings frequentes
let (tx, _) = broadcast::channel(128); // 128 para high-FPS sessions
```

**Trade-off:**
- Capacity maior = mais memória por sessão (128 × frame_size ≈ 50MB com JPEG q70)
- Capacity menor = mais lagged errors mas menos memória
- Recomendação: 64 para uso normal, 128 para sessões de debugging ativo

### Message Serialization

```rust
// Evitar alocação por frame com buffer reutilizável
thread_local! {
    static SERDE_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(512_000));
}

fn serialize_frame(msg: &RelayServerMsg) -> Result<String, serde_json::Error> {
    SERDE_BUF.with(|buf| {
        let mut b = buf.borrow_mut();
        b.clear();
        serde_json::to_writer(&mut *b, msg)?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    })
}
```

**Impacto:** Reduz GC pressure em ~30% sob load alto.

## Parâmetros do Cockpit

### Canvas Rendering

```typescript
// Em phone_relay_panel.rs (via wasm-bindgen)
// Usar createImageBitmap para decode assíncrono
async function renderFrame(base64Data: string, canvas: HTMLCanvasElement) {
    const binary = atob(base64Data);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    
    const blob = new Blob([bytes], { type: 'image/jpeg' });
    const bitmap = await createImageBitmap(blob);
    
    const ctx = canvas.getContext('2d');
    ctx.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    bitmap.close(); // Liberar memória
}
```

**Evitar:** `new Image()` + `onload` — bloqueia main thread e causa jank.

### Indicadores de Qualidade

```rust
// Em phone_relay_panel.rs
#[derive(Clone, PartialEq)]
struct QualityMetrics {
    fps: f64,
    latency_ms: u64,
    bitrate_kbps: u64,
    degraded: bool,
}

// Atualizar via WS message handler
Effect::new(move |_| {
    let metrics = quality_signal;
    spawn_local(async move {
        while let Some(msg) = ws.next().await {
            if let RelayServerMsg::Quality { fps, latency_ms, .. } = msg {
                metrics.set(QualityMetrics {
                    fps,
                    latency_ms,
                    degraded: latency_ms > 500 || fps < 5.0,
                    ..metrics.get()
                });
            }
        }
    });
});
```

**UI:** Exibir badge vermelho quando `degraded == true`, verde quando dentro dos targets.

## Tuning Scenarios

### Cenário 1: Mobile com 4G (bandwidth limitado)

```toml
[relay.capture]
max_width = 640
max_height = 360
jpeg_quality = 50
target_fps = 5
```

**Resultado esperado:** ~100KB/frame, ~500KB/s, latência < 300ms em 4G.

### Cenário 2: Desktop LAN (alta qualidade)

```toml
[relay.capture]
max_width = 1920
max_height = 1080
jpeg_quality = 85
target_fps = 15
```

**Resultado esperado:** ~800KB/frame, ~12MB/s, latência < 100ms em LAN.

### Cenário 3: Debugging ativo (prioriza FPS sobre qualidade)

```toml
[relay.capture]
max_width = 1280
max_height = 720
jpeg_quality = 40
target_fps = 20
broadcast_capacity = 128
```

**Resultado esperado:** ~150KB/frame, ~3MB/s, 20 FPS, latência aceitável até 400ms.

## Benchmarking

### Medir throughput real

```bash
# Contar frames recebidos em 10 segundos
timeout 10 websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | \
 jq -r 'select(.type=="frame") | .seq' | wc -l
# Dividir por 10 para obter FPS real
```

### Medir latência round-trip

```bash
# Enviar 100 pings e calcular média
for i in $(seq 1 100); do
 echo '{"type":"ping","ts":'$(date +%s%3N)'}'
 sleep 0.1
done | websocat ws://localhost:3000/api/v1/ws/computer-view/{session_id} | \
 jq -r 'select(.type=="quality") | .latency_ms' | \
 datamash mean 1 median 1 perc:99 1
```

### Profile de CPU no gateway

```bash
# Durante sessão ativa
perf record -g -p $(pgrep -f zylos-gateway) -- sleep 30
perf report --sort=symbol | head -20
# Procurar por: base64::encode, serde_json::to_writer, png::encode
```

## Checklist de Tuning

- [ ] Resolução configurada conforme bandwidth disponível
- [ ] JPEG quality entre 50-85 (nunca PNG)
- [ ] Target FPS throttled no producer
- [ ] Broadcast capacity adequada ao use case (64 default, 128 high-FPS)
- [ ] Canvas usando `createImageBitmap` (não `new Image`)
- [ ] Indicadores de qualidade visíveis no cockpit
- [ ] Adaptive quality implementado no producer
- [ ] Logs de métricas ativados (`tracing::info!` a cada 100 frames)
- [ ] Benchmark de FPS/latência executado e documentado
- [ ] Memory leak test: 1h de sessão contínua, RSS estável

## References

- [pb-webrtc-setup.md](./pb-webrtc-setup.md) — setup inicial
- [rb-webrtc-relay.md](../runbooks/rb-webrtc-relay.md) — troubleshooting
- [soar-relay-latency-alert.md](../docs/soar/soar-relay-latency-alert.md) — alertas automáticos
- [MDN createImageBitmap](https://developer.mozilla.org/en-US/docs/Web/API/createImageBitmap)
- [tokio broadcast tuning](https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html)