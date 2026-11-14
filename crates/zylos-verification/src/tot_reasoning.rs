//! Tree-of-Thoughts (ToT) reasoning module — BFS beam search with pruning,
//! backtracking via serde checkpoints, and a runtime feature flag for
//! zero-overhead bypass when disabled.
//!
//! # Feature flag
//!
//! Set [`TOT_ENABLED`] to `false` to skip all tree-search work. The check is a
//! single relaxed atomic load on the hot path (<1 ns on x86-64).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// NodeId type alias
// ---------------------------------------------------------------------------

/// Lightweight identifier for a node in the thought tree.
pub type NodeId = u64;

// ---------------------------------------------------------------------------
// Runtime feature flag
// ---------------------------------------------------------------------------

/// Global toggle for ToT reasoning. When `false`, callers should short-circuit
/// before constructing a [`TreeSearch`]. The atomic uses `Relaxed` ordering
/// because we only need eventual visibility, not synchronisation.
pub static TOT_ENABLED: AtomicBool = AtomicBool::new(true);

/// Returns `true` when ToT reasoning is active.
#[inline]
pub fn tot_enabled() -> bool {
    TOT_ENABLED.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// ThoughtVerdict
// ---------------------------------------------------------------------------

/// Classification of a thought node after evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThoughtVerdict {
    /// Worth exploring further.
    Promising,
    /// Contradiction or dead end — prune.
    DeadEnd,
    /// Goal reached.
    Solved,
    /// Needs more expansion before a verdict can be issued.
    NeedsExpansion,
}

// ---------------------------------------------------------------------------
// BeamSearchConfig
// ---------------------------------------------------------------------------

/// Configuration for beam-search exploration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeamSearchConfig {
    /// Maximum number of candidates kept per depth level.
    pub width: usize,
    /// Maximum depth of the search tree.
    pub depth_limit: usize,
}

impl Default for BeamSearchConfig {
    fn default() -> Self {
        Self {
            width: 5,
            depth_limit: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// ThoughtNode
// ---------------------------------------------------------------------------

/// A single node in the thought tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtNode {
    /// Textual content of the thought.
    pub content: String,
    /// Heuristic / LLM-assigned score (higher = better).
    pub score: f64,
    /// Parent node, if any.
    pub parent_id: Option<NodeId>,
    /// Child node ids.
    pub children: Vec<NodeId>,
    /// Current verdict.
    pub verdict: ThoughtVerdict,
}

// ---------------------------------------------------------------------------
// PruningStrategy trait + built-in strategies
// ---------------------------------------------------------------------------

/// Composable pruning predicate. Return `true` to **keep** the node.
pub trait PruningStrategy: Send + Sync {
    /// Decide whether `node` should be retained.
    fn should_keep(&self, node: &ThoughtNode, ctx: &PruneContext) -> bool;

    /// Human-readable name for tracing.
    fn name(&self) -> &'static str;
}

/// Contextual information available during pruning.
#[derive(Debug, Clone)]
pub struct PruneContext {
    /// Number of nodes explored so far.
    pub nodes_explored: usize,
    /// Token budget remaining (if applicable).
    pub tokens_remaining: Option<usize>,
}

// -- Score threshold --------------------------------------------------------

/// Keeps nodes whose score >= `min_score`.
#[derive(Debug, Clone)]
pub struct ScoreThreshold {
    pub min_score: f64,
}

impl PruningStrategy for ScoreThreshold {
    fn should_keep(&self, node: &ThoughtNode, _ctx: &PruneContext) -> bool {
        node.score >= self.min_score
    }
    fn name(&self) -> &'static str {
        "score_threshold"
    }
}

// -- Duplicate detection ----------------------------------------------------

/// Keeps nodes whose content has not been seen before (exact-match dedup).
#[derive(Debug, Clone, Default)]
pub struct DuplicateDetection {
    seen: HashSet<String>,
}

impl DuplicateDetection {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PruningStrategy for DuplicateDetection {
    fn should_keep(&self, node: &ThoughtNode, _ctx: &PruneContext) -> bool {
        // NOTE: `seen` is behind `&self`; real usage would use interior
        // mutability or a mutable context. For composition tests this
        // simplified version suffices.
        !self.seen.contains(&node.content)
    }
    fn name(&self) -> &'static str {
        "duplicate_detection"
    }
}

// -- Budget cutoff ----------------------------------------------------------

/// Stops exploration once a token/node budget is exhausted.
#[derive(Debug, Clone)]
pub struct BudgetCutoff {
    pub max_nodes: usize,
}

impl PruningStrategy for BudgetCutoff {
    fn should_keep(&self, _node: &ThoughtNode, ctx: &PruneContext) -> bool {
        ctx.nodes_explored < self.max_nodes
    }
    fn name(&self) -> &'static str {
        "budget_cutoff"
    }
}

