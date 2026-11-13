# RB-zylos-brief: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-brief](../crates/zylos-brief/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-morning-brief-config.md](../playbooks/pb-morning-brief-config.md)
**Related POP:** [pop-brief-regenerate.md](../playbooks/pop-brief-regenerate.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- SQLite com WAL mode habilitado
- Crate registrado no workspace (`Cargo.toml` members)
- Tabelas `approval_requests` e `action_audit_log` existentes (Fase 1)

### Build
```bash
cargo build -p zylos-brief --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_brief::{BriefStore, MorningBriefGenerator};

// No build_router():
let brief_store = BriefStore::new(pool.clone());
brief_store.run_migrations().await?;
let generator = MorningBriefGenerator::new(pool.clone());
let brief_router = create_brief_router(brief_store, generator);
app.merge(brief_router)
```

### Health Check
```bash
curl -sf "http://localhost:3000/api/v1/brief/today?user_id=default" | jq '.sections | length'
# Esperado: 4
# Falha: HTTP 500 ou connection refused
```

## Migration

Migrations são aplicadas via `BriefStore::run_migrations()` — statements individuais para compatibilidade com sqlx SQLite.

Se migration falhar:
1. Verificar se tabela já existe: `SELECT name FROM sqlite_master WHERE type='table' AND name='morning_briefs';`
2. Se parcial, dropar e reaplicar: `DROP TABLE IF EXISTS morning_briefs;`
3. Reiniciar gateway para reaplicar

## Troubleshooting

### Erro 500 em /brief/today
**Causa:** Migration não aplicada ou tabelas fonte inexistentes
**Solução:** Verificar se `approval_requests` e `action_audit_log` existem. Se faltando, aplicar migrations dos crates `zylos-approval` e `zylos-audit-log` primeiro.

### Brief sempre vazio
**Causa:** Tabelas fonte estão vazias (nenhum approval pendente, nenhuma ação recente)
**Solução:** Comportamento esperado quando não há dados. Para testar, criar approval pendente via API e regenerar brief.

### Brief não atualiza após regeneração
**Causa:** Cache do browser ou SSE não invalidou estado anterior
**Solução:** Hard refresh no cockpit (Ctrl+Shift+R). Se persistir, verificar Network tab para confirmar que `/brief/today` retorna novo `generated_at`.

### Regeneração demora >10s
**Causa:** Muitas rows nas tabelas fonte sem índices adequados
**Solução:** Verificar índices em `approval_requests` (idx_approval_pending) e `action_audit_log` (idx_audit_bot_date). Se faltando, reaplicar migrations dos crates respectivos.

### Histórico não mostra briefs anteriores
**Causa:** Retenção configurada para poucos dias ou cleanup job deletou entradas
**Solução:** Verificar `brief.retention_days` no config. Aumentar se necessário. Entradas deletadas não são recuperáveis.

## Cron Job (Geração Automática)

Configurar no gateway para gerar brief diariamente às 06:00 local:

```rust
// Em apps/zylos-gateway/src/main.rs ou scheduler module
tokio::spawn(async move {
    loop {
        let now = chrono::Local::now();
        if now.hour() == 6 && now.minute() == 0 {
            for user_id in get_active_users().await {
                let today = now.format("%Y-%m-%d").to_string();
                if store.get_today(&user_id, &today).await?.is_none() {
                    let brief = generator.generate(&user_id, &today).await?;
                    store.save(&brief).await?;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
});
```

Alternativamente, usar cron job externo chamando `POST /api/v1/brief/regenerate` às 06:00.

## Monitoring

| Métrica | Query | Alerta Se |
|---------|-------|-----------|
| Brief generation latency | Prometheus histogram on `/brief/today` | >5s p99 |
| Empty brief rate | `SELECT COUNT(*) FILTER (WHERE items_count=0) * 100.0 / COUNT(*) FROM morning_briefs WHERE delivered_at > datetime('now', '-7 days')` | >80% (possível problema nas fontes) |
| History size | `SELECT COUNT(*) FROM morning_briefs` | >10k por usuário (verificar retention) |
| Read rate | `SELECT COUNT(*) FILTER (WHERE read_at IS NOT NULL) * 100.0 / COUNT(*) ...` | <20% (usuários não estão lendo briefs) |

## Backup/Restore

```bash
# Backup
sqlite3 zylos.db ".backup brief_backup_$(date +%Y%m%d).db"

# Restore específico
sqlite3 zylos.db ".restore brief_backup_20261111.db"

# Export histórico para CSV
sqlite3 zylos.db "SELECT user_id, date, items_count, needs_you_count, delivered_at FROM morning_briefs ORDER BY date DESC" > brief_history.csv
```

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.1
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDuet morning-brief.ts](https://github.com/CopilotKit/OpenDuet/blob/main/src/shared/morning-brief.ts)