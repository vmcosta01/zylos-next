//! WriteGate — Preventive gate that validates evidence capsules before external writes.
//!
//! Blocks writes that lack a valid evidence capsule, ensuring all mutations
//! to the memory system are backed by verifiable evidence.

use serde::{Deserialize, Serialize};

/// Disposition of an evidence capsule after evaluation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Disposition {
    /// Evidence is sufficient; write may proceed
    Approved,
    /// Evidence is insufficient or invalid; write must be blocked
    Rejected(String),
    /// Evidence requires human or higher-level review before proceeding
    PendingReview,
}

/// A capsule bundling a finding with its supporting evidence and disposition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCapsule {
    /// The finding or claim being made
    pub finding: String,
    /// Supporting evidence items (e.g., file paths, log lines, test results)
    pub evidence: Vec<String>,
    /// The decision derived from the evidence
    pub decision: String,
    /// Whether this capsule approves, rejects, or defers the write
    pub disposition: Disposition,
}

impl EvidenceCapsule {
    /// Create a new evidence capsule
    pub fn new(
        finding: impl Into<String>,
        evidence: Vec<String>,
        decision: impl Into<String>,
        disposition: Disposition,
    ) -> Self {
        Self {
            finding: finding.into(),
            evidence,
            decision: decision.into(),
            disposition,
        }
    }

    /// Returns true if the capsule has at least one piece of evidence
    pub fn has_evidence(&self) -> bool {
        !self.evidence.is_empty()
    }

    /// Returns true if the disposition is Approved
    pub fn is_approved(&self) -> bool {
        matches!(self.disposition, Disposition::Approved)
    }
}

/// Result of a WriteGate validation check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    /// Validation passed; write may proceed
    Pass,
    /// Validation failed; write must be blocked with reason
    Block(String),
    /// Validation produced a warning but write may proceed
    Warn(String),
}

impl GateResult {
    /// Returns true if the gate allows the write to proceed (Pass or Warn)
    pub fn is_allowed(&self) -> bool {
        !matches!(self, GateResult::Block(_))
    }
}

/// Preventive gate that validates evidence capsules before any external write
pub struct WriteGate {
    /// Minimum number of evidence items required for approval
    min_evidence_count: usize,
}

impl WriteGate {
    /// Create a new WriteGate with default settings (requires at least 1 evidence item)
    pub fn new() -> Self {
        Self {
            min_evidence_count: 1,
        }
    }

    /// Set the minimum evidence count required for a capsule to pass
    pub fn with_min_evidence(mut self, count: usize) -> Self {
        self.min_evidence_count = count;
        self
    }

    /// Validate an evidence capsule and return whether the write should proceed
    pub fn validate(&self, capsule: &EvidenceCapsule) -> GateResult {
        // Check disposition first
        match &capsule.disposition {
            Disposition::Rejected(reason) => {
                return GateResult::Block(format!("Evidence capsule rejected: {}", reason));
            }
            Disposition::PendingReview => {
                return GateResult::Block(
                    "Evidence capsule pending review; write not permitted until reviewed"
                        .to_string(),
                );
            }
            Disposition::Approved => {}
        }

        // Check evidence sufficiency
        if capsule.evidence.len() < self.min_evidence_count {
            return GateResult::Block(format!(
                "Insufficient evidence: found {} items, need at least {}",
                capsule.evidence.len(),
                self.min_evidence_count
            ));
        }

        // Check for empty evidence strings
        let empty_evidence = capsule
            .evidence
            .iter()
            .filter(|e| e.trim().is_empty())
            .count();
        if empty_evidence > 0 {
            return GateResult::Warn(format!(
                "{} empty evidence item(s) detected in approved capsule",
                empty_evidence
            ));
        }

        // Check finding is non-empty
        if capsule.finding.trim().is_empty() {
            return GateResult::Warn("Evidence capsule has empty finding field".to_string());
        }

        GateResult::Pass
    }
}

impl Default for WriteGate {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approved_capsule_passes() {
        let gate = WriteGate::new();
        let capsule = EvidenceCapsule::new(
            "User authentication verified",
            vec!["log:auth_success_2024.log".to_string()],
            "Allow profile update".to_string(),
            Disposition::Approved,
        );
        assert_eq!(gate.validate(&capsule), GateResult::Pass);
    }

    #[test]
    fn test_rejected_capsule_blocks() {
        let gate = WriteGate::new();
        let capsule = EvidenceCapsule::new(
            "Unverified claim",
            vec!["some_evidence".to_string()],
            "Reject".to_string(),
            Disposition::Rejected("No corroborating source".to_string()),
        );
        match gate.validate(&capsule) {
            GateResult::Block(msg) => assert!(msg.contains("rejected")),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_pending_review_blocks() {
        let gate = WriteGate::new();
        let capsule = EvidenceCapsule::new(
            "Needs review",
            vec!["evidence".to_string()],
            "Defer".to_string(),
            Disposition::PendingReview,
        );
        assert!(!gate.validate(&capsule).is_allowed());
    }

    #[test]
    fn test_insufficient_evidence_blocks() {
        let gate = WriteGate::new().with_min_evidence(3);
        let capsule = EvidenceCapsule::new(
            "Finding",
            vec!["one_item".to_string()],
            "Approve".to_string(),
            Disposition::Approved,
        );
        match gate.validate(&capsule) {
            GateResult::Block(msg) => assert!(msg.contains("Insufficient evidence")),
            other => panic!("Expected Block, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_evidence_warns() {
        let gate = WriteGate::new();
        let capsule = EvidenceCapsule::new(
            "Finding",
            vec!["".to_string()],
            "Approve".to_string(),
            Disposition::Approved,
        );
        match gate.validate(&capsule) {
            GateResult::Warn(_) => {}
            other => panic!("Expected Warn, got {:?}", other),
        }
    }

    #[test]
    fn test_gate_result_is_allowed() {
        assert!(GateResult::Pass.is_allowed());
        assert!(GateResult::Warn("w".to_string()).is_allowed());
        assert!(!GateResult::Block("b".to_string()).is_allowed());
    }
}
