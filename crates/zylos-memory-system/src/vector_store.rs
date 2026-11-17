//! VectorStore trait and implementations for zylos-memory-system
//!
//! Implements Task 2.1.2 of Fase 2: Rig-compatible VectorStore trait
//! with InMemory backend for testing and feature-gated adapters for
//! Qdrant and LanceDB.
//!
//! Target: <10ms p95 for search in 10k embeddings (per EVALUATION.md)

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::error::Result;

/// Unique identifier for a vector document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VectorId(Uuid);

impl VectorId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for VectorId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VectorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for VectorId {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Uuid::parse_str(s)
            .map(VectorId)
            .map_err(|e| format!("Invalid VectorId: {}", e))
    }
}

/// A document with an embedding vector and optional metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDocument {
    /// Unique identifier
    pub id: VectorId,
    /// Embedding vector (typically 384-1536 dimensions)
    pub embedding: Vec<f32>,
    /// Optional text content
    pub content: Option<String>,
    /// Optional metadata (JSON)
    pub metadata: Option<serde_json::Value>,
}

impl VectorDocument {
    pub fn new(embedding: Vec<f32>) -> Self {
        Self {
            id: VectorId::new(),
            embedding,
            content: None,
            metadata: None,
        }
    }

    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// Search result with similarity score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The matched document
    pub document: VectorDocument,
    /// Similarity score (0.0 to 1.0, higher = more similar)
    pub score: f64,
}

/// Search parameters for vector queries
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Query embedding vector
    pub query_embedding: Vec<f32>,
    /// Maximum number of results
    pub limit: usize,
    /// Minimum similarity threshold (0.0 to 1.0)
    pub min_score: Option<f64>,
    /// Optional metadata filter (key-value pairs that must match)
    pub metadata_filter: Option<HashMap<String, String>>,
}

impl SearchParams {
    pub fn new(query_embedding: Vec<f32>, limit: usize) -> Self {
        Self {
            query_embedding,
            limit,
            min_score: None,
            metadata_filter: None,
        }
    }

    pub fn with_min_score(mut self, score: f64) -> Self {
        self.min_score = Some(score);
        self
    }

    pub fn with_metadata_filter(mut self, filter: HashMap<String, String>) -> Self {
        self.metadata_filter = Some(filter);
        self
    }
}

/// Object-safe trait for vector storage backends
///
/// This trait is designed to be compatible with the Rig VectorStore interface
/// while supporting multiple backends (InMemory, Qdrant, LanceDB).
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Insert documents into the store
    async fn insert(&self, documents: Vec<VectorDocument>) -> Result<Vec<VectorId>>;

    /// Search for similar documents by embedding
    async fn search(&self, params: SearchParams) -> Result<Vec<SearchResult>>;

    /// Delete documents by ID
    async fn delete(&self, ids: &[VectorId]) -> Result<usize>;

    /// Count total documents in the store
    async fn count(&self) -> Result<usize>;

    /// Get a document by ID
    async fn get(&self, id: VectorId) -> Result<Option<VectorDocument>>;
}

// ============================================================================
// InMemoryVectorStore — For testing and small-scale usage
// ============================================================================

/// In-memory vector store using brute-force cosine similarity
///
/// Suitable for testing and datasets up to ~50k embeddings.
/// For larger datasets, use Qdrant or LanceDB backends.
pub struct InMemoryVectorStore {
    documents: Arc<RwLock<HashMap<VectorId, VectorDocument>>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn insert(&self, documents: Vec<VectorDocument>) -> Result<Vec<VectorId>> {
        let mut store = self.documents.write().await;
        let ids: Vec<VectorId> = documents.iter().map(|d| d.id).collect();
        for doc in documents {
            store.insert(doc.id, doc);
        }
        Ok(ids)
    }

