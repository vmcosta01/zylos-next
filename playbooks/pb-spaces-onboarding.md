# PB-006: Onboarding de Spaces e Documentos

**Version:** 1.0.0
**Owner:** Zylos Platform Team
**Last Updated:** 2026-11-11
**Related POPs:** [pop-space-page-recovery.md](./pop-space-page-recovery.md)
**Related Runbooks:** [rb-zylos-spaces.md](../runbooks/rb-zylos-spaces.md)

## Objective

Configurar o sistema de Spaces/Docs para que usuários possam criar, organizar e gerenciar documentação persistente diretamente no cockpit. Este playbook cobre criação do primeiro space, estruturação de páginas hierárquicas, uso do outline e importação de documentos existentes.

## Prerequisites

- Crate `zylos-spaces` compilado e integrado ao gateway
- Migrações das tabelas `spaces` e `pages` aplicadas via `SpacesStore::run_migrations()`
- Gateway servindo na porta 3000
- Cockpit servindo na porta 8080 com aba "Docs" registrada

## Steps

### 1. Verificar integridade dos endpoints

```bash
curl -sf "http://localhost:3000/api/v1/spaces" | jq 'length'
# Esperado: 0 ou array válido (não erro 500)
```

Se retornar erro, verificar se migrations foram aplicadas e router integrado no gateway.

### 2. Criar primeiro Space

```bash
curl -X POST http://localhost:3000/api/v1/spaces \
 -H "Content-Type: application/json" \
 -d '{
 "name": "Documentação Zylos",
 "description": "Documentação técnica e operacional do projeto",
 "owner_id": "user-1"
 }'
```

Resposta esperada: objeto `Space` com `id` UUID, `name`, `created_at`.

### 3. Criar páginas raiz

```bash
SPACE_ID="<id_do_space_criado>"

# Página de visão geral
curl -X POST "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" \
 -H "Content-Type: application/json" \
 -d '{
 "title": "Visão Geral",
 "content_markdown": "# Visão Geral\n\nBem-vindo à documentação do Zylos.\n\n## Arquitetura\n\nO sistema é composto por...\n\n## Módulos\n\n- Approval Gates\n- Egress Checker\n- Audit Log",
 "position": 0
 }'

# Página de setup
curl -X POST "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" \
 -H "Content-Type: application/json" \
 -d '{
 "title": "Setup e Instalação",
 "content_markdown": "# Setup\n\n## Pré-requisitos\n\n- Rust 1.82+\n- SQLite\n\n## Build\n\n```bash\ncargo build --release\n```",
 "position": 1
 }'
```

### 4. Criar subpáginas (hierarquia)

```bash
ROOT_PAGE_ID="<id_da_pagina_visao_geral>"

curl -X POST "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" \
 -H "Content-Type: application/json" \
 -d "{
 \"title\": \"Arquitetura Detalhada\",
 \"content_markdown\": \"# Arquitetura\\n\\n## Crates\\n\\n### zylos-approval\\n\\nApproval gates HITL...\",
 \"parent_id\": \"${ROOT_PAGE_ID}\",
 \"position\": 0
 }"
```

### 5. Validar árvore de navegação

```bash
curl -s "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" | jq '.tree'
# Esperado: array com nós hierárquicos, children aninhados
```

### 6. Validar outline de headings

```bash
PAGE_ID="<id_de_uma_pagina_com_headings>"
curl -s "http://localhost:3000/api/v1/pages/${PAGE_ID}/outline" | jq '.'
# Esperado: array de {level, text, anchor}
```

### 7. Importar documento existente (markdown file → page)

```bash
# Ler arquivo markdown local e criar página
CONTENT=$(cat meu_documento.md | python3 -c "import sys,json; print(json.dumps(sys.stdin.read()))")

curl -X POST "http://localhost:3000/api/v1/spaces/${SPACE_ID}/pages" \
 -H "Content-Type: application/json" \
 -d "{\"title\": \"Documento Importado\", \"content_markdown\": ${CONTENT}, \"position\": 2}"
```

## Decision Points

| Situação | Decisão | Critério |
|----------|---------|----------|
| Documento grande (>100KB) | Dividir em múltiplas páginas hierárquicas | Cada página <50KB para performance |
| Conteúdo sensível em documento | Não armazenar em Spaces (usar vault separado) | Spaces não tem encryption at rest |
| Múltiplos usuários editando mesma página | Last-write-wins (updated_at como timestamp de conflito) | Futuro: CRDT ou locking otimista |
| Espaço órfão (sem owner ativo) | Transferir ownership ou arquivar | Cleanup mensal |

## Rollback Procedure

1. Deletar space problemático: `DELETE /api/v1/spaces/:id` (cascade deleta todas as páginas)
2. Ou deletar páginas individuais: `DELETE /api/v1/pages/:id` (recursivo)
3. Dados são permanentemente removidos — exportar antes de deletar se necessário

## SLA/RTO/RPO

- **SLA:** 99% disponibilidade (não crítico — documentação pode ser acessada offline)
- **RTO:** 5 minutos (restart do gateway + reaplicação de migrations)
- **RPO:** Último backup do SQLite (recomendado backup diário)

## References

- [IMPLEMENTATION_PLAN_DETAILED.md](../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.5
- [DOCUMENT_COMPLIANCE_FASE9.md](../docs/DOCUMENT_COMPLIANCE_FASE9.md)
- [OpenDots SpaceWorkspace.tsx](https://github.com/CopilotKit/OpenDots/blob/main/src/client/SpaceWorkspace.tsx)