//! ContextGraph — Persistent graph storage using SQLite WAL + petgraph
//!
//! Implements Task 2.1.1 of Fase 2: ContextGraph with SQLite WAL persistence,
//! lazy loading, neighborhood queries, and shortest path calculations.
//!
//! Target: <1ms p95 for queries (per EVALUATION.md)
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::algo::astar;
use petgraph::visit::EdgeRef;
use rusqlite::{params, Connection};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};
use crate::error::Result;
use crate::types::{ContextEdge, ContextNode, ContextQuery, MemoryScope, NodeId};
use crate::utils::cosine_similarity;

/// Persistent context graph with SQLite WAL backend
pub struct ContextGraph {
    /// In-memory graph representation (loaded on demand)
    graph: Arc<RwLock<DiGraph<ContextNode, ContextEdge>>>,
    /// Node ID to graph index mapping for O(1) lookups
    node_index_map: Arc<RwLock<HashMap<NodeId, NodeIndex>>>,
    /// SQLite connection (WAL mode enabled). Uses Mutex because rusqlite::Connection
    /// is Send but not Sync — concurrent access must be serialized.
    conn: Arc<Mutex<Connection>>,
}

impl ContextGraph {
    /// Open or create a context graph database at the given path
    ///
    /// Enables WAL mode for concurrent read/write performance.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path.as_ref())?;
        
        // Enable WAL mode for better concurrent performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch("PRAGMA cache_size=-64000;")?; // 64MB cache
        
        Self::initialize_schema_static(&conn)?;
                let (graph_data, node_map_data) = Self::load_all_static(&conn)?;

