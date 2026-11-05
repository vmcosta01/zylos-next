//! zylos-verification — VERIFICATION_PROOF system for Zylos-Next v6
//!
//! Provides:
//! - [`Ground`]: Certainty levels (Observado, Confirmado, Crido)
//! - [`Proof`]: Verifiable claims with evidence chains
//! - [`Guardrail`]: Sub-millisecond false-success detection
//! - [`Judge`] / [`AdversarialPanel`]: Multi-judge deliberation (Sprint 3.2)
//! - [`TreeOfThoughts`]: BFS/DFS reasoning exploration (Sprint 3.3)

pub mod ground;
pub mod ground_adapter;
pub mod finding_validator;
pub mod proof;
pub mod guardrail;
pub mod judge;
pub mod rsi;
pub mod goal_contract;
pub mod tot_reasoning;
pub mod screenshot_verification;

pub use ground::Ground;
pub use proof::{Proof, ProofId, Evidence};
pub use guardrail::{Guardrail, GuardrailResult};
pub use judge::{AdversarialPanel, PanelConfig, PanelResult, Verdict, JudgeId};
pub use rsi::{
    CorrectionMethod, DecisionOutcome, EnvSnapshot, Experiment, ExperimentStatus, PhaseRecord,
    PreRegistration, RsiError, RsiPhase, StatisticalGuardrail,
};
pub use goal_contract::{
    GoalContract, Criterion, VerifySpec, ArtifactSpec, Expr, RetryPolicy,
    EvidenceCapsule, ContractVersion, ValidationResult, ContractError, ContractId,
};
