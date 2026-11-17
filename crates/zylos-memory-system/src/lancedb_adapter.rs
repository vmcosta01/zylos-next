//! LanceDB vector store adapter for zylos-memory-system
//!
//! Implements the VectorStore trait using LanceDB as the backend.
//! Enabled via the `lancedb` feature flag.

use std::sync::Arc;

use arrow_array::Array;
use async_trait::async_trait;
use lancedb::connection::{ConnectBuilder, Connection};
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::table::Table;
use tokio_stream::StreamExt;

use crate::error::{MemoryError, Result};
use crate::vector_store::{SearchParams, SearchResult, VectorDocument, VectorId, VectorStore};

/// LanceDB-backed vector store adapter
pub struct LanceDbVectorStore {
    connection: Arc<Connection>,
    table_name: String,
}

impl LanceDbVectorStore {
    /// Create a new LanceDbVectorStore at the given URI (file path or s3://)
    pub async fn new(uri: &str, table_name: impl Into<String>) -> Result<Self> {
        let connection = ConnectBuilder::new(uri).execute().await.map_err(|e| {
            MemoryError::VectorStore(format!("Failed to connect to LanceDB: {}", e))
        })?;

        Ok(Self {
            connection: Arc::new(connection),
            table_name: table_name.into(),
        })
    }

    /// Ensure the table exists by creating it if needed with a sample schema
    pub async fn ensure_table(&self, dimension: usize) -> Result<()> {
        let table_names = self
            .connection
            .table_names()
            .execute()
            .await
            .map_err(|e| MemoryError::VectorStore(format!("Failed to list tables: {}", e)))?;

        if !table_names.iter().any(|n| n == &self.table_name) {
            use arrow_array::{ArrayRef, FixedSizeListArray, RecordBatch, StringArray};
            use arrow_schema::{DataType, Field, Schema};

            let field = Arc::new(Field::new("item", DataType::Float32, true));
            let schema = Arc::new(Schema::new(vec![
                Field::new("id", DataType::Utf8, false),
                Field::new(
                    "embedding",
                    DataType::FixedSizeList(field.clone(), dimension as i32),
                    false,
                ),
                Field::new("content", DataType::Utf8, true),
                Field::new("metadata", DataType::Utf8, true),
            ]));

            let empty_batch = RecordBatch::try_new(
                schema.clone(),
                vec![
                    Arc::new(StringArray::from(Vec::<&str>::new())) as ArrayRef,
                    Arc::new(FixedSizeListArray::new_null(field, dimension as i32, 0)) as ArrayRef,
                    Arc::new(StringArray::from(Vec::<Option<&str>>::new())) as ArrayRef,
                    Arc::new(StringArray::from(Vec::<Option<&str>>::new())) as ArrayRef,
                ],
            )
            .map_err(|e| {
                MemoryError::VectorStore(format!("Failed to create empty batch: {}", e))
            })?;

            let _ = schema;
            self.connection
                .create_table(&self.table_name, empty_batch)
                .execute()
                .await
                .map_err(|e| MemoryError::VectorStore(format!("Failed to create table: {}", e)))?;
        }

        Ok(())
    }

    async fn get_table(&self) -> Result<Table> {
        self.connection
            .open_table(&self.table_name)
            .execute()
            .await
            .map_err(|e| {
                MemoryError::VectorStore(format!(
                    "Failed to open table '{}': {}",
                    self.table_name, e
                ))
            })
    }
}

