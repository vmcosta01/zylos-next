//! # zylos-spaces
//!
//! Spaces/Docs Tab para Zylos Cockpit v2 — sistema de documentos hierárquicos
//! com editor markdown, outline lateral e persistência SQLite.
//!
//! ## Propósito
//! Fornecer um sistema de documentação persistente integrado ao cockpit,
//! onde usuários podem criar Spaces (containers), Pages (documentos markdown)
//! com hierarquia pai-filho, outline para navegação e autosave.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (Space, Page, PageTreeNode, CreatePageRequest)
//! - `store`: persistência SQLite com sqlx, queries hierárquicas, upsert
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/spaces | Lista todos os spaces |
//! | POST | /api/v1/spaces | Cria novo space |
//! | GET | /api/v1/spaces/:id/pages | Lista páginas do space (com hierarquia) |
//! | POST | /api/v1/spaces/:id/pages | Cria nova página no space |
//! | GET | /api/v1/pages/:id | Busca página por ID |
//! | PUT | /api/v1/pages/:id | Atualiza conteúdo/título da página |
//! | DELETE | /api/v1/pages/:id | Deleta página (e subpáginas) |
//! | GET | /api/v1/pages/:id/outline | Retorna outline (árvore de headings) |
//!
//! ## Related Documents
//! - Playbook: [pb-spaces-onboarding.md](../../playbooks/pb-spaces-onboarding.md)
//! - Runbook: [rb-zylos-spaces.md](../../runbooks/rb-zylos-spaces.md)
//! - POP: [pop-space-page-recovery.md](../../playbooks/pop-space-page-recovery.md)
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.5

pub mod schema;
pub mod store;

pub use schema::*;
pub use store::SpacesStore;
