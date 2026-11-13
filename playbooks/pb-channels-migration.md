# Playbook: Migração de Channels para SQLite (Fase 10.1)

**Objetivo:** Migrar o sistema de canais de DashMap em memória para persistência real via SQLite (sqlx).
**Pré-condição:** Workspace zylos-next clonado, Rust toolchain instalada.
**Pós-condição:** `cargo test -p zylos-channels` passa com ≥15 testes; gateway compila com novo store.

## Passos

### 1. Criar crate zylos-channels
```bash
# Estrutura já criada em crates/zylos-channels/
# Cargo.toml, src/lib.rs, src/models.rs, src/store.rs
# migrations/*.sql (3 arquivos individuais)
```

### 2. Registrar no workspace
Adicionar `"crates/zylos-channels"` ao `members` no `Cargo.toml` raiz.

### 3. Rodar testes do crate
```bash
cargo test -p zylos-channels
```
Esperado: 20 testes passando.

### 4. Refatorar channels_api.rs no gateway
- Substituir `DashMap` por `ChannelsStore` do crate
- Adicionar endpoints `POST /channels/:id/join` e `POST /channels/:id/leave`
- Manter compatibilidade de resposta JSON com cockpit

### 5. Atualizar channel_panel.rs no cockpit
- Consumir novos endpoints join/leave
- Manter fallback mock se gateway indisponível

### 6. Gate final
```bash
cargo test -p zylos-channels && cargo check -p zylos-gateway
```

## Rollback
Se falhar no passo 4 ou 5, reverter `channels_api.rs` para versão DashMap (git checkout). O crate é aditivo e não quebra o build existente.

## Referências
- Crate: `crates/zylos-channels/README.md`
- SOAR: `docs/soar/soar-channel-data-loss.md`
- Runbook: `runbooks/rb-zylos-channels.md`