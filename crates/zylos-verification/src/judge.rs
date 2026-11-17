//! Adversarial Panel — Multi-judge deliberation for claim validation.
//!
//! Implements the dual-audit convention: multiple independent judges evaluate
//! a claim, and the panel produces a consolidated verdict with ground level.

use crate::ground::Ground;
use crate::proof::{Evidence, Proof};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Unique identifier for a judge in the panel.
pub type JudgeId = String;

/// A single judge's verdict on a claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    /// Which judge issued this verdict.
    pub judge_id: JudgeId,
    /// Whether the judge believes the claim is valid.
    pub valid: bool,
    /// The ground level the judge assigns to the claim.
    pub ground: Ground,
    /// Human-readable rationale (max 500 chars enforced by panel).
    pub rationale: String,
    /// Wall-clock time the judge took (microseconds).
    pub duration_us: u64,
}

/// Configuration for an adversarial panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelConfig {
    /// Minimum number of judges that must agree for a claim to be confirmed.
    pub min_agreement: usize,
    /// Maximum rationale length in characters.
    pub max_rationale_len: usize,
    /// If true, a single Refuted verdict overrides all Valid verdicts.
    pub veto_on_refute: bool,
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            min_agreement: 2,
            max_rationale_len: 500,
            veto_on_refute: true,
        }
    }
}

/// Consolidated result from an adversarial panel deliberation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelResult {
    /// All individual verdicts collected.
    pub verdicts: Vec<Verdict>,
    /// The consolidated ground level after deliberation.
    pub consensus_ground: Ground,
    /// Whether the panel reached agreement (≥ min_agreement valid verdicts).
    pub agreed: bool,
    /// Total wall-clock time for the full deliberation (microseconds).
    pub total_duration_us: u64,
    /// Number of judges that voted valid.
    pub valid_count: usize,
    /// Number of judges that voted invalid/refuted.
    pub refute_count: usize,
}

/// An adversarial panel that collects and consolidates multiple judge verdicts.
///
/// # Usage
/// ```ignore
/// let mut panel = AdversarialPanel::new(PanelConfig::default());
/// panel.add_verdict(verdict_1);
/// panel.add_verdict(verdict_2);
/// let result = panel.deliberate();
/// assert!(result.agreed);
/// ```
#[derive(Debug, Clone)]
pub struct AdversarialPanel {
    config: PanelConfig,
    verdicts: Vec<Verdict>,
    start: Option<Instant>,
}

impl AdversarialPanel {
    /// Create a new panel with the given configuration.
    pub fn new(config: PanelConfig) -> Self {
        Self {
            config,
            verdicts: Vec::new(),
            start: None,
        }
    }

    /// Start timing the deliberation. Call before adding verdicts.
    pub fn begin(&mut self) {
        self.start = Some(Instant::now());
    }

    /// Add a judge's verdict to the panel.
    ///
    /// Truncates rationale to `max_rationale_len` if exceeded.
    pub fn add_verdict(&mut self, mut verdict: Verdict) {
        if verdict.rationale.len() > self.config.max_rationale_len {
            verdict.rationale.truncate(self.config.max_rationale_len);
        }
        self.verdicts.push(verdict);
    }

    /// Number of verdicts collected so far.
    pub fn verdict_count(&self) -> usize {
        self.verdicts.len()
    }

    /// Deliberate: consolidate all verdicts into a single panel result.
    ///
    /// Rules:
    /// - If `veto_on_refute` is true and any judge voted invalid, consensus is Crido.
    /// - Otherwise, consensus is Confirmado if ≥ `min_agreement` judges voted valid.
    /// - If fewer than `min_agreement` voted valid, consensus is Entendido.
    /// - If no verdicts, returns Pending-equivalent (Crido with agreed=false).
    pub fn deliberate(&self) -> PanelResult {
        let total_us = self
            .start
            .map(|s| s.elapsed().as_micros() as u64)
            .unwrap_or(0);

        if self.verdicts.is_empty() {
            return PanelResult {
                verdicts: Vec::new(),
                consensus_ground: Ground::Crido,
                agreed: false,
                total_duration_us: total_us,
                valid_count: 0,
                refute_count: 0,
            };
        }

        let valid_count = self.verdicts.iter().filter(|v| v.valid).count();
        let refute_count = self.verdicts.len() - valid_count;

        // Veto rule: single refute overrides everything
        let has_refute = refute_count > 0 && self.config.veto_on_refute;

        let (consensus_ground, agreed) = if has_refute {
            (Ground::Crido, false)
        } else if valid_count >= self.config.min_agreement {
            (Ground::Confirmado, true)
        } else if valid_count > 0 {
            (Ground::Observado, false)
        } else {
            (Ground::Crido, false)
        };

        PanelResult {
            verdicts: self.verdicts.clone(),
            consensus_ground,
            agreed,
            total_duration_us: total_us,
            valid_count,
            refute_count,
        }
    }

