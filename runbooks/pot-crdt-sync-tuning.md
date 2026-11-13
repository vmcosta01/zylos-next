# POT: CRDT Sync Tuning (pot-crdt-sync-tuning)

## Objetivo
Ajustar parâmetros do servidor CRDT para manter latência < 100ms e throughput > 50 updates/s por documento com múltiplos editores.

## Métricas-Chave
| Métrica | Target | Alerta |
|---------|--------|--------|
| Latência p99 broadcast | < 100ms | > 200ms |
| Updates/s por documento | > 50 | < 20 |
| Broadcast channel lag | < 10 msgs | > 50 msgs |
| Documentos ativos | < 200 | > 500 |
| Presença stale prune | < 5% | > 20% |

## Parâmetros Ajustáveis

### Broadcast channel capacity
```rust
// Em crdt.rs — padrão 256
let (tx, _) = broadcast::channel(512); // aumentar se houver lag
```

### Stale presence timeout
```bash
# Via API ou config
curl -X POST http://localhost:8080/api/v1/presence/prune \
 -d '{"timeout_ms": 30000}' # reduzir se muitos usuários fantasmas
```

### Snapshot interval
```rust
// Salvar state vector a cada N updates para acelerar sync de novos clientes
const SNAPSHOT_INTERVAL: usize = 100; // padrão
```

### WebSocket message batching
Agrupar updates pequenos em batches de 10-20ms para reduzir overhead de frame:
```rust
// No handler WS — coletar updates por 15ms antes de broadcast
tokio::time::sleep(Duration::from_millis(15)).await;
```

## Diagnóstico

### Channel lag alto
```bash
curl -H "Authorization: Bearer $TOKEN" \
 http://localhost:8080/api/v1/crdt/stats?doc_id=skill-abc
# {"subscribers":12,"pending_msgs":45,"updates_stored":890}
```
- Se `pending_msgs > 50`: aumentar channel capacity ou adicionar consumers
- Se `subscribers > 30`: considerar sharding por região

### Latência alta
1. Verificar CPU/memory do gateway (`top`, `htop`)
2. Checar se há GC pauses (Rust não tem GC, mas alocações excessivas causam latency spikes)
3. Profile com `tracing` spans no broadcast path
4. Verificar se `DashMap` contention é gargalo (muitos writes concorrentes)

### Throughput baixo
1. Verificar se clientes estão enviando updates individuais vs batched
2. Checar tamanho médio dos updates (>64KB indica falta de incremental sync)
3. Verificar rede (WS compression habilitada?)

## Load Testing
```bash
# Simular 50 clientes editando mesmo documento
cargo bench -p zylos-auth --bench crdt_throughput
# Target: >5000 msg/s broadcast, <5ms p99 latency
```

## Rollback
- Reverter channel capacity para 256 se memory usage subir >2GB
- Aumentar stale timeout para 60s se prune agressivo desconectar usuários legítimos
- Desabilitar snapshotting se I/O disk for gargalo

## Referências
- `crates/zylos-auth/src/crdt.rs`
- `pop-crdt-conflict-resolve`
- `pb-crdt-onboarding`