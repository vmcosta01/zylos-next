# Runbook: zylos-channels

**Escopo:** Operações diárias do sistema de canais persistente.
**Responsável:** SRE / Platform Team

## Inicialização

O gateway inicializa o store automaticamente:
```rust
let store = ChannelsStore::connect("sqlite://data/channels.db").await?;
store.run_migrations().await?;
store.seed_defaults().await?;
```

Migrations são idempotentes e rodam a cada startup. Seed usa `INSERT OR IGNORE`.

## Health Check

```bash
# Verificar se endpoint responde
curl -s http://localhost:3000/api/v1/channels | jq '.total'
```
Esperado: número ≥ 4 (canais padrão).

## Backup

### Backup Diário (cron)
```bash
# Adicionar ao crontab
0 2 * * * sqlite3 /path/to/data/channels.db ".backup '/backup/channels-$(date +\%Y\%m\%d).db'"
```

### Backup Manual
```bash
sqlite3 data/channels.db ".backup 'channels-backup.db'"
```

## Monitoramento

### Métricas a Observar
| Métrica | Threshold | Ação |
|---------|-----------|------|
| Tempo de query list_channels | >100ms | Verificar índices |
| Tamanho do DB | >100MB | Arquivar mensagens antigas |
| Erros sqlx | >5/min | Verificar SOAR channel-data-loss |

### Queries de Diagnóstico
```sql
-- Contagem de mensagens por canal
SELECT c.name, COUNT(m.id) as msg_count 
FROM channels c 
LEFT JOIN channel_messages m ON c.id = m.channel_id 
GROUP BY c.id ORDER BY msg_count DESC;

-- Canais inativos (>30 dias sem mensagem)
SELECT id, name, last_message_at FROM channels 
WHERE last_message_at < datetime('now', '-30 days');
```

## Manutenção

### Limpeza de Mensagens Antigas
```sql
DELETE FROM channel_messages WHERE timestamp < datetime('now', '-90 days');
VACUUM;
```

### Reindex
```sql
REINDEX;
```

## Escalonamento
- Problemas de dados → POP: `docs/pop-channel-recovery.md`
- Perda total → SOAR: `docs/soar/soar-channel-data-loss.md`
- Evolução de schema → POT: `docs/pot-channels-schema-evolution.md`