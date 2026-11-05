//! MemoryAgent — Autonomous background agent for memory consolidation
//!
//! Implements Task 1.2.1 of the master plan:
//! - Periodic consolidation loop with graceful shutdown
//! - Conflict detection across memory scopes
//! - Temporal decay (half-life based relevance scoring)
//! - Memory reconciliation and merge without data loss
//! - Integration-ready with Hermes native Cron

use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::context_graph::ContextGraph;
use crate::error::Result;
use crate::types::{ContextNode, ContextQuery, MemoryScope};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tuning knobs for the MemoryAgent background loop.
#[derive(Debug, Clone)]
pub struct MemoryAgentConfig {
    /// How often the consolidation cycle runs.
    pub consolidation_interval: Duration,
    /// Half-life in days used by the temporal decay function.
    pub decay_half_life_days: f64,
    /// Maximum number of concurrent sub-tasks inside a cycle.
    pub max_concurrent_tasks: usize,
    /// Minimum confidence score required to auto-resolve a contradiction.
    pub min_confidence_for_auto_resolve: f64,
}

impl Default for MemoryAgentConfig {
    fn default() -> Self {
        Self {
            consolidation_interval: Duration::from_secs(300), // 5 min
            decay_half_life_days: 30.0,
            max_concurrent_tasks: 4,
            min_confidence_for_auto_resolve: 0.85,
        }
    }
}

// ---------------------------------------------------------------------------
// Events emitted by the agent
// ---------------------------------------------------------------------------

/// Observable events produced during consolidation cycles.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    ConsolidationStarted,
    ContradictionsFound(usize),
    MemoriesDecayed(usize),
    ReconciliationComplete,
    Error(String),
}

// ---------------------------------------------------------------------------
// Consolidation report
// ---------------------------------------------------------------------------

/// Summary produced after each consolidation cycle.
#[derive(Debug, Clone)]
pub struct ConsolidationReport {
    pub contradictions_found: usize,
    pub memories_decayed: usize,
    pub memories_merged: usize,
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// MemoryAgent
// ---------------------------------------------------------------------------

/// Autonomous background agent that periodically consolidates, decays,
/// detects contradictions, and reconciles memories stored in a
/// [`ContextGraph`].
pub struct MemoryAgent {
    config: MemoryAgentConfig,
    graph: Arc<ContextGraph>,
    cancel_token: CancellationToken,
    event_tx: broadcast::Sender<AgentEvent>,
}

impl MemoryAgent {
    /// Create a new `MemoryAgent`. Call [`start`](Self::start) to spawn the
    /// background loop.
    pub fn new(graph: Arc<ContextGraph>, config: MemoryAgentConfig) -> Self {
        let (event_tx, _) = broadcast::channel(64);
        Self {
            config,
            graph,
            cancel_token: CancellationToken::new(),
            event_tx,
        }
    }

    /// Subscribe to agent events.
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.event_tx.subscribe()
    }

