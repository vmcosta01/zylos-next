# POP: Recuperação de Dados de Canais

**Propósito:** Procedimento operacional para recuperar dados de canais após corrupção ou perda parcial do SQLite.
**Aplicabilidade:** Quando `channels.db` está corrompido, inacessível ou com dados inconsistentes.

## Diagnóstico

1. Verificar integridade do banco:
```bash
sqlite3 data/channels.db "PRAGMA integrity_check;"
```
- Se retornar `ok`: problema é de dados, não de arquivo.
- Se retornar erro: arquivo corrompido, seguir recuperação.

2. Verificar tabelas existentes:
```bash
sqlite3 data/channels.db ".tables"
```

## Recuperação de Arquivo Corrompido

### Opção A: Backup WAL
Se WAL mode estava ativo:
```bash
cp data/channels.db-wal data/channels.db.recovered
sqlite3 data/channels.db.recovered "PRAGMA integrity_check;"
```

### Opção B: Dump e Reimport
```bash
sqlite3 data/channels.db ".dump" > dump.sql
rm data/channels.db
sqlite3 data/channels.db < dump.sql
```

### Opção C: Recriar do Zero
Se nenhuma recuperação funcionar:
```bash
rm data/channels.db
# O gateway recria via run_migrations() + seed_defaults() na próxima inicialização
```

## Recuperação de Dados Inconsistentes

### Canais órfãos (sem membros mas com mensagens)
```sql
-- Identificar
SELECT c.id FROM channels c 
LEFT JOIN channel_members m ON c.id = m.channel_id 
WHERE m.channel_id IS NULL AND c.is_private = 1;

-- Ação: revisar manualmente ou adicionar membro admin
```

### Mensagens sem canal
```sql
DELETE FROM channel_messages WHERE channel_id NOT IN (SELECT id FROM channels);
```

## Pós-Recuperação

1. Rodar integrity check novamente
2. Reiniciar gateway: `cargo run -p zylos-gateway`
3. Verificar no cockpit se canais carregam corretamente
4. Documentar incidente no audit log

## Prevenção
- WAL mode sempre ativo (configurado no store)
- Backups periódicos via cron (ver runbook)
- Migrations idempotentes (INSERT OR IGNORE no seed)