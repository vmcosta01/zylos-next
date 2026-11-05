//! BriefAwareRerank — Context-aware memory reranking using research briefs
//!
//! Implements Task 2.1.3 of Fase 2: PaperRadar-inspired brief-aware rerank
//! that filters and ranks memories by relevance to the current objective,
//! reducing noise by >40% compared to pure Jaccard similarity.
//!
//! Target: <100ms p95 for rerank of 1k memories (per EVALUATION.md)

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use crate::error::Result;
use crate::types::ContextNode;

/// Research brief representing the current objective/context for reranking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchBrief {
    /// Unique identifier for this brief
    pub id: String,
    /// Current objective or research question
    pub objective: String,
    /// Key concepts extracted from the objective (for keyword matching)
    pub key_concepts: Vec<String>,
    /// Optional embedding vector for semantic matching
    pub embedding: Option<Vec<f32>>,
    /// Tags that define the scope of interest
    pub tags: Vec<String>,
    /// Weight for semantic vs keyword matching (0.0 = pure keyword, 1.0 = pure semantic)
    pub semantic_weight: f64,
    /// Minimum relevance score to include in results (0.0 to 1.0)
    pub min_relevance: f64,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// TTL in days before brief needs refresh (default: 7)
    pub ttl_days: u32,
}

impl ResearchBrief {
    /// Create a new research brief with the given objective
    pub fn new(id: impl Into<String>, objective: impl Into<String>) -> Self {
        let objective_str = objective.into();
        // Auto-extract key concepts from objective (simple word extraction)
        let key_concepts = extract_key_concepts(&objective_str);

        Self {
            id: id.into(),
            objective: objective_str,
            key_concepts,
            embedding: None,
            tags: Vec::new(),
            semantic_weight: 0.6, // Default: 60% semantic, 40% keyword
            min_relevance: 0.3,
            updated_at: Utc::now(),
            ttl_days: 7,
        }
    }

    /// Set the embedding vector for semantic matching
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Add tags to the brief
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set the semantic weight (0.0 to 1.0)
    pub fn with_semantic_weight(mut self, weight: f64) -> Self {
        self.semantic_weight = weight.clamp(0.0, 1.0);
        self
    }

    /// Set minimum relevance threshold
    pub fn with_min_relevance(mut self, min: f64) -> Self {
        self.min_relevance = min.clamp(0.0, 1.0);
        self
    }

    /// Check if the brief needs refresh based on TTL
    pub fn needs_refresh(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.updated_at);
        age.num_days() >= self.ttl_days as i64
    }

    /// Update the brief's timestamp (call after manual refresh)
    pub fn mark_updated(&mut self) {
        self.updated_at = Utc::now();
    }
}

/// A ranked memory result with relevance score breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedMemory {
    /// The original context node
    pub node: ContextNode,
    /// Combined relevance score (0.0 to 1.0)
    pub relevance_score: f64,
    /// Semantic similarity component (0.0 to 1.0)
    pub semantic_score: f64,
    /// Keyword overlap component (0.0 to 1.0)
    pub keyword_score: f64,
    /// Tag match component (0.0 to 1.0)
    pub tag_score: f64,
    /// Recency boost factor (0.0 to 1.0, newer = higher)
    pub recency_factor: f64,
}

/// Configuration for the brief-aware reranker
#[derive(Debug, Clone)]
pub struct RerankConfig {
    /// Maximum number of results to return
    pub max_results: usize,
    /// Weight for recency in final score (0.0 to 1.0)
    pub recency_weight: f64,
    /// Half-life in days for recency decay (older memories lose relevance)
    pub recency_half_life_days: f64,
    /// Whether to apply scope filtering before reranking
    pub filter_by_scope: bool,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            max_results: 50,
            recency_weight: 0.15,
            recency_half_life_days: 30.0,
            filter_by_scope: true,
        }
    }
}

/// Brief-aware reranker that ranks memories by relevance to a research brief
pub struct BriefAwareReranker {
    config: RerankConfig,
}

impl BriefAwareReranker {
    pub fn new(config: RerankConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(RerankConfig::default())
    }

