//! Approval workflow for high-risk container operations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level of a container operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// No approval needed (read-only, ephemeral).
    Low,
    /// Requires explicit approval (filesystem writes, network access).
    Medium,
    /// Requires approval + audit log (privileged ops, host mounts).
    High,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

/// Decision from the approval workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalDecision {
    Approved { reason: String },
    Denied { reason: String },
    Pending { requested_at: chrono::DateTime<chrono::Utc> },
}

/// Approval workflow that gates high-risk container operations.
#[derive(Debug, Clone)]
pub struct ApprovalWorkflow {
    /// Whether approval is required for medium-risk operations.
    pub require_medium: bool,
    /// Whether approval is required for low-risk operations.
    pub require_low: bool,
    /// Auto-approve after timeout (None = never auto-approve).
    pub auto_approve_after: Option<std::time::Duration>,
}

impl Default for ApprovalWorkflow {
    fn default() -> Self {
        Self {
            require_medium: true,
            require_low: false,
            auto_approve_after: None,
        }
    }
}

impl ApprovalWorkflow {
    /// Create a new approval workflow with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether an operation at the given risk level requires approval.
    pub fn requires_approval(&self, level: RiskLevel) -> bool {
        match level {
            RiskLevel::High => true,
            RiskLevel::Medium => self.require_medium,
            RiskLevel::Low => self.require_low,
        }
    }

    /// Evaluate an approval request. In production this would consult
    /// an external policy engine or human inbox; here we implement the
    /// decision logic skeleton.
    pub fn evaluate(
        &self,
        level: RiskLevel,
        _operation: &str,
    ) -> ApprovalDecision {
        if !self.requires_approval(level) {
            return ApprovalDecision::Approved {
                reason: format!("Risk level '{}' does not require approval", level),
            };
        }

        // Default: pending until external system responds.
        ApprovalDecision::Pending {
            requested_at: chrono::Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_always_requires_approval() {
        let wf = ApprovalWorkflow::default();
        assert!(wf.requires_approval(RiskLevel::High));
    }

    #[test]
    fn test_medium_requires_approval_by_default() {
        let wf = ApprovalWorkflow::default();
        assert!(wf.requires_approval(RiskLevel::Medium));
    }

    #[test]
    fn test_low_does_not_require_approval_by_default() {
        let wf = ApprovalWorkflow::default();
        assert!(!wf.requires_approval(RiskLevel::Low));
    }

    #[test]
    fn test_auto_approve_low_risk() {
        let wf = ApprovalWorkflow {
            require_low: false,
            ..Default::default()
        };
        let decision = wf.evaluate(RiskLevel::Low, "read /tmp");
        assert!(matches!(decision, ApprovalDecision::Approved { .. }));
    }

    #[test]
    fn test_pending_for_high_risk() {
        let wf = ApprovalWorkflow::default();
        let decision = wf.evaluate(RiskLevel::High, "mount /host");
        assert!(matches!(decision, ApprovalDecision::Pending { .. }));
    }
}