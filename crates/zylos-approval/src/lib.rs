//! # zylos-approval
//!
//! Approval gates HITL (Human-in-the-Loop) para Zylos Cockpit v2.
//!
//! ## Propósito
//! Garantir que ações externas (enviar email, deploy, pagamento) exigem
//! confirmação humana com 3 opções: allow-once, allow-always, refuse.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (ApprovalRequest, ApprovalRule, enums)
//! - `store`: persistência SQLite com sqlx
//! - `error`: erros tipados com thiserror
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/approvals/pending | Lista aprovações pendentes |
//! | GET | /api/v1/approvals/:id | Detalhes de uma aprovação |
//! | POST | /api/v1/approvals/:id/decide | Registra decisão do usuário |
//! | GET | /api/v1/approvals/rules | Lista regras persistentes |
//! | DELETE | /api/v1/approvals/rules/:id | Revoga uma regra |
//!
//! ## Related Documents
//! - Playbook: [pb-approval-gate-setup.md](../../playbooks/pb-approval-gate-setup.md)
//! - Runbook: [rb-zylos-approval.md](../../runbooks/rb-zylos-approval.md)
//! - POP: [pop-approval-decide.md](../../playbooks/pop-approval-decide.md)
//! - SOAR: [soar-approval-timeout.md](../soar/soar-approval-timeout.md)

pub mod error;
pub mod schema;
pub mod store;

pub use error::ApprovalError;
pub use schema::*;
pub use store::ApprovalStore;
