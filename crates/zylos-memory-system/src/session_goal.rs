//! SessionGoal — Criteria-oriented session goals inspired by OpenChamber.
//!
//! Autonomous execution continues until all criteria are satisfied with
//! verifiable evidence attached to each criterion.

use serde::{Deserialize, Serialize};

/// Status of a session goal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GoalStatus {
    /// Goal is actively being worked on
    Active,
    /// All criteria satisfied with evidence
    Satisfied,
    /// Goal cannot be completed (terminal failure)
    Failed,
    /// Goal exceeded its time window or was superseded
    Expired,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GoalStatus::Active => write!(f, "active"),
            GoalStatus::Satisfied => write!(f, "satisfied"),
            GoalStatus::Failed => write!(f, "failed"),
            GoalStatus::Expired => write!(f, "expired"),
        }
    }
}

/// A single criterion within a session goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion {
    /// Human-readable description of what must be true
    pub description: String,
    /// Whether this criterion has been satisfied
    pub satisfied: bool,
    /// Evidence proving satisfaction (e.g., test output path, log excerpt)
    pub evidence: Option<String>,
}

impl Criterion {
    /// Create a new unsatisfied criterion
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            satisfied: false,
            evidence: None,
        }
    }

    /// Mark this criterion as satisfied with supporting evidence
    pub fn satisfy(&mut self, evidence: impl Into<String>) {
        self.satisfied = true;
        self.evidence = Some(evidence.into());
    }
}

/// A session goal composed of multiple verifiable criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionGoal {
    /// Unique identifier for this goal
    pub id: String,
    /// Ordered list of criteria that must all be satisfied
    pub criteria: Vec<Criterion>,
    /// Evidence items required before the goal can be considered complete
    pub evidence_required: Vec<String>,
    /// Current status of the goal
    pub status: GoalStatus,
}

impl SessionGoal {
    /// Create a new active session goal
    pub fn new(id: impl Into<String>, criteria: Vec<Criterion>, evidence_required: Vec<String>) -> Self {
        Self {
            id: id.into(),
            criteria,
            evidence_required,
            status: GoalStatus::Active,
        }
    }

    /// Returns true if all criteria are satisfied and the goal is still Active
    pub fn is_complete(&self) -> bool {
        self.status == GoalStatus::Active
            && !self.criteria.is_empty()
            && self.criteria.iter().all(|c| c.satisfied)
    }

    /// Check all criteria and update status to Satisfied if all are met.
    /// Returns true if the goal transitioned to Satisfied.
    pub fn check_criteria(&mut self) -> bool {
        if self.status != GoalStatus::Active {
            return false;
        }
        if self.is_complete() {
            self.status = GoalStatus::Satisfied;
            true
        } else {
            false
        }
    }

    /// Mark a specific criterion as satisfied by its index, attaching evidence.
    /// Returns Err if the index is out of bounds.
    pub fn mark_evidence(&mut self, index: usize, evidence: impl Into<String>) -> Result<(), String> {
        if index >= self.criteria.len() {
            return Err(format!(
                "Criterion index {} out of bounds (have {})",
                index,
                self.criteria.len()
            ));
        }
        self.criteria[index].satisfy(evidence);
        Ok(())
    }

    /// Mark a criterion as satisfied by matching its description substring.
    /// Returns true if a matching criterion was found and updated.
    pub fn mark_evidence_by_description(&mut self, description_contains: &str, evidence: impl Into<String>) -> bool {
        for criterion in &mut self.criteria {
            if criterion.description.contains(description_contains) && !criterion.satisfied {
                criterion.satisfy(evidence);
                return true;
            }
        }
        false
    }

    /// Transition the goal to Failed status
    pub fn fail(&mut self) {
        self.status = GoalStatus::Failed;
    }

    /// Transition the goal to Expired status
    pub fn expire(&mut self) {
        self.status = GoalStatus::Expired;
    }

    /// Count of satisfied criteria
    pub fn satisfied_count(&self) -> usize {
        self.criteria.iter().filter(|c| c.satisfied).count()
    }

    /// Total number of criteria
    pub fn total_criteria(&self) -> usize {
        self.criteria.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_goal_is_active_and_incomplete() {
        let goal = SessionGoal::new(
            "goal-1",
            vec![Criterion::new("deploy succeeds"), Criterion::new("tests pass")],
            vec!["ci_log".to_string()],
        );
        assert_eq!(goal.status, GoalStatus::Active);
        assert!(!goal.is_complete());
        assert_eq!(goal.satisfied_count(), 0);
        assert_eq!(goal.total_criteria(), 2);
    }

    #[test]
    fn test_satisfy_all_criteria_transitions_to_satisfied() {
        let mut goal = SessionGoal::new(
            "goal-2",
            vec![Criterion::new("step A"), Criterion::new("step B")],
            vec![],
        );
        goal.mark_evidence(0, "evidence_a").unwrap();
        assert!(!goal.is_complete());

        goal.mark_evidence(1, "evidence_b").unwrap();
        assert!(goal.is_complete());

        let transitioned = goal.check_criteria();
        assert!(transitioned);
        assert_eq!(goal.status, GoalStatus::Satisfied);
    }

    #[test]
    fn test_mark_evidence_out_of_bounds() {
        let mut goal = SessionGoal::new("goal-3", vec![Criterion::new("only")], vec![]);
        let result = goal.mark_evidence(5, "nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_evidence_by_description() {
        let mut goal = SessionGoal::new(
            "goal-4",
            vec![Criterion::new("build docker image"), Criterion::new("push to registry")],
            vec![],
        );
        assert!(goal.mark_evidence_by_description("docker", "sha256:abc123"));
        assert_eq!(goal.satisfied_count(), 1);
        assert!(!goal.mark_evidence_by_description("nonexistent", "x"));
    }

    #[test]
    fn test_fail_and_expire_prevent_completion() {
        let mut goal = SessionGoal::new("goal-5", vec![Criterion::new("do thing")], vec![]);
        goal.fail();
        goal.mark_evidence(0, "evidence").unwrap();
        assert!(!goal.check_criteria());
        assert_eq!(goal.status, GoalStatus::Failed);

        let mut goal2 = SessionGoal::new("goal-6", vec![Criterion::new("do thing")], vec![]);
        goal2.expire();
        assert_eq!(goal2.status, GoalStatus::Expired);
    }

    #[test]
    fn test_empty_criteria_never_complete() {
        let mut goal = SessionGoal::new("goal-7", vec![], vec![]);
        assert!(!goal.is_complete());
        assert!(!goal.check_criteria());
    }
}