    /// Rerank a set of search results using the given research brief
    ///
    /// Returns memories sorted by relevance score (descending), filtered by
    /// the brief's minimum relevance threshold.
    pub fn rerank(
        &self,
        brief: &ResearchBrief,
        candidates: Vec<ContextNode>,
    ) -> Result<Vec<RankedMemory>> {
        let mut ranked: Vec<RankedMemory> = candidates
            .into_iter()
            .filter_map(|node| {
                // Filter by scope if configured
                if self.config.filter_by_scope && !self.scope_matches(brief, &node) {
                    return None;
                }

                let semantic_score = self.compute_semantic_score(brief, &node);
                let keyword_score = self.compute_keyword_score(brief, &node);
                let tag_score = self.compute_tag_score(brief, &node);
                let recency_factor = self.compute_recency_factor(&node);

                // Combine scores using brief's semantic weight
                let content_score = brief.semantic_weight * semantic_score
                    + (1.0 - brief.semantic_weight) * keyword_score;

                // Apply tag boost and recency
                let combined = content_score * (0.8 + 0.2 * tag_score)
                    * (1.0 - self.config.recency_weight
                        + self.config.recency_weight * recency_factor);

                // Filter by minimum relevance
                if combined < brief.min_relevance {
                    return None;
                }

                Some(RankedMemory {
                    node,
                    relevance_score: combined,
                    semantic_score,
                    keyword_score,
                    tag_score,
                    recency_factor,
                })
            })
            .collect();

        // Sort by relevance score descending
        ranked.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to max results
        ranked.truncate(self.config.max_results);

        Ok(ranked)
    }

    /// Compute semantic similarity between brief embedding and node embedding
    fn compute_semantic_score(&self, brief: &ResearchBrief, node: &ContextNode) -> f64 {
        match (&brief.embedding, &node.embedding) {
            (Some(brief_emb), Some(node_emb)) => {
                cosine_similarity(brief_emb, node_emb) as f64
            }
            _ => 0.0, // No embeddings available, fall back to keyword
        }
    }

    /// Compute keyword overlap score between brief concepts and node content
    fn compute_keyword_score(&self, brief: &ResearchBrief, node: &ContextNode) -> f64 {
        if brief.key_concepts.is_empty() {
            return 0.0;
        }

        let content_str = serde_json::to_string(&node.content).unwrap_or_default().to_lowercase();
        let matched = brief
            .key_concepts
            .iter()
            .filter(|concept| content_str.contains(&concept.to_lowercase()))
            .count();

        matched as f64 / brief.key_concepts.len() as f64
    }

    /// Compute tag overlap score
    fn compute_tag_score(&self, brief: &ResearchBrief, node: &ContextNode) -> f64 {
        if brief.tags.is_empty() || node.tags.is_empty() {
            return 0.5; // Neutral score when no tags to compare
        }

        let brief_tags: std::collections::HashSet<&str> =
            brief.tags.iter().map(|s| s.as_str()).collect();
        let node_tags: std::collections::HashSet<&str> =
            node.tags.iter().map(|s| s.as_str()).collect();

        let intersection = brief_tags.intersection(&node_tags).count();
        let union = brief_tags.union(&node_tags).count();

        if union == 0 {
            0.5
        } else {
            intersection as f64 / union as f64
        }
    }

    /// Compute recency factor using exponential decay
    fn compute_recency_factor(&self, node: &ContextNode) -> f64 {
        let created = match chrono::DateTime::parse_from_rfc3339(&node.created_at) {
            Ok(dt) => dt.with_timezone(&Utc),
            Err(_) => return 0.5, // Parse error, neutral score
        };

        let age_days = Utc::now().signed_duration_since(created).num_seconds().max(0) as f64
            / 86400.0;

        // Exponential decay: score = 2^(-age/half_life)
        2.0_f64.powf(-age_days / self.config.recency_half_life_days)
    }

    /// Check if node scope is compatible with the brief's context
    fn scope_matches(&self, _brief: &ResearchBrief, _node: &ContextNode) -> bool {
        // For now, all scopes match. Future: brief could specify allowed scopes.
        true
    }
}

