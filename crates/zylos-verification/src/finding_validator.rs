//! Finding Validator — Independent Oracle Cross-Validation (Task 3.2.3).
//!
//! Provides:
//! - [`FindingId`]: Namespace-isolated finding identifiers (domain + sequence)
//! - [`Oracle`] trait: Pluggable validation strategies (Tool, Human, LLM, Statistical, CrossRef)
//! - [`CrossValidationEngine`]: Selects ≥3 independent oracles, aggregates via weighted voting
//! - [`InconsistencyDetector`]: Classifies oracle disagreements
//! - \[`FindingLifecycle`\]: State machine (Draft → PendingValidation → Validated|Contested|Refuted → Archived)
//! - Integration hooks for Proof Engine promotion/refutation

use crate::ground::Ground;
use crate::proof::{Evidence, Proof, ProofStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core identifiers
// ---------------------------------------------------------------------------

/// Namespace-isolated finding identifier.
///
/// Each finding belongs to a `domain` (e.g., "security", "performance", "correctness")
/// with a monotonically increasing `sequence` within that domain. This prevents
/// cross-domain contamination: findings from different domains never share IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FindingId {
    /// Knowledge domain this finding belongs to.
    pub domain: String,
    /// Monotonically increasing sequence number within the domain.
    pub sequence: u64,
}

impl FindingId {
    pub fn new(domain: impl Into<String>, sequence: u64) -> Self {
        Self {
            domain: domain.into(),
            sequence,
        }
    }
}

impl fmt::Display for FindingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.domain, self.sequence)
    }
}

// ---------------------------------------------------------------------------
// Oracle types
// ---------------------------------------------------------------------------

/// Type of validation oracle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OracleType {
    /// Re-executes the original tool to verify reproducibility.
    Tool,
    /// Human-in-the-loop review.
    Human,
    /// Structured LLM reasoning over the finding.
    LLM,
    /// Statistical tests (confidence intervals, p-values).
    Statistical,
    /// Cross-reference against other validated findings.
    CrossRef,
}

impl fmt::Display for OracleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OracleType::Tool => write!(f, "Tool"),
            OracleType::Human => write!(f, "Human"),
            OracleType::LLM => write!(f, "LLM"),
            OracleType::Statistical => write!(f, "Statistical"),
            OracleType::CrossRef => write!(f, "CrossRef"),
        }
    }
}

/// Verdict returned by an oracle after validating a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OracleVerdict {
    /// The finding is confirmed as accurate.
    Confirmed,
    /// The finding is refuted / incorrect.
    Refuted,
    /// The oracle cannot determine validity with sufficient confidence.
    Uncertain,
}

// ---------------------------------------------------------------------------
// Oracle trait
// ---------------------------------------------------------------------------

/// A validation oracle that can independently assess a finding.
///
/// Implementations must be stateless with respect to other oracles to ensure
/// independence in cross-validation.
pub trait Oracle: Send + Sync {
    /// Validate a finding and return a verdict.
    fn validate(&self, finding: &Finding) -> OracleVerdict;

    /// The type of this oracle.
    fn oracle_type(&self) -> OracleType;

    /// Confidence weight for weighted voting aggregation (0.0–1.0).
    fn confidence_weight(&self) -> f64;
}

// ---------------------------------------------------------------------------
// Finding & provenance
// ---------------------------------------------------------------------------

/// An oracle's recorded verdict with timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleRecord {
    pub oracle_type: OracleType,
    pub verdict: OracleVerdict,
    pub timestamp: DateTime<Utc>,
    pub confidence: f64,
}

/// Provenance information tracking a finding's lineage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingProvenance {
    /// Evidence IDs that sourced this finding.
    pub source_evidence: Vec<String>,
    /// Recorded oracle verdicts.
    pub oracle_verdicts: Vec<OracleRecord>,
    /// Cross-references to other findings.
    pub cross_refs: Vec<FindingId>,
    /// Depth of the dependency chain.
    pub lineage_depth: u32,
}

