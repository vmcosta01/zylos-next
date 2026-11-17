//! ModelConfigStore — persistência SQLite para Multi-Model Selector
//!
//! Suporta hierarquia de resolução: bot-specific > global default > hardcoded fallback.
//! Todas as operações são async via sqlx com runtime tokio.

use crate::schema::*;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Store para operações CRUD de configurações de modelo
#[derive(Clone)]
pub struct ModelConfigStore {
    pool: SqlitePool,
}

impl ModelConfigStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Cria ou atualiza config de modelo para um bot (ou "__global__")
    pub async fn upsert(
        &self,
        bot_id: &str,
        config: UpsertModelConfig,
    ) -> Result<String, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let tier = config
            .tier_strategy
            .unwrap_or_else(|| TierStrategy::default().as_str().to_string());
        let effort = config
            .reasoning_effort
            .unwrap_or_else(|| ReasoningEffort::default().as_str().to_string());
        let is_default = config.is_default.unwrap_or(false);

        // Verificar se já existe config para este bot
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM model_configs WHERE bot_id = ?")
                .bind(bot_id)
                .fetch_optional(&self.pool)
                .await?;

        let id = if let Some((existing_id,)) = existing {
            // Atualizar existente
            sqlx::query(
                "UPDATE model_configs SET provider = ?, model_id = ?, tier_strategy = ?, reasoning_effort = ?, max_tokens = ?, temperature = ?, is_default = ?, updated_at = ? WHERE id = ?"
            )
            .bind(&config.provider)
            .bind(&config.model_id)
            .bind(&tier)
            .bind(&effort)
            .bind(config.max_tokens)
            .bind(config.temperature)
            .bind(is_default)
            .bind(&now)
            .bind(&existing_id)
            .execute(&self.pool)
            .await?;

            tracing::info!(bot_id = %bot_id, model = %config.model_id, "Model config updated");
            existing_id
        } else {
            // Criar nova
            let new_id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO model_configs (id, bot_id, provider, model_id, tier_strategy, reasoning_effort, max_tokens, temperature, is_default, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&new_id)
            .bind(bot_id)
            .bind(&config.provider)
            .bind(&config.model_id)
            .bind(&tier)
            .bind(&effort)
            .bind(config.max_tokens)
            .bind(config.temperature)
            .bind(is_default)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;

            tracing::info!(bot_id = %bot_id, model = %config.model_id, "Model config created");
            new_id
        };

        Ok(id)
    }

    /// Busca config específica de um bot (sem herança)
    pub async fn get_bot_config(&self, bot_id: &str) -> Result<Option<ModelConfig>, sqlx::Error> {
        sqlx::query_as::<_, ModelConfig>("SELECT * FROM model_configs WHERE bot_id = ?")
            .bind(bot_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Resolve config efetiva com herança: bot > global > default hardcoded
    pub async fn resolve(&self, bot_id: &str) -> ResolvedModelConfig {
        // Tentar config específica do bot
        if let Ok(Some(config)) = self.get_bot_config(bot_id).await {
            return ResolvedModelConfig {
                provider: config.provider,
                model_id: config.model_id,
                tier_strategy: config.tier_strategy,
                reasoning_effort: config.reasoning_effort,
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                source: "bot".into(),
            };
        }

        // Tentar config global
        if let Ok(Some(config)) = self.get_bot_config("__global__").await {
            return ResolvedModelConfig {
                provider: config.provider,
                model_id: config.model_id,
                tier_strategy: config.tier_strategy,
                reasoning_effort: config.reasoning_effort,
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                source: "global".into(),
            };
        }

        // Fallback hardcoded
        ResolvedModelConfig {
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            tier_strategy: TierStrategy::Balanced.as_str().to_string(),
            reasoning_effort: ReasoningEffort::Medium.as_str().to_string(),
            max_tokens: Some(4096),
            temperature: Some(0.7),
            source: "default".into(),
        }
    }

    /// Remove config específica de um bot (volta a usar global/default)
    pub async fn delete_bot_config(&self, bot_id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM model_configs WHERE bot_id = ?")
            .bind(bot_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Lista todas as configs
    pub async fn list_all(&self) -> Result<Vec<ModelConfig>, sqlx::Error> {
        sqlx::query_as::<_, ModelConfig>("SELECT * FROM model_configs ORDER BY bot_id, created_at")
            .fetch_all(&self.pool)
            .await
    }

    /// Aplica migrations SQL (statements individuais para sqlx SQLite)
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_configs (
                id TEXT PRIMARY KEY,
                bot_id TEXT NOT NULL UNIQUE,
                provider TEXT NOT NULL,
                model_id TEXT NOT NULL,
                tier_strategy TEXT NOT NULL DEFAULT 'balanced',
                reasoning_effort TEXT NOT NULL DEFAULT 'medium',
                max_tokens INTEGER,
                temperature REAL,
                is_default BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_model_bot ON model_configs(bot_id)")
            .execute(&self.pool)
            .await?;

        tracing::info!("Model config migrations applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_store() -> ModelConfigStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = ModelConfigStore::new(pool);
        store.run_migrations().await.unwrap();
        store
    }

    fn sample_config() -> UpsertModelConfig {
        UpsertModelConfig {
            provider: "openai".into(),
            model_id: "gpt-4o".into(),
            tier_strategy: Some("balanced".into()),
            reasoning_effort: Some("high".into()),
            max_tokens: Some(8192),
            temperature: Some(0.7),
            is_default: Some(true),
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let store = setup_test_store().await;
        let id = store.upsert("bot-1", sample_config()).await.unwrap();

        let config = store.get_bot_config("bot-1").await.unwrap().unwrap();
        assert_eq!(config.id, id);
        assert_eq!(config.provider, "openai");
        assert_eq!(config.model_id, "gpt-4o");
        assert_eq!(config.tier_strategy, "balanced");
        assert_eq!(config.reasoning_effort, "high");
        assert_eq!(config.max_tokens, Some(8192));
    }

    #[tokio::test]
    async fn test_upsert_updates_existing() {
        let store = setup_test_store().await;
        let _id = store.upsert("bot-1", sample_config()).await.unwrap();

        let updated = UpsertModelConfig {
            provider: "anthropic".into(),
            model_id: "claude-3.5-sonnet".into(),
            tier_strategy: Some("smartest".into()),
            reasoning_effort: None, // Deve usar default
            max_tokens: None,
            temperature: None,
            is_default: None,
        };
        store.upsert("bot-1", updated).await.unwrap();

        let config = store.get_bot_config("bot-1").await.unwrap().unwrap();
        assert_eq!(config.provider, "anthropic");
        assert_eq!(config.model_id, "claude-3.5-sonnet");
        assert_eq!(config.tier_strategy, "smartest");
        assert_eq!(config.reasoning_effort, "medium"); // Default
    }

    #[tokio::test]
    async fn test_resolve_bot_specific() {
        let store = setup_test_store().await;

        // Config global
        let global = UpsertModelConfig {
            provider: "openai".into(),
            model_id: "gpt-4o-mini".into(),
            ..sample_config()
        };
        store.upsert("__global__", global).await.unwrap();

        // Config específica do bot
        store.upsert("bot-1", sample_config()).await.unwrap();

        let resolved = store.resolve("bot-1").await;
        assert_eq!(resolved.source, "bot");
        assert_eq!(resolved.model_id, "gpt-4o");
    }

    #[tokio::test]
    async fn test_resolve_falls_back_to_global() {
        let store = setup_test_store().await;

        let global = UpsertModelConfig {
            provider: "anthropic".into(),
            model_id: "claude-3-haiku".into(),
            ..sample_config()
        };
        store.upsert("__global__", global).await.unwrap();

        let resolved = store.resolve("bot-unknown").await;
        assert_eq!(resolved.source, "global");
        assert_eq!(resolved.model_id, "claude-3-haiku");
    }

    #[tokio::test]
    async fn test_resolve_falls_back_to_default() {
        let store = setup_test_store().await;

        let resolved = store.resolve("bot-unknown").await;
        assert_eq!(resolved.source, "default");
        assert_eq!(resolved.model_id, "gpt-4o-mini");
        assert_eq!(resolved.provider, "openai");
    }

    #[tokio::test]
    async fn test_delete_bot_config() {
        let store = setup_test_store().await;
        store.upsert("bot-1", sample_config()).await.unwrap();

        let deleted = store.delete_bot_config("bot-1").await.unwrap();
        assert!(deleted);

        let config = store.get_bot_config("bot-1").await.unwrap();
        assert!(config.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_returns_false() {
        let store = setup_test_store().await;
        let deleted = store.delete_bot_config("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_list_all() {
        let store = setup_test_store().await;
        store.upsert("__global__", sample_config()).await.unwrap();
        store.upsert("bot-1", sample_config()).await.unwrap();
        store.upsert("bot-2", sample_config()).await.unwrap();

        let all = store.list_all().await.unwrap();
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_defaults_applied_on_create() {
        let store = setup_test_store().await;
        let minimal = UpsertModelConfig {
            provider: "ollama".into(),
            model_id: "llama3".into(),
            tier_strategy: None,
            reasoning_effort: None,
            max_tokens: None,
            temperature: None,
            is_default: None,
        };
        store.upsert("bot-minimal", minimal).await.unwrap();

        let config = store.get_bot_config("bot-minimal").await.unwrap().unwrap();
        assert_eq!(config.tier_strategy, "balanced"); // Default
        assert_eq!(config.reasoning_effort, "medium"); // Default
        assert!(!config.is_default); // Default false
    }
}
