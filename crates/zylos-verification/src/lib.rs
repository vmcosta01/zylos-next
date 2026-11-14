//! zylos-verification — VERIFICATION_PROOF system for Zylos-Next v6
//!
//! Provides:
//! - [`Ground`]: Certainty levels (Observado, Confirmado, Crido)
//! - [`Proof`]: Verifiable claims with evidence chains
//! - [`Guardrail`]: Sub-millisecond false-success detection
//! - [`Judge`] / [`AdversarialPanel`]: Multi-judge deliberation (Sprint 3.2)
//! - [`TreeOfThoughts`]: BFS/DFS reasoning exploration (Sprint 3.3)

pub mod finding_validator;
pub mod goal_contract;
pub mod ground;
pub mod ground_adapter;
pub mod guardrail;
pub mod judge;
pub mod proof;
pub mod rsi;
pub mod screenshot_verification;
pub mod tot_reasoning;

pub use goal_contract::{
    ArtifactSpec, ContractError, ContractId, ContractVersion, Criterion, EvidenceCapsule, Expr,
    GoalContract, RetryPolicy, ValidationResult, VerifySpec,
};
pub use ground::Ground;
pub use guardrail::{Guardrail, GuardrailResult};
pub use judge::{AdversarialPanel, JudgeId, PanelConfig, PanelResult, Verdict};
pub use proof::{Evidence, Proof, ProofId};
pub use rsi::{
    CorrectionMethod, DecisionOutcome, EnvSnapshot, Experiment, ExperimentStatus, PhaseRecord,
    PreRegistration, RsiError, RsiPhase, StatisticalGuardrail,
};
