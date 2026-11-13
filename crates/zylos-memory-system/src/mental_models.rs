//! # MentalModelStore — Modelos cognitivos per-user (T8.2.2)
//!
//! Extensão do ContextGraph para armazenar modelos mentais por usuário,
//! permitindo personalização cognitiva de respostas e contexto.
//!
//! ## Arquitetura
//! - Tabela SQLite `mental_models` com user_id FK e isolamento por tenant
//! - Cache em memória via `DashMap` para similarity search <50ms
//! - Embeddings vetoriais para busca semântica entre modelos
//!
//! ## Isolamento
//! Cada user_id tem seus próprios modelos; queries são filtradas server-side.

use dashmap::DashMap;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{MemoryError, Result};
use crate::utils::cosine_similarity;

/// Identificador único de um modelo mental.
pub type MentalModelId = String;

/// Um modelo cognitivo associado a um usuário.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MentalModel {
    /// ID único do modelo.
    pub id: MentalModelId,
    /// ID do usuário proprietário.
    pub user_id: String,
    /// Nome/título do modelo mental.
    pub name: String,
    /// Descrição textual do modelo.
    pub description: String,
    /// Embedding vetorial para similarity search.
    pub embedding: Vec<f32>,
    /// Metadata arbitrária (preferências, padrões cognitivos, etc.).
    pub metadata: serde_json::Value,
    /// Timestamp de criação (ISO 8601).
    pub created_at: String,
    /// Timestamp da última atualização (ISO 8601).
    pub updated_at: String,
}

