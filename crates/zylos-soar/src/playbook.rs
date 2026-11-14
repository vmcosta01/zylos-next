//! Playbook orchestrator: YAML-driven state machine for incident response.

use crate::error::{Result, SoarError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single state in a playbook state machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura PlaybookState.
pub struct PlaybookState {
    /// Campo name.
    pub name: String,
    #[serde(default)]
    /// Campo auto.
    pub auto: bool,
    #[serde(default)]
    /// Campo approval required.
    pub approval_required: bool,
    /// Runbook to execute in this state (None = use engine logic like auto-triage).
    pub runbook: Option<String>,
    #[serde(default)]
    /// Campo timeout minutes.
    pub timeout_minutes: Option<u32>,
    /// Escalation target if timeout or failure occurs.
    pub escalation: Option<String>,
    /// Transition rules to next states.
    #[serde(default)]
    /// Campo next.
    pub next: Vec<Transition>,
    /// Terminal actions (post-incident, notifications, etc.).
    #[serde(default)]
    /// Campo actions.
    pub actions: Vec<String>,
    #[serde(default)]
    /// Campo terminal.
    pub terminal: bool,
}

/// Conditional transition between playbook states.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura Transition.
pub struct Transition {
    /// Optional condition expression (evaluated against incident context).
    pub condition: Option<String>,
    /// Target state name.
    pub to: String,
}

/// Role assignments for a playbook execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// Estrutura PlaybookRoles.
pub struct PlaybookRoles {
    #[serde(default)]
    /// Campo incident commander.
    pub incident_commander: Option<String>,
    #[serde(default)]
    /// Campo executor.
    pub executor: Option<String>,
    #[serde(default)]
    /// Campo approver.
    pub approver: Option<String>,
}

/// Trigger condition that activates this playbook.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura PlaybookTrigger.
pub struct PlaybookTrigger {
    /// Expression evaluated against incoming events.
    pub expression: String,
}

/// Executable playbook parsed from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Estrutura Playbook.
pub struct Playbook {
    /// Campo name.
    pub name: String,
    /// Campo version.
    pub version: String,
    #[serde(default)]
    /// Campo triggers.
    pub triggers: Vec<PlaybookTrigger>,
    #[serde(default)]
    /// Campo roles.
    pub roles: PlaybookRoles,
    /// Campo states.
    pub states: Vec<PlaybookState>,
}

impl Playbook {
    /// Parse a playbook from YAML content.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let playbook: Self = serde_yaml::from_str(yaml)?;
        playbook.validate()?;
        // Variante Ok.
        Ok(playbook)
    }

    /// Validate structural integrity of the playbook.
    pub fn validate(&self) -> Result<()> {
        if self.states.is_empty() {
            return Err(SoarError::PlaybookParse(
                "playbook must have at least one state".to_string(),
            ));
        }

        // Build state name index
        let state_names: HashMap<&str, &PlaybookState> =
            self.states.iter().map(|s| (s.name.as_str(), s)).collect();

        // Verify all transitions reference existing states
        for state in &self.states {
            for transition in &state.next {
                if !state_names.contains_key(transition.to.as_str()) {
                    return Err(SoarError::PlaybookParse(format!(
                        "state '{}' transitions to unknown state '{}'",
                        state.name, transition.to
                    )));
                }
            }
        }

        // Verify at least one terminal state exists
        if !self.states.iter().any(|s| s.terminal) {
            return Err(SoarError::PlaybookParse(
                "playbook must have at least one terminal state".to_string(),
            ));
        }

        // Verify approval_required states have an escalation target
        for state in &self.states {
            if state.approval_required && state.escalation.is_none() {
                return Err(SoarError::PlaybookParse(format!(
                    "state '{}' requires approval but has no escalation target",
                    state.name
                )));
            }
        }

        // Variante Ok.
        Ok(())
    }

    /// Find a state by name.
    pub fn find_state(&self, name: &str) -> Option<&PlaybookState> {
        self.states.iter().find(|s| s.name == name)
    }

    /// Return the first state (entry point).
    pub fn entry_state(&self) -> &PlaybookState {
        &self.states[0]
    }
}

/// Current execution position within a playbook.
#[derive(Debug, Clone)]
/// Estrutura PlaybookCursor.
pub struct PlaybookCursor {
    /// Campo playbook name.
    pub playbook_name: String,
    /// Campo current state.
    pub current_state: String,
    /// Campo awaiting approval.
    pub awaiting_approval: bool,
    /// Campo completed states.
    pub completed_states: Vec<String>,
}

impl PlaybookCursor {
    /// Função new.
    pub fn new(playbook: &Playbook) -> Self {
        // Variante Self.
        Self {
            playbook_name: playbook.name.clone(),
            current_state: playbook.entry_state().name.clone(),
            awaiting_approval: false,
            completed_states: Vec::new(),
        }
    }

