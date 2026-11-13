# zylos-brief

> Morning Brief Generator para Zylos Cockpit v2 — briefing diário automático agregando métricas, tarefas e calendário.

## Purpose

Gerar um resumo matinal personalizado que responde quatro perguntas essenciais:
1. **What needs you** — aprovações pendentes, tarefas bloqueadas, erros críticos
2. **What can wait** — tarefas em andamento, goals com deadline distante
3. **Calendar** — eventos do dia (integração futura com Google Calendar/Outlook)
4. **Unchecked** — integrações com falhas recentes, ações sem verificação

Inspirado em: OpenDuet (`brief.ts`, `shared/morning-brief.ts`), awesome-grok-bot (Daily Briefing template).

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│  Cockpit Chat   │────▶│ BriefGenerator   │────▶│ SQLite DB   │
│  /api/v1/brief  │     │ (aggregates      │     │ morning_    │
│                 │     │  from N sources) │     │ briefs      │
└─────────────────┘     └──────────────────┘     └─────────────┘
                              ▲
                    ┌─────────┼─────────┐
                    ▼         ▼         ▼
              approvals   audit_log  (calendar)
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `MorningBrief`, `BriefSection`, `BriefItem`, `BriefRow` |
| `generator` | Lógica de agregação: consulta approval_requests + action_audit_log |
| `store` | Persistência SQLite com upsert por user_id+date, histórico, marcação de leitura |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/brief/today?user_id=X` | Retorna brief do dia (gera se não existir) |
| POST | `/api/v1/brief/regenerate` | Força regeneração do brief |
| GET | `/api/v1/brief/history?user_id=X&limit=N` | Histórico de briefs anteriores |
| POST | `/api/v1/brief/:id/read` | Marca brief como lido |

## Schema

### morning_briefs

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| user_id | TEXT | Identificador do usuário |
| date | TEXT | YYYY-MM-DD |
| content_json | TEXT | MorningBrief serializado em JSON |
| delivered_at | TEXT | ISO 8601 timestamp de geração |
| read_at | TEXT | ISO 8601 timestamp de leitura (null = não lido) |
| items_count | INTEGER | Total de itens no brief |
| needs_you_count | INTEGER | Itens na seção needs_you |

### Brief Sections

| Seção | Ícone | Fonte de Dados | Prioridade |
|-------|-------|----------------|------------|
| needs_you | 🔴 | approval_requests (status=pending) | critical |
| can_wait | 🟡 | action_audit_log (result_status=null) | low |
| calendar | 📅 | Placeholder (integração futura) | medium |
| unchecked | ⚠️ | action_audit_log (result_status=failure) | high |

## Configuration

Sem variáveis de ambiente específicas. O generator recebe `SqlitePool` via injeção.

Trigger de geração:
- **Automático:** primeira abertura do cockpit após 06:00 local
- **Manual:** `POST /api/v1/brief/regenerate`
- **Cron:** job periódico no gateway (configurável)

## Testing

```bash
cargo test -p zylos-brief
```

- **14 testes unitários** cobrindo: empty brief structure, finalize counts, serialization roundtrip, save/get/upsert, mark_read, history ordering, delete for regeneration, generate with pending approvals, generate with failed actions, finalize summaries, exclude decided approvals
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON |
| serde_json | workspace | Parsing de content_json |
| sqlx | 0.8 (sqlite) | Persistência async SQLite |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601 |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |

## Related Documents

- **Playbook:** [pb-morning-brief-config.md](../../playbooks/pb-morning-brief-config.md)
- **Runbook:** [rb-zylos-brief.md](../../runbooks/rb-zylos-brief.md)
- **POP:** [pop-brief-regenerate.md](../../playbooks/pop-brief-regenerate.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.1
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)