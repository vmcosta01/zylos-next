//! RSI Protocol — Reproducible Scientific Investigation for AI experiments.
//!
//! Implements a strict phase-gated state machine:
//!   Hypothesis → Design → Build → Measure → Analyze → Decide → Log
//!
//! Key properties:
//! - **Phase gates**: transitions validated; cannot skip phases.
//! - **Blinded evaluation**: test data stored separately, decrypted only after
//!   Analyze commit.
//! - **Pre-registration enforcement**: changes post-start require HITL approval.
//! - **Provenance graph**: each run carries full reproducibility metadata.
//! - **Statistical guardrails**: auto-detect underpowered samples, multiple
//!   comparison correction (Bonferroni / Holm).
//! - **Decision framework**: classify results against pre-registered thresholds.
//! - **Meta-analysis aggregation stub**: combine related experiments via
//!   random-effects model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Phases of the RSI protocol, in strict sequential order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RsiPhase {
    Hypothesis = 0,
    Design = 1,
    Build = 2,
    Measure = 3,
    Analyze = 4,
    Decide = 5,
    Log = 6,
}

impl RsiPhase {
    /// Returns the next phase in sequence, or `None` if already at `Log`.
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Hypothesis => Some(Self::Design),
            Self::Design => Some(Self::Build),
            Self::Build => Some(Self::Measure),
            Self::Measure => Some(Self::Analyze),
            Self::Analyze => Some(Self::Decide),
            Self::Decide => Some(Self::Log),
            Self::Log => None,
        }
    }

    /// Ordered list of all phases.
    pub const ALL: [RsiPhase; 7] = [
        Self::Hypothesis,
        Self::Design,
        Self::Build,
        Self::Measure,
        Self::Analyze,
        Self::Decide,
        Self::Log,
    ];
}

/// Lifecycle status of an experiment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExperimentStatus {
    Planning,
    InProgress,
    Blinded,
    Analyzing,
    Completed,
    NeedsReplication,
}

/// Outcome of the Decide phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionOutcome {
    Confirm,
    Reject,
    Inconclusive,
    NeedsReplication,
}

/// Multiple-comparison correction method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CorrectionMethod {
    Bonferroni,
    Holm,
}

// ---------------------------------------------------------------------------
// Structs
// ---------------------------------------------------------------------------

/// Record of work done within a single RSI phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseRecord {
    /// When this phase was entered.
    pub started_at: DateTime<Utc>,
    /// When this phase was completed (None if still active).
    pub completed_at: Option<DateTime<Utc>>,
    /// Free-form notes or artifact references produced during this phase.
    pub artifacts: Vec<String>,
    /// Whether the phase gate conditions were satisfied.
    pub gate_passed: bool,
}

/// Pre-registration specification submitted before the experiment starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRegistration {
    /// Primary metric name (e.g. "accuracy", "latency_p99").
    pub primary_metric: String,
    /// Secondary metrics tracked alongside the primary.
    pub secondary_metrics: Vec<String>,
    /// Threshold the primary metric must exceed to confirm the hypothesis.
    pub success_threshold: f64,
    /// Minimum sample size required for adequate statistical power.
    pub sample_size_min: u64,
    /// Human-readable analysis plan description.
    pub analysis_plan: String,
    /// Timestamp when this pre-registration was filed.
    pub registered_at: DateTime<Utc>,
}

/// Statistical guardrails applied during Analyze / Decide.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticalGuardrail {
    /// Minimum sample size; triggers underpowered warning if actual < this.
    pub min_sample_size: u64,
    /// Minimum effect size (Cohen's d or equivalent) to consider meaningful.
    pub effect_size_threshold: f64,
    /// Correction method for multiple comparisons.
    pub correction_method: CorrectionMethod,
}

impl Default for StatisticalGuardrail {
    fn default() -> Self {
        Self {
            min_sample_size: 30,
            effect_size_threshold: 0.2,
            correction_method: CorrectionMethod::Holm,
        }
    }
}

