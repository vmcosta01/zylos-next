# zylos-approval

> Approval gates HITL (Human-in-the-Loop) para Zylos Cockpit v2.

## Purpose

Garantir que ações externas do agente (enviar email, fazer deploy, pagamento, assinar documento) exigem confirmação humana explícita antes da execução. Suporta 3 níveis de aprovação: allow-once, allow-session, allow-always — e recusa com motivo opcional.

Inspirado em: OpenBot (`ApprovalBlock.tsx`), OpenDots (`PageReviewCard.tsx`), OpenDuet (`approvals/service.ts`), AnyModelBot (`cu-permission.js`), Rerun (`ApprovalBlock.tsx`).

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│  ChatPanel      │────▶│  ApprovalCard    │────▶│ Gateway API │
│  (Leptos WASM)  │     │  (inline no chat)│     │ /approvals/*│
└─────────────────┘     └──────────────────┘     └──────┬──────┘
                                                         │
                                              ┌──────────▼──────────┐
                                              │  ApprovalStore      │
                                              │  (SQLite via sqlx)  │
                                              └─────────────────────┘
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos de dados: `ApprovalRequest`, `ApprovalRule`, enums (`ApprovalDecision`, `ApprovalStatus`, `ActionCategory`) |
| `store` | Persistência SQLite CRUD, migrations inline, receipt persistence |
| `error` | Erros tipados com `thiserror`: NotFound, AlreadyDecided, Expired, InvalidDecision, InvalidCategory, DuplicateRule |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/approvals/pending` | Lista aprovações pendentes (não expiradas) |
| GET | `/api/v1/approvals/:id` | Detalhes de uma aprovação específica |
| POST | `/api/v1/approvals/:id/decide` | Registra decisão do usuário (allow_once/session/always/refuse) |
| GET | `/api/v1/approvals/rules` | Lista regras persistentes (allow-always) para um bot |
| DELETE | `/api/v1/approvals/rules/:id` | Revoga uma regra persistente |

## Schema

### approval_requests

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| bot_id | TEXT | Identificador do bot solicitante |
| tool_name | TEXT | Nome da ferramenta/ação |
| action_category | TEXT | Categoria: send_message, deploy, payment, sign_document, delete_resource, read_sensitive, low_risk |
| summary | TEXT | Descrição legível da ação |
| arguments_json | TEXT | Argumentos exatos em JSON |
| status | TEXT | pending, approved, refused, expired, cancelled |
| decision | TEXT | allow_once, allow_session, allow_always, refuse |
| refuse_reason | TEXT | Motivo da recusa (opcional) |
| created_at | TEXT | ISO 8601 timestamp |
| decided_at | TEXT | ISO 8601 timestamp da decisão |
| expires_at | TEXT | ISO 8601 expiry (opcional) |
| receipt_json | TEXT | Receipt para sobrevivência a refresh |

### approval_rules

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| bot_id | TEXT | Bot ao qual a regra se aplica |
| tool_name | TEXT | Ferramenta coberta pela regra |
| action_category | TEXT | Categoria da ação |
| arguments_pattern | TEXT | Regex opcional para filtrar argumentos |
| decision | TEXT | allow_always (única decisão persistente) |
| created_at | TEXT | ISO 8601 timestamp |
| created_by | TEXT | "user" ou "system" |

## Configuration

Sem variáveis de ambiente específicas. O store recebe `SqlitePool` via injeção de dependência.

Migrations são aplicadas via `ApprovalStore::run_migrations()` (statements individuais para compatibilidade com sqlx SQLite).

## Testing

```bash
cargo test -p zylos-approval
```

- **12 testes unitários** cobrindo: create, get, decide (allow_once/refuse/allow_always), double-decide rejection, list_pending, expired request, has_allow_always_rule, delete_rule, invalid_category, save_receipt
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON para API |
| serde_json | workspace | Parsing de arguments_json |
| sqlx | workspace (sqlite) | Persistência async SQLite |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601 |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |

## Related Documents

- **Playbook:** [pb-approval-gate-setup.md](../../playbooks/pb-approval-gate-setup.md)
- **Runbook:** [rb-zylos-approval.md](../../runbooks/rb-zylos-approval.md)
- **POP:** [pop-approval-decide.md](../../playbooks/pop-approval-decide.md)
- **SOAR:** [soar-approval-timeout.md](../../docs/soar/soar-approval-timeout.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.1
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)