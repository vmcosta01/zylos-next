//! zylos-live-comm — AG-UI streaming, EventBus, Cockpit API, Hermes Bridge
//!
//! Part of Zylos-Next v6.0 — Fase 3 (LIVE_COMM_SYSTEM)

/// Eventos e streaming AG-UI para comunicação em tempo real.
pub mod agui;
/// Módulo approval.
pub mod approval;
/// Módulo auth.
pub mod auth;
/// Módulo cache.
pub mod cache;
/// Módulo cockpit.
pub mod cockpit;
/// Módulo equivalence approval.
pub mod equivalence_approval;
/// Tipos e estruturas de erros para live-comm.
pub mod error;
/// Módulo event bus.
pub mod event_bus;
/// Módulo event filter.
pub mod event_filter;
/// Módulo event persistence.
pub mod event_persistence;
/// Módulo event types.
pub mod event_types;
/// Gateway HTTP com rate limiting, RBAC e graceful shutdown.
pub mod gateway;
/// Módulo hermes bridge.
pub mod hermes_bridge;
/// Módulo openapi.
pub mod openapi;
/// Módulo outcome.
pub mod outcome;
/// Módulo rate limit.
pub mod rate_limit;
/// Rotas HTTP e endpoints da API live-comm.
pub mod routes;
/// Módulo security.
pub mod security;
/// Módulo sentinel.
pub mod sentinel;
/// Módulo skills api.
pub mod skills_api;
/// Módulo soar bridge.
pub mod soar_bridge;
/// Módulo task durability.
pub mod task_durability;
/// Módulo triage.
pub mod triage;
/// Módulo ws.
pub mod ws;

pub use agui::AgUiEvent;
pub use error::LiveCommError;
pub use event_bus::EventBus;
pub use event_types::{CustomEvent, ZylosEvent};

// Gateway re-exports
pub use gateway::{
    build_gateway, CorrelationId, GatewayConfig, GatewayState, GracefulShutdown, HealthResponse,
    MultiTierRateLimiter, RbacCache, RbacLevel, ReadinessChecker, TenantContext,
};
