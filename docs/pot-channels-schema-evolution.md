# POT: Evolução de Schema de Channels

**Propósito:** Procedimento para evoluir o schema SQLite do zylos-channels sem perda de dados.
**Aplicabilidade:** Quando novas colunas, tabelas ou índices precisam ser adicionados.

## Regras de Ouro

1. **Nunca modificar migrations existentes** — sempre criar nova migration com timestamp posterior
2. **Migrations devem ser idempotentes** — usar `IF NOT EXISTS`, `IF EXISTS`, `INSERT OR IGNORE`
3. **Uma migration = um arquivo SQL** — não multi-statement em único arquivo
4. **Backward compatible** — nova coluna deve ter DEFAULT ou permitir NULL
5. **Testar em staging antes de prod**

## Convenção de Nomenclatura

```
YYYYMMDDHHMMSS_descrição_curta.sql
```

Exemplo: `20261113000001_add_channel_topic.sql`

## Adicionar Nova Coluna

### Exemplo: adicionar `topic` a channels

1. Criar migration:
```sql
-- migrations/20261113000001_add_channel_topic.sql
ALTER TABLE channels ADD COLUMN topic TEXT DEFAULT '';
```

2. Atualizar modelo em `src/models.rs`:
```rust
pub struct Channel {
    // ... campos existentes
    pub topic: String,
}
```

3. Atualizar queries no `src/store.rs` que fazem SELECT explícito

4. Rodar testes:
```bash
cargo test -p zylos-channels
```

## Adicionar Nova Tabela

### Exemplo: tabela de reações a mensagens

1. Criar migration:
```sql
-- migrations/20261113000002_create_message_reactions.sql
CREATE TABLE IF NOT EXISTS message_reactions (
    message_id TEXT NOT NULL REFERENCES channel_messages(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    emoji TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (message_id, user_id, emoji)
);

CREATE INDEX IF NOT EXISTS idx_reactions_message ON message_reactions(message_id);
```

2. Adicionar ao array `migrations` em `store.rs::run_migrations()`

3. Criar métodos CRUD no store

4. Adicionar testes

## Remover Coluna (SQLite Limitation)

SQLite não suporta `DROP COLUMN` diretamente em versões antigas. Alternativa:

1. Criar nova tabela sem a coluna
2. Copiar dados: `INSERT INTO new_table SELECT col1, col2 FROM old_table`
3. Drop antiga: `DROP TABLE old_table`
4. Renomear: `ALTER TABLE new_table RENAME TO old_table`

**Sempre fazer backup antes.**

## Breaking Changes

Se a mudança quebrar a API pública (`ChannelResponse`, etc.):

1. Versionar endpoint: `/api/v2/channels`
2. Manter v1 funcional por pelo menos 1 release
3. Documentar no changelog
4. Atualizar cockpit para nova versão

## Checklist Pré-Deploy

- [ ] Migration criada com timestamp correto
- [ ] Migration é idempotente
- [ ] Modelo atualizado
- [ ] Queries atualizadas
- [ ] Testes passando (`cargo test -p zylos-channels`)
- [ ] Backup do DB feito
- [ ] Rollback plan documentado
- [ ] Cockpit atualizado se API mudou

## Rollback

Se migration falhar em prod:

1. Parar gateway imediatamente
2. Restaurar backup pré-migration
3. Reverter código (git revert)
4. Investigar causa raiz
5. Corrigir migration e retestar em staging

## Referências
- Migrations atuais: `crates/zylos-channels/migrations/`
- Store: `crates/zylos-channels/src/store.rs`
- Runbook: `runbooks/rb-zylos-channels.md`