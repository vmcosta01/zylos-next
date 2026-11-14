//! Contradiction detection and resolution for the MEMORY_SYSTEM
//!
//! Detects direct (A ∧ ¬A), implicit (semantic inference), and temporal
//! contradictions between context nodes, then resolves them using a
//! political-decay weighting scheme that favours recency, source trust,
//! and verification status.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{ContextNode, NodeId};
use crate::utils::cosine_similarity;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The kind of contradiction detected between two nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContradictionType {
    /// Explicit logical negation (A and not-A).
    Direct,
    /// Semantic / inferential conflict detected via embedding similarity.
    Implicit,
    /// Same claim with different truth values across time.
    Temporal,
}

/// How a contradiction was resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Resolution {
    KeepNewer,
    KeepHigherConfidence,
    MergeDecay,
    ManualReview,
}

/// A detected contradiction between two context nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    pub kind: ContradictionType,
    pub node_a: NodeId,
    pub node_b: NodeId,
    pub confidence: f64,
    pub detected_at: DateTime<Utc>,
    pub resolution: Option<Resolution>,
}

// ---------------------------------------------------------------------------
// Detector
// ---------------------------------------------------------------------------

/// Stateless contradiction detector.
///
/// All public methods are pure functions over `ContextNode` references so
/// the detector can be shared across threads without synchronisation.
pub struct ContradictionDetector;

impl ContradictionDetector {
    // -- individual detectors ------------------------------------------------

    /// Detect a *direct* contradiction: both nodes carry a boolean `value`
    /// field under the same `claim_key` and disagree.
    pub fn detect_direct(a: &ContextNode, b: &ContextNode) -> Option<Contradiction> {
        let key_a = a.content.get("claim_key")?.as_str()?;
        let key_b = b.content.get("claim_key")?.as_str()?;

        if key_a != key_b {
            return None;
        }

        let val_a = a.content.get("value")?.as_bool()?;
        let val_b = b.content.get("value")?.as_bool()?;

        if val_a == val_b {
            return None;
        }

        Some(Contradiction {
            kind: ContradictionType::Direct,
            node_a: a.id,
            node_b: b.id,
            confidence: 1.0,
            detected_at: Utc::now(),
            resolution: None,
        })
    }

    /// Detect an *implicit* contradiction via embedding similarity.
    ///
    /// Two nodes whose embeddings are highly similar (`>= threshold`) but
    /// whose `value` booleans disagree are flagged as implicitly
    /// contradictory.
    pub fn detect_implicit(
        a: &ContextNode,
        b: &ContextNode,
        threshold: f64,
    ) -> Option<Contradiction> {
        let emb_a = a.embedding.as_ref()?;
        let emb_b = b.embedding.as_ref()?;

        let sim = cosine_similarity(emb_a, emb_b) as f64;
        if sim < threshold {
            return None;
        }

        // Need opposing boolean signals to qualify as contradiction.
        let val_a = a.content.get("value").and_then(|v| v.as_bool());
        let val_b = b.content.get("value").and_then(|v| v.as_bool());

        match (val_a, val_b) {
            (Some(va), Some(vb)) if va != vb => {}
            _ => return None,
        }

        Some(Contradiction {
            kind: ContradictionType::Implicit,
            node_a: a.id,
            node_b: b.id,
            confidence: sim.clamp(0.0, 1.0),
            detected_at: Utc::now(),
            resolution: None,
        })
    }

    /// Detect a *temporal* contradiction: same `claim_key`, different
    /// `value`, and distinct timestamps.
    pub fn detect_temporal(a: &ContextNode, b: &ContextNode) -> Option<Contradiction> {
        let key_a = a.content.get("claim_key")?.as_str()?;
        let key_b = b.content.get("claim_key")?.as_str()?;

        if key_a != key_b {
            return None;
        }

        let val_a = a.content.get("value")?.as_bool()?;
        let val_b = b.content.get("value")?.as_bool()?;

        if val_a == val_b {
            return None;
        }

        // Must have distinct timestamps to be *temporal*.
        if a.created_at == b.created_at {
            return None;
        }

        Some(Contradiction {
            kind: ContradictionType::Temporal,
            node_a: a.id,
            node_b: b.id,
            confidence: 0.95,
            detected_at: Utc::now(),
            resolution: None,
        })
    }

    // -- resolution ----------------------------------------------------------