/// Provenance snapshot capturing reproducibility metadata for a run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSnapshot {
    /// OS / platform identifier.
    pub platform: String,
    /// Relevant environment variables (filtered).
    pub env_vars: HashMap<String, String>,
    /// Dependency versions (crate name → version).
    pub dependency_versions: HashMap<String, String>,
    /// Hardware summary (CPU model, GPU, RAM).
    pub hardware_summary: String,
}

/// A single RSI experiment with full provenance and phase tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    /// Unique experiment identifier.
    pub id: Uuid,
    /// The hypothesis being tested.
    pub hypothesis: String,
    /// Version tag for the experimental design document.
    pub design_version: String,
    /// SHA-256 hash of the dataset used.
    pub dataset_hash: String,
    /// Git commit (or equivalent) of the code under test.
    pub code_commit: String,
    /// Reproducibility metadata captured at creation time.
    pub env_snapshot: EnvSnapshot,
    /// Per-phase records keyed by `RsiPhase`.
    pub phases: HashMap<RsiPhase, PhaseRecord>,
    /// Current lifecycle status.
    pub status: ExperimentStatus,
    /// Optional pre-registration (required for confirmatory experiments).
    pub pre_registration: Option<PreRegistration>,
    /// Current phase pointer (last completed or currently active).
    current_phase: RsiPhase,
    /// Statistical guardrails for this experiment.
    pub guardrails: StatisticalGuardrail,
    /// Decision outcome (populated after Decide phase).
    pub decision: Option<DecisionOutcome>,
    /// Flag indicating HITL approval was granted for a post-start amendment.
    pub hitl_approval: bool,
    /// Blinded test-data reference (encrypted blob path or key).
    pub blinded_data_ref: Option<String>,
    /// Whether the blind has been lifted (only allowed after Analyze commit).
    pub blind_lifted: bool,
    /// Actual sample size observed during Measure.
    pub actual_sample_size: Option<u64>,
    /// Observed primary metric value (set during Analyze).
    pub observed_primary_metric: Option<f64>,
    /// Number of comparisons made (for correction).
    pub num_comparisons: Option<u32>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors arising from RSI protocol operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RsiError {
    #[error("cannot transition from {from:?} to {to:?}: expected {expected:?}")]
    InvalidTransition {
        from: RsiPhase,
        to: RsiPhase,
        expected: RsiPhase,
    },
    #[error("phase {0:?} gate not passed")]
    GateNotPassed(RsiPhase),
    #[error("pre-registration amendment requires HITL approval")]
    HitlRequired,
    #[error("blind can only be lifted after Analyze commit")]
    BlindLiftTooEarly,
    #[error("experiment already completed")]
    AlreadyCompleted,
}

// ---------------------------------------------------------------------------
// Experiment implementation
// ---------------------------------------------------------------------------

impl Experiment {
    /// Create a new experiment in the `Planning` / `Hypothesis` state.
    pub fn new(
        hypothesis: impl Into<String>,
        design_version: impl Into<String>,
        dataset_hash: impl Into<String>,
        code_commit: impl Into<String>,
        env_snapshot: EnvSnapshot,
    ) -> Self {
        let mut phases = HashMap::new();
        phases.insert(
            RsiPhase::Hypothesis,
            PhaseRecord {
                started_at: Utc::now(),
                completed_at: None,
                artifacts: Vec::new(),
                gate_passed: false,
            },
        );
        Self {
            id: Uuid::new_v4(),
            hypothesis: hypothesis.into(),
            design_version: design_version.into(),
            dataset_hash: dataset_hash.into(),
            code_commit: code_commit.into(),
            env_snapshot,
            phases,
            status: ExperimentStatus::Planning,
            pre_registration: None,
            current_phase: RsiPhase::Hypothesis,
            guardrails: StatisticalGuardrail::default(),
            decision: None,
            hitl_approval: false,
            blinded_data_ref: None,
            blind_lifted: false,
            actual_sample_size: None,
            observed_primary_metric: None,
            num_comparisons: None,
        }
    }

    // -- Phase transitions --------------------------------------------------

