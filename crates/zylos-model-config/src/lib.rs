//! # zylos-model-config
//!
//! Multi-Model Selector para Zylos Cockpit v2 — configuração de modelos
//! por bot com tier strategy, reasoning effort e herança global→bot→conversation.
//!
//! ## Propósito
//! Permitir que cada bot tenha configuração individual de modelo LLM,
//! com hierarquia de resolução: conversation > bot > global default.
//! Suporta tier strategy (cheapest/balanced/fastest) e reasoning effort.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (ModelConfig, TierStrategy, ReasoningEffort)
//! - `store`: persistência SQLite com sqlx, resolução hierárquica
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/models/config?bot_id=X | Config efetiva do bot (com herança) |
//! | PUT | /api/v1/models/config/:bot_id | Define config específica do bot |
//! | DELETE | /api/v1/models/config/:bot_id | Remove override do bot (volta ao global) |
//! | GET | /api/v1/models/list | Lista modelos disponíveis por provider |
//!
//! ## Related Documents
//! - Playbook: [pb-model-tier-strategy.md](../../playbooks/pb-model-tier-strategy.md)
//! - Runbook: [rb-zylos-model-config.md](../../runbooks/rb-zylos-model-config.md)
//! - POP: [pop-model-fallback.md](../../playbooks/pop-model-fallback.md)
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.3

pub mod schema;
pub mod store;

pub use schema::*;
pub use store::ModelConfigStore;