    /// Advance to the next state based on transition conditions.
    pub fn advance(
        &mut self,
        playbook: &Playbook,
        _context: &serde_json::Value,
    ) -> Result<Option<String>> {
        let state = playbook.find_state(&self.current_state).ok_or_else(|| {
            SoarError::PlaybookParse(format!(
                "current state '{}' not found in playbook '{}'",
                self.current_state, self.playbook_name
            ))
        })?;

        if state.terminal {
            return Ok(None);
        }

        self.completed_states.push(self.current_state.clone());

        // Find matching transition
        for transition in &state.next {
            if let Some(ref _condition) = transition.condition {
                // TODO: implement condition evaluation against context
                // For now, take the first transition with a matching condition
                // or fall through to unconditional transitions
                continue;
            }
            // Unconditional transition
            self.current_state = transition.to.clone();
            let next_state = playbook.find_state(&self.current_state).ok_or_else(|| {
                SoarError::PlaybookParse(format!(
                    "transition target '{}' not found",
                    self.current_state
                ))
            })?;
            self.awaiting_approval = next_state.approval_required;
            return Ok(Some(self.current_state.clone()));
        }

        // If we have conditional transitions, take the first one as default
        if let Some(transition) = state.next.first() {
            self.current_state = transition.to.clone();
            let next_state = playbook.find_state(&self.current_state).ok_or_else(|| {
                SoarError::PlaybookParse(format!(
                    "transition target '{}' not found",
                    self.current_state
                ))
            })?;
            self.awaiting_approval = next_state.approval_required;
            return Ok(Some(self.current_state.clone()));
        }

        // Variante Err.
        Err(SoarError::PlaybookParse(format!(
            "state '{}' has no transitions and is not terminal",
            state.name
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PLAYBOOK: &str = r#"
name: security-incident-response
version: "1.0"
triggers:
  - expression: "event.category == \"Breach\" && event.severity == \"Critical\""
roles:
  incident_commander: "on-call-security-lead"
  executor: "zylos-agent-soar"
  approver: "security-manager"
states:
  - name: triage
    auto: true
    next:
      - condition: "triage.confirmed_breach == true"
        to: containment
      - condition: "triage.false_positive == true"
        to: closed
  - name: containment
    auto: false
    approval_required: true
    runbook: "isolate-compromised-host"
    timeout_minutes: 10
    escalation: "incident-commander"
    next:
      - to: eradication
  - name: eradication
    auto: true
    runbook: "remove-malware-and-restore"
    next:
      - to: recovery
  - name: recovery
    auto: true
    runbook: "validate-system-integrity"
    next:
      - to: post_incident
  - name: post_incident
    auto: true
    actions:
      - generate_postmortem
      - update_metrics
    terminal: true
  - name: closed
    auto: true
    terminal: true
"#;

    #[test]
    fn parse_valid_playbook() {
        let playbook = Playbook::from_yaml(SAMPLE_PLAYBOOK).unwrap();
        assert_eq!(playbook.name, "security-incident-response");
        assert_eq!(playbook.states.len(), 6);
        assert_eq!(playbook.entry_state().name, "triage");
    }

    #[test]
    fn validate_rejects_empty_states() {
        let yaml = r#"
name: empty
version: "1.0"
states: []
"#;
        let result = Playbook::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_missing_terminal() {
        let yaml = r#"
name: no-terminal
version: "1.0"
states:
  - name: start
    auto: true
    next:
      - to: start
"#;
        let result = Playbook::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_unknown_transition_target() {
        let yaml = r#"
name: bad-transition
version: "1.0"
states:
  - name: start
    auto: true
    next:
      - to: nonexistent
  - name: end
    terminal: true
"#;
        let result = Playbook::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn cursor_advances_through_states() {
        let playbook = Playbook::from_yaml(SAMPLE_PLAYBOOK).unwrap();
        let mut cursor = PlaybookCursor::new(&playbook);
        let ctx = serde_json::json!({});

        assert_eq!(cursor.current_state, "triage");

        let next = cursor.advance(&playbook, &ctx).unwrap();
        assert_eq!(next, Some("containment".to_string()));
        assert!(cursor.awaiting_approval);

        let next = cursor.advance(&playbook, &ctx).unwrap();
        assert_eq!(next, Some("eradication".to_string()));
        assert!(!cursor.awaiting_approval);
    }

    #[test]
    fn find_state_returns_correct_state() {
        let playbook = Playbook::from_yaml(SAMPLE_PLAYBOOK).unwrap();
        let state = playbook.find_state("containment").unwrap();
        assert!(state.approval_required);
        assert_eq!(state.runbook.as_deref(), Some("isolate-compromised-host"));
    }
}