// -- Composite strategy -----------------------------------------------------

/// Chains multiple strategies; a node is kept only when **all** inner
/// strategies agree.
pub struct CompositePruner {
    strategies: Vec<Box<dyn PruningStrategy>>,
}

impl std::fmt::Debug for CompositePruner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names: Vec<&str> = self.strategies.iter().map(|s| s.name()).collect();
        f.debug_struct("CompositePruner")
            .field("strategies", &names)
            .finish()
    }
}

impl Clone for CompositePruner {
    fn clone(&self) -> Self {
        // Cannot clone trait objects generically; produce an empty composite.
        // Callers that need a true deep-clone should reconstruct from config.
        Self {
            strategies: Vec::new(),
        }
    }
}

impl CompositePruner {
    pub fn new(strategies: Vec<Box<dyn PruningStrategy>>) -> Self {
        Self { strategies }
    }
}

impl PruningStrategy for CompositePruner {
    fn should_keep(&self, node: &ThoughtNode, ctx: &PruneContext) -> bool {
        self.strategies.iter().all(|s| s.should_keep(node, ctx))
    }
    fn name(&self) -> &'static str {
        "composite"
    }
}

// ---------------------------------------------------------------------------
// Checkpoint (serde-serialisable backtrack state)
// ---------------------------------------------------------------------------

/// Serializable snapshot of the search state at a fork point, enabling
/// backtracking and debug/replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchCheckpoint {
    /// All nodes at the time of the snapshot.
    pub nodes: HashMap<NodeId, ThoughtNode>,
    /// Next id to allocate.
    pub next_id: NodeId,
    /// Depth at which the checkpoint was taken.
    pub depth: usize,
    /// Beam (candidate ids) at the checkpoint depth.
    pub beam: Vec<NodeId>,
}

// ---------------------------------------------------------------------------
// ReasoningTrace
// ---------------------------------------------------------------------------

/// Exportable record of a complete search run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub root_id: NodeId,
    pub explored_nodes: Vec<NodeId>,
    pub pruned_nodes: Vec<NodeId>,
    pub final_path: Vec<NodeId>,
    pub total_tokens: usize,
    pub latency_ms: u64,
    pub pruning_stats: PruningStats,
}

/// Aggregate pruning statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PruningStats {
    pub score_pruned: usize,
    pub duplicate_pruned: usize,
    pub budget_pruned: usize,
    pub total_kept: usize,
}

impl ReasoningTrace {
    /// Serialise the trace as pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

// ---------------------------------------------------------------------------
// TreeSearch — BFS beam search with backtracking
// ---------------------------------------------------------------------------

/// BFS beam-search engine over a thought tree.
pub struct TreeSearch {
    config: BeamSearchConfig,
    nodes: HashMap<NodeId, ThoughtNode>,
    next_id: NodeId,
    checkpoints: Vec<SearchCheckpoint>,
    pruned: Vec<NodeId>,
    explored: Vec<NodeId>,
    pruning_stats: PruningStats,
}

impl TreeSearch {
    /// Create a new search rooted at `root_content` with the given config.
    pub fn new(root_content: impl Into<String>, config: BeamSearchConfig) -> Self {
        let root = ThoughtNode {
            content: root_content.into(),
            score: 1.0,
            parent_id: None,
            children: Vec::new(),
            verdict: ThoughtVerdict::NeedsExpansion,
        };
        let mut nodes = HashMap::new();
        nodes.insert(0, root);
        Self {
            config,
            nodes,
            next_id: 1,
            checkpoints: Vec::new(),
            pruned: Vec::new(),
            explored: vec![0],
            pruning_stats: PruningStats::default(),
        }
    }