    /// Resolve a contradiction using political-decay weighting.
    ///
    /// Weight formula per node:
    /// ```text
    /// w = recency_factor × source_trust × verification_bonus
    /// ```
    ///
    /// * `recency_factor` — exponential decay from node age (half-life ≈ 30 d).
    /// * `source_trust`   — read from `metadata.source_trust` (default 0.5).
    /// * `verification_bonus` — 1.5 when `metadata.verified == true`, else 1.0.
    pub fn resolve(contradiction: &mut Contradiction) -> Resolution {
        // We cannot access the original nodes here without changing the API,
        // so we apply a simple heuristic based on the contradiction kind.
        // Callers that need full decay should use `resolve_with_nodes`.
        let resolution = match contradiction.kind {
            ContradictionType::Temporal => Resolution::KeepNewer,
            ContradictionType::Direct => Resolution::KeepHigherConfidence,
            ContradictionType::Implicit => {
                if contradiction.confidence >= 0.95 {
                    Resolution::MergeDecay
                } else {
                    Resolution::ManualReview
                }
            }
        };
        contradiction.resolution = Some(resolution);
        resolution
    }

    /// Full political-decay resolution given access to both nodes.
    pub fn resolve_with_nodes(
        contradiction: &mut Contradiction,
        a: &ContextNode,
        b: &ContextNode,
    ) -> Resolution {
        // Temporal contradictions always prefer the newer node regardless of
        // political weight — recency *is* the signal for this kind.
        if contradiction.kind == ContradictionType::Temporal {
            let resolution = if a.created_at == b.created_at {
                Resolution::ManualReview
            } else if b.created_at > a.created_at {
                Resolution::KeepNewer
            } else {
                // a is newer; swap semantics so "keep newer" still applies.
                Resolution::KeepNewer
            };
            contradiction.resolution = Some(resolution);
            return resolution;
        }

        let wa = Self::political_weight(a);
        let wb = Self::political_weight(b);

        let resolution = if (wa - wb).abs() < 0.01 {
            Resolution::ManualReview
        } else {
            Resolution::KeepHigherConfidence
        };

        contradiction.resolution = Some(resolution);
        resolution
    }

    // -- batch detection -----------------------------------------------------

