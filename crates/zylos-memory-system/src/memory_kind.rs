//! Memory kind classification, provenance tracking, backend routing,
//! compression pipeline, and evidence capsules for the WriteGate.
//!
//! Each [`MemoryKind`] maps to a specific storage backend via [`route_backend`],
//! enforcing compile-time separation of concerns across the memory system.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// MemoryKind
// ---------------------------------------------------------------------------

/// Classification of a memory entry that determines its storage backend
/// and lifecycle semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryKind {
    /// Short-lived context window data (in-memory only)
    Working,
    /// Time-stamped experiential records → SQLite context_graph
    Episodic,
    /// Embedding-indexed knowledge → vector store
    Semantic,
    /// Skill / procedure definitions → SKILL.md files on disk
    Procedural,
    /// Reasoning traces and meta-cognition → reasoning bank
    Meta,
}

impl fmt::Display for MemoryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Working => write!(f, "working"),
            Self::Episodic => write!(f, "episodic"),
            Self::Semantic => write!(f, "semantic"),
            Self::Procedural => write!(f, "procedural"),
            Self::Meta => write!(f, "meta"),
        }
    }
}

impl FromStr for MemoryKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "working" => Ok(Self::Working),
            "episodic" => Ok(Self::Episodic),
            "semantic" => Ok(Self::Semantic),
            "procedural" => Ok(Self::Procedural),
            "meta" => Ok(Self::Meta),
            other => Err(format!("Invalid MemoryKind: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// BackendTarget
// ---------------------------------------------------------------------------

/// Storage backend selected by [`route_backend`] for a given [`MemoryKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendTarget {
    /// In-process context window (no persistence)
    ContextWindow,
    /// SQLite-backed context graph
    SqliteContextGraph,
    /// Vector similarity store
    VectorStore,
    /// SKILL.md files on the filesystem
    SkillFiles,
    /// Structured reasoning bank
    ReasoningBank,
}

impl fmt::Display for BackendTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContextWindow => write!(f, "context_window"),
            Self::SqliteContextGraph => write!(f, "sqlite_context_graph"),
            Self::VectorStore => write!(f, "vector_store"),
            Self::SkillFiles => write!(f, "skill_files"),
            Self::ReasoningBank => write!(f, "reasoning_bank"),
        }
    }
}

/// Pure function: maps a [`MemoryKind`] to its designated [`BackendTarget`].
pub fn route_backend(kind: MemoryKind) -> BackendTarget {
    match kind {
        MemoryKind::Working => BackendTarget::ContextWindow,
        MemoryKind::Episodic => BackendTarget::SqliteContextGraph,
        MemoryKind::Semantic => BackendTarget::VectorStore,
        MemoryKind::Procedural => BackendTarget::SkillFiles,
        MemoryKind::Meta => BackendTarget::ReasoningBank,
    }
}

// ---------------------------------------------------------------------------
// ProvenanceTriple
// ---------------------------------------------------------------------------

/// Scope dimension of a provenance triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProvenanceScope {
    Session,
    User,
    Agent,
    Global,
}

impl fmt::Display for ProvenanceScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session => write!(f, "session"),
            Self::User => write!(f, "user"),
            Self::Agent => write!(f, "agent"),
            Self::Global => write!(f, "global"),
        }
    }
}

/// Origin of a memory entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ProvenanceOrigin {
    /// Produced by a tool call (carries the tool_call_id)
    ToolCall(String),
    /// Direct human input
    HumanInput,
    /// Model inference / synthesis
    Inference,
}

impl fmt::Display for ProvenanceOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ToolCall(id) => write!(f, "tool_call({})", id),
            Self::HumanInput => write!(f, "human_input"),
            Self::Inference => write!(f, "inference"),
        }
    }
}

/// Completeness / verification status of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Completeness {
    Partial,
    Verified,
    Contradicted,
}

impl fmt::Display for Completeness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Partial => write!(f, "partial"),
            Self::Verified => write!(f, "verified"),
            Self::Contradicted => write!(f, "contradicted"),
        }
    }
}

/// Mandatory provenance triple attached to every memory write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceTriple {
    pub scope: ProvenanceScope,
    pub origin: ProvenanceOrigin,
    pub completeness: Completeness,
}

