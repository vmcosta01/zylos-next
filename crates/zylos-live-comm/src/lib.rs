//! zylos-live-comm — AG-UI streaming, EventBus, Cockpit API, Hermes Bridge
//!
//! Part of Zylos-Next v6.0 — Fase 3 (LIVE_COMM_SYSTEM)

pub mod event_bus;
pub mod event_types;
pub mod event_persistence;
pub mod event_filter;
pub mod agui;
pub mod routes;
pub mod ws;
pub mod cockpit;
pub mod hermes_bridge;
pub mod skills_api;
pub mod auth;
pub mod rate_limit;
pub mod openapi;
pub mod security;
pub mod cache;
pub mod soar_bridge;
pub mod approval;
pub mod task_durability;
pub mod outcome;
pub mod triage;
pub mod equivalence_approval;
pub mod sentinel;
pub mod error;
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