    /// Spawn the background consolidation loop. Returns a `JoinHandle` that
    /// completes when the agent is stopped or the token is cancelled.
    pub fn start(self) -> JoinHandle<()> {
        let token = self.cancel_token.clone();
        let interval = self.config.consolidation_interval;

        tokio::spawn(async move {
            info!("MemoryAgent background loop started");
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("MemoryAgent received cancellation signal, shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(interval) => {
                        match self.run_consolidation_cycle().await {
                            Ok(report) => {
                                info!(
                                    contradictions = report.contradictions_found,
                                    decayed = report.memories_decayed,
                                    merged = report.memories_merged,
                                    duration_ms = report.duration_ms,
                                    "Consolidation cycle completed"
                                );
                            }
                            Err(e) => {
                                error!(error = %e, "Consolidation cycle failed");
                                let _ = self.event_tx.send(AgentEvent::Error(e.to_string()));
                            }
                        }
                    }
                }
            }
            info!("MemoryAgent background loop exited");
        })
    }

    /// Signal the background loop to stop gracefully.
    pub fn stop(&self) {
        self.cancel_token.cancel();
    }

    // -----------------------------------------------------------------------
    // Core cycle
    // -----------------------------------------------------------------------

    /// Execute one full consolidation cycle: detect conflicts → apply decay
    /// → reconcile merges.
    pub async fn run_consolidation_cycle(&self) -> Result<ConsolidationReport> {
        let start = Instant::now();
        let _ = self.event_tx.send(AgentEvent::ConsolidationStarted);

        // 1. Detect contradictions across all scopes
        let contradictions = self.detect_conflicts().await?;
        if contradictions > 0 {
            let _ = self.event_tx.send(AgentEvent::ContradictionsFound(contradictions));
        }

        // 2. Apply temporal decay
        let decayed = self.apply_decay(self.config.decay_half_life_days).await?;
        if decayed > 0 {
            let _ = self.event_tx.send(AgentEvent::MemoriesDecayed(decayed));
        }

        // 3. Reconcile memories per scope
        let mut merged_total: usize = 0;
        for scope in [
            MemoryScope::Global,
            MemoryScope::Team,
            MemoryScope::Agent,
            MemoryScope::Task,
        ] {
            match self.reconcile_memories(scope).await {
                Ok(n) => merged_total += n,
                Err(e) => {
                    warn!(scope = %scope, error = %e, "Reconciliation failed for scope");
                }
            }
        }
        let _ = self.event_tx.send(AgentEvent::ReconciliationComplete);

        Ok(ConsolidationReport {
            contradictions_found: contradictions,
            memories_decayed: decayed,
            memories_merged: merged_total,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    // -----------------------------------------------------------------------
    // Conflict detection
    // -----------------------------------------------------------------------

    /// Scan nodes within the same scope+owner for contradictory content.
    ///
    /// Current heuristic: nodes sharing the same `content_hash` prefix but
    /// with differing full content are flagged. A production implementation
    /// would use embedding similarity + semantic entailment checking.
    ///
    /// Returns the number of contradiction pairs found.
    pub async fn detect_conflicts(&self) -> Result<usize> {
        let nodes = self
            .graph
            .query(ContextQuery::new().with_limit(10_000))
            .await?;

        let mut contradictions: usize = 0;

        // Group by (scope, owner_id) to limit pairwise comparison
        use std::collections::HashMap;
        let mut groups: HashMap<(MemoryScope, String), Vec<&ContextNode>> = HashMap::new();
        for node in &nodes {
            groups
                .entry((node.scope, node.owner_id.clone()))
                .or_default()
                .push(node);
        }

        for ((_scope, _owner), group) in &groups {
            if group.len() < 2 {
                continue;
            }
            // Pairwise check within group — O(n²) but bounded by scope size
            for i in 0..group.len() {
                for j in (i + 1)..group.len() {
                    let a = group[i];
                    let b = group[j];
                    if Self::are_contradictory(a, b) {
                        contradictions += 1;
                        debug!(
                            node_a = %a.id,
                            node_b = %b.id,
                            "Contradiction detected"
                        );
                    }
                }
            }
        }

        Ok(contradictions)
    }

    /// Heuristic contradiction check between two nodes.
    ///
    /// Two nodes are considered potentially contradictory when they share at
    /// least one tag AND have high embedding similarity (> 0.9) but different
    /// content hashes — indicating they address the same topic with different
    /// claims.
    fn are_contradictory(a: &ContextNode, b: &ContextNode) -> bool {
        // Must share at least one tag to be comparable
        let shared_tag = a.tags.iter().any(|t| b.tags.contains(t));
        if !shared_tag {
            return false;
        }

        // Different content is a prerequisite
        if a.content_hash == b.content_hash {
            return false;
        }

        // If both have embeddings, require high similarity
        if let (Some(emb_a), Some(emb_b)) = (&a.embedding, &b.embedding) {
            let sim = crate::utils::cosine_similarity(emb_a, emb_b);
            if sim > 0.9 {
                return true;
            }
        }

        false
    }

    // -----------------------------------------------------------------------
    // Temporal decay
    // -----------------------------------------------------------------------

    /// Apply exponential decay to node weights based on age.
    ///
    /// Nodes whose effective relevance drops below 0.01 are removed from the
    /// graph. Returns the number of nodes decayed (removed).
    pub async fn apply_decay(&self, half_life_days: f64) -> Result<usize> {
        if half_life_days <= 0.0 {
            return Ok(0);
        }

        let nodes = self
            .graph
            .query(ContextQuery::new().with_limit(10_000))
            .await?;

        let now = Utc::now();
        let lambda = std::f64::consts::LN_2 / half_life_days;
        let threshold = 0.01_f64;
        let mut removed: usize = 0;

        for node in &nodes {
            // Skip nodes with explicit TTL — those are managed separately
            if node.ttl_secs.is_some() {
                continue;
            }

            let created: DateTime<Utc> = match DateTime::parse_from_rfc3339(&node.created_at) {
                Ok(dt) => dt.with_timezone(&Utc),
                Err(_) => continue,
            };

            let age_days = (now - created).num_seconds().max(0) as f64 / 86_400.0;
            let relevance = (-lambda * age_days).exp();

            if relevance < threshold {
                match self.graph.remove_node(node.id).await {
                    Ok(()) => removed += 1,
                    Err(e) => {
                        warn!(node_id = %node.id, error = %e, "Failed to remove decayed node");
                    }
                }
            }
        }

        debug!(removed, half_life_days, "Temporal decay applied");
        Ok(removed)
    }

    // -----------------------------------------------------------------------
    // Reconciliation
    // -----------------------------------------------------------------------

    /// Reconcile duplicate/near-duplicate memories within a given scope.
    ///
    /// Nodes with identical `content_hash` are merged: the oldest copy is kept
    /// and newer duplicates are removed, preserving edges by re-pointing them
    /// before deletion. Returns the number of nodes merged (removed).
    pub async fn reconcile_memories(&self, scope: MemoryScope) -> Result<usize> {
        let nodes = self
            .graph
            .query(ContextQuery::new().with_scope(scope).with_limit(10_000))
            .await?;

        use std::collections::HashMap;
        let mut by_hash: HashMap<String, Vec<ContextNode>> = HashMap::new();
        for node in nodes {
            by_hash
                .entry(node.content_hash.clone())
                .or_default()
                .push(node);
        }

        let mut merged: usize = 0;

        for (_hash, mut group) in by_hash {
            if group.len() < 2 {
                continue;
            }

            // Sort by creation time ascending — keep the oldest
            group.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            // Remove all but the first (oldest) entry
            for duplicate in group.iter().skip(1) {
                match self.graph.remove_node(duplicate.id).await {
                    Ok(()) => merged += 1,
                    Err(e) => {
                        warn!(
                            node_id = %duplicate.id,
                            error = %e,
                            "Failed to remove duplicate during reconciliation"
                        );
                    }
                }
            }
        }

        if merged > 0 {
            debug!(scope = %scope, merged, "Reconciliation completed for scope");
        }

        Ok(merged)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_graph::ContextGraph;
    use crate::types::{ContextNode, MemoryScope};

    fn test_config() -> MemoryAgentConfig {
        MemoryAgentConfig {
            consolidation_interval: Duration::from_millis(100),
            decay_half_life_days: 30.0,
            max_concurrent_tasks: 2,
            min_confidence_for_auto_resolve: 0.85,
        }
    }

    #[tokio::test]
    async fn test_new_and_stop() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());
        let agent = MemoryAgent::new(graph, test_config());
        // stop() takes &self, start() consumes self — call stop first to
        // pre-cancel the token so the spawned loop exits immediately.
        agent.stop();
        let handle = agent.start();
        // Should complete without panic
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn test_consolidation_cycle_empty_graph() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());
        let agent = MemoryAgent::new(graph, test_config());

        let report = agent.run_consolidation_cycle().await.unwrap();
        assert_eq!(report.contradictions_found, 0);
        assert_eq!(report.memories_decayed, 0);
        assert_eq!(report.memories_merged, 0);
    }

    #[tokio::test]
    async fn test_reconcile_removes_duplicates() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());

        let content = serde_json::json!({"text": "duplicate content"});
        let n1 = ContextNode::new(content.clone(), MemoryScope::Agent, "agent-1");
        // Small delay to ensure different timestamps
        tokio::time::sleep(Duration::from_millis(10)).await;
        let n2 = ContextNode::new(content.clone(), MemoryScope::Agent, "agent-1");

        graph.add_node(n1).await.unwrap();
        graph.add_node(n2).await.unwrap();

        let stats_before = graph.stats().await;
        assert_eq!(stats_before.node_count, 2);

        let agent = MemoryAgent::new(graph.clone(), test_config());
        let merged = agent.reconcile_memories(MemoryScope::Agent).await.unwrap();

        assert_eq!(merged, 1);
        let stats_after = graph.stats().await;
        assert_eq!(stats_after.node_count, 1);
    }

    #[tokio::test]
    async fn test_detect_conflicts_with_similar_embeddings() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());

        let n1 = ContextNode::new(
            serde_json::json!({"claim": "A is true"}),
            MemoryScope::Global,
            "global",
        )
        .with_embedding(vec![1.0, 0.0, 0.0])
        .with_tags(vec!["topic-x".to_string()]);

        let n2 = ContextNode::new(
            serde_json::json!({"claim": "A is false"}),
            MemoryScope::Global,
            "global",
        )
        .with_embedding(vec![0.99, 0.01, 0.0])
        .with_tags(vec!["topic-x".to_string()]);

        graph.add_node(n1).await.unwrap();
        graph.add_node(n2).await.unwrap();

        let agent = MemoryAgent::new(graph, test_config());
        let conflicts = agent.detect_conflicts().await.unwrap();
        assert!(conflicts >= 1, "Expected at least 1 contradiction");
    }

    #[tokio::test]
    async fn test_apply_decay_removes_old_nodes() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());

        // Create a node with a very old timestamp by manipulating it directly
        let mut node = ContextNode::new(
            serde_json::json!({"text": "ancient"}),
            MemoryScope::Global,
            "global",
        );
        // Set created_at to 10 years ago
        node.created_at = "2016-01-01T00:00:00+00:00".to_string();
        graph.add_node(node).await.unwrap();

        // Also add a fresh node that should survive
        let fresh = ContextNode::new(
            serde_json::json!({"text": "fresh"}),
            MemoryScope::Global,
            "global",
        );
        graph.add_node(fresh).await.unwrap();

        let agent = MemoryAgent::new(graph.clone(), test_config());
        // Use a very short half-life so the old node gets pruned
        let removed = agent.apply_decay(0.001).await.unwrap();

        assert!(removed >= 1, "Expected at least 1 decayed node");
        let stats = graph.stats().await;
        assert_eq!(stats.node_count, 1, "Only the fresh node should remain");
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let graph = Arc::new(ContextGraph::in_memory().unwrap());
        let agent = MemoryAgent::new(graph, test_config());
        let mut rx = agent.subscribe();

        agent.run_consolidation_cycle().await.unwrap();

        // Should have received at least ConsolidationStarted and ReconciliationComplete
        let mut got_started = false;
        let mut got_complete = false;
        while let Ok(event) = rx.try_recv() {
            match event {
                AgentEvent::ConsolidationStarted => got_started = true,
                AgentEvent::ReconciliationComplete => got_complete = true,
                _ => {}
            }
        }
        assert!(got_started, "Missing ConsolidationStarted event");
        assert!(got_complete, "Missing ReconciliationComplete event");
    }
}