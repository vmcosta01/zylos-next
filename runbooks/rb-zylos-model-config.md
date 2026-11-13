# RB-zylos-model-config: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-model-config](../crates/zylos-model-config/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-model-tier-strategy.md](../playbooks/pb-model-tier-strategy.md)
**Related POP:** [pop-model-fallback.md](../playbooks/pop-model-fallback.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- SQLite com WAL mode habilitado
- Crate registrado no workspace (`Cargo.toml` members)

### Build
```bash
cargo build -p zylos-model-config --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_model_config::ModelConfigStore;

// No build_router():
let model_store = ModelConfigStore::new(pool.clone());
model_store.run_migrations().await?;
let model_router = create_model_router(model_store);
app.merge(model_router)
```

### Health Check
```bash
curl -sf "http://localhost:3000/api/v1/models/config?bot_id=__global__" | jq '.source'
# Esperado: "global" ou "default"
# Falha: HTTP 500 ou connection refused
```

## Migration

Migrations são aplicadas via `ModelConfigStore::run_migrations()` — statements individuais para compatibilidade com sqlx SQLite.

Se migration falhar:
1. Verificar se tabela já existe: `SELECT name FROM sqlite_master WHERE type='table' AND name='model_configs';`
2. Se parcial, dropar e reaplicar: `DROP TABLE IF EXISTS model_configs;`
3. Reiniciar gateway para reaplicar

## Troubleshooting

### Erro 500 em /models/config
**Causa:** Migration não aplicada ou schema corrompido
**Solução:** Reiniciar gateway (fora da sessão ativa via `hermes gateway restart`)

### Bot usa modelo errado (não respeita override)
**Causa:** Cache de config no gateway ou bot_id incorreto na query
**Solução:** Verificar bot_id exato usado pelo bot. Limpar cache reiniciando gateway. Testar com `GET /models/config?bot_id={id_exato}`.

### Resolução sempre retorna "default"
**Causa:** Nenhuma config global ou bot-specific foi criada
**Solução:** Criar config global primeiro: `PUT /models/config/__global__`. Depois criar overrides por bot conforme necessário.

### Upsert falha com UNIQUE constraint error
**Causa:** Concorrência rara ou bug no upsert logic
**Solução:** Verificar logs do gateway. Se persistente, deletar config existente e recriar: `DELETE /models/config/:bot_id` seguido de `PUT`.

### Mudança de config não reflete imediatamente
**Causa:** Bot mantém config em memória e não recarrega
**Solução:** Reiniciar bot ou enviar sinal de reload via API do bot. Configs são resolvidas a cada request no gateway, mas bots podem cachear localmente.

## Monitoring

| Métrica | Query | Alerta Se |
|---------|-------|-----------|
| Config count | `SELECT COUNT(*) FROM model_configs` | >100 (possível leak de configs órfãs) |
| Bots sem override | Contar bots ativos sem entry em model_configs | >80% (global pode estar sobrecarregado) |
| Provider distribution | `SELECT provider, COUNT(*) FROM model_configs GROUP BY provider` | Single provider >90% (risco de concentração) |
| Stale configs | `SELECT COUNT(*) FROM model_configs WHERE updated_at < datetime('now', '-90 days')` | >20% (revisar configs antigas) |

## Backup/Restore

```bash
# Backup
sqlite3 zylos.db ".backup model_config_backup_$(date +%Y%m%d).db"

# Restore específico
sqlite3 zylos.db ".restore model_config_backup_20261111.db"

# Export configs para JSON
sqlite3 zylos.db "SELECT json_group_array(json_object('bot_id', bot_id, 'provider', provider, 'model_id', model_id, 'tier_strategy', tier_strategy)) FROM model_configs" > model_configs_export.json
```

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.3
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [AnyModelBot model-picker.js](https://github.com/modelflare-ai/AnyModelBot/blob/master/ui/components/model-picker.js)