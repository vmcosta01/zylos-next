//! Approval workflow for high-risk container operations.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Risk level of a container operation, used to gate actions through the
/// [`ApprovalWorkflow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// No approval needed (read-only, ephemeral operations).
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

/// Outcome of an approval check performed by [`ApprovalWorkflow::evaluate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApprovalDecision {
    /// The operation was approved and may proceed.
    Approved {
        /// Human-readable explanation of why the operation was allowed.
        reason: String,
    },
    /// The operation was rejected and must not proceed.
    Denied {
        /// Human-readable explanation of why the operation was blocked.
        reason: String,
    },
    /// The operation is awaiting external approval (human or policy engine).
    Pending {
        /// Timestamp when the approval request was created.
        requested_at: chrono::DateTime<chrono::Utc>,
    },
}

/// Approval workflow that gates high-risk container operations.
///
/// Controls which [`RiskLevel`]s require explicit approval before a
/// [`DockerSandbox`](crate::DockerSandbox) operation may proceed. By default,
/// only [`RiskLevel::High`] and [`RiskLevel::Medium`] operations are gated.
#[derive(Debug, Clone)]
pub struct ApprovalWorkflow {
    /// Whether approval is required for medium-risk operations (default: `true`).
    pub require_medium: bool,
    /// Whether approval is required for low-risk operations (default: `false`).
    pub require_low: bool,
    /// Auto-approve after this duration elapses without an external decision.
    /// `None` means requests remain [`ApprovalDecision::Pending`] indefinitely.
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
    ///
    /// Equivalent to [`ApprovalWorkflow::default()`]: medium-risk operations
    /// require approval, low-risk do not, and no auto-approve timeout is set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check whether an operation at the given risk level requires approval.
    ///
    /// [`RiskLevel::High`] always returns `true`. [`RiskLevel::Medium`] and
    /// [`RiskLevel::Low`] are gated by [`require_medium`](Self::require_medium)
    /// and [`require_low`](Self::require_low) respectively.
    pub fn requires_approval(&self, level: RiskLevel) -> bool {
        match level {
            RiskLevel::High => true,
            RiskLevel::Medium => self.require_medium,
            RiskLevel::Low => self.require_low,
        }
    }

    /// Evaluate an approval request for the given risk level and operation.
    ///
    /// Returns [`ApprovalDecision::Approved`] when the risk level does not
    /// require approval. For gated operations, checks `auto_approve_after`
    /// timeout: if set and elapsed, auto-approves with audit note. Otherwise
    /// returns [`ApprovalDecision::Pending`] awaiting external decision.
    ///
    /// # Arguments
    /// * `level` — risk classification of the operation.
    /// * `operation` — human-readable operation name for audit logging.
    pub fn evaluate(
        &self,
        level: RiskLevel,
        operation: &str,
    ) -> ApprovalDecision {
        if !self.requires_approval(level) {
            return ApprovalDecision::Approved {
                reason: format!("Risk level '{}' does not require approval", level),
            };
        }

        // Check auto-approve timeout
        if let Some(timeout) = self.auto_approve_after {
            let requested_at = chrono::Utc::now();
            // In a real system, we'd store the request timestamp and check
            // against it on subsequent calls. For now, auto-approve immediately
            // if timeout is zero (useful for testing and low-friction workflows).
            if timeout.is_zero() {
                tracing::info!(
                    operation = %operation,
                    risk = %level,
                    "Auto-approved via zero-timeout policy"
                );
                return ApprovalDecision::Approved {
                    reason: format!(
                        "Auto-approved: risk='{}', operation='{}', policy=zero-timeout",
                        level, operation
                    ),
                };
            }

            // Non-zero timeout: return Pending with the timeout info embedded
            // so the caller can re-evaluate after the duration elapses.
            tracing::info!(
                operation = %operation,
                risk = %level,
                timeout_secs = timeout.as_secs(),
                "Approval pending, awaiting external decision or timeout"
            );
            return ApprovalDecision::Pending { requested_at };
        }

        // No auto-approve configured: always pending
        tracing::info!(
            operation = %operation,
            risk = %level,
            "Approval pending, no auto-approve policy configured"
        );
        ApprovalDecision::Pending {
            requested_at: chrono::Utc::now(),
        }
    }

