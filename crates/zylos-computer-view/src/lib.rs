//! # zylos-computer-view
//!
//! Computer View Panel para Zylos Cockpit v2 — gerenciamento de sessões
//! de visualização remota de desktop do agente com controle humano.
//!
//! ## Propósito
//! Permitir que o usuário visualize e controle o desktop/browser do agente
//! em tempo real via VNC/WebSocket proxy, com suporte a take/release control
//! e fallback para screenshots estáticos quando VNC indisponível.
//!
//! ## Arquitetura
//! - `schema`: tipos de dados (ComputerSession, SessionState, ControlLease)
//! - `session`: gerenciamento de sessões ativas, leases de controle, heartbeat
//!
//! ## API Endpoints
//! | Method | Path | Description |
//! |--------|------|-------------|
//! | GET | /api/v1/computer/sessions | Lista sessões ativas |
//! | POST | /api/v1/computer/sessions | Cria nova sessão de computer view |
//! | POST | /api/v1/computer/sessions/:id/take-control | Usuário assume controle |
//! | POST | /api/v1/computer/sessions/:id/release-control | Usuário devolve controle ao agente |
//! | DELETE | /api/v1/computer/sessions/:id | Encerra sessão |
//! | GET | /api/v1/computer/sessions/:id/screenshot | Screenshot estático (fallback) |
//!
//! ## Related Documents
//! - Playbook: [pb-computer-view-setup.md](../../playbooks/pb-computer-view-setup.md)
//! - Runbook: [rb-zylos-computer-view.md](../../runbooks/rb-zylos-computer-view.md)
//! - POP: [pop-computer-control-emergency.md](../../playbooks/pop-computer-control-emergency.md)
//! - SOAR: [soar-computer-control-breach.md](../../docs/soar/soar-computer-control-breach.md)
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.4

pub mod schema;
pub mod session;

pub use schema::*;
pub use session::ComputerSessionManager;
