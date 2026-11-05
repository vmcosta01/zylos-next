//! Qdrant vector store adapter for zylos-memory-system
//!
//! Implements the VectorStore trait using Qdrant as the backend.
//! Enabled via the `qdrant` feature flag.

#![allow(deprecated)]

use std::collections::HashMap;

use async_trait::async_trait;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance, Filter,
    GetPointsBuilder, PointStruct, SearchPointsBuilder, UpsertPointsBuilder, VectorParams,
    VectorsConfig,
};
use qdrant_client::Qdrant;

use crate::error::{MemoryError, Result};
use crate::vector_store::{SearchParams, SearchResult, VectorDocument, VectorId, VectorStore};

/// Qdrant-backed vector store adapter
pub struct QdrantVectorStore {
    client: Qdrant,
    collection_name: String,
}

impl QdrantVectorStore {
    /// Create a new QdrantVectorStore connected to the given URL
    pub async fn new(url: &str, collection_name: impl Into<String>) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| MemoryError::VectorStore(format!("Failed to create Qdrant client: {}", e)))?;

        Ok(Self {
            client,
            collection_name: collection_name.into(),
        })
    }

    /// Ensure the collection exists with the given vector dimension
    pub async fn ensure_collection(&self, dimension: u64) -> Result<()> {
        let exists = self
            .client
            .collection_exists(&self.collection_name)
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Failed to check collection: {}", e)))?;

        if !exists {
            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection_name)
                        .vectors_config(VectorsConfig {
                            config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                                VectorParams {
                                    size: dimension,
                                    distance: Distance::Cosine.into(),
                                    ..Default::default()
                                },
                            )),
                        }),
                )
                .await
                .map_err(|e| {
                    MemoryError::VectorStore(format!("Failed to create collection: {}", e))
                })?;
        }

        Ok(())
    }

    fn doc_to_point(doc: &VectorDocument) -> Result<PointStruct> {
        let id_str = doc.id.as_str().to_string();
        let mut payload: HashMap<String, qdrant_client::qdrant::Value> = HashMap::new();

        if let Some(ref content) = doc.content {
            payload.insert("content".to_string(), content.clone().into());
        }

        if let Some(ref metadata) = doc.metadata {
            let meta_str = serde_json::to_string(metadata)
                .map_err(MemoryError::Serialization)?;
            payload.insert("metadata".to_string(), meta_str.into());
        }

        Ok(PointStruct::new(id_str, doc.embedding.clone(), payload))
    }

    fn scored_point_to_result(
        scored: &qdrant_client::qdrant::ScoredPoint,
    ) -> Result<SearchResult> {
        let id = scored
            .id
            .as_ref()
            .and_then(|pid| match &pid.point_id_options {
                Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s)) => Some(s.clone()),
                _ => None,
            })
            .ok_or_else(|| MemoryError::VectorStore("Missing point ID".to_string()))?;

        let vector_id: VectorId = id
            .parse()
            .map_err(|e: String| MemoryError::VectorStore(e))?;

        let embedding = scored
            .vectors
            .as_ref()
            .and_then(|v| match &v.vectors_options {
                Some(qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(vec)) => {
                    Some(vec.data.clone())
                }
                _ => None,
            })
            .unwrap_or_default();

        let content = scored
            .payload
            .get("content")
            .and_then(|v| match &v.kind {
                Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            });

        let metadata = scored
            .payload
            .get("metadata")
            .and_then(|v| match &v.kind {
                Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                    serde_json::from_str(s).ok()
                }
                _ => None,
            });

        Ok(SearchResult {
            document: VectorDocument {
                id: vector_id,
                embedding,
                content,
                metadata,
            },
            score: scored.score as f64,
        })
    }

    fn build_filter(params: &SearchParams) -> Option<Filter> {
        params.metadata_filter.as_ref().map(|filter| {
            let conditions: Vec<Condition> = filter
                .iter()
                .map(|(key, value)| {
                    Condition::matches(format!("metadata_{}", key), value.clone())
                })
                .collect();
            Filter {
                must: conditions,
                ..Default::default()
            }
        })
    }
}

