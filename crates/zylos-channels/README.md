# zylos-channels

Persistência real de canais e mensagens via SQLite (sqlx) — Fase 10.1.

## Visão Geral

Substitui o backend em memória (`DashMap`) por um store persistente com migrations individuais. Fornece `ChannelsStore` para uso no `zylos-gateway`.

## Schema

Três tabelas com migrations individuais:

| Migration | Tabela | Descrição |
|-----------|--------|-----------|
| `20261112000001` | `channels` | Canais com nome, descrição, privacidade |
| `20261112000002` | `channel_members` | Relação N:N entre canais e usuários |
| `20261112000003` | `channel_messages` | Mensagens com autor, conteúdo e timestamp |

## API Pública

```rust
let store = ChannelsStore::connect("sqlite://data/channels.db").await?;
store.run_migrations().await?;
store.seed_defaults().await?;

// CRUD
let channels = store.list_channels().await?;
let channel = store.get_channel("ch-geral").await?;
let new_ch = store.create_channel("Novo", "desc", false).await?;

// Mensagens
let msgs = store.list_messages("ch-geral").await?;
let msg = store.send_message("ch-geral", "Alice", "Olá!").await?;

// Membros (join/leave)
store.join_channel("ch-geral", "user-1").await?;
store.leave_channel("ch-geral", "user-1").await?;
let members = store.list_members("ch-geral").await?;
```

## Testes

```bash
cargo test -p zylos-channels
```

20 testes unitários cobrindo: migrations, seed idempotente, CRUD de canais, envio/listagem de mensagens, join/leave de membros, contagem de membros, timestamps.

## Documentos Operacionais

| Documento | Caminho |
|-----------|---------|
| Playbook | `playbooks/pb-channels-migration.md` |
| POP | `docs/pop-channel-recovery.md` |
| Runbook | `runbooks/rb-zylos-channels.md` |
| SOAR | `docs/soar/soar-channel-data-loss.md` |
| POT | `docs/pot-channels-schema-evolution.md` |

## Dependências

- `sqlx` 0.8 (runtime-tokio-rustls, sqlite, chrono)
- `chrono` (timestamps RFC 3339)
- `uuid` (IDs)
- `thiserror` (erros tipados)
- `serde` / `serde_json` (serialização)