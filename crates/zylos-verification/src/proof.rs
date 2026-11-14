//! Proof engine — verifiable claims with evidence chains.
//!
//! A [`Proof`] binds a claim to its ground level, supporting evidence, timestamps,
//! and composition/refutation operations. All mutations are logged via tracing.

use crate::ground::Ground;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a proof.
pub type ProofId = Uuid;

/// A piece of evidence supporting or refuting a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Human-readable description of the evidence.
    pub description: String,
    /// Source of the evidence (tool name, file path, URL, agent ID).
    pub source: String,
    /// Ground level of this specific evidence.
    pub ground: Ground,
    /// When this evidence was collected.
    pub timestamp: DateTime<Utc>,
    /// Optional raw data (JSON-encoded tool output, file hash, etc.).
    pub raw_data: Option<String>,
}

impl Evidence {
    /// Create new evidence with current timestamp.
    pub fn new(description: impl Into<String>, source: impl Into<String>, ground: Ground) -> Self {
        Self {
            description: description.into(),
            source: source.into(),
            ground,
            timestamp: Utc::now(),
            raw_data: None,
        }
    }

    /// Attach raw data to this evidence.
    pub fn with_raw_data(mut self, data: impl Into<String>) -> Self {
        self.raw_data = Some(data.into());
        self
    }
}

/// Outcome of a proof verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofStatus {
    /// Proof is valid and accepted at its stated ground level.
    Valid,
    /// Proof has been refuted by contradictory evidence.
    Refuted,
    /// Proof is pending additional verification.
    Pending,
}

/// A verifiable claim with evidence chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proof {
    /// Unique identifier.
    pub id: ProofId,
    /// The claim being made.
    pub claim: String,
    /// Certainty level of this claim.
    pub ground: Ground,
    /// Current status.
    pub status: ProofStatus,
    /// Supporting evidence.
    pub evidence: Vec<Evidence>,
    /// IDs of proofs this one was composed from (if any).
    pub composed_from: Vec<ProofId>,
    /// When this proof was created.
    pub created_at: DateTime<Utc>,
    /// When this proof was last modified.
    pub updated_at: DateTime<Utc>,
    /// Optional notes or reasoning.
    pub notes: Option<String>,
}