                Ok(Self {
                    graph: Arc::new(RwLock::new(graph_data)),
                    node_index_map: Arc::new(RwLock::new(node_map_data)),
                    conn: Arc::new(Mutex::new(conn)),
                })
    }
    
    /// Create an in-memory context graph (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        
        Self::initialize_schema_static(&conn)?;
        
        Ok(Self {
            graph: Arc::new(RwLock::new(DiGraph::new())),
            node_index_map: Arc::new(RwLock::new(HashMap::new())),
            conn: Arc::new(Mutex::new(conn)),
        })
    }
    
    /// Initialize database schema (static version that takes &Connection directly)
    fn initialize_schema_static(conn: &Connection) -> Result<()> {
        
        // Nodes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_nodes (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                scope TEXT NOT NULL,
                owner_id TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                ttl_secs INTEGER,
                embedding BLOB,
                tags TEXT,
                CHECK (scope IN ('global', 'team', 'agent', 'task'))
            )",
            [],
        )?;
        
        // Edges table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS context_edges (
                from_id TEXT NOT NULL,
                to_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL,
                PRIMARY KEY (from_id, to_id, relation),
                FOREIGN KEY (from_id) REFERENCES context_nodes(id) ON DELETE CASCADE,
                FOREIGN KEY (to_id) REFERENCES context_nodes(id) ON DELETE CASCADE
            )",
            [],
        )?;
        
        // Indexes for performance
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_scope ON context_nodes(scope)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_owner ON context_nodes(owner_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_nodes_hash ON context_nodes(content_hash)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_from ON context_edges(from_id)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_edges_to ON context_edges(to_id)",
            [],
        )?;
        
        info!("Context graph schema initialized");
        Ok(())
    }
    
    /// Load all nodes and edges from database into a new graph (static version)
        fn load_all_static(conn: &Connection) -> Result<(DiGraph<ContextNode, ContextEdge>, HashMap<NodeId, NodeIndex>)> {
            let mut graph = DiGraph::new();
            let mut node_map = HashMap::new();

            // Load nodes
            let mut stmt = conn.prepare(
                "SELECT id, content, scope, owner_id, content_hash, created_at, ttl_secs, embedding, tags
                 FROM context_nodes"
            )?;

            let nodes = stmt.query_map([], |row| {
                let id_str: String = row.get(0)?;
                let content_str: String = row.get(1)?;
                let scope_str: String = row.get(2)?;
                let owner_id: String = row.get(3)?;
                let content_hash: String = row.get(4)?;
                let created_at: String = row.get(5)?;
                let ttl_secs: Option<i64> = row.get(6)?;
                let embedding_blob: Option<Vec<u8>> = row.get(7)?;
                let tags_str: String = row.get(8)?;

                let id = NodeId::from_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
                })?;

                let content: serde_json::Value = serde_json::from_str(&content_str)
                    .unwrap_or(serde_json::Value::Null);

                let scope: MemoryScope = scope_str.parse()
                    .unwrap_or(MemoryScope::Global);

                let embedding = embedding_blob.and_then(|blob| {
                    if blob.len() % 4 != 0 {
                        return None;
                    }
                    let mut vec = Vec::with_capacity(blob.len() / 4);
                    for chunk in blob.chunks_exact(4) {
                        let bytes: [u8; 4] = chunk.try_into().ok()?;
                        vec.push(f32::from_le_bytes(bytes));
                    }
                    Some(vec)
                });

                let tags: Vec<String> = if tags_str.is_empty() {
                    Vec::new()
                } else {
                    tags_str.split(',').map(|s| s.to_string()).collect()
                };

                Ok(ContextNode {
                    id,
                    content,
                    scope,
                    owner_id,
                    content_hash,
                    created_at,
                    ttl_secs: ttl_secs.map(|t| t as u64),
                    embedding,
                    tags,
                })
            })?;

            for node_result in nodes {
                let node = node_result?;
                let idx = graph.add_node(node.clone());
                node_map.insert(node.id, idx);
            }

            // Load edges
            let mut stmt = conn.prepare(
                "SELECT from_id, to_id, relation, weight, created_at FROM context_edges"
            )?;

            let edges = stmt.query_map([], |row| {
                let from_str: String = row.get(0)?;
                let to_str: String = row.get(1)?;
                let relation: String = row.get(2)?;
                let weight: f64 = row.get(3)?;
                let created_at: String = row.get(4)?;

                let from = NodeId::from_str(&from_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
                })?;

                let to = NodeId::from_str(&to_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
                })?;

                Ok(ContextEdge {
                    from,
                    to,
                    relation,
                    weight,
                    created_at,
                })
            })?;

            for edge_result in edges {
                let edge = edge_result?;
                if let (Some(&from_idx), Some(&to_idx)) = (node_map.get(&edge.from), node_map.get(&edge.to)) {
                    graph.add_edge(from_idx, to_idx, edge);
                }
            }

            debug!(
                nodes = graph.node_count(),
                edges = graph.edge_count(),
                "Loaded context graph from database"
            );

            Ok((graph, node_map))
        }
    
    /// Add a node to the graph (persists to database)
    pub async fn add_node(&self, node: ContextNode) -> Result<NodeId> {
        let node_id = node.id;
        
        // Serialize fields
        let content_str = serde_json::to_string(&node.content)?;
        let scope_str = node.scope.to_string();
        let embedding_blob = node.embedding.as_ref().map(|vec| {
            vec.iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<u8>>()
        });
        let tags_str = node.tags.join(",");
        
        // Insert into database
        {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT OR REPLACE INTO context_nodes 
                 (id, content, scope, owner_id, content_hash, created_at, ttl_secs, embedding, tags)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    node_id.as_str(),
                    content_str,
                    scope_str,
                    node.owner_id,
                    node.content_hash,
                    node.created_at,
                    node.ttl_secs.map(|t| t as i64),
                    embedding_blob,
                    tags_str,
                ],
            )?;
        }
        
        // Update in-memory graph
        {
            let mut graph = self.graph.write().await;
            let mut node_map = self.node_index_map.write().await;
            let idx = graph.add_node(node);
            node_map.insert(node_id, idx);
        }
        
        debug!(node_id = %node_id, "Added node to context graph");
        Ok(node_id)
    }
    
    /// Add an edge between two nodes (persists to database)
    pub async fn add_edge(&self, edge: ContextEdge) -> Result<()> {
        let from_str = edge.from.as_str();
        let to_str = edge.to.as_str();
        
        // Insert into database
        {
            let conn = self.conn.lock().await;
            conn.execute(
                "INSERT OR REPLACE INTO context_edges (from_id, to_id, relation, weight, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![from_str, to_str, edge.relation, edge.weight, edge.created_at],
            )?;
        }
        
        // Update in-memory graph
        {
            let graph = self.graph.read().await;
            let node_map = self.node_index_map.read().await;
            
            if let (Some(&from_idx), Some(&to_idx)) = (node_map.get(&edge.from), node_map.get(&edge.to)) {
                drop(graph); // Release read lock before acquiring write lock
                let mut graph = self.graph.write().await;
                graph.add_edge(from_idx, to_idx, edge.clone());
            } else {
                warn!(
                    from = %edge.from,
                    to = %edge.to,
                    "Cannot add edge: one or both nodes not found"
                );
            }
        }
        
        debug!(from = %edge.from, to = %edge.to, "Added edge to context graph");
        Ok(())
    }
    
    /// Get a node by ID
    pub async fn get_node(&self, id: NodeId) -> Result<Option<ContextNode>> {
        let graph = self.graph.read().await;
        let node_map = self.node_index_map.read().await;
        
        if let Some(&idx) = node_map.get(&id) {
            Ok(Some(graph[idx].clone()))
        } else {
            Ok(None)
        }
    }
    
    /// Remove a node and all its edges
    pub async fn remove_node(&self, id: NodeId) -> Result<()> {
        let id_str = id.as_str();
        
        // Remove from database (cascades to edges)
        {
            let conn = self.conn.lock().await;
            conn.execute("DELETE FROM context_nodes WHERE id = ?1", params![id_str])?;
        }
        
        // Remove from in-memory graph
        {
            let mut graph = self.graph.write().await;
            let mut node_map = self.node_index_map.write().await;
            
            if let Some(idx) = node_map.remove(&id) {
                graph.remove_node(idx);
                // Rebuild index map since indices shift after removal
                node_map.clear();
                for idx in graph.node_indices() {
                    node_map.insert(graph[idx].id, idx);
                }
            }
        }
        
        debug!(node_id = %id, "Removed node from context graph");
        Ok(())
    }
    
    /// Query nodes matching criteria
    pub async fn query(&self, query: ContextQuery) -> Result<Vec<ContextNode>> {
        let graph = self.graph.read().await;
        let mut results = Vec::new();
        
        for idx in graph.node_indices() {
            let node = &graph[idx];
            
            // Apply filters
            if let Some(scope) = &query.scope {
                if &node.scope != scope {
                    continue;
                }
            }
            
            if let Some(owner) = &query.owner_id {
                if &node.owner_id != owner {
                    continue;
                }
            }
            
            if let Some(search) = &query.search_text {
                let content_str = serde_json::to_string(&node.content).unwrap_or_default();
                if !content_str.to_lowercase().contains(&search.to_lowercase()) {
                    continue;
                }
            }
            
            if !query.tags.is_empty() {
                let has_all_tags = query.tags.iter().all(|tag| node.tags.contains(tag));
                if !has_all_tags {
                    continue;
                }
            }
            
            // Embedding similarity filter (simplified brute-force for MVP)
            if let (Some(query_emb), Some(min_sim)) = (&query.embedding, query.min_similarity) {
                if let Some(node_emb) = &node.embedding {
                    let similarity = cosine_similarity(query_emb, node_emb);
                    if similarity < min_sim as f32 {
                        continue;
                    }
                } else {
                    continue; // Node has no embedding, skip
                }
            }
            
            results.push(node.clone());
            
            // Apply limit
            if let Some(limit) = query.limit {
                if results.len() >= limit {
                    break;
                }
            }
        }
        
        Ok(results)
    }
    
    /// Get neighbors of a node (incoming and outgoing edges)
    pub async fn get_neighbors(&self, id: NodeId) -> Result<Vec<(ContextNode, ContextEdge)>> {
        let graph = self.graph.read().await;
        let node_map = self.node_index_map.read().await;
        
        let mut neighbors = Vec::new();
        
        if let Some(&idx) = node_map.get(&id) {
            // Outgoing edges
            for edge_ref in graph.edges(idx) {
                let target_idx = edge_ref.target();
                let edge = edge_ref.weight().clone();
                let target_node = graph[target_idx].clone();
                neighbors.push((target_node, edge));
            }
            
            // Incoming edges
            for edge_ref in graph.edges_directed(idx, petgraph::Direction::Incoming) {
                let source_idx = edge_ref.source();
                let edge = edge_ref.weight().clone();
                let source_node = graph[source_idx].clone();
                neighbors.push((source_node, edge));
            }
        }
        
        Ok(neighbors)
    }
    
    /// Find shortest path between two nodes using A* algorithm
    pub async fn shortest_path(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> Result<Option<(Vec<NodeId>, f64)>> {
        let graph = self.graph.read().await;
        let node_map = self.node_index_map.read().await;
        
        let from_idx = node_map.get(&from).copied();
        let to_idx = node_map.get(&to).copied();
        
        if let (Some(from_idx), Some(to_idx)) = (from_idx, to_idx) {
            // Use edge weight as cost (inverse of weight for "stronger = shorter")
            let result = astar(
                &*graph,
                from_idx,
                |idx| idx == to_idx,
                |edge| 1.0 / edge.weight().weight.max(0.001),
                |_| 0.0,
            );
            
            if let Some((cost, path)) = result {
                let node_ids: Vec<NodeId> = path.iter().map(|&idx| graph[idx].id).collect();
                Ok(Some((node_ids, cost)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    /// Get graph statistics
    pub async fn stats(&self) -> GraphStats {
        let graph = self.graph.read().await;
        
        let mut scope_counts = HashMap::new();
        for idx in graph.node_indices() {
            let scope = graph[idx].scope;
            *scope_counts.entry(scope).or_insert(0) += 1;
        }
        
        GraphStats {
            node_count: graph.node_count(),
            edge_count: graph.edge_count(),
            scope_counts,
        }
    }
    
    /// Perform checkpoint to flush WAL to main database
    pub async fn checkpoint(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        info!("Context graph WAL checkpoint completed");
        Ok(())
    }
}

/// Graph statistics
#[derive(Debug, Clone)]
pub struct GraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub scope_counts: HashMap<MemoryScope, usize>,
}



#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[tokio::test]
    async fn test_add_and_get_node() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let node = ContextNode::new(
            serde_json::json!({"text": "Hello, world!"}),
            MemoryScope::Agent,
            "agent-1",
        );
        
        let node_id = graph.add_node(node.clone()).await.unwrap();
        let retrieved = graph.get_node(node_id).await.unwrap().unwrap();
        
        assert_eq!(retrieved.id, node.id);
        assert_eq!(retrieved.scope, MemoryScope::Agent);
        assert_eq!(retrieved.owner_id, "agent-1");
    }
    
    #[tokio::test]
    async fn test_add_edge_and_get_neighbors() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let node1 = ContextNode::new(
            serde_json::json!({"text": "Node 1"}),
            MemoryScope::Global,
            "global",
        );
        let node2 = ContextNode::new(
            serde_json::json!({"text": "Node 2"}),
            MemoryScope::Global,
            "global",
        );
        
        let id1 = graph.add_node(node1).await.unwrap();
        let id2 = graph.add_node(node2).await.unwrap();
        
        let edge = ContextEdge::new(id1, id2, "references");
        graph.add_edge(edge).await.unwrap();
        
        let neighbors = graph.get_neighbors(id1).await.unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0.id, id2);
        assert_eq!(neighbors[0].1.relation, "references");
    }
    
    #[tokio::test]
    async fn test_query_with_filters() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let node1 = ContextNode::new(
            serde_json::json!({"text": "Agent task"}),
            MemoryScope::Agent,
            "agent-1",
        )
        .with_tags(vec!["task".to_string(), "important".to_string()]);
        
        let node2 = ContextNode::new(
            serde_json::json!({"text": "Team decision"}),
            MemoryScope::Team,
            "team-alpha",
        )
        .with_tags(vec!["decision".to_string()]);
        
        graph.add_node(node1).await.unwrap();
        graph.add_node(node2).await.unwrap();
        
        // Query by scope
        let results = graph
            .query(ContextQuery::new().with_scope(MemoryScope::Agent))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].scope, MemoryScope::Agent);
        
        // Query by tags
        let results = graph
            .query(ContextQuery::new().with_tags(vec!["task".to_string()]))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        
        // Query by search text
        let results = graph
            .query(ContextQuery::new().with_search("decision"))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }
    
    #[tokio::test]
    async fn test_shortest_path() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let n1 = ContextNode::new(serde_json::json!({}), MemoryScope::Global, "global");
        let n2 = ContextNode::new(serde_json::json!({}), MemoryScope::Global, "global");
        let n3 = ContextNode::new(serde_json::json!({}), MemoryScope::Global, "global");
        
        let id1 = graph.add_node(n1).await.unwrap();
        let id2 = graph.add_node(n2).await.unwrap();
        let id3 = graph.add_node(n3).await.unwrap();
        
        graph.add_edge(ContextEdge::new(id1, id2, "next").with_weight(1.0)).await.unwrap();
        graph.add_edge(ContextEdge::new(id2, id3, "next").with_weight(1.0)).await.unwrap();
        
        let path = graph.shortest_path(id1, id3).await.unwrap();
        assert!(path.is_some());
        
        let (node_ids, cost) = path.unwrap();
        assert_eq!(node_ids.len(), 3);
        assert_eq!(node_ids[0], id1);
        assert_eq!(node_ids[1], id2);
        assert_eq!(node_ids[2], id3);
        assert!(cost > 0.0);
    }
    
    #[tokio::test]
    async fn test_persistence() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();
        
        // Create graph and add data
        {
            let graph = ContextGraph::open(&path).unwrap();
            let node = ContextNode::new(
                serde_json::json!({"persistent": true}),
                MemoryScope::Global,
                "global",
            );
            graph.add_node(node).await.unwrap();
        }
        
        // Reopen and verify data persisted
        {
            let graph = ContextGraph::open(&path).unwrap();
            let stats = graph.stats().await;
            assert_eq!(stats.node_count, 1);
        }
    }
    
    #[tokio::test]
    async fn test_remove_node() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let node = ContextNode::new(
            serde_json::json!({"text": "To be removed"}),
            MemoryScope::Agent,
            "agent-1",
        );
        
        let node_id = graph.add_node(node).await.unwrap();
        assert!(graph.get_node(node_id).await.unwrap().is_some());
        
        graph.remove_node(node_id).await.unwrap();
        assert!(graph.get_node(node_id).await.unwrap().is_none());
    }
    
    #[tokio::test]
    async fn test_embedding_similarity() {
        let graph = ContextGraph::in_memory().unwrap();
        
        let node1 = ContextNode::new(
            serde_json::json!({"text": "AI agent"}),
            MemoryScope::Global,
            "global",
        )
        .with_embedding(vec![1.0, 0.0, 0.0]);
        
        let node2 = ContextNode::new(
            serde_json::json!({"text": "Machine learning"}),
            MemoryScope::Global,
            "global",
        )
        .with_embedding(vec![0.9, 0.1, 0.0]);
        
        graph.add_node(node1).await.unwrap();
        graph.add_node(node2).await.unwrap();
        
        // Query with similar embedding
        let results = graph
            .query(
                ContextQuery::new()
                    .with_embedding(vec![1.0, 0.0, 0.0], 0.9),
            )
            .await
            .unwrap();
        
        assert!(!results.is_empty());
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
        
        let c = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 0.001);
        
        let d = vec![0.9, 0.1, 0.0];
        let sim = cosine_similarity(&a, &d);
        assert!(sim > 0.9 && sim < 1.0);
    }
}