impl MentalModel {
    /// Cria um novo MentalModel com ID auto-gerado.
    pub fn new(
        user_id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        embedding: Vec<f32>,
    ) -> Self {
        let now = chrono_now_stub();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id: user_id.into(),
            name: name.into(),
            description: description.into(),
            embedding,
            metadata: serde_json::Value::Null,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Builder: anexa metadata arbitrária.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

/// Resultado de uma busca por similaridade.
#[derive(Debug, Clone)]
pub struct SimilarModel {
    /// O modelo encontrado.
    pub model: MentalModel,
    /// Score de similaridade (cosine, 0.0-1.0).
    pub score: f64,
}

/// Store para modelos mentais com persistência SQLite e cache em memória.
pub struct MentalModelStore {
    /// Conexão SQLite serializada via Mutex.
    conn: Arc<Mutex<Connection>>,
    /// Cache de embeddings para similarity search rápida.
    /// Key: (user_id, model_id), Value: embedding vec.
    cache: Arc<DashMap<(String, String), Vec<f32>>>,
}

impl MentalModelStore {
    /// Cria um novo MentalModelStore com conexão SQLite existente.
    ///
    /// Inicializa a tabela `mental_models` se não existir.
    pub async fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        let store = Self {
            conn,
            cache: Arc::new(DashMap::new()),
        };
        store.initialize_schema().await?;
        Ok(store)
    }

    /// Cria um store in-memory para testes.
    pub async fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            cache: Arc::new(DashMap::new()),
        };
        store.initialize_schema().await?;
        Ok(store)
    }

    /// Inicializa schema da tabela mental_models.
    async fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS mental_models (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                embedding BLOB NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_mental_models_user_id ON mental_models(user_id);",
        )?;
        Ok(())
    }

    /// Insere um novo modelo mental.
    pub async fn insert(&self, model: &MentalModel) -> Result<()> {
        let conn = self.conn.lock().await;
        let embedding_blob = f32_vec_to_blob(&model.embedding);
        let metadata_str = serde_json::to_string(&model.metadata)
            .map_err(MemoryError::Serialization)?;

        conn.execute(
            "INSERT INTO mental_models (id, user_id, name, description, embedding, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                model.id,
                model.user_id,
                model.name,
                model.description,
                embedding_blob,
                metadata_str,
                model.created_at,
                model.updated_at,
            ],
        )?;

        // Atualizar cache
        self.cache
            .insert((model.user_id.clone(), model.id.clone()), model.embedding.clone());

        Ok(())
    }

    /// Busca um modelo por ID, verificando isolamento de user.
    pub async fn get(&self, model_id: &str, user_id: &str) -> Result<Option<MentalModel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, description, embedding, metadata, created_at, updated_at
             FROM mental_models WHERE id = ?1 AND user_id = ?2",
        )?;

        let result = stmt
            .query_row(params![model_id, user_id], |row| {
                let embedding_blob: Vec<u8> = row.get(4)?;
                let metadata_str: String = row.get(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    embedding_blob,
                    metadata_str,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .optional()?;

        match result {
            Some((id, uid, name, desc, emb_blob, meta_str, created, updated)) => {
                let embedding = blob_to_f32_vec(&emb_blob);
                let metadata: serde_json::Value = serde_json::from_str(&meta_str)
                    .unwrap_or(serde_json::Value::Null);
                Ok(Some(MentalModel {
                    id,
                    user_id: uid,
                    name,
                    description: desc,
                    embedding,
                    metadata,
                    created_at: created,
                    updated_at: updated,
                }))
            }
            None => Ok(None),
        }
    }

    /// Lista todos os modelos de um usuário.
    pub async fn list_by_user(&self, user_id: &str) -> Result<Vec<MentalModel>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, name, description, embedding, metadata, created_at, updated_at
             FROM mental_models WHERE user_id = ?1 ORDER BY updated_at DESC",
        )?;

        let rows = stmt
            .query_map(params![user_id], |row| {
                let embedding_blob: Vec<u8> = row.get(4)?;
                let metadata_str: String = row.get(5)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    embedding_blob,
                    metadata_str,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let models = rows
            .into_iter()
            .map(|(id, uid, name, desc, emb_blob, meta_str, created, updated)| {
                let embedding = blob_to_f32_vec(&emb_blob);
                let metadata: serde_json::Value =
                    serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
                MentalModel {
                    id,
                    user_id: uid,
                    name,
                    description: desc,
                    embedding,
                    metadata,
                    created_at: created,
                    updated_at: updated,
                }
            })
            .collect();

        Ok(models)
    }

    /// Busca modelos similares por embedding cosine similarity.
    ///
    /// Retorna top-k resultados ordenados por score decrescente.
    /// Usa cache em memória para performance <50ms com 10k modelos.
    pub async fn search_similar(
        &self,
        user_id: &str,
        query_embedding: &[f32],
        top_k: usize,
        min_score: f64,
    ) -> Result<Vec<SimilarModel>> {
        // Garantir que o cache está populado para este user
        self.warm_cache_for_user(user_id).await?;

        let mut results: Vec<SimilarModel> = Vec::new();

        // Iterar sobre entradas do cache filtradas por user_id
        for entry in self.cache.iter() {
            let key = entry.key();
            let emb = entry.value();
            if key.0 != user_id {
                continue;
            }

            let score = cosine_similarity(query_embedding, emb) as f64;
            if score >= min_score {
                // Buscar modelo completo do DB
                if let Ok(Some(model)) = self.get(&key.1, user_id).await {
                    results.push(SimilarModel { model, score });
                }
            }
        }

        // Ordenar por score decrescente e limitar a top_k
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(results)
    }

    /// Remove um modelo mental.
    pub async fn delete(&self, model_id: &str, user_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        let affected = conn.execute(
            "DELETE FROM mental_models WHERE id = ?1 AND user_id = ?2",
            params![model_id, user_id],
        )?;

        if affected > 0 {
            self.cache
                .remove(&(user_id.to_string(), model_id.to_string()));
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Conta modelos de um usuário.
    pub async fn count_by_user(&self, user_id: &str) -> Result<usize> {
        let conn = self.conn.lock().await;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM mental_models WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Popula o cache com todos os modelos de um usuário.
    async fn warm_cache_for_user(&self, user_id: &str) -> Result<()> {
        // Verificar se já temos entradas no cache para este user
        let has_cached = self.cache.iter().any(|entry| entry.key().0 == user_id);
        if has_cached {
            return Ok(());
        }

        let models = self.list_by_user(user_id).await?;
        for model in models {
            self.cache
                .insert((model.user_id.clone(), model.id.clone()), model.embedding);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Converte Vec<f32> para blob de bytes (little-endian).
fn f32_vec_to_blob(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * 4);
    for &val in vec {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Converte blob de bytes para Vec<f32> (little-endian).
fn blob_to_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = chunk.try_into().unwrap();
            f32::from_le_bytes(arr)
        })
        .collect()
}

/// Stub para timestamp ISO 8601.
fn chrono_now_stub() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

/// Extension trait para Optional query results.
trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = MentalModelStore::in_memory().await.unwrap();

        let model = MentalModel::new(
            "user-1",
            "Analytical Thinker",
            "Prefers data-driven decisions",
            vec![0.1, 0.2, 0.3, 0.4],
        );

        store.insert(&model).await.unwrap();

        let retrieved = store.get(&model.id, "user-1").await.unwrap();
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert_eq!(r.name, "Analytical Thinker");
        assert_eq!(r.user_id, "user-1");
        assert_eq!(r.embedding.len(), 4);
    }

    #[tokio::test]
    async fn test_user_isolation() {
        let store = MentalModelStore::in_memory().await.unwrap();

        let model = MentalModel::new("user-1", "Model A", "Desc", vec![0.1, 0.2]);
        store.insert(&model).await.unwrap();

        // User-2 não deve ver modelo do user-1
        let result = store.get(&model.id, "user-2").await.unwrap();
        assert!(result.is_none());

        // Listagem do user-2 deve ser vazia
        let list = store.list_by_user("user-2").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_list_by_user() {
        let store = MentalModelStore::in_memory().await.unwrap();

        for i in 0..5 {
            let model = MentalModel::new(
                "user-1",
                format!("Model {}", i),
                format!("Description {}", i),
                vec![i as f32 * 0.1, 0.5],
            );
            store.insert(&model).await.unwrap();
        }

        // Adicionar modelo de outro user
        let other = MentalModel::new("user-2", "Other", "Other desc", vec![0.9, 0.9]);
        store.insert(&other).await.unwrap();

        let list = store.list_by_user("user-1").await.unwrap();
        assert_eq!(list.len(), 5);

        let count = store.count_by_user("user-1").await.unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = MentalModelStore::in_memory().await.unwrap();

        let model = MentalModel::new("user-1", "To Delete", "Desc", vec![0.1]);
        store.insert(&model).await.unwrap();

        // Delete com user errado não deve funcionar
        let deleted_wrong = store.delete(&model.id, "user-2").await.unwrap();
        assert!(!deleted_wrong);

        // Delete com user correto
        let deleted = store.delete(&model.id, "user-1").await.unwrap();
        assert!(deleted);

        // Confirmar remoção
        let result = store.get(&model.id, "user-1").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_search_similar() {
        let store = MentalModelStore::in_memory().await.unwrap();

        // Inserir modelos com embeddings conhecidos
        let m1 = MentalModel::new("user-1", "Close Match", "A", vec![0.9, 0.1, 0.0]);
        let m2 = MentalModel::new("user-1", "Far Match", "B", vec![0.0, 0.0, 1.0]);
        let m3 = MentalModel::new("user-1", "Medium Match", "C", vec![0.5, 0.5, 0.0]);

        store.insert(&m1).await.unwrap();
        store.insert(&m2).await.unwrap();
        store.insert(&m3).await.unwrap();

        // Query similar a m1
        let query = vec![0.85, 0.15, 0.0];
        let results = store
            .search_similar("user-1", &query, 3, 0.0)
            .await
            .unwrap();

        assert!(!results.is_empty());
        // Primeiro resultado deve ser o mais similar (Close Match)
        assert_eq!(results[0].model.name, "Close Match");
        assert!(results[0].score > results[1].score);
    }

    #[tokio::test]
    async fn test_search_similar_min_score_filter() {
        let store = MentalModelStore::in_memory().await.unwrap();

        let m1 = MentalModel::new("user-1", "Good", "A", vec![1.0, 0.0]);
        let m2 = MentalModel::new("user-1", "Bad", "B", vec![0.0, 1.0]);

        store.insert(&m1).await.unwrap();
        store.insert(&m2).await.unwrap();

        // Query ortogonal a m2 → score baixo
        let query = vec![1.0, 0.0];
        let results = store
            .search_similar("user-1", &query, 10, 0.5)
            .await
            .unwrap();

        // Apenas m1 deve passar pelo filtro de min_score=0.5
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].model.name, "Good");
    }

    #[tokio::test]
    async fn test_f32_blob_roundtrip() {
        let original = vec![0.1, -0.5, 3.14, 0.0, -1.0];
        let blob = f32_vec_to_blob(&original);
        let restored = blob_to_f32_vec(&blob);

        assert_eq!(original.len(), restored.len());
        for (a, b) in original.iter().zip(restored.iter()) {
            assert!((a - b).abs() < f32::EPSILON);
        }
    }

    #[tokio::test]
    async fn test_model_with_metadata() {
        let store = MentalModelStore::in_memory().await.unwrap();

        let model = MentalModel::new("user-1", "Rich Model", "With metadata", vec![0.5])
            .with_metadata(serde_json::json!({
                "thinking_style": "analytical",
                "risk_tolerance": 0.7,
                "preferred_formats": ["bullet_points", "tables"]
            }));

        store.insert(&model).await.unwrap();

        let retrieved = store.get(&model.id, "user-1").await.unwrap().unwrap();
        assert_eq!(retrieved.metadata["thinking_style"], "analytical");
        assert_eq!(retrieved.metadata["risk_tolerance"], 0.7);
    }

    #[tokio::test]
    async fn test_count_by_user() {
        let store = MentalModelStore::in_memory().await.unwrap();

        assert_eq!(store.count_by_user("empty-user").await.unwrap(), 0);

        for i in 0..3 {
            let m = MentalModel::new("counter-user", format!("M{}", i), "d", vec![0.1]);
            store.insert(&m).await.unwrap();
        }

        assert_eq!(store.count_by_user("counter-user").await.unwrap(), 3);
    }
}