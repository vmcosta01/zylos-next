//! zylos-live-comm — AG-UI streaming, EventBus, Cockpit API, Hermes Bridge
//!
//! Part of Zylos-Next v6.0 — Fase 3 (LIVE_COMM_SYSTEM)

/// Módulo event bus.
pub mod event_bus;
/// Módulo event types.
pub mod event_types;
/// Módulo event persistence.
pub mod event_persistence;
/// Módulo event filter.
pub mod event_filter;
/// Eventos e streaming AG-UI para comunicação em tempo real.
pub mod agui;
/// Rotas HTTP e endpoints da API live-comm.
pub mod routes;
/// Módulo ws.
pub mod ws;
/// Módulo cockpit.
pub mod cockpit;
/// Módulo hermes bridge.
pub mod hermes_bridge;
/// Módulo skills api.
pub mod skills_api;
/// Módulo auth.
pub mod auth;
/// Módulo rate limit.
pub mod rate_limit;
/// Módulo openapi.
pub mod openapi;
/// Módulo security.
pub mod security;
/// Módulo cache.
pub mod cache;
/// Módulo soar bridge.
pub mod soar_bridge;
/// Módulo approval.
pub mod approval;
/// Módulo task durability.
pub mod task_durability;
/// Módulo outcome.
pub mod outcome;
/// Módulo triage.
pub mod triage;
/// Módulo equivalence approval.
pub mod equivalence_approval;
/// Módulo sentinel.
pub mod sentinel;
/// Tipos e estruturas de erros para live-comm.
pub mod error;
/// Gateway HTTP com rate limiting, RBAC e graceful shutdown.
pub mod gateway;

pub use event_bus::EventBus;
pub use agui::AgUiEvent;
pub use error::LiveCommError;
pub use event_types::{ZylosEvent, CustomEvent};

// Gateway re-exports
pub use gateway::{
    build_gateway, GatewayConfig, GatewayState, GracefulShutdown,
        MultiTierRateLimiter, RbacCache, RbacLevel, ReadinessChecker,
        TenantContext, CorrelationId, HealthResponse,
};
