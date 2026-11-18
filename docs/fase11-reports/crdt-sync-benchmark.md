# Relatório: Benchmark de sincronização CRDT (T11.9.7 / T11.9.8)

**Data:** 2026-11-17 · **Branch:** `feat/f11-crdt` · **Biblioteca:** `yrs 0.28.1` (ADR-012, `docs/DECISIONS_FASE11.md`)

## Ambiente
- Windows 11 Pro, 12 threads lógicas, `rustc 1.99.1`, perfil `dev` (debug, sem otimização).
- Servidor real `apps/zylos-gateway/src/crdt_sync.rs` (axum WS) em `127.0.0.1` (loopback), clientes yrs nativos via `tokio-tungstenite` (`apps/zylos-gateway/tests/crdt_common/mod.rs`).
- Sem persistência nos benches (`TestServer::start(None)`); os testes de persistência ficam em `crdt_multi_client.rs`.

> Números de loopback em build debug: medem o custo do servidor + protocolo, **não** a latência de rede real. Em produção some o RTT da rede.

## Comando
```bash
export CARGO_TARGET_DIR=E:/zylos-target/crdt
cargo test -p zylos-gateway --test crdt_multi_client --test crdt_latency_bench -- --nocapture
```

## Resultados — latência (ms)

| Cenário | Execução | n | média | p50 | p95 | p99 | máx |
|---|---|---|---|---|---|---|---|
| Ping RTT (Custom tag 100) | 1 (paralelo) | 500 | 0.125 | 0.087 | 0.271 | 0.421 | 0.597 |
| Ping RTT | 2 (`--test-threads=1`) | 500 | 0.224 | 0.173 | 0.564 | 0.859 | 1.146 |
| Propagação de edição, 3 clientes | 1 | 300 | 0.677 | 0.404 | 1.576 | 8.177 | 10.885 |
| Propagação de edição, 3 clientes | 2 | 300 | 0.683 | 0.566 | 1.398 | 2.566 | 3.456 |
| Propagação de edição, 10 clientes | 1 | 300 | 1.115 | 0.652 | 3.807 | 6.805 | 9.961 |
| Propagação de edição, 10 clientes | 2 | 300 | 1.181 | 0.870 | 2.872 | 6.868 | 10.135 |

## Resultados — throughput

| Cenário | Execução | Edições | Convergência | Throughput |
|---|---|---|---|---|
| Burst, 10 clientes × 100 edições | 2 | 1000 | 139.749 ms | 7156 edições/s |

(Na execução 1 a linha de burst não foi capturada pelo filtro do `grep`; o teste passou.)

## Verificação de convergência (T11.9.7) — `crdt_multi_client.rs`
`test result: ok. 12 passed; 0 failed` — incluindo:
- `crdt_three_clients_concurrent_inserts_converge` (3 clientes, inserções concorrentes)
- `crdt_ten_clients_simultaneous_edits_zero_data_loss` (10 clientes, zero perda)
- `crdt_twelve_clients_interleaved_insert_delete_stress`
- `crdt_concurrent_insert_and_delete_converge`, `crdt_same_position_conflict_is_deterministic`
- `crdt_offline_edits_merge_on_reconnect`, `crdt_late_joiner_receives_full_state`, `crdt_rooms_are_isolated`
- `crdt_presence_cursor_broadcast_and_cleanup` (T11.9.4)
- `crdt_persistence_survives_server_restart`, `crdt_last_client_leaving_creates_snapshot_version`, `crdt_rest_lists_active_documents_and_versions` (T11.9.6)

Cliente do cockpit (`apps/zylos-cockpit/src/crdt_client.rs`): `cargo test -p zylos-cockpit --lib crdt_client` → `7 passed; 0 failed` (convergência de 3 réplicas, handshake, presença, merge/manter local/aceitar remoto, diff, unicode).

## Comparação com metas (`runbooks/pot-crdt-sync-tuning.md`)
| Meta | Medido | Status |
|---|---|---|
| p99 broadcast < 100 ms | ≤ 8.177 ms (pior p99 observado) | ✅ |
| > 50 updates/s por documento | 7156 edições/s (burst, 10 clientes) | ✅ |
| p99 < 5 ms (meta de load test) | 10 clientes: 6.805–6.868 ms; 3 clientes: 2.566–8.177 ms | ⚠️ não atingida em debug; medir em `--release` |

## Limitações
- Somente loopback e build debug; sem medição em `--release` nem com rede real.
- Não houve teste com 50 clientes (o maior é 12, no stress test).
- O cliente WASM do cockpit foi verificado com `cargo check --target wasm32-unknown-unknown` e testes nativos da lógica; não houve teste E2E em navegador.