/// Extract key concepts from text (simple heuristic: words > 3 chars, lowercase)
fn extract_key_concepts(text: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "the", "and", "for", "are", "but", "not", "you", "all", "can", "had",
        "her", "was", "one", "our", "out", "has", "have", "from", "this", "that",
        "with", "they", "been", "will", "each", "which", "their", "would", "there",
        "what", "about", "know", "time", "could", "than", "other", "were", "then",
        "into", "some", "when", "make", "like", "just", "over", "such", "take",
        "year", "only", "also", "more", "most", "home", "same", "back", "much",
        "well", "even", "here", "where", "after", "before", "between", "through",
        "during", "being", "under", "does", "did", "how", "any", "these", "those",
        "para", "com", "uma", "por", "não", "que", "dos", "das", "nos", "nas",
        "como", "mais", "mas", "seu", "sua", "aos", "ter", "ser", "quando", "muito",
        "ainda", "já", "está", "estão", "foi", "foram", "esse", "essa", "este",
        "esta", "pelo", "pela", "pelos", "pelas", "num", "numa", "neste", "nesta",
    ]
    .iter()
    .copied()
    .collect();

    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.trim().to_lowercase())
        .filter(|w| w.len() > 3 && !stop_words.contains(w.as_str()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

use crate::utils::cosine_similarity;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContextNode;
    use crate::MemoryScope;

    fn make_node(content: &str, tags: Vec<&str>, days_ago: i64) -> ContextNode {
        let created = Utc::now() - chrono::Duration::days(days_ago);
        let mut node = ContextNode::new(
            serde_json::json!({"text": content}),
            MemoryScope::Agent,
            "agent-1",
        );
        node.tags = tags.into_iter().map(|s| s.to_string()).collect();
        node.created_at = created.to_rfc3339();
        node
    }

    #[test]
    fn test_extract_key_concepts() {
        let concepts = extract_key_concepts("Machine learning algorithms for natural language processing");
        assert!(concepts.contains(&"machine".to_string()));
        assert!(concepts.contains(&"learning".to_string()));
        assert!(concepts.contains(&"algorithms".to_string()));
        assert!(concepts.contains(&"natural".to_string()));
        assert!(concepts.contains(&"language".to_string()));
        assert!(concepts.contains(&"processing".to_string()));
        // Stop words should be excluded
        assert!(!concepts.contains(&"for".to_string()));
    }

    #[test]
    fn test_research_brief_creation() {
        let brief = ResearchBrief::new("b1", "Build a Rust memory system for AI agents");
        assert_eq!(brief.id, "b1");
        assert!(!brief.key_concepts.is_empty());
        assert!(brief.key_concepts.contains(&"rust".to_string()));
        assert!(brief.key_concepts.contains(&"memory".to_string()));
        assert!(brief.key_concepts.contains(&"agents".to_string()));
        assert!(!brief.needs_refresh());
    }

    #[test]
    fn test_keyword_scoring() {
        let reranker = BriefAwareReranker::with_default_config();
        let brief = ResearchBrief::new("b1", "Rust async runtime performance");
        let node = make_node("Tokio provides an async runtime for Rust applications", vec![], 0);

        let score = reranker.compute_keyword_score(&brief, &node);
        assert!(score > 0.0, "Should have some keyword overlap");
    }

    #[test]
    fn test_tag_scoring() {
        let reranker = BriefAwareReranker::with_default_config();
        let brief = ResearchBrief::new("b1", "test").with_tags(vec!["rust".to_string(), "async".to_string()]);

        let node_match = make_node("content", vec!["rust", "async"], 0);
        let node_partial = make_node("content", vec!["rust"], 0);
        let node_none = make_node("content", vec!["python"], 0);

        let score_match = reranker.compute_tag_score(&brief, &node_match);
        let score_partial = reranker.compute_tag_score(&brief, &node_partial);
        let score_none = reranker.compute_tag_score(&brief, &node_none);

        assert!(score_match > score_partial);
        assert!(score_partial > score_none);
    }

    #[test]
    fn test_recency_decay() {
        let reranker = BriefAwareReranker::new(RerankConfig {
            recency_half_life_days: 30.0,
            ..Default::default()
        });

        let recent = make_node("recent", vec![], 1);
        let old = make_node("old", vec![], 90);

        let score_recent = reranker.compute_recency_factor(&recent);
        let score_old = reranker.compute_recency_factor(&old);

        assert!(score_recent > score_old, "Recent nodes should score higher");
        assert!(score_recent > 0.9, "Very recent should be near 1.0");
        assert!(score_old < 0.2, "90 days old with 30-day half-life should be low");
    }

    #[tokio::test]
        async fn test_rerank_filters_below_threshold() {
            let reranker = BriefAwareReranker::with_default_config();
            let brief = ResearchBrief::new("b1", "Rust memory systems ownership borrowing")
                .with_tags(vec!["rust".to_string(), "memory".to_string()])
                .with_min_relevance(0.3);

            let candidates = vec![
                make_node("Rust memory management with ownership and borrowing rules", vec!["rust", "memory"], 0),
                make_node("Python garbage collection basics explained", vec!["python"], 0),
                make_node("Completely unrelated cooking recipe for pasta", vec!["food"], 0),
            ];

            let results = reranker.rerank(&brief, candidates).unwrap();
            // At least the Rust-related one should pass; unrelated ones should be filtered
            assert!(!results.is_empty(), "Expected at least one result above threshold");
            for r in &results {
                assert!(r.relevance_score >= 0.3, "Score {} below min_relevance 0.3", r.relevance_score);
            }
            // The Rust node should rank first
            let first_text = serde_json::to_string(&results[0].node.content).unwrap_or_default();
            assert!(first_text.contains("Rust") || first_text.contains("ownership"));
        }

    #[tokio::test]
    async fn test_rerank_sorts_by_relevance() {
        let reranker = BriefAwareReranker::with_default_config();
        let brief = ResearchBrief::new("b1", "Rust async performance optimization")
            .with_min_relevance(0.0); // Accept all

        let candidates = vec![
            make_node("Cooking pasta al dente", vec!["food"], 0),
            make_node("Rust async runtime tokio performance tuning", vec!["rust", "async"], 0),
            make_node("General programming tips", vec![], 0),
        ];

        let results = reranker.rerank(&brief, candidates).unwrap();

        assert!(results.len() >= 2);
        // First result should be the most relevant (Rust async)
        let first_text = serde_json::to_string(&results[0].node.content).unwrap_or_default();
        assert!(first_text.contains("Rust") || first_text.contains("rust"));
    }

    #[tokio::test]
    async fn test_rerank_respects_max_results() {
        let reranker = BriefAwareReranker::new(RerankConfig {
            max_results: 3,
            ..Default::default()
        });
        let brief = ResearchBrief::new("b1", "test").with_min_relevance(0.0);

        let candidates: Vec<ContextNode> = (0..10)
            .map(|i| make_node(&format!("Content item {}", i), vec![], 0))
            .collect();

        let results = reranker.rerank(&brief, candidates).unwrap();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_semantic_scoring_with_embeddings() {
        let reranker = BriefAwareReranker::with_default_config();
        let brief = ResearchBrief::new("b1", "test")
            .with_embedding(vec![1.0, 0.0, 0.0]);

        let mut node_similar = make_node("similar", vec![], 0);
        node_similar.embedding = Some(vec![0.9, 0.1, 0.0]);

        let mut node_orthogonal = make_node("orthogonal", vec![], 0);
        node_orthogonal.embedding = Some(vec![0.0, 1.0, 0.0]);

        let score_similar = reranker.compute_semantic_score(&brief, &node_similar);
        let score_ortho = reranker.compute_semantic_score(&brief, &node_orthogonal);

        assert!(score_similar > 0.8);
        assert!(score_ortho < 0.1);
    }

    #[test]
    fn test_brief_needs_refresh() {
        let mut brief = ResearchBrief::new("b1", "test");
        brief.ttl_days = 7;
        assert!(!brief.needs_refresh());

        // Simulate old brief
        brief.updated_at = Utc::now() - chrono::Duration::days(10);
        assert!(brief.needs_refresh());
    }

    #[test]
    fn test_cosine_similarity_edge_cases() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0); // Different lengths
        assert!((cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 0.001);
    }
}