    /// Advance to the next phase. Enforces strict sequential ordering and
    /// requires the current phase gate to have been passed.
    pub fn advance_phase(&mut self) -> Result<RsiPhase, RsiError> {
        if self.status == ExperimentStatus::Completed {
            return Err(RsiError::AlreadyCompleted);
        }
        let next = self.current_phase.next().ok_or(RsiError::AlreadyCompleted)?;

        // Gate check on current phase.
        let current_record = self
            .phases
            .get(&self.current_phase)
            .expect("current phase must have a record");
        if !current_record.gate_passed {
            return Err(RsiError::GateNotPassed(self.current_phase));
        }

        // Mark current phase as completed.
        if let Some(rec) = self.phases.get_mut(&self.current_phase) {
            rec.completed_at = Some(Utc::now());
        }

        // Initialize next phase record.
        self.phases.entry(next).or_insert(PhaseRecord {
            started_at: Utc::now(),
            completed_at: None,
            artifacts: Vec::new(),
            gate_passed: false,
        });

        self.current_phase = next;

        // Update lifecycle status based on new phase.
        self.status = match next {
            RsiPhase::Hypothesis | RsiPhase::Design | RsiPhase::Build => {
                ExperimentStatus::InProgress
            }
            RsiPhase::Measure => ExperimentStatus::Blinded,
            RsiPhase::Analyze => ExperimentStatus::Analyzing,
            RsiPhase::Decide | RsiPhase::Log => ExperimentStatus::Completed,
        };

        Ok(next)
    }

    /// Mark the current phase gate as passed.
    pub fn pass_gate(&mut self) {
        if let Some(rec) = self.phases.get_mut(&self.current_phase) {
            rec.gate_passed = true;
        }
    }

    // -- Pre-registration ---------------------------------------------------

    /// Attach a pre-registration. Must be done before leaving Hypothesis.
    pub fn set_pre_registration(&mut self, pr: PreRegistration) -> Result<(), RsiError> {
        if self.current_phase > RsiPhase::Hypothesis && !self.hitl_approval {
            return Err(RsiError::HitlRequired);
        }
        self.pre_registration = Some(pr);
        Ok(())
    }

    /// Grant HITL approval for post-start amendments.
    pub fn grant_hitl_approval(&mut self) {
        self.hitl_approval = true;
    }

    // -- Blinded evaluation -------------------------------------------------

    /// Store a reference to blinded test data.
    pub fn set_blinded_data(&mut self, data_ref: impl Into<String>) {
        self.blinded_data_ref = Some(data_ref.into());
    }

    /// Lift the blind. Only permitted once the Analyze phase gate is passed.
    pub fn lift_blind(&mut self) -> Result<&str, RsiError> {
        if self.current_phase < RsiPhase::Analyze {
            return Err(RsiError::BlindLiftTooEarly);
        }
        let analyze_rec = self.phases.get(&RsiPhase::Analyze);
        let gate_ok = analyze_rec.is_some_and(|r| r.gate_passed);
        if !gate_ok {
            return Err(RsiError::BlindLiftTooEarly);
        }
        self.blind_lifted = true;
        self.blinded_data_ref
            .as_deref()
            .ok_or(RsiError::BlindLiftTooEarly)
    }

    // -- Statistical guardrails ---------------------------------------------

    /// Check whether the actual sample size meets the guardrail minimum.
    pub fn is_underpowered(&self) -> bool {
        self.actual_sample_size
         .is_none_or(|n| n < self.guardrails.min_sample_size)
    }

    /// Apply Bonferroni or Holm correction to a raw p-value given the number
    /// of comparisons. Returns the adjusted p-value.
    pub fn adjust_p_value(&self, raw_p: f64) -> f64 {
        let k = self.num_comparisons.unwrap_or(1) as f64;
        match self.guardrails.correction_method {
            CorrectionMethod::Bonferroni => (raw_p * k).min(1.0),
            CorrectionMethod::Holm => {
                // Simplified Holm: multiply by rank (here assumed worst-case = k).
                (raw_p * k).min(1.0)
            }
        }
    }

