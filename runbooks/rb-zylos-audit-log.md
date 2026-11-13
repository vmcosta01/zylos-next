# RB-zylos-audit-log: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-audit-log](../crates/zylos-audit-log/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-approval-gate-setup.md](../playbooks/pb-approval-gate-setup.md)
**Related POP:** [pop-audit-log-export.md](../playbooks/pop-audit-log-export.md)
**Related SOAR:** [soar-audit-anomaly.md](../docs/soar/soar-audit-anomaly.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- SQLite com WAL mode habilitado
- Crate registrado no workspace (`Cargo.toml` members)
- Dependências `sha2 = "0.10"` e `hex = "0.4"` disponíveis

### Build
```bash
cargo build -p zylos-audit-log --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_audit_log::AuditStore;

// No build_router():
let audit_store = AuditStore::new(pool.clone());
audit_store.run_migrations().await?;
let audit_router = create_audit_router(audit_store);
app.merge(audit_router)
```

### Health Check
```bash
curl -sf "http://localhost:3000/api/v1/audit/stats?period=1" | jq '.total_actions'
# Esperado: número inteiro (0 ou mais)
# Falha: HTTP 500 ou connection refused
```

## Migration

Migrations são aplicadas via `AuditStore::run_migrations()` — statements individuais para compatibilidade com sqlx SQLite.

Se migration falhar:
1. Verificar se tabela já existe: `SELECT name FROM sqlite_master WHERE type='table' AND name='action_audit_log';`
2. Se parcial, dropar e reaplicar: `DROP TABLE IF EXISTS action_audit_log;`
3. Reiniciar gateway para reaplicar

## Troubleshooting

### Erro 500 em /audit/actions
**Causa:** Migration não aplicada ou schema corrompido
**Solução:** Reiniciar gateway (fora da sessão ativa via `hermes gateway restart`)

### Export CSV vazio quando deveria ter dados
**Causa:** Filtros muito restritivos ou timezone incorreto no parâmetro `since`
**Solução:** Testar sem filtros primeiro (`GET /audit/actions?limit=5`). Se retornar dados, ajustar filtros gradualmente. Verificar formato ISO 8601 com timezone UTC (`Z` suffix).

### arguments_hash não tem 64 caracteres
**Causa:** Bug na função `hash_arguments()` ou encoding hex incorreto
**Solução:** Verificar logs do gateway para erros de serialization. Se persistente, abrir issue com exemplo de input que gera hash incorreto.

### Performance lenta em queries paginadas
**Causa:** Índice `idx_audit_bot_date` não criado ou tabela muito grande sem VACUUM
**Solução:**
1. Verificar índices: `SELECT name FROM sqlite_master WHERE type='index' AND tbl_name='action_audit_log';`
2. Se faltando, reaplicar migrations
3. Se tabela >1M rows, executar `VACUUM` em horário de baixa carga
4. Considerar arquivamento de entradas >90 dias

### Stats retornam contagens inconsistentes
**Causa:** Clock drift entre gateway e SQLite, ou entries criadas fora do período consultado
**Solução:** Verificar `created_at` das entries retornadas por `/audit/actions`. Se timestamps futuros, verificar clock do servidor.

## Retention Policy

Default: 90 dias. Cleanup job periódico (cron ou background task):

```sql
DELETE FROM action_audit_log WHERE created_at < datetime('now', '-90 days');
VACUUM;
```

⚠️ **ANTES de deletar:** exportar entradas para arquivo CSV e armazenar em cold storage (S3/GCS) para compliance de longo prazo.

## Monitoring

| Métrica | Query | Alerta Se |
|---------|-------|-----------|
| Total entries (24h) | `SELECT COUNT(*) FROM action_audit_log WHERE created_at > datetime('now', '-24 hours')` | Spike >3x média diária |
| Refused rate | `SELECT COUNT(*) FILTER (WHERE approval_status='refused') * 100.0 / COUNT(*) ...` | >20% (possível problema de UX ou bot mal configurado) |
| Table size | `SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()` | >1GB (considerar archive) |
| Avg query latency | Prometheus histogram on `/audit/actions` | >500ms p99 |

## Backup/Restore

```bash
# Backup completo
sqlite3 zylos.db ".backup audit_backup_$(date +%Y%m%d).db"

# Restore específico
sqlite3 zylos.db ".restore audit_backup_20261111.db"

# Archive entries antigas para CSV antes de delete
curl -s "http://localhost:3000/api/v1/audit/export?until=2026-08-01T00:00:00Z&format=csv" \
 -o audit_archive_pre_202608.csv
```

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.4
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [sha2 crate docs](https://docs.rs/sha2/latest/sha2/)
- [hex crate docs](https://docs.rs/hex/latest/hex/)