impl ProvenanceTriple {
    pub fn new(
        scope: ProvenanceScope,
        origin: ProvenanceOrigin,
        completeness: Completeness,
    ) -> Self {
        Self {
            scope,
            origin,
            completeness,
        }
    }
}

impl fmt::Display for ProvenanceTriple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{scope: {}, origin: {}, completeness: {}}}",
            self.scope, self.origin, self.completeness
        )
    }
}

// ---------------------------------------------------------------------------
// CompressionPipeline (pure functions)
// ---------------------------------------------------------------------------

/// A single extracted fact from raw memory content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
}

/// An entity link produced by the entity-linking stage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityLink {
    pub mention: String,
    pub canonical_id: String,
    pub score: f64,
}

/// Decay score for a memory entry, used to schedule garbage collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecayScore {
    pub entry_id: String,
    pub score: f64,
    pub reason: String,
}

/// GC action scheduled by the compression pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GcAction {
    pub entry_id: String,
    pub action: GcDisposition,
    pub priority: u32,
}

/// What to do with an entry during garbage collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GcDisposition {
    Retain,
    Compress,
    Evict,
}

/// Stateless compression pipeline exposed as pure functions.
///
/// Each stage takes immutable input and returns owned output so the
/// pipeline can be tested, composed, and parallelised freely.
pub struct CompressionPipeline;

impl CompressionPipeline {
    /// Stage 1 — extract structured facts from raw text.
    ///
    /// This is a placeholder implementation; production code will delegate
    /// to an LLM or NER model. The signature is stable.
    pub fn extract_facts(raw_text: &str) -> Vec<ExtractedFact> {
        // Minimal heuristic: split on '.' and treat each sentence as a fact
        raw_text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, line)| ExtractedFact {
                subject: format!("sentence_{}", i),
                predicate: "contains".to_string(),
                object: line.trim().to_string(),
                confidence: 0.5,
            })
            .collect()
    }

    /// Stage 2 — resolve mentions to canonical entity IDs.
    pub fn link_entities(facts: &[ExtractedFact]) -> Vec<EntityLink> {
        facts
            .iter()
            .map(|f| EntityLink {
                mention: f.subject.clone(),
                canonical_id: format!("entity::{}", f.subject),
                score: f.confidence,
            })
            .collect()
    }

    /// Stage 3 — compute a decay score for each entry.
    ///
    /// Lower scores indicate candidates for eviction.
    pub fn score_decay(entry_ids: &[String], access_counts: &[u64]) -> Vec<DecayScore> {
        entry_ids
            .iter()
            .zip(access_counts.iter())
            .map(|(id, count)| {
                let score = 1.0 / (1.0 + *count as f64);
                DecayScore {
                    entry_id: id.clone(),
                    score,
                    reason: format!("access_count={}", count),
                }
            })
            .collect()
    }

    /// Stage 4 — decide GC disposition based on decay scores.
    pub fn schedule_gc(scores: &[DecayScore], evict_threshold: f64, compress_threshold: f64) -> Vec<GcAction> {
        scores
            .iter()
            .enumerate()
            .map(|(i, ds)| {
                let action = if ds.score <= evict_threshold {
                    GcDisposition::Evict
                } else if ds.score <= compress_threshold {
                    GcDisposition::Compress
                } else {
                    GcDisposition::Retain
                };
                GcAction {
                    entry_id: ds.entry_id.clone(),
                    action,
                    priority: i as u32,
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// EvidenceCapsule (WriteGate artifact)
// ---------------------------------------------------------------------------

/// Disposition decided by the WriteGate after evaluating evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceDisposition {
    Accept,
    Reject,
    Defer,
    ContradictsExisting,
}

impl fmt::Display for EvidenceDisposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accept => write!(f, "accept"),
            Self::Reject => write!(f, "reject"),
            Self::Defer => write!(f, "defer"),
            Self::ContradictsExisting => write!(f, "contradicts_existing"),
        }
    }
}

/// Immutable capsule produced by the WriteGate for audit and replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCapsule {
    /// The proposed finding or claim
    pub finding: String,
    /// Supporting evidence (provenance, citations, tool outputs)
    pub evidence: Vec<String>,
    /// Gate decision rationale
    pub decision: String,
    /// Final disposition
    pub disposition: EvidenceDisposition,
}