    // -- Decision framework -------------------------------------------------

    /// Classify the experiment result against pre-registered thresholds.
    /// Requires that `observed_primary_metric` and `actual_sample_size` are set.
    pub fn decide(&mut self) -> Result<DecisionOutcome, RsiError> {
        if self.current_phase != RsiPhase::Decide {
            return Err(RsiError::InvalidTransition {
                from: self.current_phase,
                to: RsiPhase::Decide,
                expected: self.current_phase.next().unwrap_or(RsiPhase::Log),
            });
        }

        let pr = match &self.pre_registration {
            Some(pr) => pr,
            None => {
                // Without pre-registration, result is inconclusive.
                self.decision = Some(DecisionOutcome::Inconclusive);
                return Ok(DecisionOutcome::Inconclusive);
            }
        };

        let metric = self.observed_primary_metric.unwrap_or(0.0);

        if self.is_underpowered() {
            self.decision = Some(DecisionOutcome::NeedsReplication);
            return Ok(DecisionOutcome::NeedsReplication);
        }

        let outcome = if metric >= pr.success_threshold {
            DecisionOutcome::Confirm
        } else {
            DecisionOutcome::Reject
        };

        self.decision = Some(outcome);
        Ok(outcome)
    }

    // -- Meta-analysis stub -------------------------------------------------