    /// Run all three detectors over every unique pair in `nodes`.
    ///
    /// O(n²) in the number of nodes; intended for bounded working sets
    /// (e.g. the result of a neighbourhood query), not the entire graph.
    pub fn detect_all(nodes: &[ContextNode]) -> Vec<Contradiction> {
        let mut results = Vec::new();
        let implicit_threshold = 0.85_f64;

        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let a = &nodes[i];
                let b = &nodes[j];

                if let Some(mut c) = Self::detect_direct(a, b) {
                    Self::resolve_with_nodes(&mut c, a, b);
                    results.push(c);
                    continue; // direct supersedes other kinds for this pair
                }

                if let Some(mut c) = Self::detect_temporal(a, b) {
                    Self::resolve_with_nodes(&mut c, a, b);
                    results.push(c);
                    continue;
                }

                if let Some(mut c) = Self::detect_implicit(a, b, implicit_threshold) {
                    Self::resolve_with_nodes(&mut c, a, b);
                    results.push(c);
                }
            }
        }

        results
    }

    // -- helpers -------------------------------------------------------------

    /// Political-decay weight for a single node.
    fn political_weight(node: &ContextNode) -> f64 {
        let recency = Self::recency_factor(&node.created_at);
        let trust = node
            .content
            .get("metadata")
            .and_then(|m| m.get("source_trust"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let verified = node
            .content
            .get("metadata")
            .and_then(|m| m.get("verified"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let verification_bonus: f64 = if verified { 1.5 } else { 1.0 };

        recency * trust * verification_bonus
    }

    /// Exponential-decay recency factor with a 30-day half-life.
    fn recency_factor(created_at: &str) -> f64 {
        let Ok(ts) = DateTime::parse_from_rfc3339(created_at) else {
            return 0.5; // fallback for unparseable timestamps
        };
        let age_days = (Utc::now() - ts.with_timezone(&Utc)).num_seconds().max(0) as f64 / 86400.0;
        // half-life = 30 days → λ = ln(2)/30
        const LAMBDA: f64 = 0.023104906; // ln(2) / 30
        (-LAMBDA * age_days).exp()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MemoryScope;
    use serde_json::json;

    fn make_node(claim_key: &str, value: bool, created_at: &str) -> ContextNode {
        let content = json!({
            "claim_key": claim_key,
            "value": value,
            "metadata": {
                "source_trust": 0.8,
                "verified": false
            }
        });
        let mut node = ContextNode::new(content, MemoryScope::Agent, "test-agent");
        node.created_at = created_at.to_string();
        node
    }

    fn make_node_with_embedding(claim_key: &str, value: bool, embedding: Vec<f32>) -> ContextNode {
        let content = json!({
            "claim_key": claim_key,
            "value": value,
        });
        let mut node = ContextNode::new(content, MemoryScope::Agent, "test-agent");
        node.embedding = Some(embedding);
        node
    }

    #[test]
    fn test_detect_direct_opposing() {
        let a = make_node("sky_blue", true, "2025-01-01T00:00:00Z");
        let b = make_node("sky_blue", false, "2025-01-02T00:00:00Z");
        let c = ContradictionDetector::detect_direct(&a, &b);
        assert!(c.is_some());
        let c = c.unwrap();
        assert_eq!(c.kind, ContradictionType::Direct);
        assert!((c.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_detect_direct_same_value_none() {
        let a = make_node("sky_blue", true, "2025-01-01T00:00:00Z");
        let b = make_node("sky_blue", true, "2025-01-02T00:00:00Z");
        assert!(ContradictionDetector::detect_direct(&a, &b).is_none());
    }

    #[test]
    fn test_detect_direct_different_keys_none() {
        let a = make_node("sky_blue", true, "2025-01-01T00:00:00Z");
        let b = make_node("grass_green", false, "2025-01-02T00:00:00Z");
        assert!(ContradictionDetector::detect_direct(&a, &b).is_none());
    }

    #[test]
    fn test_detect_temporal() {
        let a = make_node("policy_x", true, "2025-01-01T00:00:00Z");
        let b = make_node("policy_x", false, "2025-06-01T00:00:00Z");
        let c = ContradictionDetector::detect_temporal(&a, &b);
        assert!(c.is_some());
        assert_eq!(c.unwrap().kind, ContradictionType::Temporal);
    }

    #[test]
    fn test_detect_temporal_same_timestamp_none() {
        let ts = "2025-01-01T00:00:00Z";
        let a = make_node("policy_x", true, ts);
        let b = make_node("policy_x", false, ts);
        assert!(ContradictionDetector::detect_temporal(&a, &b).is_none());
    }

    #[test]
    fn test_detect_implicit_above_threshold() {
        let emb = vec![1.0, 0.0, 0.0];
        let a = make_node_with_embedding("q1", true, emb.clone());
        let b = make_node_with_embedding("q2", false, emb);
        let c = ContradictionDetector::detect_implicit(&a, &b, 0.85);
        assert!(c.is_some());
        assert_eq!(c.unwrap().kind, ContradictionType::Implicit);
    }

    #[test]
    fn test_detect_implicit_below_threshold_none() {
        let a = make_node_with_embedding("q1", true, vec![1.0, 0.0, 0.0]);
        let b = make_node_with_embedding("q2", false, vec![0.0, 1.0, 0.0]);
        assert!(ContradictionDetector::detect_implicit(&a, &b, 0.85).is_none());
    }

    #[test]
    fn test_resolve_temporal_keeps_newer() {
        let a = make_node("k", true, "2025-01-01T00:00:00Z");
        let b = make_node("k", false, "2025-06-01T00:00:00Z");
        let mut c = ContradictionDetector::detect_temporal(&a, &b).unwrap();
        let r = ContradictionDetector::resolve_with_nodes(&mut c, &a, &b);
        assert_eq!(r, Resolution::KeepNewer);
    }

    #[test]
    fn test_detect_all_finds_multiple() {
        let nodes = vec![
            make_node("a", true, "2025-01-01T00:00:00Z"),
            make_node("a", false, "2025-01-02T00:00:00Z"),
            make_node("b", true, "2025-03-01T00:00:00Z"),
            make_node("b", false, "2025-06-01T00:00:00Z"),
        ];
        let contradictions = ContradictionDetector::detect_all(&nodes);
        // pairs (0,1) and (2,3) each produce at least one contradiction
        assert!(
            contradictions.len() >= 2,
            "expected ≥2 contradictions, got {}",
            contradictions.len()
        );
    }

    #[test]
    fn test_political_weight_verified_higher() {
        let a = make_node("k", true, "2025-06-01T00:00:00Z");
        let mut b = make_node("k", false, "2025-06-01T00:00:00Z");
        // Mark b as verified
        if let Some(obj) = b.content.as_object_mut() {
            if let Some(meta) = obj.get_mut("metadata").and_then(|m| m.as_object_mut()) {
                meta.insert("verified".into(), json!(true));
            }
        }
        let wa = ContradictionDetector::political_weight(&a);
        let wb = ContradictionDetector::political_weight(&b);
        assert!(wb > wa, "verified node should weigh more");
    }
}
