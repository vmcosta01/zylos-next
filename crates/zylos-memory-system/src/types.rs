//! Core types for the MEMORY_SYSTEM ContextGraph

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a context graph node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl NodeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for NodeId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for NodeId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(NodeId)
            .map_err(|e| format!("Invalid NodeId: {}", e))
    }
}

/// Scope isolation level for memory entries (4-scope model)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryScope {
    /// Global scope — shared across all agents and teams
    Global,
    /// Team scope — shared within a team of agents
    Team,
    /// Agent scope — private to a single agent
    Agent,
    /// Task scope — ephemeral, tied to a specific task execution
    Task,
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryScope::Global => write!(f, "global"),
            MemoryScope::Team => write!(f, "team"),
            MemoryScope::Agent => write!(f, "agent"),
            MemoryScope::Task => write!(f, "task"),
        }
    }
}

impl std::str::FromStr for MemoryScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "global" => Ok(MemoryScope::Global),
            "team" => Ok(MemoryScope::Team),
            "agent" => Ok(MemoryScope::Agent),
            "task" => Ok(MemoryScope::Task),
            _ => Err(format!("Invalid scope: {}", s)),
        }
    }
}

/// A node in the context graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    /// Unique identifier
    pub id: NodeId,
    /// Content payload (JSON)
    pub content: serde_json::Value,
    /// Scope isolation level
    pub scope: MemoryScope,
    /// Owner identifier (agent_id, team_id, or "global")
    pub owner_id: String,
    /// SHA-256 hash of content for deduplication
    pub content_hash: String,
    /// Creation timestamp (ISO 8601)
    pub created_at: String,
    /// Optional TTL in seconds (None = permanent)
    pub ttl_secs: Option<u64>,
    /// Optional embedding vector for similarity search
    pub embedding: Option<Vec<f32>>,
    /// Metadata tags for filtering
    pub tags: Vec<String>,
}

impl ContextNode {
    /// Create a new context node with auto-generated ID and timestamp
    pub fn new(
        content: serde_json::Value,
        scope: MemoryScope,
        owner_id: impl Into<String>,
    ) -> Self {
        let content_str = serde_json::to_string(&content).unwrap_or_default();
        let content_hash = format!("{:x}", md5::compute(content_str.as_bytes()));
        Self {
            id: NodeId::new(),
            content,
            scope,
            owner_id: owner_id.into(),
            content_hash,
            created_at: chrono::Utc::now().to_rfc3339(),
            ttl_secs: None,
            embedding: None,
            tags: Vec::new(),
        }
    }

    /// Set TTL for this node
    pub fn with_ttl(mut self, secs: u64) -> Self {
        self.ttl_secs = Some(secs);
        self
    }

    /// Set embedding vector for this node
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }

    /// Add tags for filtering
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// An edge in the context graph representing a relationship between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEdge {
    /// Source node ID
    pub from: NodeId,
    /// Target node ID
    pub to: NodeId,
    /// Relationship type (e.g., "references", "causes", "relates_to")
    pub relation: String,
    /// Edge weight for ranking (higher = stronger relationship)
    pub weight: f64,
    /// Creation timestamp
    pub created_at: String,
}

impl ContextEdge {
    /// Create a new edge between two nodes
    pub fn new(from: NodeId, to: NodeId, relation: impl Into<String>) -> Self {
        Self {
            from,
            to,
            relation: relation.into(),
            weight: 1.0,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Set edge weight
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Query parameters for context graph searches
#[derive(Debug, Clone, Default)]
pub struct ContextQuery {
    /// Filter by scope
    pub scope: Option<MemoryScope>,
    /// Filter by owner
    pub owner_id: Option<String>,
    /// Full-text search in content
    pub search_text: Option<String>,
    /// Filter by tags (all must match)
    pub tags: Vec<String>,
    /// Maximum results to return
    pub limit: Option<usize>,
    /// Embedding vector for similarity search
    pub embedding: Option<Vec<f32>>,
    /// Minimum similarity threshold for embedding search
    pub min_similarity: Option<f64>,
}

impl ContextQuery {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scope(mut self, scope: MemoryScope) -> Self {
        self.scope = Some(scope);
        self
    }

    pub fn with_owner(mut self, owner_id: impl Into<String>) -> Self {
        self.owner_id = Some(owner_id.into());
        self
    }

    pub fn with_search(mut self, text: impl Into<String>) -> Self {
        self.search_text = Some(text.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_embedding(mut self, embedding: Vec<f32>, min_similarity: f64) -> Self {
        self.embedding = Some(embedding);
        self.min_similarity = Some(min_similarity);
        self
    }
}
