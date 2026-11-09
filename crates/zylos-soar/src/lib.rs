//! zylos-soar: Security Orchestration, Automation and Response engine.
//!
//! Phase 1: Event intake, auto-triage, audit trail primitives.

/// Módulo event.
pub mod event;
/// Módulo triage.
pub mod triage;
/// Módulo triage message.
pub mod triage_message;
/// Módulo audit.
pub mod audit;
/// Módulo playbook.
pub mod playbook;
/// Módulo runnable.
pub mod runnable;
/// Módulo registry.
pub mod registry;
/// Módulo executor.
pub mod executor;
/// Módulo router.
pub mod router;
/// Módulo webhook.
pub mod webhook;
/// Módulo orchestrator.
pub mod orchestrator;
/// Módulo validation.
pub mod validation;
/// Módulo export.
pub mod export;
/// Módulo error.
pub mod error;
/// Módulo loader: filesystem loaders for runbooks/playbooks from YAML specs.
pub mod loader;
/// Módulo yaml_runbook: YAML-driven runbook bridge implementing SoarRunnable.
pub mod yaml_runbook;

pub use event::{ZylosSecurityEvent, Severity, EventCategory};
pub use triage::{TriageEngine, TriageDecision, TriageAction};
pub use audit::{AuditEntry, AuditTrail};
pub use playbook::{Playbook, PlaybookState, PlaybookCursor, Transition, PlaybookRoles, PlaybookTrigger};
pub use runnable::{SoarRunnable, RunbookResult, IncidentContext, BoxFuture};
pub use registry::RunbookRegistry;
pub use executor::{PlaybookExecutor, ExecutorConfig, StateExecutionResult};
pub use error::SoarError;