impl Proof {
    /// Create a new proof at the given ground level.
    pub fn new(claim: impl Into<String>, ground: Ground) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            claim: claim.into(),
            ground,
            status: ProofStatus::Pending,
            evidence: Vec::new(),
            composed_from: Vec::new(),
            created_at: now,
            updated_at: now,
            notes: None,
        }
    }

    /// Add evidence to this proof.
    pub fn add_evidence(&mut self, evidence: Evidence) {
        tracing::debug!(
            proof_id = %self.id,
            evidence_source = %evidence.source,
            ground = %evidence.ground,
            "Adding evidence to proof"
        );
        self.evidence.push(evidence);
        self.updated_at = Utc::now();
    }

    /// Verify this proof: checks that evidence supports the claimed ground level.
    /// Returns `ProofStatus::Valid` if all evidence meets or exceeds the proof's ground.
    pub fn verify(&mut self) -> ProofStatus {
        if self.status == ProofStatus::Refuted {
            tracing::warn!(proof_id = %self.id, "Cannot verify a refuted proof");
            return ProofStatus::Refuted;
        }

        let all_evidence_meets_ground = self.evidence.iter().all(|e| e.ground.meets(self.ground));

        self.status = if self.evidence.is_empty() {
            tracing::info!(proof_id = %self.id, "Proof has no evidence, remains Pending");
            ProofStatus::Pending
        } else if all_evidence_meets_ground {
            tracing::info!(
                proof_id = %self.id,
                ground = %self.ground,
                evidence_count = self.evidence.len(),
                "Proof verified successfully"
            );
            ProofStatus::Valid
        } else {
            tracing::warn!(
                proof_id = %self.id,
                ground = %self.ground,
                "Evidence does not meet claimed ground level"
            );
            ProofStatus::Pending
        };

        self.updated_at = Utc::now();
        self.status.clone()
    }

    /// Compose two proofs into a new combined proof.
    /// The resulting proof takes the MINIMUM ground of the two inputs.
    pub fn compose(a: &Proof, b: &Proof, combined_claim: impl Into<String>) -> Proof {
        let min_ground = if a.ground < b.ground {
            a.ground
        } else {
            b.ground
        };

        let mut composed = Proof::new(combined_claim, min_ground);
        composed.composed_from = vec![a.id, b.id];

        // Merge evidence from both proofs
        composed.evidence.extend(a.evidence.iter().cloned());
        composed.evidence.extend(b.evidence.iter().cloned());

        // Only valid if both inputs are valid
        composed.status = if a.status == ProofStatus::Valid && b.status == ProofStatus::Valid {
            ProofStatus::Valid
        } else {
            ProofStatus::Pending
        };

        tracing::info!(
            parent_a = %a.id,
            parent_b = %b.id,
            child = %composed.id,
            ground = %min_ground,
            "Composed two proofs"
        );

        composed
    }

    /// Refute this proof with counter-evidence.
    pub fn refute(&mut self, counter_evidence: Evidence) {
        tracing::warn!(
            proof_id = %self.id,
            counter_source = %counter_evidence.source,
            "Proof refuted"
        );
        self.evidence.push(counter_evidence);
        self.status = ProofStatus::Refuted;
        self.updated_at = Utc::now();
    }

    /// Returns true if this proof is currently valid.
    pub fn is_valid(&self) -> bool {
        self.status == ProofStatus::Valid
    }

    /// Returns true if this proof has been refuted.
    pub fn is_refuted(&self) -> bool {
        self.status == ProofStatus::Refuted
    }

    /// Get the weakest (lowest) ground among all evidence pieces.
    pub fn weakest_evidence_ground(&self) -> Option<Ground> {
        self.evidence.iter().map(|e| e.ground).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_proof_is_pending() {
        let proof = Proof::new("test claim", Ground::Observado);
        assert_eq!(proof.status, ProofStatus::Pending);
        assert_eq!(proof.ground, Ground::Observado);
        assert!(proof.evidence.is_empty());
    }

    #[test]
    fn test_verify_with_sufficient_evidence() {
        let mut proof = Proof::new("file exists", Ground::Observado);
        proof.add_evidence(Evidence::new(
            "read_file returned content",
            "read_file",
            Ground::Observado,
        ));
        let status = proof.verify();
        assert_eq!(status, ProofStatus::Valid);
        assert!(proof.is_valid());
    }

    #[test]
    fn test_verify_with_insufficient_evidence() {
        let mut proof = Proof::new("deployment succeeded", Ground::Confirmado);
        // Only Observado-level evidence, but claim requires Confirmado
        proof.add_evidence(Evidence::new(
            "log shows success",
            "terminal",
            Ground::Observado,
        ));
        let status = proof.verify();
        assert_eq!(status, ProofStatus::Pending);
    }

    #[test]
    fn test_verify_no_evidence_stays_pending() {
        let mut proof = Proof::new("empty claim", Ground::Crido);
        let status = proof.verify();
        assert_eq!(status, ProofStatus::Pending);
    }

    #[test]
    fn test_refute() {
        let mut proof = Proof::new("server is up", Ground::Observado);
        proof.add_evidence(Evidence::new(
            "curl returned 200",
            "curl",
            Ground::Observado,
        ));
        proof.verify();
        assert!(proof.is_valid());

        proof.refute(Evidence::new(
            "health check failed",
            "monitor",
            Ground::Observado,
        ));
        assert!(proof.is_refuted());
        assert_eq!(proof.status, ProofStatus::Refuted);
    }

    #[test]
    fn test_compose_takes_minimum_ground() {
        let a = Proof::new("claim A", Ground::Confirmado);
        let b = Proof::new("claim B", Ground::Observado);
        let composed = Proof::compose(&a, &b, "A and B");
        assert_eq!(composed.ground, Ground::Observado);
        assert_eq!(composed.composed_from.len(), 2);
    }

    #[test]
    fn test_compose_both_valid() {
        let mut a = Proof::new("claim A", Ground::Observado);
        a.add_evidence(Evidence::new("evidence A", "src_a", Ground::Observado));
        a.verify();

        let mut b = Proof::new("claim B", Ground::Observado);
        b.add_evidence(Evidence::new("evidence B", "src_b", Ground::Observado));
        b.verify();

        let composed = Proof::compose(&a, &b, "combined");
        assert_eq!(composed.status, ProofStatus::Valid);
    }

    #[test]
    fn test_compose_one_pending() {
        let a = Proof::new("claim A", Ground::Observado); // no evidence = pending
        let mut b = Proof::new("claim B", Ground::Observado);
        b.add_evidence(Evidence::new("evidence B", "src_b", Ground::Observado));
        b.verify();

        let composed = Proof::compose(&a, &b, "combined");
        assert_eq!(composed.status, ProofStatus::Pending);
    }

    #[test]
    fn test_weakest_evidence_ground() {
        let mut proof = Proof::new("mixed", Ground::Confirmado);
        proof.add_evidence(Evidence::new("high", "src1", Ground::Confirmado));
        proof.add_evidence(Evidence::new("low", "src2", Ground::Crido));
        assert_eq!(proof.weakest_evidence_ground(), Some(Ground::Crido));
    }

    #[test]
    fn test_serde_roundtrip() {
        let mut proof = Proof::new("serializable claim", Ground::Observado);
        proof.add_evidence(Evidence::new(
            "test evidence",
            "test_src",
            Ground::Observado,
        ));
        proof.verify();

        let json = serde_json::to_string(&proof).unwrap();
        let deserialized: Proof = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, proof.id);
        assert_eq!(deserialized.claim, proof.claim);
        assert_eq!(deserialized.ground, proof.ground);
        assert_eq!(deserialized.status, proof.status);
        assert_eq!(deserialized.evidence.len(), 1);
    }

    #[test]
    fn test_verify_refuted_proof_returns_refuted() {
        let mut proof = Proof::new("already refuted", Ground::Observado);
        proof.refute(Evidence::new("counter", "src", Ground::Observado));
        let status = proof.verify();
        assert_eq!(status, ProofStatus::Refuted);
    }
}
