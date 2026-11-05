//! zylos-soar: Security Orchestration, Automation and Response engine.
//!
//! Phase 1: Event intake, auto-triage, audit trail primitives.

pub mod event;
pub mod triage;
pub mod triage_message;
pub mod audit;
pub mod playbook;
pub mod runnable;
pub mod registry;
pub mod executor;
pub mod router;
pub mod webhook;
pub mod orchestrator;
pub mod validation;
pub mod export;
pub mod error;

pub use event::{ZylosSecurityEvent, Severity, EventCategory};
pub use triage::{TriageEngine, TriageDecision, TriageAction};
pub use audit::{AuditEntry, AuditTrail};
pub use playbook::{Playbook, PlaybookState, PlaybookCursor, Transition, PlaybookRoles, PlaybookTrigger};
pub use runnable::{SoarRunnable, RunbookResult, IncidentContext, BoxFuture};
pub use registry::RunbookRegistry;
pub use executor::{PlaybookExecutor, ExecutorConfig, StateExecutionResult};
pub use error::SoarError;