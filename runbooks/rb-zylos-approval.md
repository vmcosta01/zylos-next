# RB-zylos-approval: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-approval](../crates/zylos-approval/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-approval-gate-setup.md](../playbooks/pb-approval-gate-setup.md)
**Related POP:** [pop-approval-decide.md](../playbooks/pop-approval-decide.md)
**Related SOAR:** [soar-approval-timeout.md](../docs/soar/soar-approval-timeout.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- SQLite com WAL mode habilitado
- Crate registrado no workspace (`Cargo.toml` members)

### Build
```bash
cargo build -p zylos-approval --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_approval::ApprovalStore;

// No build_router():
let approval_store = ApprovalStore::new(pool.clone());
approval_store.run_migrations().await?;
let approval_router = create_approval_router(approval_store);
app.merge(approval_router)
```

### Health Check
```bash
curl -sf http://localhost:3000/api/v1/approvals/pending | jq 'length'
# Esperado: número inteiro (0 ou mais)
# Falha: HTTP 500 ou connection refused
```

## Migration

Migrations são aplicadas via `ApprovalStore::run_migrations()` — statements individuais para compatibilidade com sqlx SQLite (multi-statement strings são silenciosamente ignoradas pelo driver).

Se migration falhar:
1. Verificar se tabelas já existem: `SELECT name FROM sqlite_master WHERE type='table' AND name='approval_requests';`
2. Se parcial, dropar e reaplicar: `DROP TABLE IF EXISTS approval_requests; DROP TABLE IF EXISTS approval_rules;`
3. Reiniciar gateway para reaplicar

## Troubleshooting

### Erro 500 em /approvals/pending
**Causa:** Migration não aplicada ou schema corrompido
**Solução:** Reiniciar gateway (fora da sessão ativa via `hermes gateway restart`)

### Decisão retorna 409 AlreadyDecided
**Causa:** Card de approval já foi decidido (duplo clique ou retry)
**Solução:** Verificar estado atual via `GET /approvals/:id` — se já decidido, não há ação necessária

### Decisão retorna 410 Expired
**Causa:** Request expirou antes da decisão humana
**Solução:** Agente deve gerar novo request se ação ainda for necessária. Verificar `default_expiry_seconds` no config se expiry é muito curto.

### Regra allow_always não funciona
**Causa:** arguments_pattern regex não matcheia argumentos atuais
**Solução:** Verificar regra via `GET /approvals/rules?bot_id=X` — se pattern existe mas não matcheia, ajustar regex ou remover pattern para match exato por tool+category

### Cockpit não mostra cards de approval
**Causa:** URL absoluta incorreta ou CORS bloqueado
**Solução:** Verificar Network tab no DevTools — requests devem ir para `http://localhost:3000/api/v1/approvals/pending`. Se CORS error, verificar gateway config `cors.allowed_origins` inclui `http://localhost:8080`.

## Performance Tuning

- **Índices:** `idx_approval_pending` (partial index) otimiza list_pending() — verificar com `EXPLAIN QUERY PLAN`
- **Cleanup:** Requests expirados devem ser limpos periodicamente (cron job ou background task a cada 5 min)
- **Pool size:** SqlitePool com max_connections=10 é suficiente para approval load típico (<100 req/min)

## Monitoring

| Métrica | Query | Alerta Se |
|---------|-------|-----------|
| Pending approvals | `SELECT COUNT(*) FROM approval_requests WHERE status='pending'` | >50 acumulado |
| Avg decision time | `SELECT AVG(julianday(decided_at) - julianday(created_at)) * 86400 FROM approval_requests WHERE decided_at IS NOT NULL` | >1800s (30min) |
| Expired rate | `SELECT COUNT(*) FILTER (WHERE status='expired') * 100.0 / COUNT(*) FROM approval_requests WHERE created_at > datetime('now', '-24 hours')` | >20% |
| Rule count per bot | `SELECT bot_id, COUNT(*) FROM approval_rules GROUP BY bot_id` | >100 regras/bot (possível leak) |

## Backup/Restore

```bash
# Backup
sqlite3 zylos.db ".backup approval_backup_$(date +%Y%m%d).db"

# Restore específico
sqlite3 zylos.db ".restore approval_backup_20261111.db"
```

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.1
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [sqlx SQLite docs](https://docs.rs/sqlx/latest/sqlx/sqlite/)
- [OpenDots HITL pattern](https://github.com/CopilotKit/OpenDots/blob/main/src/client/PageReviewCard.tsx)