# RB-zylos-spaces: Runbook Operacional

**Version:** 1.0.0
**Crate:** [zylos-spaces](../crates/zylos-spaces/)
**Last Updated:** 2026-11-11
**Related Playbook:** [pb-spaces-onboarding.md](../playbooks/pb-spaces-onboarding.md)
**Related POP:** [pop-space-page-recovery.md](../playbooks/pop-space-page-recovery.md)

## Deploy

### Pré-requisitos
- Rust 1.82+ instalado
- SQLite com WAL mode habilitado
- Crate registrado no workspace (`Cargo.toml` members)

### Build
```bash
cargo build -p zylos-spaces --release
```

### Integração ao Gateway
Adicionar em `apps/zylos-gateway/src/lib.rs`:
```rust
use zylos_spaces::SpacesStore;

// No build_router():
let spaces_store = SpacesStore::new(pool.clone());
spaces_store.run_migrations().await?;
let spaces_router = create_spaces_router(spaces_store);
app.merge(spaces_router)
```

### Health Check
```bash
curl -sf "http://localhost:3000/api/v1/spaces" | jq 'length'
# Esperado: número inteiro (0 ou mais)
# Falha: HTTP 500 ou connection refused
```

## Migration

Migrations são aplicadas via `SpacesStore::run_migrations()` — statements individuais para compatibilidade com sqlx SQLite. Cria tabelas `spaces` e `pages` com índices `idx_pages_space` e `idx_pages_parent`.

Se migration falhar:
1. Verificar se tabelas já existem: `SELECT name FROM sqlite_master WHERE type='table' AND name IN ('spaces','pages');`
2. Se parcial, dropar e reaplicar: `DROP TABLE IF EXISTS pages; DROP TABLE IF EXISTS spaces;`
3. Reiniciar gateway para reaplicar

## Troubleshooting

### Erro 500 em /spaces ou /pages
**Causa:** Migration não aplicada ou schema corrompido
**Solução:** Reiniciar gateway (fora da sessão ativa via `hermes gateway restart`)

### FK constraint failed ao deletar página
**Causa:** Tentativa de deletar página pai antes dos filhos
**Solução:** `delete_page()` já faz deleção recursiva em ordem reversa (filhos primeiro). Se erro persistir, verificar se há referência circular órfã na tabela. Executar: `SELECT id, parent_id FROM pages WHERE parent_id IS NOT NULL AND parent_id NOT IN (SELECT id FROM pages);`

### Outline retorna array vazio quando página tem headings
**Causa:** Headings não seguem formato `# Texto` (ex: sem espaço após `#`, ou dentro de code block)
**Solução:** Verificar markdown source. O parser exige `# ` (hash + espaço) no início da linha. Headings dentro de blocos de código são ignorados por design.

### Árvore de páginas mostra ordem errada
**Causa:** Campo `position` não definido ou duplicado entre irmãos
**Solução:** Atualizar posições via `PUT /pages/:id` com campo `position`. A ordenação é por `position ASC, created_at ASC`.

### Conteúdo markdown truncado
**Causa:** Limite de tamanho de coluna TEXT no SQLite (teórico ~1GB, prático depende de memória)
**Solução:** Para documentos >1MB, dividir em múltiplas páginas hierárquicas. Verificar tamanho: `SELECT id, length(content_markdown) FROM pages ORDER BY length(content_markdown) DESC LIMIT 10;`

### Performance lenta em list_pages com muitas páginas
**Causa:** >1000 páginas no mesmo space sem paginação
**Solução:** Adicionar paginação à API (limit/offset). Considerar arquivar espaços antigos. Índice `idx_pages_space` deve estar presente.

## Backup Strategy

### Backup Automático Diário
```bash
#!/bin/bash
BACKUP_DIR="/path/to/backups"
DATE=$(date +%Y%m%d_%H%M%S)
sqlite3 zylos.db ".backup ${BACKUP_DIR}/spaces_backup_${DATE}.db"

# Reter últimos 30 dias
find ${BACKUP_DIR} -name "spaces_backup_*.db" -mtime +30 -delete
```

### Export Periódico para JSON
```bash
curl -s "http://localhost:3000/api/v1/spaces" | jq -r '.[].id' | while read space_id; do
 curl -s "http://localhost:3000/api/v1/spaces/${space_id}/pages" \
 > "exports/pages_${space_id}_$(date +%Y%m%d).json"
done
```

## Monitoring

| Métrica | Query | Alerta Se |
|---------|-------|-----------|
| Total spaces | `SELECT COUNT(*) FROM spaces` | >100 (possível proliferação descontrolada) |
| Total pages | `SELECT COUNT(*) FROM pages` | >10k (considerar arquivamento) |
| Pages sem conteúdo | `SELECT COUNT(*) FROM pages WHERE content_markdown = ''` | >30% (páginas placeholder órfãs) |
| Orphan pages | `SELECT COUNT(*) FROM pages WHERE space_id NOT IN (SELECT id FROM spaces)` | >0 (integridade comprometida) |
| Avg page size | `SELECT AVG(length(content_markdown)) FROM pages` | >100KB (documentos muito grandes) |
| Stale pages | `SELECT COUNT(*) FROM pages WHERE updated_at < datetime('now', '-90 days')` | >50% (revisão de conteúdo necessária) |

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.5
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDots SpaceWorkspace.tsx](https://github.com/CopilotKit/OpenDots/blob/main/src/client/SpaceWorkspace.tsx)
- [OpenDots PageOutline.tsx](https://github.com/CopilotKit/OpenDots/blob/main/src/client/PageOutline.tsx)