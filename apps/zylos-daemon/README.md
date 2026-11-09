# zylos-daemon

**Fase:** 7 - Daemon Background
**Status:** ✅ Implementado (327 LOC, 1 arquivo Rust, 6 testes)

## Descricao

Processo background responsavel por gerenciar o ciclo de vida dos agentes locais, manter monitoramento de saude, executar heartbeat, e coordenar graceful shutdown.

## Arquitetura

```
zylos-daemon/
├── src/
│   └── main.rs          # Entry point com health monitor + graceful shutdown
└── Cargo.toml
```

## Responsabilidades

- Gerenciar ciclo de vida dos agentes locais (spawn/kill/suspend)
- Monitoramento de saude periodico (health checks)
- Auto-restart de agentes unhealthy (com limite de tentativas)
- Endpoint HTTP interno para health/metrics
- Graceful shutdown com drenagem de agentes
- Aggregacao de logs estruturados

## Endpoints

| Endpoint | Descricao |
|----------|-----------|
| `GET /health` | Health check local |
| `GET /agents` | Listar agentes locais e status |

## Configuracao

Arquivo: `/etc/zylos/daemon.toml` ou path via env `ZYLOS_DAEMON_CONFIG`

```toml
health_addr = "127.0.0.1:9090"
health_interval_secs = 30
max_restarts = 5
agents_dir = "/var/lib/zylos/agents"
state_dir = "/var/lib/zylos/daemon"
```

Override via CLI:
- `--foreground` (nao daemoniza)
- `--health-addr 0.0.0.0:8080`

## Estado do Agente

```rust
enum AgentStatus {
    Stopped,
    Starting,
    Running,
    Unhealthy,
    Failed,
    ShuttingDown,
}
```

O daemon mantem estado compartilhado via `Arc<RwLock<HashMap<String, AgentState>>>` e monitora agentes a cada `health_interval_secs`. Agentes unhealthy sao reiniciados ate `max_restarts` vezes antes de serem marcados como `Failed`.

## Graceful Shutdown

Recebe SIGINT/SIGTERM e:
1. Cancela loops de monitoramento
2. Para servidor HTTP de health
3. Envia sinal de shutdown para todos agentes `Running`
4. Aguarda 5 segundos para drenagem
5. Finaliza processo

## Dependencias

- `zylos-core` (tipos compartilhados)
- `zylos-bot-system` (lifecycle de agentes)
- `tokio` 1.40 (async runtime)
- `axum` 0.7 (HTTP server para health endpoint)
- `serde` + `serde_json` (serializacao)
- `tracing` + `tracing-subscriber` (logging JSON)
- `anyhow` (error handling)
- `clap` 4.6 (CLI argument parsing)

## Testes

6 testes unitarios cobrindo:
- Configuracao padrao e overrides
- Serializacao de AgentState e AgentStatus
- Parsing de CLI com flags

## Runbook

- [`zylos-daemon.runbook.yaml`](../../docs/soar/runbooks/zylos-daemon.runbook.yaml)
- [`zylos-daemon.md`](../../runbooks/zylos-daemon.md)
