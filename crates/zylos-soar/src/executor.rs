//! Playbook executor engine: drives a playbook through its states using registered runbooks.

use crate::audit::{AuditResult, AuditTrail};
use crate::error::{Result, SoarError};
use crate::playbook::{Playbook, PlaybookCursor};
use crate::registry::RunbookRegistry;
use crate::runnable::{IncidentContext, RunbookResult};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Outcome of executing a single playbook state.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura StateExecutionResult.
pub struct StateExecutionResult {
    /// Campo state name.
    pub state_name: String,
    /// Campo runbook name.
    pub runbook_name: Option<String>,
    /// Campo runbook result.
    pub runbook_result: Option<RunbookResult>,
    /// Campo audit entry id.
    pub audit_entry_id: Uuid,
    /// Campo next state.
    pub next_state: Option<String>,
}

/// Configuration for the executor engine.
#[derive(Debug, Clone)]
/// Estrutura ExecutorConfig.
pub struct ExecutorConfig {
    /// Maximum time (seconds) to wait for a runbook execution before timing out.
    pub default_timeout_secs: u64,
    /// Number of retry attempts for transient failures.
    pub max_retries: u32,
    /// Delay between retries in milliseconds.
    pub retry_delay_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        // Variante Self.
        Self {
            default_timeout_secs: 300,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Drives a playbook through its state machine, executing runbooks and recording audit entries.
pub struct PlaybookExecutor<'a> {
    registry: &'a RunbookRegistry,
    audit_trail: &'a mut AuditTrail,
    config: ExecutorConfig,
}

impl<'a> PlaybookExecutor<'a> {
    /// Função new.
    pub fn new(
        registry: &'a RunbookRegistry,
        audit_trail: &'a mut AuditTrail,
        config: ExecutorConfig,
    ) -> Self {
        // Variante Self.
        Self {
            registry,
            audit_trail,
            config,
        }
    }

    /// Execute a single state of a playbook.
    pub async fn execute_state(
        &mut self,
        playbook: &Playbook,
        cursor: &mut PlaybookCursor,
        context: &mut IncidentContext,
    ) -> Result<StateExecutionResult> {
        let state = playbook
            .find_state(&cursor.current_state)
            .ok_or_else(|| {
                SoarError::PlaybookParse(format!(
                    "state '{}' not found in playbook '{}'",
                    cursor.current_state, playbook.name
                ))
            })?;

        let start = Utc::now();
        let state_hash = format!("{}:{}:{}", playbook.name, cursor.current_state, start.timestamp_millis());

        // If state has a runbook, look it up and execute
        let (runbook_result, audit_result) = if let Some(ref runbook_name) = state.runbook {
            let runbook = self.registry.get(runbook_name).ok_or_else(|| {
                SoarError::RunbookExecution(format!(
                    "runbook '{}' not found in registry",
                    runbook_name
                ))
            })?;

            // Validate preconditions
            runbook.validate_preconditions(context).await.map_err(|e| {
                SoarError::RunbookExecution(format!(
                    "precondition validation failed for '{}': {}",
                    runbook_name, e
                ))
            })?;

            // Execute with retries
            let mut last_error = None;
            let mut result = None;

            for attempt in 0..=self.config.max_retries {
                match runbook.execute(context.clone()).await {
                    // Variante Ok.
                    Ok(r) => {
                        result = Some(r);
                        break;
                    }
                    // Variante Err.
                    Err(e) => {
                        last_error = Some(e);
                        if attempt < self.config.max_retries {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                self.config.retry_delay_ms,
                            ))
                            .await;
                        }
                    }
                }
            }

