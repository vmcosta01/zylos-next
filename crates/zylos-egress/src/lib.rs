//! # zylos-egress
//!
//! Verificador de egress — detecta PII, secrets e dados sensíveis
//! antes de enviar mensagens para destinos externos.
//!
//! ## Propósito
//! Prevenir vazamento de dados sensíveis (emails, telefones, CPF/CNPJ,
//! cartões de crédito, API keys, passwords) verificando texto no frontend
//! antes do envio e no backend como segunda linha de defesa.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (SensitiveCategory, SensitiveFinding, EgressCheckResult, EgressRule)
//! - `checker`: engine de detecção com patterns built-in + regras customizáveis
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | POST | /api/v1/egress/check | Verifica texto e retorna findings + versão redigida |
//! | GET | /api/v1/egress/rules | Lista regras customizadas |
//! | POST | /api/v1/egress/rules | Cria regra customizada |
//! | DELETE | /api/v1/egress/rules/:id | Remove regra |
//!
//! ## Related Documents
//! - Playbook: [pb-egress-rules-management.md](../../playbooks/pb-egress-rules-management.md)
//! - Runbook: [rb-zylos-egress.md](../../runbooks/rb-zylos-egress.md)
//! - POP: [pop-egress-false-positive.md](../../playbooks/pop-egress-false-positive.md)
//! - SOAR: [soar-egress-block-response.md](../../docs/soar/soar-egress-block-response.md)

pub mod checker;
pub mod schema;

pub use checker::EgressChecker;
pub use schema::*;
