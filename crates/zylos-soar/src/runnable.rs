//! Runbook executor interface: trait for atomic executable units.

use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

/// Type alias for boxed futures returned by async trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Context passed to every runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentContext {
    pub incident_id: Uuid,
    pub event_id: Uuid,
    pub severity: String,
    pub category: String,
    pub affected_assets: Vec<String>,
    pub actor: String,
    pub started_at: DateTime<Utc>,
    /// Arbitrary key-value data accumulated during playbook execution.
    pub metadata: serde_json::Value,
}

impl IncidentContext {
    pub fn new(
        incident_id: Uuid,
        event_id: Uuid,
        severity: impl Into<String>,
        category: impl Into<String>,
        affected_assets: Vec<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self {
            incident_id,
            event_id,
            severity: severity.into(),
            category: category.into(),
            affected_assets,
            actor: actor.into(),
            started_at: Utc::now(),
            metadata: serde_json::json!({}),
        }
    }
}

/// Outcome of a runbook execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunbookResult {
    pub success: bool,
    pub message: String,
    pub artifacts: Vec<String>,
    pub duration_ms: u64,
    pub metadata: serde_json::Value,
}

impl RunbookResult {
    pub fn ok(message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: true,
            message: message.into(),
            artifacts: Vec::new(),
            duration_ms,
            metadata: serde_json::json!({}),
        }
    }

    pub fn fail(message: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            success: false,
            message: message.into(),
            artifacts: Vec::new(),
            duration_ms,
            metadata: serde_json::json!({}),
        }
    }
}

/// Trait that every SOAR-executable runbook must implement.
///
/// Provides pre-condition validation, execution, and optional rollback.
/// Uses `BoxFuture` return types instead of `async fn` to remain dyn-compatible.
pub trait SoarRunnable: Send + Sync {
    /// Human-readable name of this runbook.
    fn name(&self) -> &str;

    /// Validate pre-conditions before execution.
    fn validate_preconditions<'a>(
        &'a self,
        context: &'a IncidentContext,
    ) -> BoxFuture<'a, Result<()>>;

    /// Execute the runbook and return a structured result.
    fn execute<'a>(&'a self, context: IncidentContext) -> BoxFuture<'a, Result<RunbookResult>>;

    /// Optional rollback procedure if execution fails mid-way.
    fn rollback(&self) -> Option<Box<dyn SoarRunnable>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopRunbook;

    impl SoarRunnable for NoopRunbook {
        fn name(&self) -> &str {
            "noop"
        }

        fn validate_preconditions<'a>(
            &'a self,
            _context: &'a IncidentContext,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn execute<'a>(
            &'a self,
            _context: IncidentContext,
        ) -> BoxFuture<'a, Result<RunbookResult>> {
            Box::pin(async { Ok(RunbookResult::ok("noop executed", 0)) })
        }
    }

    #[tokio::test]
    async fn noop_runbook_executes() {
        let runbook = NoopRunbook;
        let ctx = IncidentContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "critical",
            "breach",
            vec!["host-01".to_string()],
            "test-agent",
        );

        runbook.validate_preconditions(&ctx).await.unwrap();
        let result = runbook.execute(ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.message, "noop executed");
    }

    #[test]
    fn incident_context_creation() {
        let ctx = IncidentContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "high",
            "anomaly",
            vec!["svc-01".to_string()],
            "soar-engine",
        );
        assert_eq!(ctx.severity, "high");
        assert_eq!(ctx.category, "anomaly");
        assert_eq!(ctx.affected_assets.len(), 1);
    }

    #[test]
    fn runbook_result_constructors() {
        let ok = RunbookResult::ok("done", 150);
        assert!(ok.success);
        assert_eq!(ok.duration_ms, 150);

        let fail = RunbookResult::fail("timeout", 30000);
        assert!(!fail.success);
        assert_eq!(fail.message, "timeout");
    }

    #[test]
    fn soar_runnable_is_dyn_compatible() {
        // Verify the trait can be used as a trait object
        let runbook: Box<dyn SoarRunnable> = Box::new(NoopRunbook);
        assert_eq!(runbook.name(), "noop");
        assert!(runbook.rollback().is_none());
    }
}