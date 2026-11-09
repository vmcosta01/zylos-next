//! Orchestrator: end-to-end SOAR pipeline wiring webhook ingest → router → executor → audit.

use crate::audit::AuditTrail;
use crate::error::Result;
use crate::event::ZylosSecurityEvent;
use crate::executor::{ExecutorConfig, PlaybookExecutor};
use crate::registry::RunbookRegistry;
use crate::router::EventRouter;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use zylos_live_comm::EventBus;
use zylos_live_comm::soar_bridge;

/// Outcome of processing a single event through the full SOAR pipeline.
#[derive(Debug, Clone, Serialize)]
/// Estrutura PipelineResult.
pub struct PipelineResult {
    /// Campo event id.
    pub event_id: Uuid,
    /// Campo dispatched playbooks.
    pub dispatched_playbooks: Vec<String>,
    /// Campo execution results.
    pub execution_results: Vec<PlaybookExecutionSummary>,
}

/// Summary of executing one playbook for an event.
#[derive(Debug, Clone, Serialize)]
/// Estrutura PlaybookExecutionSummary.
pub struct PlaybookExecutionSummary {
    /// Campo playbook name.
    pub playbook_name: String,
    /// Campo states executed.
    pub states_executed: usize,
    /// Campo success.
    pub success: bool,
    /// Campo error.
    pub error: Option<String>,
}

/// Shared orchestrator state — thread-safe access to all SOAR subsystems.
pub struct OrchestratorState {
    /// Campo router.
    pub router: EventRouter,
    /// Campo registry.
    pub registry: RunbookRegistry,
    /// Campo audit trail.
    pub audit_trail: AuditTrail,
    /// Campo executor config.
    pub executor_config: ExecutorConfig,
    /// Campo event bus.
    pub event_bus: Option<EventBus>,
}

impl OrchestratorState {
    /// Função new.
    pub fn new(
        router: EventRouter,
        registry: RunbookRegistry,
        audit_trail: AuditTrail,
        executor_config: ExecutorConfig,
    ) -> Self {
        // Variante Self.
        Self {
            router,
            registry,
            audit_trail,
            executor_config,
            event_bus: None,
        }
    }

    /// Attach an EventBus for real-time AG-UI streaming of SOAR events.
    pub fn with_event_bus(mut self, bus: EventBus) -> Self {
        self.event_bus = Some(bus);
        self
    }
}

/// The SOAR orchestrator: processes events through the full pipeline.
pub struct SoarOrchestrator {
    state: Arc<RwLock<OrchestratorState>>,
}

impl SoarOrchestrator {
    /// Função new.
    pub fn new(state: Arc<RwLock<OrchestratorState>>) -> Self {
        // Variante Self.
        Self { state }
    }

