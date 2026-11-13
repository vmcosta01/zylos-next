# zylos-spaces

> Spaces/Docs Tab para Zylos Cockpit v2 — sistema de documentos hierárquicos com editor markdown e persistência SQLite.

## Purpose

Fornecer um sistema de documentação persistente integrado ao cockpit, onde usuários podem criar Spaces (containers de documentos), Pages (documentos markdown) com hierarquia pai-filho, outline para navegação lateral e autosave. Inspirado em OpenDots (`SpaceWorkspace.tsx`, `PageDocument.tsx`, `PageOutline.tsx`).

## Architecture

```
┌─────────────────┐ ┌──────────────────┐ ┌─────────────┐
│ Cockpit Docs Tab │────▶│ SpacesStore │────▶│ SQLite DB │
│ /api/v1/spaces │ │ (hierarchical │ │ spaces + │
│ /api/v1/pages │ │ queries) │ │ pages │
└─────────────────┘ └──────────────────┘ └─────────────┘
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `Space`, `Page`, `PageTreeNode`, `OutlineNode`, requests/responses, `extract_outline()`, `build_page_tree()` |
| `store` | Persistência SQLite com CRUD hierárquico, deleção recursiva, outline extraction |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/spaces` | Lista todos os spaces |
| POST | `/api/v1/spaces` | Cria novo space |
| GET | `/api/v1/spaces/:id` | Busca space por ID |
| DELETE | `/api/v1/spaces/:id` | Deleta space e todas as suas páginas |
| GET | `/api/v1/spaces/:id/pages` | Lista páginas do space (plana + árvore) |
| POST | `/api/v1/spaces/:id/pages` | Cria nova página no space |
| GET | `/api/v1/pages/:id` | Busca página por ID |
| PUT | `/api/v1/pages/:id` | Atualiza título/conteúdo/parent/posição |
| DELETE | `/api/v1/pages/:id` | Deleta página e todas as subpáginas recursivamente |
| GET | `/api/v1/pages/:id/outline` | Retorna outline (headings markdown) |

## Schema

### spaces

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| name | TEXT | Nome do space |
| description | TEXT | Descrição opcional |
| owner_id | TEXT | Proprietário do space |
| created_at | TEXT | ISO 8601 timestamp |
| updated_at | TEXT | ISO 8601 timestamp |

### pages

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| space_id | TEXT FK | Space ao qual pertence |
| parent_id | TEXT FK | Página pai (null = root) |
| title | TEXT | Título da página |
| content_markdown | TEXT | Conteúdo em markdown |
| position | INTEGER | Ordem de exibição entre irmãos |
| created_at | TEXT | ISO 8601 timestamp |
| updated_at | TEXT | ISO 8601 timestamp |

## Features

- **Hierarquia ilimitada**: Páginas podem ter subpáginas em qualquer profundidade
- **Outline automático**: Headings markdown (h1-h6) extraídos e disponíveis via API
- **Árvore de navegação**: `build_page_tree()` retorna estrutura hierárquica para sidebar
- **Deleção recursiva**: Deletar página pai remove todas as subpáginas (filhos antes dos pais)
- **Partial updates**: Atualizar apenas título ou apenas conteúdo preserva os demais campos

## Testing

```bash
cargo test -p zylos-spaces
```

- **20 testes unitários** cobrindo: create/get space, list spaces, delete space cascades pages, create/get page, list pages with tree, update page, partial update, delete page recursive (3 levels), get outline, outline anchors, outline empty/invalid, build page tree flat/nested/empty, update nonexistent returns error, get outline nonexistent
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON |
| serde_json | workspace | Parsing de request/response |
| sqlx | 0.8 (sqlite) | Persistência async SQLite |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601 |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |

## Related Documents

- **Playbook:** [pb-spaces-onboarding.md](../../playbooks/pb-spaces-onboarding.md)
- **Runbook:** [rb-zylos-spaces.md](../../runbooks/rb-zylos-spaces.md)
- **POP:** [pop-space-page-recovery.md](../../playbooks/pop-space-page-recovery.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.5
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)