    async fn search(&self, params: SearchParams) -> Result<Vec<SearchResult>> {
        let store = self.documents.read().await;
        let mut results: Vec<SearchResult> = store
            .values()
            .filter(|doc| {
                // Apply metadata filter if present
                if let Some(ref filter) = params.metadata_filter {
                    if let Some(ref metadata) = doc.metadata {
                        for (key, value) in filter {
                            match metadata.get(key) {
                                Some(v) if v.as_str() == Some(value) => continue,
                                _ => return false,
                            }
                        }
                    } else {
                        return false;
                    }
                }
                true
            })
            .filter_map(|doc| {
                let score = cosine_similarity(&params.query_embedding, &doc.embedding);
                if let Some(min_score) = params.min_score {
                    if score < min_score as f32 {
                        return None;
                    }
                }
                Some(SearchResult {
                    document: doc.clone(),
                    score: score as f64,
                })
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(params.limit);
        Ok(results)
    }

    async fn delete(&self, ids: &[VectorId]) -> Result<usize> {
        let mut store = self.documents.write().await;
        let mut deleted = 0;
        for id in ids {
            if store.remove(id).is_some() {
                deleted += 1;
            }
        }
        Ok(deleted)
    }

    async fn count(&self) -> Result<usize> {
        let store = self.documents.read().await;
        Ok(store.len())
    }

    async fn get(&self, id: VectorId) -> Result<Option<VectorDocument>> {
        let store = self.documents.read().await;
        Ok(store.get(&id).cloned())
    }
}

use crate::utils::cosine_similarity;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_count() {
        let store = InMemoryVectorStore::new();
        let docs = vec![
            VectorDocument::new(vec![1.0, 0.0, 0.0]),
            VectorDocument::new(vec![0.0, 1.0, 0.0]),
            VectorDocument::new(vec![0.0, 0.0, 1.0]),
        ];
        let ids = store.insert(docs).await.unwrap();
        assert_eq!(ids.len(), 3);
        assert_eq!(store.count().await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_search_cosine_similarity() {
        let store = InMemoryVectorStore::new();
        let docs = vec![
            VectorDocument::new(vec![1.0, 0.0, 0.0]).with_content("X axis"),
            VectorDocument::new(vec![0.0, 1.0, 0.0]).with_content("Y axis"),
            VectorDocument::new(vec![0.7, 0.7, 0.0]).with_content("Diagonal"),
        ];
        store.insert(docs).await.unwrap();

        // Search for vectors similar to X axis
        let results = store
            .search(SearchParams::new(vec![1.0, 0.0, 0.0], 3))
            .await
            .unwrap();

        assert!(!results.is_empty());
        // First result should be the exact match
        assert!((results[0].score - 1.0).abs() < 0.001);
        assert_eq!(results[0].document.content.as_deref(), Some("X axis"));
    }

    #[tokio::test]
    async fn test_search_with_min_score() {
        let store = InMemoryVectorStore::new();
        let docs = vec![
            VectorDocument::new(vec![1.0, 0.0, 0.0]),
            VectorDocument::new(vec![0.0, 1.0, 0.0]), // Orthogonal, score = 0
        ];
        store.insert(docs).await.unwrap();

        let results = store
            .search(SearchParams::new(vec![1.0, 0.0, 0.0], 10).with_min_score(0.5))
            .await
            .unwrap();

        // Only the similar vector should pass the threshold
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryVectorStore::new();
        let docs = vec![
            VectorDocument::new(vec![1.0, 0.0, 0.0]),
            VectorDocument::new(vec![0.0, 1.0, 0.0]),
        ];
        let ids = store.insert(docs).await.unwrap();

        let deleted = store.delete(&[ids[0]]).await.unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(store.count().await.unwrap(), 1);
        assert!(store.get(ids[0]).await.unwrap().is_none());
        assert!(store.get(ids[1]).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_get_document() {
        let store = InMemoryVectorStore::new();
        let doc = VectorDocument::new(vec![1.0, 2.0, 3.0])
            .with_content("test content")
            .with_metadata(serde_json::json!({"tag": "important"}));
        let ids = store.insert(vec![doc]).await.unwrap();

        let retrieved = store.get(ids[0]).await.unwrap().unwrap();
        assert_eq!(retrieved.embedding, vec![1.0, 2.0, 3.0]);
        assert_eq!(retrieved.content.as_deref(), Some("test content"));
        assert_eq!(
            retrieved
                .metadata
                .as_ref()
                .and_then(|m| m.get("tag").and_then(|v| v.as_str())),
            Some("important")
        );
    }

    #[tokio::test]
    async fn test_search_with_metadata_filter() {
        let store = InMemoryVectorStore::new();
        let docs = vec![
            VectorDocument::new(vec![1.0, 0.0, 0.0])
                .with_metadata(serde_json::json!({"category": "A"})),
            VectorDocument::new(vec![0.9, 0.1, 0.0])
                .with_metadata(serde_json::json!({"category": "B"})),
            VectorDocument::new(vec![0.8, 0.2, 0.0])
                .with_metadata(serde_json::json!({"category": "A"})),
        ];
        store.insert(docs).await.unwrap();

        let mut filter = HashMap::new();
        filter.insert("category".to_string(), "A".to_string());

        let results = store
            .search(SearchParams::new(vec![1.0, 0.0, 0.0], 10).with_metadata_filter(filter))
            .await
            .unwrap();

        // Should only return documents with category=A
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }
}