    /// Build a Proof from the panel result, linking all verdicts as evidence.
    pub fn to_proof(&self, claim: &str) -> Proof {
        let result = self.deliberate();
        let mut proof = Proof::new(claim.to_string(), result.consensus_ground);

        for verdict in &result.verdicts {
            let evidence_text = format!(
                "Judge '{}': {} (ground={}, rationale='{}')",
                verdict.judge_id,
                if verdict.valid { "VALID" } else { "REFUTED" },
                verdict.ground,
                verdict.rationale
            );
            proof.add_evidence(Evidence::new(
                evidence_text,
                format!("adversarial-panel:{}", verdict.judge_id),
                verdict.ground,
            ));
        }

        proof
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_verdict(id: &str, valid: bool, ground: Ground) -> Verdict {
        Verdict {
            judge_id: id.to_string(),
            valid,
            ground,
            rationale: format!("Rationale from {}", id),
            duration_us: 100,
        }
    }

    #[test]
    fn test_empty_panel_not_agreed() {
        let panel = AdversarialPanel::new(PanelConfig::default());
        let result = panel.deliberate();
        assert!(!result.agreed);
        assert_eq!(result.consensus_ground, Ground::Crido);
        assert_eq!(result.valid_count, 0);
        assert_eq!(result.refute_count, 0);
    }

    #[test]
    fn test_two_valid_judges_confirm() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.begin();
        panel.add_verdict(make_verdict("j1", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j2", true, Ground::Confirmado));
        let result = panel.deliberate();
        assert!(result.agreed);
        assert_eq!(result.consensus_ground, Ground::Confirmado);
        assert_eq!(result.valid_count, 2);
        assert_eq!(result.refute_count, 0);
    }

    #[test]
    fn test_one_valid_not_enough_for_confirm() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.add_verdict(make_verdict("j1", true, Ground::Observado));
        let result = panel.deliberate();
        assert!(!result.agreed);
        assert_eq!(result.consensus_ground, Ground::Observado);
        assert_eq!(result.valid_count, 1);
    }

    #[test]
    fn test_veto_on_refute() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.add_verdict(make_verdict("j1", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j2", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j3", false, Ground::Crido));
        let result = panel.deliberate();
        assert!(!result.agreed);
        assert_eq!(result.consensus_ground, Ground::Crido);
        assert_eq!(result.valid_count, 2);
        assert_eq!(result.refute_count, 1);
    }

    #[test]
    fn test_no_veto_allows_confirm_despite_refute() {
        let config = PanelConfig {
            veto_on_refute: false,
            ..PanelConfig::default()
        };
        let mut panel = AdversarialPanel::new(config);
        panel.add_verdict(make_verdict("j1", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j2", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j3", false, Ground::Crido));
        let result = panel.deliberate();
        assert!(result.agreed);
        assert_eq!(result.consensus_ground, Ground::Confirmado);
    }

    #[test]
    fn test_all_refuted() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.add_verdict(make_verdict("j1", false, Ground::Crido));
        panel.add_verdict(make_verdict("j2", false, Ground::Crido));
        let result = panel.deliberate();
        assert!(!result.agreed);
        assert_eq!(result.consensus_ground, Ground::Crido);
        assert_eq!(result.valid_count, 0);
        assert_eq!(result.refute_count, 2);
    }

    #[test]
    fn test_rationale_truncation() {
        let config = PanelConfig {
            max_rationale_len: 10,
            ..PanelConfig::default()
        };
        let mut panel = AdversarialPanel::new(config);
        let long_rationale = "This is a very long rationale that exceeds the limit";
        panel.add_verdict(Verdict {
            judge_id: "j1".to_string(),
            valid: true,
            ground: Ground::Confirmado,
            rationale: long_rationale.to_string(),
            duration_us: 50,
        });
        assert_eq!(panel.verdicts[0].rationale.len(), 10);
        assert_eq!(panel.verdicts[0].rationale, "This is a ");
    }

    #[test]
    fn test_to_proof_links_evidence() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.add_verdict(make_verdict("alpha", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("beta", true, Ground::Confirmado));
        let proof = panel.to_proof("Test claim");
        assert_eq!(proof.claim, "Test claim");
        assert_eq!(proof.evidence.len(), 2);
        assert!(proof.evidence[0].source.contains("alpha"));
        assert!(proof.evidence[1].source.contains("beta"));
    }

    #[test]
    fn test_custom_min_agreement_three() {
        let config = PanelConfig {
            min_agreement: 3,
            ..PanelConfig::default()
        };
        let mut panel = AdversarialPanel::new(config);
        panel.add_verdict(make_verdict("j1", true, Ground::Confirmado));
        panel.add_verdict(make_verdict("j2", true, Ground::Confirmado));
        let result = panel.deliberate();
        assert!(!result.agreed);
        assert_eq!(result.consensus_ground, Ground::Observado);

        panel.add_verdict(make_verdict("j3", true, Ground::Confirmado));
        let result = panel.deliberate();
        assert!(result.agreed);
        assert_eq!(result.consensus_ground, Ground::Confirmado);
    }

    #[test]
    fn test_serde_roundtrip_panel_result() {
        let mut panel = AdversarialPanel::new(PanelConfig::default());
        panel.add_verdict(make_verdict("j1", true, Ground::Confirmado));
        let result = panel.deliberate();
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: PanelResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.consensus_ground, result.consensus_ground);
        assert_eq!(deserialized.agreed, result.agreed);
        assert_eq!(deserialized.valid_count, result.valid_count);
    }
}