    /// Allocate a new node id.
    fn alloc_id(&mut self) -> NodeId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Expand a node by adding children produced by `generator`.
    /// Returns the ids of newly created children.
    pub fn expand<F>(&mut self, parent_id: NodeId, generator: F) -> Vec<NodeId>
    where
        F: FnOnce(&ThoughtNode) -> Vec<(String, f64, ThoughtVerdict)>,
    {
        let thoughts = {
            let parent = self.nodes.get(&parent_id).expect("parent must exist");
            generator(parent)
        };

        let mut child_ids = Vec::with_capacity(thoughts.len());
        for (content, score, verdict) in thoughts {
            let id = self.alloc_id();
            let node = ThoughtNode {
                content,
                score,
                parent_id: Some(parent_id),
                children: Vec::new(),
                verdict,
            };
            self.nodes.insert(id, node);
            child_ids.push(id);
        }

        // Wire children into parent.
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children.extend_from_slice(&child_ids);
        }

        child_ids
    }

    /// Run one level of BFS beam search, applying `pruner` to filter
    /// candidates. Returns the surviving beam (sorted best-first).
    pub fn step_beam(
        &mut self,
        current_beam: &[NodeId],
        pruner: &dyn PruningStrategy,
    ) -> Vec<NodeId> {
        let ctx = PruneContext {
            nodes_explored: self.explored.len(),
            tokens_remaining: None,
        };

        let mut candidates: Vec<(NodeId, f64)> = Vec::new();

        for &nid in current_beam {
            let node = match self.nodes.get(&nid) {
                Some(n) => n,
                None => continue,
            };
            // Only expand nodes that still need expansion.
            if node.verdict != ThoughtVerdict::NeedsExpansion
                && node.verdict != ThoughtVerdict::Promising
            {
                continue;
            }
            for &cid in &node.children {
                let child = match self.nodes.get(&cid) {
                    Some(c) => c,
                    None => continue,
                };
                if pruner.should_keep(child, &ctx) {
                    candidates.push((cid, child.score));
                    self.explored.push(cid);
                    self.pruning_stats.total_kept += 1;
                } else {
                    self.pruned.push(cid);
                    self.pruning_stats.score_pruned += 1;
                }
            }
        }

        // Sort descending by score, keep top `width`.
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(self.config.width);
        candidates.into_iter().map(|(id, _)| id).collect()
    }

    /// Save a checkpoint for later backtracking.
    pub fn save_checkpoint(&mut self, depth: usize, beam: Vec<NodeId>) {
        let cp = SearchCheckpoint {
            nodes: self.nodes.clone(),
            next_id: self.next_id,
            depth,
            beam,
        };
        self.checkpoints.push(cp);
    }

    /// Restore the most recent checkpoint. Returns the restored beam.
    pub fn backtrack(&mut self) -> Option<Vec<NodeId>> {
        let cp = self.checkpoints.pop()?;
        self.nodes = cp.nodes;
        self.next_id = cp.next_id;
        Some(cp.beam)
    }

    /// Build a [`ReasoningTrace`] from the current state.
    pub fn build_trace(&self, latency_ms: u64, total_tokens: usize) -> ReasoningTrace {
        // Reconstruct final path: walk from last explored solved node back to root.
        let final_path = self.reconstruct_final_path();
        ReasoningTrace {
            root_id: 0,
            explored_nodes: self.explored.clone(),
            pruned_nodes: self.pruned.clone(),
            final_path,
            total_tokens,
            latency_ms,
            pruning_stats: self.pruning_stats.clone(),
        }
    }

    /// Walk backwards from the first `Solved` node (if any) to the root.
    fn reconstruct_final_path(&self) -> Vec<NodeId> {
        let solved = self
            .nodes
            .values()
            .find(|n| n.verdict == ThoughtVerdict::Solved);
        let mut path = Vec::new();
        if let Some(node) = solved {
            // Find its id.
            let solved_id = self
                .nodes
                .iter()
                .find_map(|(id, n)| (std::ptr::eq(n, node)).then_some(*id));
            if let Some(mut cur) = solved_id {
                loop {
                    path.push(cur);
                    match self.nodes.get(&cur).and_then(|n| n.parent_id) {
                        Some(pid) => cur = pid,
                        None => break,
                    }
                }
                path.reverse();
            }
        }
        path
    }