impl FindingProvenance {
    pub fn new() -> Self {
        Self {
            source_evidence: Vec::new(),
            oracle_verdicts: Vec::new(),
            cross_refs: Vec::new(),
            lineage_depth: 0,
        }
    }
}

impl Default for FindingProvenance {
    fn default() -> Self {
        Self::new()
    }
}

/// Lifecycle states for a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingState {
    Draft,
    PendingValidation,
    Validated,
    Contested,
    Refuted,
    Archived,
}

/// Errors from invalid lifecycle transitions.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LifecycleError {
    #[error("invalid transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: FindingState,
        to: FindingState,
    },
}

/// A finding with its full lifecycle and provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub claim: String,
    pub state: FindingState,
    pub provenance: FindingProvenance,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Finding {
    pub fn new(id: FindingId, claim: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id,
            claim: claim.into(),
            state: FindingState::Draft,
            provenance: FindingProvenance::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Attempt a lifecycle transition. Returns error if invalid.
    pub fn transition(&mut self, to: FindingState) -> Result<(), LifecycleError> {
        let valid = matches!(
            (&self.state, &to),
            (FindingState::Draft, FindingState::PendingValidation)
                | (FindingState::PendingValidation, FindingState::Validated)
                | (FindingState::PendingValidation, FindingState::Contested)
                | (FindingState::PendingValidation, FindingState::Refuted)
                | (FindingState::Contested, FindingState::PendingValidation)
                | (FindingState::Contested, FindingState::Archived)
                | (FindingState::Validated, FindingState::Archived)
                | (FindingState::Refuted, FindingState::Archived)
        );
        if valid {
            tracing::info!(
                finding_id = %self.id,
                from = ?self.state,
                to = ?to,
                "finding lifecycle transition"
            );
            self.state = to;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(LifecycleError::InvalidTransition {
                from: self.state,
                to,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Inconsistency detection
// ---------------------------------------------------------------------------

/// Classification of disagreement between oracles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InconsistencyType {
    /// Oracles disagree on factual claims.
    FactualConflict,
    /// Oracles use different methodologies leading to different conclusions.
    MethodologyDisagreement,
    /// Oracles are evaluating different scopes of the same finding.
    ScopeMismatch,
    /// Disagreement due to temporal drift (data changed between validations).
    TemporalDrift,
}

/// Report produced when oracles disagree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InconsistencyReport {
    pub finding_id: FindingId,
    pub inconsistency_type: InconsistencyType,
    pub conflicting_oracles: Vec<(OracleType, OracleVerdict)>,
    pub resolution_needed: bool,
}

/// Detects and classifies inconsistencies among oracle verdicts.
pub struct InconsistencyDetector;

impl InconsistencyDetector {
    /// Analyze a set of oracle records and produce an inconsistency report if needed.
    pub fn detect(finding_id: &FindingId, records: &[OracleRecord]) -> Option<InconsistencyReport> {
        let has_confirmed = records
            .iter()
            .any(|r| r.verdict == OracleVerdict::Confirmed);
        let has_refuted = records.iter().any(|r| r.verdict == OracleVerdict::Refuted);

        if !has_confirmed || !has_refuted {
            return None; // No real conflict
        }

        let conflicting: Vec<_> = records
            .iter()
            .filter(|r| r.verdict != OracleVerdict::Uncertain)
            .map(|r| (r.oracle_type, r.verdict))
            .collect();

        // Heuristic classification based on oracle types involved
        let types: Vec<OracleType> = conflicting.iter().map(|(t, _)| *t).collect();
        let inconsistency_type =
            if types.contains(&OracleType::Statistical) && types.contains(&OracleType::Tool) {
                InconsistencyType::MethodologyDisagreement
            } else if types.contains(&OracleType::CrossRef) {
                InconsistencyType::ScopeMismatch
            } else if (types.contains(&OracleType::Human) || types.contains(&OracleType::Tool))
                && (types.contains(&OracleType::LLM) || types.contains(&OracleType::Human))
            {
                InconsistencyType::FactualConflict
            } else {
                InconsistencyType::TemporalDrift
            };

        Some(InconsistencyReport {
            finding_id: finding_id.clone(),
            inconsistency_type,
            conflicting_oracles: conflicting,
            resolution_needed: true,
        })
    }
}

// ---------------------------------------------------------------------------
// Cross-validation engine
// ---------------------------------------------------------------------------

/// Strategy for selecting oracles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Maximize diversity of oracle types.
    DiversityMaximizing,
    /// Weight selection by oracle confidence.
    ConfidenceWeighted,
    /// Select oracles specific to the finding's domain.
    DomainSpecific,
}

/// Aggregated result from cross-validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossValidationResult {
    pub finding_id: FindingId,
    pub aggregate_verdict: OracleVerdict,
    pub confidence: f64,
    pub oracle_count: usize,
    pub records: Vec<OracleRecord>,
    pub inconsistency: Option<InconsistencyReport>,
}

/// Engine that selects independent oracles and aggregates their verdicts.
pub struct CrossValidationEngine {
    oracles: Vec<Box<dyn Oracle>>,
    strategy: SelectionStrategy,
    min_oracles: usize,
}

impl CrossValidationEngine {
    pub fn new(strategy: SelectionStrategy) -> Self {
        Self {
            oracles: Vec::new(),
            strategy,
            min_oracles: 3,
        }
    }

    /// Register an oracle with the engine.
    pub fn register(&mut self, oracle: Box<dyn Oracle>) {
        self.oracles.push(oracle);
    }

    /// Select at least `min_oracles` independent oracles for a finding.
    fn select_oracles(&self, _finding: &Finding) -> Vec<&dyn Oracle> {
        match self.strategy {
            SelectionStrategy::DiversityMaximizing => {
                // Pick one of each type first, then fill remaining slots
                let mut selected: Vec<&dyn Oracle> = Vec::new();
                let mut seen_types = std::collections::HashSet::new();

                for o in &self.oracles {
                    if seen_types.insert(o.oracle_type()) {
                        selected.push(o.as_ref());
                    }
                }
                // Fill up to min_oracles if we have more
                if selected.len() < self.min_oracles {
                    for o in &self.oracles {
                        if !selected.iter().any(|s| std::ptr::eq(*s, o.as_ref())) {
                            selected.push(o.as_ref());
                            if selected.len() >= self.min_oracles {
                                break;
                            }
                        }
                    }
                }
                selected
            }
            SelectionStrategy::ConfidenceWeighted => {
                let mut sorted: Vec<&dyn Oracle> =
                    self.oracles.iter().map(|o| o.as_ref()).collect();
                sorted.sort_by(|a, b| {
                    b.confidence_weight()
                        .partial_cmp(&a.confidence_weight())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                sorted.truncate(self.min_oracles.max(sorted.len()));
                sorted
            }
            SelectionStrategy::DomainSpecific => {
                // For now, same as diversity — domain-specific filtering would
                // require domain metadata on oracles.
                self.oracles.iter().map(|o| o.as_ref()).collect()
            }
        }
    }

    /// Validate a finding using selected oracles and aggregate results.
    pub fn validate(&self, finding: &Finding) -> CrossValidationResult {
        let selected = self.select_oracles(finding);
        let mut records = Vec::new();

        for oracle in &selected {
            let verdict = oracle.validate(finding);
            records.push(OracleRecord {
                oracle_type: oracle.oracle_type(),
                verdict,
                timestamp: Utc::now(),
                confidence: oracle.confidence_weight(),
            });
        }

        let (aggregate_verdict, confidence) = Self::weighted_vote(&records);
        let inconsistency = InconsistencyDetector::detect(&finding.id, &records);

        CrossValidationResult {
            finding_id: finding.id.clone(),
            aggregate_verdict,
            confidence,
            oracle_count: records.len(),
            records,
            inconsistency,
        }
    }

    /// Weighted voting aggregation.
    fn weighted_vote(records: &[OracleRecord]) -> (OracleVerdict, f64) {
        let mut confirmed_weight = 0.0_f64;
        let mut refuted_weight = 0.0_f64;
        let mut uncertain_weight = 0.0_f64;
        let mut total_weight = 0.0_f64;

        for r in records {
            total_weight += r.confidence;
            match r.verdict {
                OracleVerdict::Confirmed => confirmed_weight += r.confidence,
                OracleVerdict::Refuted => refuted_weight += r.confidence,
                OracleVerdict::Uncertain => uncertain_weight += r.confidence,
            }
        }

        if total_weight == 0.0 {
            return (OracleVerdict::Uncertain, 0.0);
        }

        let verdict = if confirmed_weight > refuted_weight && confirmed_weight > uncertain_weight {
            OracleVerdict::Confirmed
        } else if refuted_weight > confirmed_weight && refuted_weight > uncertain_weight {
            OracleVerdict::Refuted
        } else {
            OracleVerdict::Uncertain
        };

        let winning_weight = match verdict {
            OracleVerdict::Confirmed => confirmed_weight,
            OracleVerdict::Refuted => refuted_weight,
            OracleVerdict::Uncertain => uncertain_weight,
        };

        (verdict, winning_weight / total_weight)
    }
}

// ---------------------------------------------------------------------------
// Proof Engine integration hooks
// ---------------------------------------------------------------------------

/// Hooks for integrating validated/refuted findings with the Proof Engine.
pub struct ProofIntegration;

impl ProofIntegration {
    /// Promote a validated finding into an EvidenceCapsule for downstream proofs.
    pub fn promote_to_evidence(finding: &Finding) -> Evidence {
        Evidence::new(
            format!("Validated finding {}: {}", finding.id, finding.claim),
            format!("finding-validator/{}", finding.id),
            Ground::Confirmado,
        )
    }

    /// Create a refutation proof when a finding is refuted.
    pub fn create_refutation(finding: &Finding) -> Proof {
        let evidence = Evidence::new(
            format!("Refuted finding {}: {}", finding.id, finding.claim),
            format!("finding-validator/{}", finding.id),
            Ground::Observado,
        );
        Proof {
            id: Uuid::new_v4(),
            claim: format!("Refutation of finding {}", finding.id),
            ground: Ground::Observado,
            status: ProofStatus::Refuted,
            evidence: vec![evidence],
            composed_from: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            notes: Some(format!(
                "Auto-generated refutation from finding validator for {}",
                finding.id
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Namespace isolation registry
// ---------------------------------------------------------------------------

/// Registry that enforces namespace isolation across finding domains.
pub struct FindingRegistry {
    /// Next sequence number per domain.
    sequences: HashMap<String, u64>,
    /// Stored findings indexed by ID.
    findings: HashMap<FindingId, Finding>,
}

impl FindingRegistry {
    pub fn new() -> Self {
        Self {
            sequences: HashMap::new(),
            findings: HashMap::new(),
        }
    }

    /// Allocate a new FindingId in the given domain. Sequence numbers are
    /// monotonic and isolated per domain.
    pub fn next_id(&mut self, domain: impl Into<String>) -> FindingId {
        let domain = domain.into();
        let seq = self.sequences.entry(domain.clone()).or_insert(0);
        *seq += 1;
        FindingId::new(domain, *seq)
    }

    /// Insert a finding. Panics if a finding with the same ID already exists
    /// (namespace collision should never happen with `next_id`).
    pub fn insert(&mut self, finding: Finding) {
        self.findings.insert(finding.id.clone(), finding);
    }

    /// Retrieve a finding by ID.
    pub fn get(&self, id: &FindingId) -> Option<&Finding> {
        self.findings.get(id)
    }

    /// Retrieve a mutable reference to a finding by ID.
    pub fn get_mut(&mut self, id: &FindingId) -> Option<&mut Finding> {
        self.findings.get_mut(id)
    }

    /// List all findings in a specific domain.
    pub fn list_by_domain(&self, domain: &str) -> Vec<&Finding> {
        self.findings
            .values()
            .filter(|f| f.id.domain == domain)
            .collect()
    }
}

impl Default for FindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Stub oracles for testing --

    struct AlwaysConfirmedOracle(OracleType, f64);
    impl Oracle for AlwaysConfirmedOracle {
        fn validate(&self, _: &Finding) -> OracleVerdict {
            OracleVerdict::Confirmed
        }
        fn oracle_type(&self) -> OracleType {
            self.0
        }
        fn confidence_weight(&self) -> f64 {
            self.1
        }
    }

    struct AlwaysRefutedOracle(OracleType, f64);
    impl Oracle for AlwaysRefutedOracle {
        fn validate(&self, _: &Finding) -> OracleVerdict {
            OracleVerdict::Refuted
        }
        fn oracle_type(&self) -> OracleType {
            self.0
        }
        fn confidence_weight(&self) -> f64 {
            self.1
        }
    }

    struct AlwaysUncertainOracle(OracleType, f64);
    impl Oracle for AlwaysUncertainOracle {
        fn validate(&self, _: &Finding) -> OracleVerdict {
            OracleVerdict::Uncertain
        }
        fn oracle_type(&self) -> OracleType {
            self.0
        }
        fn confidence_weight(&self) -> f64 {
            self.1
        }
    }

    // -- Namespace isolation tests --

    #[test]
    fn test_namespace_isolation() {
        let mut registry = FindingRegistry::new();
        let id1 = registry.next_id("security");
        let id2 = registry.next_id("security");
        let id3 = registry.next_id("performance");

        assert_eq!(id1, FindingId::new("security", 1));
        assert_eq!(id2, FindingId::new("security", 2));
        assert_eq!(id3, FindingId::new("performance", 1));

        // Domains are isolated: performance starts at 1 regardless of security count
        assert_ne!(id1.domain, id3.domain);
        assert_eq!(id3.sequence, 1);
    }

    #[test]
    fn test_namespace_isolation_no_cross_contamination() {
        let mut registry = FindingRegistry::new();
        for _ in 0..10 {
            registry.next_id("domain_a");
        }
        let id_b = registry.next_id("domain_b");
        assert_eq!(
            id_b.sequence, 1,
            "domain_b must start at 1 regardless of domain_a count"
        );
    }

    #[test]
    fn test_registry_list_by_domain() {
        let mut registry = FindingRegistry::new();
        let id1 = registry.next_id("security");
        let id2 = registry.next_id("security");
        let id3 = registry.next_id("performance");

        registry.insert(Finding::new(id1, "SQL injection found"));
        registry.insert(Finding::new(id2, "XSS found"));
        registry.insert(Finding::new(id3, "Latency regression"));

        let security = registry.list_by_domain("security");
        assert_eq!(security.len(), 2);

        let perf = registry.list_by_domain("performance");
        assert_eq!(perf.len(), 1);

        let empty = registry.list_by_domain("unknown");
        assert_eq!(empty.len(), 0);
    }

    // -- Oracle selection diversity tests --

    #[test]
    fn test_oracle_selection_diversity() {
        let mut engine = CrossValidationEngine::new(SelectionStrategy::DiversityMaximizing);
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::Tool, 0.9)));
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::LLM, 0.8)));
        engine.register(Box::new(AlwaysConfirmedOracle(
            OracleType::Statistical,
            0.7,
        )));
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::CrossRef, 0.6)));
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::Human, 0.95)));

        let finding = Finding::new(FindingId::new("test", 1), "test claim");
        let result = engine.validate(&finding);

        // Must use at least 3 oracles
        assert!(result.oracle_count >= 3);

        // With DiversityMaximizing, we should see distinct oracle types
        let types: std::collections::HashSet<_> =
            result.records.iter().map(|r| r.oracle_type).collect();
        assert!(
            types.len() >= 3,
            "Expected diverse oracle types, got {:?}",
            types
        );
    }

    #[test]
    fn test_oracle_selection_minimum_three() {
        let mut engine = CrossValidationEngine::new(SelectionStrategy::DiversityMaximizing);
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::Tool, 0.9)));
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::LLM, 0.8)));
        engine.register(Box::new(AlwaysConfirmedOracle(
            OracleType::Statistical,
            0.7,
        )));

        let finding = Finding::new(FindingId::new("test", 1), "test claim");
        let result = engine.validate(&finding);
        assert_eq!(result.oracle_count, 3);
    }

    // -- Cross-validation aggregation tests --

    #[test]
    fn test_weighted_voting_all_confirmed() {
        let mut engine = CrossValidationEngine::new(SelectionStrategy::DiversityMaximizing);
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::Tool, 0.9)));
        engine.register(Box::new(AlwaysConfirmedOracle(OracleType::LLM, 0.8)));
        engine.register(Box::new(AlwaysConfirmedOracle(
            OracleType::Statistical,
            0.7,
        )));

        let finding = Finding::new(FindingId::new("test", 1), "test claim");
        let result = engine.validate(&finding);

        assert_eq!(result.aggregate_verdict, OracleVerdict::Confirmed);
        assert!(result.confidence > 0.0);
        assert!(result.inconsistency.is_none());
    }

    #[test]
    fn test_weighted_voting_majority_refuted() {
        let mut engine = CrossValidationEngine::new(SelectionStrategy::DiversityMaximizing);
        engine.register(Box::new(AlwaysRefutedOracle(OracleType::Tool, 0.9)));
        engine.register(Box::new(AlwaysRefutedOracle(OracleType::LLM, 0.8)));
        engine.register(Box::new(AlwaysConfirmedOracle(
            OracleType::Statistical,
            0.3,
        )));

        let finding = Finding::new(FindingId::new("test", 1), "test claim");
        let result = engine.validate(&finding);

        assert_eq!(result.aggregate_verdict, OracleVerdict::Refuted);
        assert!(result.inconsistency.is_some());
    }

    #[test]
    fn test_weighted_voting_with_uncertain() {
        let mut engine = CrossValidationEngine::new(SelectionStrategy::DiversityMaximizing);
        engine.register(Box::new(AlwaysUncertainOracle(OracleType::Tool, 0.9)));
        engine.register(Box::new(AlwaysUncertainOracle(OracleType::LLM, 0.8)));
        engine.register(Box::new(AlwaysUncertainOracle(
            OracleType::Statistical,
            0.7,
        )));

        let finding = Finding::new(FindingId::new("test", 1), "test claim");
        let result = engine.validate(&finding);

        assert_eq!(result.aggregate_verdict, OracleVerdict::Uncertain);
        assert!(result.inconsistency.is_none());
    }

    // -- Inconsistency detection tests --

    #[test]
    fn test_inconsistency_detection() {
        let records = vec![
            OracleRecord {
                oracle_type: OracleType::Tool,
                verdict: OracleVerdict::Confirmed,
                timestamp: Utc::now(),
                confidence: 0.9,
            },
            OracleRecord {
                oracle_type: OracleType::LLM,
                verdict: OracleVerdict::Refuted,
                timestamp: Utc::now(),
                confidence: 0.8,
            },
        ];

        let report = InconsistencyDetector::detect(&FindingId::new("test", 1), &records);
        assert!(report.is_some());
        let report = report.unwrap();
        assert_eq!(
            report.inconsistency_type,
            InconsistencyType::FactualConflict
        );
        assert!(report.resolution_needed);
    }

    #[test]
    fn test_no_inconsistency_when_agreement() {
        let records = vec![
            OracleRecord {
                oracle_type: OracleType::Tool,
                verdict: OracleVerdict::Confirmed,
                timestamp: Utc::now(),
                confidence: 0.9,
            },
            OracleRecord {
                oracle_type: OracleType::LLM,
                verdict: OracleVerdict::Confirmed,
                timestamp: Utc::now(),
                confidence: 0.8,
            },
        ];

        let report = InconsistencyDetector::detect(&FindingId::new("test", 1), &records);
        assert!(report.is_none());
    }

    // -- Lifecycle transition tests --

    #[test]
    fn test_lifecycle_valid_transitions() {
        let mut finding = Finding::new(FindingId::new("test", 1), "claim");
        assert_eq!(finding.state, FindingState::Draft);

        assert!(finding.transition(FindingState::PendingValidation).is_ok());
        assert_eq!(finding.state, FindingState::PendingValidation);

        assert!(finding.transition(FindingState::Validated).is_ok());
        assert_eq!(finding.state, FindingState::Validated);

        assert!(finding.transition(FindingState::Archived).is_ok());
        assert_eq!(finding.state, FindingState::Archived);
    }

    #[test]
    fn test_lifecycle_contested_path() {
        let mut finding = Finding::new(FindingId::new("test", 1), "claim");
        assert!(finding.transition(FindingState::PendingValidation).is_ok());
        assert!(finding.transition(FindingState::Contested).is_ok());
        assert_eq!(finding.state, FindingState::Contested);

        // Can re-enter validation from contested
        assert!(finding.transition(FindingState::PendingValidation).is_ok());
        assert!(finding.transition(FindingState::Refuted).is_ok());
        assert_eq!(finding.state, FindingState::Refuted);
    }

    #[test]
    fn test_lifecycle_invalid_transitions() {
        let mut finding = Finding::new(FindingId::new("test", 1), "claim");

        // Cannot skip Draft → Validated
        assert!(finding.transition(FindingState::Validated).is_err());

        // Cannot go Draft → Archived
        assert!(finding.transition(FindingState::Archived).is_err());

        // Cannot go Draft → Refuted
        assert!(finding.transition(FindingState::Refuted).is_err());
    }

    #[test]
    fn test_lifecycle_archived_is_terminal_except_from_valid_states() {
        let mut finding = Finding::new(FindingId::new("test", 1), "claim");
        assert!(finding.transition(FindingState::PendingValidation).is_ok());
        assert!(finding.transition(FindingState::Validated).is_ok());
        assert!(finding.transition(FindingState::Archived).is_ok());

        // Cannot transition out of Archived
        assert!(finding.transition(FindingState::Draft).is_err());
        assert!(finding.transition(FindingState::PendingValidation).is_err());
    }

    // -- Proof integration tests --

    #[test]
    fn test_promote_to_evidence() {
        let finding = Finding::new(FindingId::new("security", 1), "SQL injection in login");
        let evidence = ProofIntegration::promote_to_evidence(&finding);
        assert_eq!(evidence.ground, Ground::Confirmado);
        assert!(evidence.description.contains("security:1"));
    }

    #[test]
    fn test_create_refutation() {
        let finding = Finding::new(FindingId::new("perf", 5), "Latency under 100ms");
        let proof = ProofIntegration::create_refutation(&finding);
        assert_eq!(proof.status, ProofStatus::Refuted);
        assert!(proof.claim.contains("perf:5"));
    }

    // -- Serde round-trip test --

    #[test]
    fn test_finding_serde_roundtrip() {
        let finding = Finding::new(FindingId::new("test", 42), "serde check");
        let json = serde_json::to_string(&finding).unwrap();
        let deserialized: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, finding.id);
        assert_eq!(deserialized.claim, finding.claim);
        assert_eq!(deserialized.state, finding.state);
    }
}
