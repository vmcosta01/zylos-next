//! Adversarial audit and validation: verifies playbook/runbook integrity,
//! binding coverage, and runs adversarial tests against the SOAR pipeline.

use crate::playbook::Playbook;
use crate::registry::RunbookRegistry;
use crate::router::EventRouter;
use serde::Serialize;
use std::collections::HashSet;

/// Result of a full validation pass.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationReport {
    pub playbook_count: usize,
    pub runbook_count: usize,
    pub unbound_playbook_states: Vec<String>,
    pub orphan_runbooks: Vec<String>,
    pub missing_terminal_states: Vec<String>,
    pub triggerless_playbooks: Vec<String>,
    pub adversarial_findings: Vec<AdversarialFinding>,
    pub passed: bool,
}

/// A single adversarial finding.
#[derive(Debug, Clone, Serialize)]
pub struct AdversarialFinding {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub affected_component: String,
}

/// Validates the complete SOAR configuration for structural integrity,
/// binding completeness, and adversarial resilience.
pub struct SoarValidator<'a> {
    #[allow(dead_code)]
    router: &'a EventRouter,
    registry: &'a RunbookRegistry,
    playbooks: Vec<&'a Playbook>,
}

impl<'a> SoarValidator<'a> {
    pub fn new(
        router: &'a EventRouter,
        registry: &'a RunbookRegistry,
        playbooks: Vec<&'a Playbook>,
    ) -> Self {
        Self {
            router,
            registry,
            playbooks,
        }
    }

    /// Run all validation checks and produce a report.
    pub fn validate_all(&self) -> ValidationReport {
        let mut report = ValidationReport {
            playbook_count: self.playbooks.len(),
            runbook_count: self.registry.len(),
            unbound_playbook_states: Vec::new(),
            orphan_runbooks: Vec::new(),
            missing_terminal_states: Vec::new(),
            triggerless_playbooks: Vec::new(),
            adversarial_findings: Vec::new(),
            passed: true,
        };

        // Check 1: Playbook-to-runbook binding coverage
        self.check_binding_coverage(&mut report);

        // Check 2: Orphan runbooks (registered but never referenced)
        self.check_orphan_runbooks(&mut report);

        // Check 3: Terminal state completeness
        self.check_terminal_states(&mut report);

        // Check 4: Trigger coverage
        self.check_trigger_coverage(&mut report);

        // Check 5: Adversarial tests
        self.run_adversarial_tests(&mut report);

        report.passed = report.unbound_playbook_states.is_empty()
            && report.orphan_runbooks.is_empty()
            && report.missing_terminal_states.is_empty()
            && report.triggerless_playbooks.is_empty()
            && report.adversarial_findings.is_empty();

        report
    }

    /// Verify every playbook state that references a runbook has a matching registry entry.
    fn check_binding_coverage(&self, report: &mut ValidationReport) {
        for playbook in &self.playbooks {
            for state in &playbook.states {
                if let Some(ref runbook_name) = state.runbook {
                    if !self.registry.contains(runbook_name) {
                        report.unbound_playbook_states.push(format!(
                            "{}:{}->{}",
                            playbook.name, state.name, runbook_name
                        ));
                    }
                }
            }
        }
    }

    /// Find runbooks registered but never referenced by any playbook state.
    fn check_orphan_runbooks(&self, report: &mut ValidationReport) {
        let mut referenced: HashSet<String> = HashSet::new();
        for playbook in &self.playbooks {
            for state in &playbook.states {
                if let Some(ref name) = state.runbook {
                    referenced.insert(name.clone());
                }
            }
        }

        for registered in self.registry.list() {
            if !referenced.contains(registered) {
                report.orphan_runbooks.push(registered.to_string());
            }
        }
    }

