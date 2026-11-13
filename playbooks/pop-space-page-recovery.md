# POP-008: Recuperação de Página Deletada em Spaces

**Version:** 1.0.0
**Playbook:** [pb-spaces-onboarding.md](./pb-spaces-onboarding.md)
**Last Updated:** 2026-11-11

## Purpose

Recuperar uma página (ou conjunto de páginas) que foi deletada acidentalmente do sistema de Spaces/Docs. Como o SQLite não tem soft-delete nativo, a recuperação depende de backups periódicos ou exportações prévias.

## Trigger

- Usuário reporta que página desapareceu após deleção acidental
- Deleção recursiva removeu subpáginas não intencionadas
- Space inteiro foi deletado por engano
- Conteúdo crítico perdido sem export recente

## Steps

### 1. Verificar se há backup disponível

```bash
# Listar backups recentes do SQLite
ls -la /path/to/backups/spaces_backup_*.db

# Ou verificar exports CSV/JSON
ls -la /path/to/exports/pages_export_*.json
```

Se nenhum backup existe, a página **não pode ser recuperada**. Documentar incidente e implementar backup automático para prevenir recorrência.

### 2. Restaurar de backup SQLite

```bash
# Parar gateway temporariamente
hermes gateway stop

# Restaurar banco específico
sqlite3 zylos.db ".restore /path/to/backups/spaces_backup_20261111.db"

# Reiniciar gateway
hermes gateway restart
```

⚠️ **AVISO:** Restore substitui TODO o banco. Se outras mudanças ocorreram após o backup, elas serão perdidas. Considere exportar dados atuais antes do restore.

### 3. Restaurar de export JSON (alternativa segura)

Se apenas páginas específicas precisam ser restauradas:

```bash
# Ler export e recriar páginas via API
cat pages_export_20261111.json | jq -c '.[]' | while read page; do
  SPACE_ID=$(echo "$page" | jq -r '.space_id')
  TITLE=$(echo "$page" | jq -r '.title')
  CONTENT=$(echo "$page" | jq -r '.content_markdown')
  
  curl -X POST "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" \
    -H "Content-Type: application/json" \
    -d "{\"title\": \"${TITLE}\", \"content_markdown\": $(echo "$CONTENT" | jq -Rs .)}"
done
```

### 4. Validar restauração

```bash
# Verificar que página existe
curl -s "http://localhost:3000/api/v1/pages/{restored_page_id}" | jq '.title'

# Verificar outline funciona
curl -s "http://localhost:3000/api/v1/pages/{restored_page_id}/outline" | jq 'length'

# Verificar árvore de navegação no cockpit
# Abrir http://localhost:8080 → aba Docs → navegar até página restaurada
```

### 5. Documentar incidente

Adicionar entrada no `TRACKRECORD_FASE9.md`:
```markdown
### YYYY-MM-DD
- **Incident:** Página "{title}" deletada acidentalmente
- **Cause:** {deleção manual / recursiva / bug}
- **Recovery:** {backup SQLite / export JSON / não recuperável}
- **Prevention:** {backup automático implementado / confirmação de deleção adicionada}
```

## Verification

- Página restaurada aparece na listagem `/spaces/:id/pages`
- Conteúdo markdown íntegro (comparar com backup/export se possível)
- Outline retorna headings corretos
- Subpáginas restauradas mantêm hierarquia pai-filho
- Cockpit exibe página normalmente após refresh

## Rollback

Se restauração corrompeu dados existentes:
1. Restaurar backup anterior ao restore (se disponível)
2. Ou deletar páginas restauradas incorretas e tentar método alternativo
3. Se dano extenso, considerar restore completo de backup mais antigo

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Nenhum backup disponível | Platform team + DevOps | Implementar backup automático urgente |
| Restore corrompeu outros dados | DBA / Senior dev | Recovery de emergência |
| Página crítica perdida sem recovery possível | Product owner + Stakeholders | Comunicar perda e plano de recreação |
| Deleção recursiva bug (removeu páginas erradas) | Dev team | Bug fix prioritário |