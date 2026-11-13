# zylos-audit-log

> Action audit trail para Zylos Cockpit v2 — registra todas as ações externas com hash SHA-256 de argumentos.

## Purpose

Fornecer registro centralizado e imutável de todas as ações executadas pelos agentes (envio de mensagens, deploys, pagamentos, aprovações), permitindo auditoria forense, compliance e detecção de anomalias. Arguments são armazenados como hash SHA-256 (não plaintext) para segurança.

Inspirado em: Rerun (InboxSurface com histórico de ações), OpenDuet (approvals/service.ts com logging), awesome-grok-bot (Bot Fleet Auditor pattern).

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│ Approval Gate │────▶│ AuditStore │────▶│ SQLite DB │
│ Egress Checker │────▶│ (SHA-256 hash) │────▶│ action_audit_log│
│ Chat Actions │────▶│ │────▶│ │
└─────────────────┘     └──────────────────┘     └─────────────┘
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `AuditEntry`, `AuditStats`, `AuditFilter`, `CreateAuditEntry`, `AuditListResponse` |
| `store` | Persistência SQLite com sqlx, hash SHA-256 via sha2+hex, queries paginadas |

## Schema

### action_audit_log

| Column | Type | Description |
|--------|------|-------------|
| id | TEXT PK | UUID v4 |
| bot_id | TEXT | Identificador do bot que executou a ação |
| action_type | TEXT | Tipo: send_email, deploy, payment, sign_document, etc. |
| target | TEXT | Destinatário/recurso afetado (opcional) |
| approval_status | TEXT | approved, refused, auto_approved, expired |
| approval_decision | TEXT | allow_once, allow_session, allow_always, refuse |
| arguments_hash | TEXT | SHA-256 hex dos argumentos JSON (64 chars) |
| result_status | TEXT | success, failure, timeout |
| result_summary | TEXT | Mensagem curta do resultado |
| created_at | TEXT | ISO 8601 timestamp |
| decided_at | TEXT | Timestamp da decisão de approval |
| executed_at | TEXT | Timestamp da execução efetiva |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/audit/actions` | Lista ações paginadas com filtros (bot_id, action_type, approval_status) |
| GET | `/api/v1/audit/stats?period=7d` | Estatísticas agregadas por período |
| POST | `/api/v1/audit/actions` | Registra nova ação (interno, chamado por outros módulos) |
| GET | `/api/v1/audit/export?since=X&format=csv` | Export CSV filtrado |

## Security

- **Arguments NUNCA persistidos em plaintext**: apenas hash SHA-256
- **Hash determinístico**: mesmo input → mesmo hash (para deduplicação futura)
- **Imutável**: entradas não podem ser atualizadas/deletadas (append-only)
- **Retenção configurável**: default 90 dias, cleanup job periódico

## Testing

```bash
cargo test -p zylos-audit-log
```

- **9 testes unitários** cobrindo: create/list, arguments hashing (não plaintext), filter by bot_id, stats aggregation, pagination, hash determinism, hash uniqueness, schema serialization, filter defaults
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON para API |
| serde_json | workspace | Parsing de request/response |
| sqlx | 0.8 (sqlite) | Persistência async SQLite |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601 |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |
| sha2 | 0.10 | Hash SHA-256 dos argumentos |
| hex | 0.4 | Encoding hex do hash |

## Related Documents

- **POP:** [pop-audit-log-export.md](../../playbooks/pop-audit-log-export.md)
- **Runbook:** [rb-zylos-audit-log.md](../../runbooks/rb-zylos-audit-log.md)
- **SOAR:** [soar-audit-anomaly.md](../../docs/soar/soar-audit-anomaly.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.4
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)