            match result {
                // Variante Some.
                Some(r) if r.success => (Some(r), AuditResult::Success),
                // Variante Some.
                Some(r) => (
                    // Variante Some.
                    Some(r.clone()),
                    AuditResult::Failure(r.message.clone()),
                ),
                // Variante None.
                None => {
                    let err_msg = last_error
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "unknown error".to_string());
                    (None, AuditResult::Failure(err_msg))
                }
            }
        } else {
            // No runbook — auto-triage or terminal state
            (None, AuditResult::Success)
        };

        // Record audit entry
        let duration_ms = (Utc::now() - start).num_milliseconds().max(0) as u64;
        let audit_entry_id = self.audit_trail.append(
            context.incident_id,
            context.actor.clone(),
            format!("execute_state:{}", cursor.current_state),
            serde_json::json!({
                "playbook": playbook.name,
                "state": cursor.current_state,
                "runbook": state.runbook,
                "duration_ms": duration_ms,
                "result": match &audit_result {
                    AuditResult::Success => "success",
                    AuditResult::Failure(_) => "failure",
                    AuditResult::Timeout => "timeout",
                    AuditResult::Skipped(_) => "skipped",
                },
            }),
            audit_result.clone(),
            &state_hash,
        )?;

        // Advance cursor
        let next_state = cursor.advance(playbook, &context.metadata)?;

        // Variante Ok.
        Ok(StateExecutionResult {
            state_name: state.name.clone(),
            runbook_name: state.runbook.clone(),
            runbook_result,
            audit_entry_id,
            next_state,
        })
    }

    /// Execute all remaining states of a playbook until a terminal state is reached.
    pub async fn execute_full(
        &mut self,
        playbook: &Playbook,
        cursor: &mut PlaybookCursor,
        context: &mut IncidentContext,
    ) -> Result<Vec<StateExecutionResult>> {
        let mut results = Vec::new();

        loop {
            let state = playbook
                .find_state(&cursor.current_state)
                .ok_or_else(|| {
                    SoarError::PlaybookParse(format!(
                        "state '{}' not found in playbook '{}'",
                        cursor.current_state, playbook.name
                    ))
                })?;

            let result = self.execute_state(playbook, cursor, context).await?;
            results.push(result.clone());

            // Stop at terminal states
            if state.terminal || result.next_state.is_none() {
                break;
            }

            // Stop if awaiting approval (human gate)
            if cursor.awaiting_approval {
                break;
            }
        }

        // Variante Ok.
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runnable::{BoxFuture, SoarRunnable};

    const TEST_PLAYBOOK_YAML: &str = r#"
name: test-playbook
version: "1.0"
states:
  - name: step1
    auto: true
    runbook: "mock-runbook"
    next:
      - to: step2
  - name: step2
    auto: true
    terminal: true
"#;

    struct MockRunbook;

    impl SoarRunnable for MockRunbook {
        fn name(&self) -> &str {
            "mock-runbook"
        }

        fn validate_preconditions<'a>(
            &'a self,
            _ctx: &'a IncidentContext,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn execute<'a>(
            &'a self,
            _ctx: IncidentContext,
        ) -> BoxFuture<'a, Result<RunbookResult>> {
            Box::pin(async { Ok(RunbookResult::ok("mock done", 5)) })
        }
    }

    #[tokio::test]
    async fn executor_drives_playbook_to_terminal() {
        let playbook = Playbook::from_yaml(TEST_PLAYBOOK_YAML).unwrap();
        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(MockRunbook));

        let mut audit_trail = AuditTrail::new(b"test-key");
        let config = ExecutorConfig::default();
        let mut executor = PlaybookExecutor::new(&registry, &mut audit_trail, config);

        let mut cursor = PlaybookCursor::new(&playbook);
        let mut context = IncidentContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "high",
            "breach",
            vec!["host-01".to_string()],
            "test-executor",
        );

        let results = executor
            .execute_full(&playbook, &mut cursor, &mut context)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].state_name, "step1");
        assert_eq!(results[0].runbook_name.as_deref(), Some("mock-runbook"));
        assert!(results[0].runbook_result.as_ref().unwrap().success);
        assert_eq!(results[1].state_name, "step2");
        assert_eq!(results[1].next_state, None);

        // Verify audit trail has entries
        assert_eq!(audit_trail.entries().len(), 2);
    }

    #[tokio::test]
    async fn executor_fails_on_missing_runbook() {
        let playbook = Playbook::from_yaml(TEST_PLAYBOOK_YAML).unwrap();
        let registry = RunbookRegistry::new(); // empty — no runbooks registered

        let mut audit_trail = AuditTrail::new(b"test-key");
        let config = ExecutorConfig::default();
        let mut executor = PlaybookExecutor::new(&registry, &mut audit_trail, config);

        let mut cursor = PlaybookCursor::new(&playbook);
        let mut context = IncidentContext::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "high",
            "breach",
            vec![],
            "test-executor",
        );

        let result = executor
            .execute_full(&playbook, &mut cursor, &mut context)
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("mock-runbook"));
    }
}