    /// Verify every playbook has at least one terminal state.
    fn check_terminal_states(&self, report: &mut ValidationReport) {
        for playbook in &self.playbooks {
            if !playbook.states.iter().any(|s| s.terminal) {
                report.missing_terminal_states.push(playbook.name.clone());
            }
        }
    }

    /// Warn about playbooks with no triggers (unreachable via event routing).
    fn check_trigger_coverage(&self, report: &mut ValidationReport) {
        for playbook in &self.playbooks {
            if playbook.triggers.is_empty() {
                report.triggerless_playbooks.push(playbook.name.clone());
            }
        }
    }

    /// Adversarial tests: probe for edge cases and misconfigurations.
    fn run_adversarial_tests(&self, report: &mut ValidationReport) {
        // Test 1: Empty playbook list should not panic
        if self.playbooks.is_empty() {
            report.adversarial_findings.push(AdversarialFinding {
                severity: "warning".to_string(),
                category: "coverage".to_string(),
                description: "No playbooks registered — SOAR engine will not respond to any events"
                    .to_string(),
                affected_component: "router".to_string(),
            });
        }

        // Test 2: Duplicate state names within a playbook
        for playbook in &self.playbooks {
            let mut seen = HashSet::new();
            for state in &playbook.states {
                if !seen.insert(&state.name) {
                    report.adversarial_findings.push(AdversarialFinding {
                        severity: "critical".to_string(),
                        category: "integrity".to_string(),
                        description: format!(
                            "Duplicate state name '{}' in playbook '{}'",
                            state.name, playbook.name
                        ),
                        affected_component: playbook.name.clone(),
                    });
                }
            }
        }

        // Test 3: Self-referencing transitions (infinite loop risk)
        for playbook in &self.playbooks {
            for state in &playbook.states {
                for transition in &state.next {
                    if transition.to == state.name && !state.terminal {
                        report.adversarial_findings.push(AdversarialFinding {
                            severity: "high".to_string(),
                            category: "liveness".to_string(),
                            description: format!(
                                "State '{}' in playbook '{}' has self-referencing transition (potential infinite loop)",
                                state.name, playbook.name
                            ),
                            affected_component: playbook.name.clone(),
                        });
                    }
                }
            }
        }

        // Test 4: Approval-required states without timeout
        for playbook in &self.playbooks {
            for state in &playbook.states {
                if state.approval_required && state.timeout_minutes.is_none() {
                    report.adversarial_findings.push(AdversarialFinding {
                        severity: "medium".to_string(),
                        category: "resilience".to_string(),
                        description: format!(
                            "State '{}' in playbook '{}' requires approval but has no timeout — may block indefinitely",
                            state.name, playbook.name
                        ),
                        affected_component: playbook.name.clone(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::playbook::Playbook;
    use crate::runnable::{BoxFuture, IncidentContext, RunbookResult, SoarRunnable};
    use crate::router::EventRouter;

    const VALID_PLAYBOOK_YAML: &str = r#"
name: valid-playbook
version: "1.0"
triggers:
  - expression: "event.category == \"breach\""
states:
  - name: respond
    auto: true
    runbook: "test-runbook"
    terminal: true
"#;

    const TRIGGERLESS_PLAYBOOK_YAML: &str = r#"
name: no-trigger
version: "1.0"
states:
  - name: idle
    auto: true
    terminal: true
"#;

    const UNBOUND_PLAYBOOK_YAML: &str = r#"
name: unbound
version: "1.0"
triggers:
  - expression: "event.severity == \"high\""
states:
  - name: act
    auto: true
    runbook: "nonexistent-runbook"
    terminal: true
"#;

    struct TestRunbook;

    impl SoarRunnable for TestRunbook {
        fn name(&self) -> &str {
            "test-runbook"
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
            Box::pin(async { Ok(RunbookResult::ok("done", 1)) })
        }
    }

    fn build_valid_setup() -> (EventRouter, RunbookRegistry, Vec<Playbook>) {
        let mut router = EventRouter::new();
        let playbook = Playbook::from_yaml(VALID_PLAYBOOK_YAML).unwrap();
        router.register_playbook(playbook.clone());

        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(TestRunbook));

        (router, registry, vec![playbook])
    }

    #[test]
    fn valid_setup_passes_all_checks() {
        let (router, registry, playbooks) = build_valid_setup();
        let refs: Vec<&Playbook> = playbooks.iter().collect();
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        assert!(report.passed);
        assert!(report.unbound_playbook_states.is_empty());
        assert!(report.orphan_runbooks.is_empty());
        assert!(report.adversarial_findings.is_empty());
    }

    #[test]
    fn detects_unbound_runbook_reference() {
        let mut router = EventRouter::new();
        let playbook = Playbook::from_yaml(UNBOUND_PLAYBOOK_YAML).unwrap();
        router.register_playbook(playbook.clone());

        let registry = RunbookRegistry::new(); // empty — no runbooks
        let refs = vec![&playbook];
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        assert!(!report.passed);
        assert_eq!(report.unbound_playbook_states.len(), 1);
        assert!(report.unbound_playbook_states[0].contains("nonexistent-runbook"));
    }

    #[test]
    fn detects_orphan_runbooks() {
        let router = EventRouter::new();
        let mut registry = RunbookRegistry::new();
        registry.register(Box::new(TestRunbook));

        let playbooks: Vec<Playbook> = vec![];
        let refs: Vec<&Playbook> = playbooks.iter().collect();
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        assert!(!report.passed);
        assert_eq!(report.orphan_runbooks.len(), 1);
        assert_eq!(report.orphan_runbooks[0], "test-runbook");
    }

    #[test]
    fn detects_triggerless_playbook() {
        let mut router = EventRouter::new();
        let playbook = Playbook::from_yaml(TRIGGERLESS_PLAYBOOK_YAML).unwrap();
        router.register_playbook(playbook.clone());

        let registry = RunbookRegistry::new();
        let refs = vec![&playbook];
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        assert!(!report.passed);
        assert_eq!(report.triggerless_playbooks.len(), 1);
        assert_eq!(report.triggerless_playbooks[0], "no-trigger");
    }

    #[test]
    fn detects_self_referencing_transition() {
        let yaml = r#"
name: loop-risk
version: "1.0"
triggers:
  - expression: "event.severity == \"low\""
states:
  - name: spin
    auto: true
    next:
      - to: spin
  - name: done
    terminal: true
"#;
        let playbook = Playbook::from_yaml(yaml).unwrap();
        let mut router = EventRouter::new();
        router.register_playbook(playbook.clone());

        let registry = RunbookRegistry::new();
        let refs = vec![&playbook];
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        let loop_findings: Vec<_> = report
            .adversarial_findings
            .iter()
            .filter(|f| f.category == "liveness")
            .collect();
        assert_eq!(loop_findings.len(), 1);
        assert!(loop_findings[0].description.contains("self-referencing"));
    }

    #[test]
    fn detects_approval_without_timeout() {
        let yaml = r#"
name: approval-no-timeout
version: "1.0"
triggers:
  - expression: "event.category == \"breach\""
states:
  - name: gate
    approval_required: true
    escalation: "manager"
    next:
      - to: done
  - name: done
    terminal: true
"#;
        let playbook = Playbook::from_yaml(yaml).unwrap();
        let mut router = EventRouter::new();
        router.register_playbook(playbook.clone());

        let registry = RunbookRegistry::new();
        let refs = vec![&playbook];
        let validator = SoarValidator::new(&router, &registry, refs);
        let report = validator.validate_all();

        let timeout_findings: Vec<_> = report
            .adversarial_findings
            .iter()
            .filter(|f| f.category == "resilience")
            .collect();
        assert_eq!(timeout_findings.len(), 1);
        assert!(timeout_findings[0].description.contains("no timeout"));
    }
}