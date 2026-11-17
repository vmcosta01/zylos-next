//! # zylos-audit-log
//!
//! Action audit trail para Zylos Cockpit v2 — registra todas as ações
//! externas com hash de argumentos para rastreabilidade sem expor dados sensíveis.
//!
//! ## Propósito
//! Fornecer registro centralizado e imutável de todas as ações executadas
//! pelos agentes (envio de mensagens, deploys, pagamentos, aprovações),
//! permitindo auditoria forense, compliance e detecção de anomalias.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (AuditEntry, AuditStats, filtros)
//! - `store`: persistência SQLite com sqlx, queries otimizadas
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/audit/actions | Lista ações paginadas com filtros |
//! | GET | /api/v1/audit/stats | Estatísticas agregadas por período |
//! | POST | /api/v1/audit/actions | Registra nova ação (interno) |
//! | GET | /api/v1/audit/export | Export CSV filtrado |
//!
//! ## Related Documents
//! - POP: [pop-audit-log-export.md](../../playbooks/pop-audit-log-export.md)
//! - Runbook: [rb-zylos-audit-log.md](../../runbooks/rb-zylos-audit-log.md)
//! - SOAR: [soar-audit-anomaly.md](../../docs/soar/soar-audit-anomaly.md)
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.4

pub mod schema;
pub mod store;

pub use schema::*;
pub use store::AuditStore;