    /// Re-evaluate a previously pending approval after time has elapsed.
    ///
    /// Returns [`ApprovalDecision::Approved`] if `auto_approve_after` is set
    /// and the elapsed time since `requested_at` exceeds the timeout.
    /// Otherwise returns the original [`ApprovalDecision::Pending`].
    pub fn re_evaluate(
        &self,
        level: RiskLevel,
        operation: &str,
        requested_at: chrono::DateTime<chrono::Utc>,
    ) -> ApprovalDecision {
        if !self.requires_approval(level) {
            return ApprovalDecision::Approved {
                reason: format!("Risk level '{}' does not require approval", level),
            };
        }

        if let Some(timeout) = self.auto_approve_after {
            let elapsed = chrono::Utc::now().signed_duration_since(requested_at);
            if elapsed.to_std().unwrap_or_default() >= timeout {
                tracing::info!(
                    operation = %operation,
                    risk = %level,
                    elapsed_secs = elapsed.num_seconds(),
                    "Auto-approved via timeout expiry"
                );
                return ApprovalDecision::Approved {
                    reason: format!(
                        "Auto-approved after {}s timeout: risk='{}', operation='{}'",
                        elapsed.num_seconds(),
                        level,
                        operation
                    ),
                };
            }
        }

        ApprovalDecision::Pending { requested_at }
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

    #[test]
    fn test_risk_level_display() {
        assert_eq!(format!("{}", RiskLevel::Low), "low");
        assert_eq!(format!("{}", RiskLevel::Medium), "medium");
        assert_eq!(format!("{}", RiskLevel::High), "high");
    }

    #[test]
    fn test_approval_decision_serialization_roundtrip() {
        let decisions = vec![
            ApprovalDecision::Approved { reason: "ok".to_string() },
            ApprovalDecision::Denied { reason: "nope".to_string() },
            ApprovalDecision::Pending { requested_at: chrono::Utc::now() },
        ];
        for d in decisions {
            let json = serde_json::to_string(&d).expect("serialize");
            let _: ApprovalDecision = serde_json::from_str(&json).expect("deserialize");
        }
    }

    #[test]
    fn test_evaluate_auto_approves_low_when_not_required() {
        let wf = ApprovalWorkflow {
            require_low: false,
            require_medium: false,
            auto_approve_after: None,
        };
        let low = wf.evaluate(RiskLevel::Low, "read");
        let med = wf.evaluate(RiskLevel::Medium, "write");
        assert!(matches!(low, ApprovalDecision::Approved { .. }));
        assert!(matches!(med, ApprovalDecision::Approved { .. }));
        // High always requires approval regardless of flags
        let high = wf.evaluate(RiskLevel::High, "rm -rf");
        assert!(matches!(high, ApprovalDecision::Pending { .. }));
    }

    #[test]
    fn test_new_equals_default() {
        let a = ApprovalWorkflow::new();
        let b = ApprovalWorkflow::default();
        assert_eq!(a.require_medium, b.require_medium);
        assert_eq!(a.require_low, b.require_low);
        assert_eq!(a.auto_approve_after, b.auto_approve_after);
    }

    #[test]
     fn test_auto_approve_zero_timeout() {
     let wf = ApprovalWorkflow {
     auto_approve_after: Some(std::time::Duration::ZERO),
     ..Default::default()
     };
     let decision = wf.evaluate(RiskLevel::High, "mount /host");
     assert!(matches!(decision, ApprovalDecision::Approved { .. }));
     if let ApprovalDecision::Approved { reason } = &decision {
     assert!(reason.contains("zero-timeout"));
     }
     }

    #[test]
    fn test_re_evaluate_after_timeout() {
        let wf = ApprovalWorkflow {
            auto_approve_after: Some(std::time::Duration::from_millis(1)),
            ..Default::default()
        };
        // Simulate a request made in the past
        let requested_at = chrono::Utc::now() - chrono::Duration::seconds(10);
        let decision = wf.re_evaluate(RiskLevel::High, "rm -rf /", requested_at);
        assert!(matches!(decision, ApprovalDecision::Approved { .. }));
        if let ApprovalDecision::Approved { reason } = &decision {
            assert!(reason.contains("Auto-approved after"));
        }
    }

    #[test]
    fn test_re_evaluate_before_timeout_stays_pending() {
        let wf = ApprovalWorkflow {
            auto_approve_after: Some(std::time::Duration::from_secs(3600)),
            ..Default::default()
        };
        let requested_at = chrono::Utc::now();
        let decision = wf.re_evaluate(RiskLevel::High, "deploy prod", requested_at);
        assert!(matches!(decision, ApprovalDecision::Pending { .. }));
    }
}