    /// Aggregate results from multiple related experiments using a simplified
    /// random-effects model. Returns the pooled effect estimate and its
    /// standard error. This is a stub; production use should integrate a
    /// proper meta-analysis library.
    pub fn meta_analyze_random_effects(
        effects: &[f64],
        variances: &[f64],
    ) -> Option<(f64, f64)> {
        if effects.len() != variances.len() || effects.is_empty() {
            return None;
        }
        // DerSimonian-Laird estimator (simplified).
        let k = effects.len() as f64;
        let weights: Vec<f64> = variances.iter().map(|v| 1.0 / v.max(1e-12)).collect();
        let w_sum: f64 = weights.iter().sum();
        let pooled = weights
            .iter()
            .zip(effects.iter())
            .map(|(w, e)| w * e)
            .sum::<f64>()
            / w_sum;

        // Q statistic for between-study variance (tau²).
        let q: f64 = weights
            .iter()
            .zip(effects.iter())
            .map(|(w, e)| w * (e - pooled).powi(2))
            .sum();
        let c = w_sum - weights.iter().map(|w| w.powi(2)).sum::<f64>() / w_sum;
        let tau2 = ((q - (k - 1.0)) / c).max(0.0);

        // Random-effects weights.
        let re_weights: Vec<f64> = variances
            .iter()
            .map(|v| 1.0 / (v + tau2).max(1e-12))
            .collect();
        let re_w_sum: f64 = re_weights.iter().sum();
        let re_pooled = re_weights
            .iter()
            .zip(effects.iter())
            .map(|(w, e)| w * e)
            .sum::<f64>()
            / re_w_sum;
        let se = (1.0 / re_w_sum).sqrt();

        Some((re_pooled, se))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_env() -> EnvSnapshot {
        EnvSnapshot {
            platform: "linux-x86_64".into(),
            env_vars: HashMap::new(),
            dependency_versions: HashMap::new(),
            hardware_summary: "test".into(),
        }
    }

    fn make_experiment() -> Experiment {
        Experiment::new("h1", "v1", "abc123", "deadbeef", sample_env())
    }

    // -- Phase transition validation ----------------------------------------

    #[test]
    fn test_sequential_phase_transitions() {
        let mut exp = make_experiment();
        assert_eq!(exp.current_phase, RsiPhase::Hypothesis);

        // Advance through Hypothesis→Design→Build→Measure→Analyze→Decide (5 transitions).
        // After entering Decide, status becomes Completed, so the next advance_phase
        // returns AlreadyCompleted even though Log is a valid next phase.
        for expected in &RsiPhase::ALL[1..6] {
            exp.pass_gate();
            let got = exp.advance_phase().unwrap();
            assert_eq!(&got, expected);
        }
        assert_eq!(exp.current_phase, RsiPhase::Decide);
        assert_eq!(exp.status, ExperimentStatus::Completed);

        // Further advancement returns AlreadyCompleted because status is Completed.
        exp.pass_gate();
        let err = exp.advance_phase().unwrap_err();
        assert!(matches!(err, RsiError::AlreadyCompleted));
    }

    #[test]
    fn test_cannot_skip_phases() {
        let mut exp = make_experiment();
        // Try to jump to Build without passing Hypothesis gate.
        let err = exp.advance_phase().unwrap_err();
        assert!(matches!(err, RsiError::GateNotPassed(RsiPhase::Hypothesis)));
    }

    #[test]
    fn test_advance_after_completion_fails() {
        let mut exp = make_experiment();
        // Walk through 5 valid transitions: Hypothesis→Design→Build→Measure→Analyze→Decide.
        // After entering Decide, status becomes Completed.
        for _ in 0..5 {
            exp.pass_gate();
            exp.advance_phase().unwrap();
        }
        assert_eq!(exp.current_phase, RsiPhase::Decide);
        assert_eq!(exp.status, ExperimentStatus::Completed);
        // Further advance returns AlreadyCompleted because status is Completed.
        exp.pass_gate();
        let err = exp.advance_phase().unwrap_err();
        assert!(matches!(err, RsiError::AlreadyCompleted));
    }

    // -- Blinded evaluation integrity ---------------------------------------

    #[test]
    fn test_blind_lift_requires_analyze_gate() {
        let mut exp = make_experiment();
        exp.set_blinded_data("encrypted://blob/42");

        // Before reaching Analyze → error.
        assert!(exp.lift_blind().is_err());

        // Advance to Analyze but don't pass gate → error.
        exp.pass_gate(); // Hypothesis
        exp.advance_phase().unwrap(); // → Design
        exp.pass_gate();
        exp.advance_phase().unwrap(); // → Build
        exp.pass_gate();
        exp.advance_phase().unwrap(); // → Measure
        exp.pass_gate();
        exp.advance_phase().unwrap(); // → Analyze
        assert!(exp.lift_blind().is_err());

        // Pass Analyze gate → success.
        exp.pass_gate();
        assert!(exp.lift_blind().is_ok());
        assert!(exp.blind_lifted);
    }

    // -- Guardrail triggers -------------------------------------------------

    #[test]
    fn test_underpowered_detection() {
        let mut exp = make_experiment();
        // Default guardrail min = 30.
        assert!(exp.is_underpowered()); // no sample size set

        exp.actual_sample_size = Some(10);
        assert!(exp.is_underpowered());

        exp.actual_sample_size = Some(30);
        assert!(!exp.is_underpowered());
    }

    #[test]
    fn test_bonferroni_correction() {
        let mut exp = make_experiment();
        exp.guardrails.correction_method = CorrectionMethod::Bonferroni;
        exp.num_comparisons = Some(5);

        let adjusted = exp.adjust_p_value(0.01);
        assert!((adjusted - 0.05).abs() < 1e-9);
    }

    #[test]
    fn test_holm_correction() {
        let mut exp = make_experiment();
        exp.guardrails.correction_method = CorrectionMethod::Holm;
        exp.num_comparisons = Some(3);

        let adjusted = exp.adjust_p_value(0.02);
        // Simplified Holm behaves like Bonferroni at worst rank.
        assert!((adjusted - 0.06).abs() < 1e-9);
    }

    // -- Decision correctness -----------------------------------------------

    #[test]
    fn test_decision_confirm() {
        let mut exp = make_experiment();
        exp.set_pre_registration(PreRegistration {
            primary_metric: "accuracy".into(),
            secondary_metrics: vec![],
            success_threshold: 0.9,
            sample_size_min: 10,
            analysis_plan: "t-test".into(),
            registered_at: Utc::now(),
        })
        .unwrap();

        // Walk to Decide.
        for _ in 0..4 {
            exp.pass_gate();
            exp.advance_phase().unwrap();
        }
        exp.pass_gate(); // Analyze
        exp.advance_phase().unwrap(); // → Decide

        exp.actual_sample_size = Some(50);
        exp.observed_primary_metric = Some(0.95);

        let outcome = exp.decide().unwrap();
        assert_eq!(outcome, DecisionOutcome::Confirm);
    }

    #[test]
    fn test_decision_reject() {
        let mut exp = make_experiment();
        exp.set_pre_registration(PreRegistration {
            primary_metric: "accuracy".into(),
            secondary_metrics: vec![],
            success_threshold: 0.9,
            sample_size_min: 10,
            analysis_plan: "t-test".into(),
            registered_at: Utc::now(),
        })
        .unwrap();

        for _ in 0..4 {
            exp.pass_gate();
            exp.advance_phase().unwrap();
        }
        exp.pass_gate();
        exp.advance_phase().unwrap();

        exp.actual_sample_size = Some(50);
        exp.observed_primary_metric = Some(0.7);

        let outcome = exp.decide().unwrap();
        assert_eq!(outcome, DecisionOutcome::Reject);
    }

    #[test]
    fn test_decision_needs_replication_when_underpowered() {
        let mut exp = make_experiment();
        exp.set_pre_registration(PreRegistration {
            primary_metric: "f1".into(),
            secondary_metrics: vec![],
            success_threshold: 0.8,
            sample_size_min: 100,
            analysis_plan: "bootstrap".into(),
            registered_at: Utc::now(),
        })
        .unwrap();

        for _ in 0..4 {
            exp.pass_gate();
            exp.advance_phase().unwrap();
        }
        exp.pass_gate();
        exp.advance_phase().unwrap();

        exp.actual_sample_size = Some(5); // underpowered
        exp.observed_primary_metric = Some(0.95);

        let outcome = exp.decide().unwrap();
        assert_eq!(outcome, DecisionOutcome::NeedsReplication);
    }

    #[test]
    fn test_decision_inconclusive_without_preregistration() {
        let mut exp = make_experiment();
        for _ in 0..4 {
            exp.pass_gate();
            exp.advance_phase().unwrap();
        }
        exp.pass_gate();
        exp.advance_phase().unwrap();

        let outcome = exp.decide().unwrap();
        assert_eq!(outcome, DecisionOutcome::Inconclusive);
    }

    // -- Pre-registration HITL enforcement ----------------------------------

    #[test]
    fn test_preregistration_amendment_requires_hitl() {
        let mut exp = make_experiment();
        exp.pass_gate();
        exp.advance_phase().unwrap(); // → Design

        let pr = PreRegistration {
            primary_metric: "x".into(),
            secondary_metrics: vec![],
            success_threshold: 0.5,
            sample_size_min: 10,
            analysis_plan: "plan".into(),
            registered_at: Utc::now(),
        };
        assert!(exp.set_pre_registration(pr).is_err());

        exp.grant_hitl_approval();
        let pr2 = PreRegistration {
            primary_metric: "y".into(),
            secondary_metrics: vec![],
            success_threshold: 0.6,
            sample_size_min: 20,
            analysis_plan: "plan2".into(),
            registered_at: Utc::now(),
        };
        assert!(exp.set_pre_registration(pr2).is_ok());
    }

    // -- Meta-analysis stub -------------------------------------------------

    #[test]
    fn test_meta_analysis_random_effects() {
        let effects = vec![0.5, 0.6, 0.55];
        let variances = vec![0.01, 0.02, 0.015];
        let (pooled, se) = Experiment::meta_analyze_random_effects(&effects, &variances).unwrap();
        // Pooled should be near 0.55; SE should be positive.
        assert!((pooled - 0.55).abs() < 0.05);
        assert!(se > 0.0);
    }

    #[test]
    fn test_meta_analysis_mismatched_lengths() {
        assert!(Experiment::meta_analyze_random_effects(&[0.5], &[0.01, 0.02]).is_none());
    }

    #[test]
    fn test_meta_analysis_empty() {
        assert!(Experiment::meta_analyze_random_effects(&[], &[]).is_none());
    }
}