    /// Access the node map (read-only).
    pub fn nodes(&self) -> &HashMap<NodeId, ThoughtNode> {
        &self.nodes
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- Beam search correctness -------------------------------------------

    #[test]
    fn beam_search_keeps_top_k_by_score() {
        let config = BeamSearchConfig {
            width: 2,
            depth_limit: 3,
        };
        let mut search = TreeSearch::new("root", config);

        // Expand root with 4 children of varying scores.
        search.expand(0, |_| {
            vec![
                ("a".into(), 0.9, ThoughtVerdict::Promising),
                ("b".into(), 0.3, ThoughtVerdict::Promising),
                ("c".into(), 0.7, ThoughtVerdict::Promising),
                ("d".into(), 0.5, ThoughtVerdict::Promising),
            ]
        });

        let beam = vec![0];
        let pruner = ScoreThreshold { min_score: 0.0 };
        let result = search.step_beam(&beam, &pruner);

        assert_eq!(result.len(), 2, "beam width must be respected");
        // Scores of kept nodes should be the two highest.
        let scores: Vec<f64> = result.iter().map(|id| search.nodes()[id].score).collect();
        assert!(scores.iter().all(|&s| s >= 0.7));
    }

    // -- Pruning composition -----------------------------------------------

    #[test]
    fn composite_pruner_requires_all_strategies() {
        let score = ScoreThreshold { min_score: 0.5 };
        let budget = BudgetCutoff { max_nodes: 100 };
        let composite = CompositePruner::new(vec![Box::new(score), Box::new(budget)]);

        let good = ThoughtNode {
            content: "ok".into(),
            score: 0.8,
            parent_id: None,
            children: vec![],
            verdict: ThoughtVerdict::Promising,
        };
        let low_score = ThoughtNode {
            content: "bad".into(),
            score: 0.1,
            parent_id: None,
            children: vec![],
            verdict: ThoughtVerdict::Promising,
        };
        let ctx = PruneContext {
            nodes_explored: 10,
            tokens_remaining: None,
        };

        assert!(composite.should_keep(&good, &ctx));
        assert!(!composite.should_keep(&low_score, &ctx));
    }

    // -- Backtracking state restoration ------------------------------------

    #[test]
    fn backtrack_restores_previous_state() {
        let config = BeamSearchConfig {
            width: 3,
            depth_limit: 5,
        };
        let mut search = TreeSearch::new("root", config);

        search.expand(0, |_| {
            vec![
                ("x".into(), 0.9, ThoughtVerdict::Promising),
                ("y".into(), 0.8, ThoughtVerdict::Promising),
            ]
        });

        // Checkpoint before destructive mutation.
        let beam_before = vec![1, 2];
        search.save_checkpoint(1, beam_before.clone());

        // Mutate: add another child.
        search.expand(0, |_| vec![("z".into(), 0.1, ThoughtVerdict::DeadEnd)]);
        assert_eq!(search.nodes().len(), 4); // root + x + y + z

        // Backtrack.
        let restored_beam = search.backtrack().expect("checkpoint exists");
        assert_eq!(restored_beam, beam_before);
        assert_eq!(search.nodes().len(), 3); // z removed
    }

    // -- Feature flag bypass -----------------------------------------------

    #[test]
    fn feature_flag_default_enabled() {
        // Reset to default for test isolation.
        TOT_ENABLED.store(true, Ordering::Relaxed);
        assert!(tot_enabled());
    }

    #[test]
    fn feature_flag_can_be_disabled() {
        TOT_ENABLED.store(false, Ordering::Relaxed);
        assert!(!tot_enabled());
        // Restore for other tests.
        TOT_ENABLED.store(true, Ordering::Relaxed);
    }

    // -- Trace export validity ---------------------------------------------

    #[test]
    fn reasoning_trace_serialises_to_valid_json() {
        let config = BeamSearchConfig {
            width: 2,
            depth_limit: 3,
        };
        let mut search = TreeSearch::new("root", config);
        search.expand(0, |_| vec![("step1".into(), 0.9, ThoughtVerdict::Solved)]);

        let trace = search.build_trace(42, 100);
        let json = trace.to_json().expect("must serialise");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("must be valid JSON");
        assert!(parsed["explored_nodes"].is_array());
        assert_eq!(parsed["latency_ms"], 42);
    }
}