#[async_trait]
impl VectorStore for LanceDbVectorStore {
    async fn insert(&self, documents: Vec<VectorDocument>) -> Result<Vec<VectorId>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }

        use arrow_array::{ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
        use arrow_schema::{DataType, Field, Schema};

        let ids: Vec<VectorId> = documents.iter().map(|d| d.id).collect();
        let dim = documents[0].embedding.len() as i32;

        let id_values: Vec<String> = documents
            .iter()
            .map(|d| d.id.as_str().to_string())
            .collect();
        let content_values: Vec<Option<String>> =
            documents.iter().map(|d| d.content.clone()).collect();
        let metadata_values: Vec<Option<String>> = documents
            .iter()
            .map(|d| d.metadata.as_ref().map(|m| m.to_string()))
            .collect();

        // Build embedding arrays
        let mut embedding_data: Vec<f32> = Vec::with_capacity(documents.len() * dim as usize);
        for doc in &documents {
            embedding_data.extend_from_slice(&doc.embedding);
        }
        let float_array = Arc::new(Float32Array::from(embedding_data));
        let field = Arc::new(Field::new("item", DataType::Float32, true));
        let embedding_array =
            FixedSizeListArray::try_new(field, dim, float_array, None).map_err(|e| {
                MemoryError::VectorStore(format!("Failed to build embedding array: {}", e))
            })?;

        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new(
                "embedding",
                DataType::FixedSizeList(Arc::new(Field::new("item", DataType::Float32, true)), dim),
                false,
            ),
            Field::new("content", DataType::Utf8, true),
            Field::new("metadata", DataType::Utf8, true),
        ]));

        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(id_values)) as ArrayRef,
                Arc::new(embedding_array) as ArrayRef,
                Arc::new(StringArray::from(content_values)) as ArrayRef,
                Arc::new(StringArray::from(metadata_values)) as ArrayRef,
            ],
        )
        .map_err(|e| MemoryError::VectorStore(format!("Failed to create record batch: {}", e)))?;

        let _ = schema;
        let table = self.get_table().await?;
        table
            .add(batch)
            .execute()
            .await
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB insert failed: {}", e)))?;

        Ok(ids)
    }

    async fn search(&self, params: SearchParams) -> Result<Vec<SearchResult>> {
        let table = self.get_table().await?;

        let query_vec = params.query_embedding;
        let search = table
            .vector_search(query_vec)
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB search setup failed: {}", e)))?
            .limit(params.limit);

        if let Some(min_score) = params.min_score {
            let _ = min_score; // Threshold filtering applied post-search below
        }

        let results = search
            .execute()
            .await
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB search failed: {}", e)))?;

        use arrow_array::cast::AsArray;

        let mut search_results: Vec<SearchResult> = Vec::new();

        use arrow_array::RecordBatch;
        let mut batches: Vec<RecordBatch> = Vec::new();
        let mut stream = results;
        while let Some(batch_result) = stream.next().await {
            if let Ok(batch) = batch_result {
                batches.push(batch);
            }
        }

        for batch in &batches {
            let batch: &RecordBatch = batch;
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_string_opt::<i32>())
                .ok_or_else(|| {
                    MemoryError::VectorStore("Missing 'id' column in results".to_string())
                })?;

            let score_col = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_primitive_opt::<arrow_array::types::Float32Type>())
                .ok_or_else(|| {
                    MemoryError::VectorStore("Missing '_distance' column in results".to_string())
                })?;

            let content_col = batch
                .column_by_name("content")
                .and_then(|c| c.as_string_opt::<i32>());
            let metadata_col = batch
                .column_by_name("metadata")
                .and_then(|c| c.as_string_opt::<i32>());

            let embedding_col = batch.column_by_name("embedding");

            for i in 0..batch.num_rows() {
                let id_str = id_col.value(i);
                let distance = score_col.value(i);
                let score = 1.0 - distance as f64;

                if let Some(min_score) = params.min_score {
                    if score < min_score {
                        continue;
                    }
                }

                let vector_id: VectorId = id_str
                    .parse()
                    .map_err(|e: String| MemoryError::VectorStore(e))?;

                let content = content_col.and_then(|c| {
                    if c.is_null(i) {
                        None
                    } else {
                        Some(c.value(i).to_string())
                    }
                });

                let metadata: Option<serde_json::Value> = metadata_col.and_then(|c| {
                    if c.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(c.value(i)).ok()
                    }
                });

                let embedding = if let Some(col) = embedding_col {
                    if let Some(fsl) = col.as_fixed_size_list_opt() {
                        if !fsl.is_null(i) {
                            let list = fsl.value(i);
                            if let Some(floats) =
                                list.as_primitive_opt::<arrow_array::types::Float32Type>()
                            {
                                floats.values().to_vec()
                            } else {
                                vec![]
                            }
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

                if let Some(ref filter) = params.metadata_filter {
                    if let Some(ref meta) = metadata {
                        let mut matches = true;
                        for (key, value) in filter {
                            match meta.get(key) {
                                Some(v) if v.as_str() == Some(value) => {}
                                _ => {
                                    matches = false;
                                    break;
                                }
                            }
                        }
                        if !matches {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }

                search_results.push(SearchResult {
                    document: VectorDocument {
                        id: vector_id,
                        embedding,
                        content,
                        metadata,
                    },
                    score,
                });
            }
        }

        search_results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        search_results.truncate(params.limit);

        Ok(search_results)
    }

    async fn delete(&self, ids: &[VectorId]) -> Result<usize> {
        if ids.is_empty() {
            return Ok(0);
        }

        let table = self.get_table().await?;
        let id_list: Vec<String> = ids.iter().map(|id| format!("'{}'", id.as_str())).collect();
        let filter = format!("id IN ({})", id_list.join(", "));

        table
            .delete(&filter)
            .await
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB delete failed: {}", e)))?;

        Ok(ids.len())
    }

    async fn count(&self) -> Result<usize> {
        let table = self.get_table().await?;
        let count = table
            .count_rows(None)
            .await
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB count failed: {}", e)))?;
        Ok(count)
    }

    async fn get(&self, id: VectorId) -> Result<Option<VectorDocument>> {
        let table = self.get_table().await?;
        let filter = format!("id = '{}'", id.as_str());

        let results = table
            .query()
            .only_if(filter)
            .limit(1)
            .execute()
            .await
            .map_err(|e| MemoryError::VectorStore(format!("LanceDB get failed: {}", e)))?;

        use arrow_array::cast::AsArray;
        use arrow_array::RecordBatch;
        let mut batches: Vec<RecordBatch> = Vec::new();
        let mut stream = results;
        while let Some(batch_result) = stream.next().await {
            if let Ok(batch) = batch_result {
                batches.push(batch);
            }
        }

        if let Some(batch) = batches.first() {
            let batch: &RecordBatch = batch;
            if batch.num_rows() == 0 {
                return Ok(None);
            }

            let _id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_string_opt::<i32>())
                .ok_or_else(|| MemoryError::VectorStore("Missing 'id' column".to_string()))?;

            let content_col = batch
                .column_by_name("content")
                .and_then(|c| c.as_string_opt::<i32>());
            let metadata_col = batch
                .column_by_name("metadata")
                .and_then(|c| c.as_string_opt::<i32>());
            let embedding_col = batch.column_by_name("embedding");

            let content = content_col.and_then(|c| {
                if c.is_null(0) {
                    None
                } else {
                    Some(c.value(0).to_string())
                }
            });

            let metadata = metadata_col.and_then(|c| {
                if c.is_null(0) {
                    None
                } else {
                    serde_json::from_str(c.value(0)).ok()
                }
            });

            let embedding = if let Some(col) = embedding_col {
                if let Some(fsl) = col.as_fixed_size_list_opt() {
                    if !fsl.is_null(0) {
                        let list = fsl.value(0);
                        if let Some(floats) =
                            list.as_primitive_opt::<arrow_array::types::Float32Type>()
                        {
                            floats.values().to_vec()
                        } else {
                            vec![]
                        }
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                }
            } else {
                vec![]
            };

            return Ok(Some(VectorDocument {
                id,
                embedding,
                content,
                metadata,
            }));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _assert_vector_store_impl<T: VectorStore>() {}

    #[test]
    fn test_lancedb_implements_vector_store() {
        _assert_vector_store_impl::<LanceDbVectorStore>();
    }
}
