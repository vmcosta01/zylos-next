//! YAML-driven runbook bridge: wraps RunbookSpec into SoarRunnable implementations.

use crate::error::{Result, SoarError};
use crate::loader::RunbookSpec;
use crate::runnable::{BoxFuture, IncidentContext, RunbookResult, SoarRunnable};
use std::process::Command;
use std::time::Instant;

/// A runbook implementation that executes commands defined in YAML specs.
pub struct YamlRunbook {
    spec: RunbookSpec,
}

impl YamlRunbook {
    /// Create a new YamlRunbook from a parsed spec.
    pub fn new(spec: RunbookSpec) -> Self {
        Self { spec }
    }

    /// Execute a shell command and return (exit_code, stdout, stderr).
    fn run_command(cmd: &str, _timeout_secs: Option<u32>) -> Result<(i32, String, String)> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", cmd]).output()
        } else {
            Command::new("sh").args(["-c", cmd]).output()
        };

        match output {
            Ok(out) => {
                let exit_code = out.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                Ok((exit_code, stdout, stderr))
            }
            Err(e) => Err(SoarError::Execution(format!(
                "Failed to execute command '{}': {}",
                cmd, e
            ))),
        }
    }
}

impl SoarRunnable for YamlRunbook {
    fn name(&self) -> &str {
        &self.spec.metadata.name
    }

    fn validate_preconditions<'a>(
        &'a self,
        _context: &'a IncidentContext,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async {
            for precondition in &self.spec.preconditions {
                let (exit_code, stdout, _stderr) = Self::run_command(&precondition.command, None)?;

                let passed = if let Some(ref expected) = precondition.expected {
                    if expected.starts_with("exit_code:") {
                        let expected_code: i32 = expected
                            .strip_prefix("exit_code:")
                            .unwrap()
                            .parse()
                            .unwrap_or(0);
                        exit_code == expected_code
                    } else {
                        stdout.trim() == expected.trim()
                    }
                } else {
                    exit_code == 0
                };

                if !passed {
                    let on_failure = precondition.on_failure.as_deref().unwrap_or("abort");
                    if on_failure == "abort" {
                        return Err(SoarError::Execution(format!(
                            "Precondition failed: {} (command: {})",
                            precondition.check, precondition.command
                        )));
                    }
                }
            }
            Ok(())
        })
    }

    fn execute<'a>(&'a self, _context: IncidentContext) -> BoxFuture<'a, Result<RunbookResult>> {
        Box::pin(async {
            let start = Instant::now();
            let artifacts = Vec::new();
            let mut all_output = String::new();

            for step in &self.spec.steps {
                if let Some(ref cmd) = step.command {
                    let timeout = step.timeout_seconds;
                    let result = Self::run_command(cmd, timeout);

                    match result {
                        Ok((exit_code, stdout, stderr)) => {
                            all_output.push_str(&format!(
                                "[{}] exit={} stdout={} stderr={}\n",
                                step.id,
                                exit_code,
                                stdout.trim(),
                                stderr.trim()
                            ));

                            if exit_code != 0 {
                                let on_failure = step.on_failure.as_deref().unwrap_or("abort");
                                if on_failure == "abort" {
                                    return Ok(RunbookResult::fail(
                                        format!(
                                            "Step {} failed with exit code {}",
                                            step.id, exit_code
                                        ),
                                        start.elapsed().as_millis() as u64,
                                    ));
                                }
                            }
                        }
                        Err(e) => {
                            return Ok(RunbookResult::fail(
                                format!("Step {} execution error: {}", step.id, e),
                                start.elapsed().as_millis() as u64,
                            ));
                        }
                    }
                }
            }

            // Run validation checks
            for validation in &self.spec.validation {
                let (exit_code, _stdout, _stderr) = Self::run_command(&validation.command, None)?;

                if exit_code != 0 {
                    return Ok(RunbookResult::fail(
                        format!("Validation failed: {}", validation.check),
                        start.elapsed().as_millis() as u64,
                    ));
                }
            }

            Ok(RunbookResult {
                success: true,
                message: format!("Executed {} steps", self.spec.steps.len()),
                artifacts,
                duration_ms: start.elapsed().as_millis() as u64,
                metadata: serde_json::json!({
                    "output": all_output,
                    "steps_count": self.spec.steps.len()
                }),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{RunbookMetadata, RunbookStep};

    #[tokio::test]
    async fn yaml_runbook_executes_simple_command() {
        let spec = RunbookSpec {
            metadata: RunbookMetadata {
                name: "test-runbook".to_string(),
                version: "1.0".to_string(),
                trigger: "test".to_string(),
                audience: vec![],
                estimated_duration_minutes: 1,
                tags: vec![],
                related_playbooks: vec![],
                playbook_ref: None,
                runbook_refs: vec![],
            },
            preconditions: vec![],
            steps: vec![RunbookStep {
                id: "echo_test".to_string(),
                action: "execute".to_string(),
                command: Some("echo hello".to_string()),
                timeout_seconds: Some(10),
                retry: None,
                on_success: None,
                on_failure: None,
                human_approval_required: false,
            }],
            validation: vec![],
            rollback: vec![],
            signature: None,
        };

        let runbook = YamlRunbook::new(spec);
        let ctx = IncidentContext::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            "test",
            "test",
            vec![],
            "test-agent",
        );

        let result = runbook.execute(ctx).await.unwrap();
        assert!(result.success);
        assert_eq!(result.metadata["steps_count"], 1);
    }
}
