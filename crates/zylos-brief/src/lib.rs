//! # zylos-brief
//!
//! Morning Brief Generator para Zylos Cockpit v2 — briefing diário automático
//! agregando métricas, tarefas pendentes, calendário e itens não verificados.
//!
//! ## Propósito
//! Gerar um resumo matinal personalizado que responde:
//! 1. What needs you (aprovações pendentes, tarefas bloqueadas)
//! 2. What can wait (tarefas em andamento, goals com deadline > 7d)
//! 3. Calendar (eventos do dia, se integrado)
//! 4. Unchecked items (integrações offline/falhas recentes)
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (MorningBrief, BriefSection, BriefItem)
//! - `generator`: lógica de agregação de múltiplas fontes de dados
//! - `store`: persistência SQLite para evitar regeneração no mesmo dia
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/brief/today | Retorna brief do dia (gera se não existir) |
//! | POST | /api/v1/brief/regenerate | Força regeneração do brief |
//! | GET | /api/v1/brief/history?limit=N | Histórico de briefs anteriores |
//!
//! ## Related Documents
//! - Playbook: [pb-morning-brief-config.md](../../playbooks/pb-morning-brief-config.md)
//! - Runbook: [rb-zylos-brief.md](../../runbooks/rb-zylos-brief.md)
//! - POP: [pop-brief-regenerate.md](../../playbooks/pop-brief-regenerate.md)
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.1

pub mod generator;
pub mod schema;
pub mod store;

pub use generator::MorningBriefGenerator;
pub use schema::*;
pub use store::BriefStore;