    /// List all registered playbooks.
    pub async fn list_playbooks(&self) -> Vec<serde_json::Value> {
        let state = self.state.read().await;
        state
            .router
            .playbooks()
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "version": p.version,
                    "triggers_count": p.triggers.len(),
                    "states_count": p.states.len(),
                })
            })
            .collect()
    }

    /// Process a single event: route → execute playbooks → record audit → publish to AG-UI.
    pub async fn process_event(&self, event: ZylosSecurityEvent) -> Result<PipelineResult> {
        let state = self.state.read().await;

        // Publish incoming security event to AG-UI dashboard
        if let Some(ref bus) = state.event_bus {
            soar_bridge::publish_soar_event(
                bus,
                &event.id.to_string(),
                &event.source,
                &event.severity.to_string(),
                &event.category.to_string(),
                &event.title,
                &event.description,
                &event.affected_assets,
                &event.received_at.to_rfc3339(),
            );
        }

        // Step 1: Route the event through playbooks
        let dispatched = state.router.route(&event)?;
        let event_id = event.id;

        let mut execution_results = Vec::new();

        // Step 2: Execute each dispatched playbook
        for incident in &dispatched {
            let playbook = &incident.playbook;

            // Publish playbook start to AG-UI
            if let Some(ref bus) = state.event_bus {
                soar_bridge::publish_playbook_started(bus, &playbook.name, &event_id.to_string());
            }

            let mut cursor = crate::playbook::PlaybookCursor::new(playbook);
            let mut context = crate::runnable::IncidentContext::new(
                event_id,
                event_id, // use event_id as incident_id for now
                &event.severity.to_string(),
                &event.category.to_string(),
                event.affected_assets.clone(),
                "soar-orchestrator",
            );

            // Placeholder audit trail for execution tracking.
            // In production, this would be a shared mutable audit store.
            let mut exec_audit = AuditTrail::new(b"placeholder");
            let mut executor = PlaybookExecutor::new(
                &state.registry,
                &mut exec_audit,
                state.executor_config.clone(),
            );

            match executor.execute_full(playbook, &mut cursor, &mut context).await {
                // Variante Ok.
                Ok(results) => {
                    let summary = PlaybookExecutionSummary {
                        playbook_name: playbook.name.clone(),
                        states_executed: results.len(),
                        success: true,
                        error: None,
                    };

                    // Publish playbook completion to AG-UI
                    if let Some(ref bus) = state.event_bus {
                        soar_bridge::publish_playbook_completed(
                            bus,
                            &summary.playbook_name,
                            &event_id.to_string(),
                            true,
                            summary.states_executed,
                            // Variante None.
                            None,
                        );
                    }

                    execution_results.push(summary);
                }
                // Variante Err.
                Err(e) => {
                    let err_msg = e.to_string();
                    let summary = PlaybookExecutionSummary {
                        playbook_name: playbook.name.clone(),
                        states_executed: 0,
                        success: false,
                        error: Some(err_msg.clone()),
                    };

                    // Publish playbook failure to AG-UI
                    if let Some(ref bus) = state.event_bus {
                        soar_bridge::publish_playbook_completed(
                            bus,
                            &summary.playbook_name,
                            &event_id.to_string(),
                            false,
                            0,
                            // Variante Some.
                            Some(&err_msg),
                        );
                    }

                    execution_results.push(summary);
                }
            }
        }

        let playbook_names: Vec<String> = dispatched
            .iter()
            .map(|d| d.playbook.name.clone())
            .collect();

        // Variante Ok.
        Ok(PipelineResult {
            event_id,
            dispatched_playbooks: playbook_names,
            execution_results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EventCategory, Severity};
    use crate::playbook::Playbook;
    use crate::runnable::{BoxFuture, RunbookResult, SoarRunnable};
    use crate::router::EventRouter;

    const SIMPLE_PLAYBOOK_YAML: &str = r#"
name: test-orchestration
version: "1.0"
triggers:
  - expression: "event.category == \"breach\""
states:
  - name: respond
    auto: true
    runbook: "test-runbook"
    terminal: true
"#;

    struct TestRunbook;

    impl SoarRunnable for TestRunbook {
        fn name(&self) -> &str {
            "test-runbook"
        }

        fn validate_preconditions<'a>(
            &'a self,
            _ctx: &'a crate::runnable::IncidentContext,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn execute<'a>(
            &'a self,
            _ctx: crate::runnable::IncidentContext,
        ) -> BoxFuture<'a, Result<RunbookResult>> {
            Box::pin(async { Ok(RunbookResult::ok("orchestrated", 1)) })
        }
    }

    fn build_orchestrator() -> SoarOrchestrator {
        let mut router = EventRouter::new();
        let playbook = Playbook::from_yaml(SIMPLE_PLAYBOOK_YAML).unwrap();
        router.register_playbook(playbook);

        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(TestRunbook));

        let audit_trail = AuditTrail::new(b"test-key");
        let config = ExecutorConfig::default();

        let state = Arc::new(RwLock::new(OrchestratorState::new(
            router,
            registry,
            audit_trail,
            config,
        )));

        SoarOrchestrator::new(state)
    }

    #[tokio::test]
    async fn orchestrator_processes_matching_event() {
        let orch = build_orchestrator();

        let event = ZylosSecurityEvent::new(
            "test",
            Severity::Critical,
            EventCategory::Breach,
            "Test Breach",
            "Orchestrator test",
            vec!["host-01".to_string()],
            serde_json::json!({}),
        );

        let result = orch.process_event(event).await.unwrap();
        assert_eq!(result.dispatched_playbooks.len(), 1);
        assert_eq!(result.dispatched_playbooks[0], "test-orchestration");
        assert_eq!(result.execution_results.len(), 1);
        assert!(result.execution_results[0].success);
        assert_eq!(result.execution_results[0].states_executed, 1);
    }

    #[tokio::test]
    async fn orchestrator_handles_non_matching_event() {
        let orch = build_orchestrator();

        let event = ZylosSecurityEvent::new(
            "test",
            Severity::Low,
            EventCategory::Anomaly,
            "Test Anomaly",
            "Should not match",
            vec![],
            serde_json::json!({}),
        );

        let result = orch.process_event(event).await.unwrap();
        assert!(result.dispatched_playbooks.is_empty());
        assert!(result.execution_results.is_empty());
    }
}