impl EvidenceCapsule {
    pub fn new(
        finding: impl Into<String>,
        evidence: Vec<String>,
        decision: impl Into<String>,
        disposition: EvidenceDisposition,
    ) -> Self {
        Self {
            finding: finding.into(),
            evidence,
            decision: decision.into(),
            disposition,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_kind_display_roundtrip() {
        let kinds = [
            MemoryKind::Working,
            MemoryKind::Episodic,
            MemoryKind::Semantic,
            MemoryKind::Procedural,
            MemoryKind::Meta,
        ];
        for k in kinds {
            let s = k.to_string();
            let parsed: MemoryKind = s.parse().expect("parse failed");
            assert_eq!(k, parsed);
        }
    }

    #[test]
    fn memory_kind_from_str_invalid() {
        assert!(MemoryKind::from_str("unknown").is_err());
    }

    #[test]
    fn memory_kind_serde_roundtrip() {
        let k = MemoryKind::Episodic;
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, "\"episodic\"");
        let back: MemoryKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, k);
    }

    #[test]
    fn route_backend_mapping() {
        assert_eq!(route_backend(MemoryKind::Working), BackendTarget::ContextWindow);
        assert_eq!(route_backend(MemoryKind::Episodic), BackendTarget::SqliteContextGraph);
        assert_eq!(route_backend(MemoryKind::Semantic), BackendTarget::VectorStore);
        assert_eq!(route_backend(MemoryKind::Procedural), BackendTarget::SkillFiles);
        assert_eq!(route_backend(MemoryKind::Meta), BackendTarget::ReasoningBank);
    }

    #[test]
    fn provenance_triple_display() {
        let pt = ProvenanceTriple::new(
            ProvenanceScope::Agent,
            ProvenanceOrigin::ToolCall("tc-42".into()),
            Completeness::Verified,
        );
        let s = pt.to_string();
        assert!(s.contains("agent"));
        assert!(s.contains("tc-42"));
        assert!(s.contains("verified"));
    }

    #[test]
    fn provenance_origin_serde() {
        let o = ProvenanceOrigin::HumanInput;
        let json = serde_json::to_string(&o).unwrap();
        let back: ProvenanceOrigin = serde_json::from_str(&json).unwrap();
        assert_eq!(back, o);
    }

    #[test]
    fn compression_extract_facts_nonempty() {
        let text = "Line one.\nLine two.\n";
        let facts = CompressionPipeline::extract_facts(text);
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].subject, "sentence_0");
    }

    #[test]
    fn compression_link_entities_matches_facts() {
        let facts = vec![
            ExtractedFact {
                subject: "alice".into(),
                predicate: "knows".into(),
                object: "bob".into(),
                confidence: 0.9,
            },
        ];
        let links = CompressionPipeline::link_entities(&facts);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].mention, "alice");
        assert_eq!(links[0].canonical_id, "entity::alice");
    }

    #[test]
    fn compression_score_decay_ordering() {
        let ids = vec!["a".into(), "b".into()];
        let counts = vec![10, 0];
        let scores = CompressionPipeline::score_decay(&ids, &counts);
        // Higher access → lower decay score → more likely retained
        assert!(scores[0].score < scores[1].score);
    }

    #[test]
    fn compression_schedule_gc_thresholds() {
        let scores = vec![
            DecayScore { entry_id: "low".into(), score: 0.1, reason: "r".into() },
            DecayScore { entry_id: "mid".into(), score: 0.5, reason: "r".into() },
            DecayScore { entry_id: "high".into(), score: 0.9, reason: "r".into() },
        ];
        let actions = CompressionPipeline::schedule_gc(&scores, 0.2, 0.6);
        assert_eq!(actions[0].action, GcDisposition::Evict);
        assert_eq!(actions[1].action, GcDisposition::Compress);
        assert_eq!(actions[2].action, GcDisposition::Retain);
    }

    #[test]
    fn evidence_capsule_construction() {
        let cap = EvidenceCapsule::new(
            "X causes Y",
            vec!["exp-001".into(), "paper-42".into()],
            "strong causal evidence",
            EvidenceDisposition::Accept,
        );
        assert_eq!(cap.disposition, EvidenceDisposition::Accept);
        assert_eq!(cap.evidence.len(), 2);
    }

    #[test]
    fn evidence_disposition_serde_roundtrip() {
        let d = EvidenceDisposition::ContradictsExisting;
        let json = serde_json::to_string(&d).unwrap();
        let back: EvidenceDisposition = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }
}