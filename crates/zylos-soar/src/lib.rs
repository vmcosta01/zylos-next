//! zylos-soar: Security Orchestration, Automation and Response engine.
//!
//! Phase 1: Event intake, auto-triage, audit trail primitives.

/// Módulo audit.
pub mod audit;
/// Módulo error.
pub mod error;
/// Módulo event.
pub mod event;
/// Módulo executor.
pub mod executor;
/// Módulo export.
pub mod export;
/// Módulo loader: filesystem loaders for runbooks/playbooks from YAML specs.
pub mod loader;
/// Módulo orchestrator.
pub mod orchestrator;
/// Módulo playbook.
pub mod playbook;
/// Módulo registry.
pub mod registry;
/// Módulo router.
pub mod router;
/// Módulo runnable.
pub mod runnable;
/// Módulo triage.
pub mod triage;
/// Módulo triage message.
pub mod triage_message;
/// Módulo validation.
pub mod validation;
/// Módulo webhook.
pub mod webhook;
/// Módulo yaml_runbook: YAML-driven runbook bridge implementing SoarRunnable.
pub mod yaml_runbook;

pub use audit::{AuditEntry, AuditTrail};
pub use error::SoarError;
pub use event::{EventCategory, Severity, ZylosSecurityEvent};
pub use executor::{ExecutorConfig, PlaybookExecutor, StateExecutionResult};
pub use playbook::{
    Playbook, PlaybookCursor, PlaybookRoles, PlaybookState, PlaybookTrigger, Transition,
};
pub use registry::RunbookRegistry;
pub use runnable::{BoxFuture, IncidentContext, RunbookResult, SoarRunnable};
pub use triage::{TriageAction, TriageDecision, TriageEngine};