#[async_trait]
impl VectorStore for QdrantVectorStore {
    async fn insert(&self, documents: Vec<VectorDocument>) -> Result<Vec<VectorId>> {
        let ids: Vec<VectorId> = documents.iter().map(|d| d.id).collect();
        let points: Vec<PointStruct> = documents
            .iter()
            .map(Self::doc_to_point)
            .collect::<Result<Vec<_>>>()?;

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Qdrant upsert failed: {}", e)))?;

        Ok(ids)
    }

    async fn search(&self, params: SearchParams) -> Result<Vec<SearchResult>> {
        let filter = Self::build_filter(&params);

        let mut request = SearchPointsBuilder::new(&self.collection_name, params.query_embedding, params.limit as u64)
            .with_payload(true)
            .with_vectors(true);

        if let Some(f) = filter {
            request = request.filter(f);
        }

        if let Some(min_score) = params.min_score {
            request = request.score_threshold(min_score as f32);
        }

        let response = self
            .client
            .search_points(request)
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Qdrant search failed: {}", e)))?;

        response
            .result
            .iter()
            .map(Self::scored_point_to_result)
            .collect()
    }

    async fn delete(&self, ids: &[VectorId]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let point_ids: Vec<qdrant_client::qdrant::PointId> = ids
            .iter()
            .map(|id| qdrant_client::qdrant::PointId {
                point_id_options: Some(
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(id.as_str().to_string()),
                ),
            })
            .collect();

        self.client
            .delete_points(
                DeletePointsBuilder::new(&self.collection_name).points(point_ids),
            )
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Qdrant delete failed: {}", e)))?;

        Ok(ids.len())
    }

    async fn count(&self) -> Result<usize> {
        let info = self
            .client
            .collection_info(&self.collection_name)
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Qdrant count failed: {}", e)))?;

        Ok(info.result.and_then(|r| r.points_count).unwrap_or(0) as usize)
    }

    async fn get(&self, id: VectorId) -> Result<Option<VectorDocument>> {
        let point_id = qdrant_client::qdrant::PointId {
            point_id_options: Some(
                qdrant_client::qdrant::point_id::PointIdOptions::Uuid(id.as_str().to_string()),
            ),
        };

        let response = self
            .client
            .get_points(
                GetPointsBuilder::new(&self.collection_name, vec![point_id])
                    .with_payload(true)
                    .with_vectors(true),
            )
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Qdrant get failed: {}", e)))?;

        if response.result.is_empty() {
            return Ok(None);
        }

        let retrieved = &response.result[0];

        let embedding = retrieved
            .vectors
            .as_ref()
            .and_then(|v| match &v.vectors_options {
                Some(qdrant_client::qdrant::vectors_output::VectorsOptions::Vector(vec)) => {
                    Some(vec.data.clone())
                }
                _ => None,
            })
            .unwrap_or_default();

        let content = retrieved
            .payload
            .get("content")
            .and_then(|v| match &v.kind {
                Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => Some(s.clone()),
                _ => None,
            });

        let metadata = retrieved
            .payload
            .get("metadata")
            .and_then(|v| match &v.kind {
                Some(qdrant_client::qdrant::value::Kind::StringValue(s)) => {
                    serde_json::from_str(s).ok()
                }
                _ => None,
            });

        Ok(Some(VectorDocument {
            id,
            embedding,
            content,
            metadata,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_vector_store_impl<T: VectorStore>() {}

    #[test]
    fn test_qdrant_implements_vector_store() {
        _assert_vector_store_impl::<QdrantVectorStore>();
    }

    #[test]
    fn test_doc_to_point_roundtrip() {
        let doc = VectorDocument::new(vec![1.0, 2.0, 3.0])
            .with_content("test")
            .with_metadata(serde_json::json!({"key": "value"}));

        let point = QdrantVectorStore::doc_to_point(&doc).unwrap();
        assert!(point.id.is_some());
        assert_eq!(point.payload.len(), 2);
    }
}
