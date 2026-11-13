//! AuditStore — persistência SQLite para Action Audit Log
//!
//! Todas as operações são async via sqlx com runtime tokio.
//! Arguments são hasheados com SHA-256 antes da persistência.

use crate::schema::*;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Store para operações CRUD do audit log
#[derive(Clone)]
pub struct AuditStore {
    pool: SqlitePool,
}

impl AuditStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Registra nova entrada no audit log
    pub async fn create(&self, entry: CreateAuditEntry) -> Result<String, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Hash SHA-256 dos argumentos (não persistir plaintext)
        let arguments_hash = hash_arguments(&entry.arguments_json);

        sqlx::query(
            "INSERT INTO action_audit_log (id, bot_id, action_type, target, approval_status, approval_decision, arguments_hash, result_status, result_summary, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&entry.bot_id)
        .bind(&entry.action_type)
        .bind(&entry.target)
        .bind(&entry.approval_status)
        .bind(&entry.approval_decision)
        .bind(&arguments_hash)
        .bind(&entry.result_status)
        .bind(&entry.result_summary)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            audit_id = %id,
            bot_id = %entry.bot_id,
            action_type = %entry.action_type,
            approval_status = %entry.approval_status,
            "Audit entry created"
        );

        Ok(id)
    }

    /// Lista entradas paginadas com filtros
    pub async fn list(&self, filter: AuditFilter) -> Result<AuditListResponse, sqlx::Error> {
        let limit = filter.limit.unwrap_or(50).min(200);
        let offset = filter.offset.unwrap_or(0);

        // Query simples sem filtros dinâmicos (filtros aplicados via SQL direto)
        // Para produção, construir query builder adequado; aqui usamos query base
        // com filtros opcionais via COALESCE pattern para evitar dyn Arguments
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE (? IS NULL OR bot_id = ?) AND (? IS NULL OR action_type = ?) AND (? IS NULL OR approval_status = ?)"
        )
            .bind(filter.bot_id.as_deref())
            .bind(filter.bot_id.as_deref())
            .bind(filter.action_type.as_deref())
            .bind(filter.action_type.as_deref())
            .bind(filter.approval_status.as_deref())
            .bind(filter.approval_status.as_deref())
            .fetch_one(&self.pool)
            .await?;

        let entries = sqlx::query_as::<_, AuditEntry>(
            "SELECT * FROM action_audit_log WHERE (? IS NULL OR bot_id = ?) AND (? IS NULL OR action_type = ?) AND (? IS NULL OR approval_status = ?) ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
            .bind(filter.bot_id.as_deref())
            .bind(filter.bot_id.as_deref())
            .bind(filter.action_type.as_deref())
            .bind(filter.action_type.as_deref())
            .bind(filter.approval_status.as_deref())
            .bind(filter.approval_status.as_deref())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;

        Ok(AuditListResponse {
            entries,
            total,
            limit,
            offset,
        })
    }

    /// Estatísticas agregadas por período
    pub async fn stats(&self, period_days: i64) -> Result<AuditStats, sqlx::Error> {
        let since = chrono::Utc::now() - chrono::Duration::days(period_days);
        let since_str = since.to_rfc3339();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE created_at >= ?"
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;

        let approved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE created_at >= ? AND approval_status = 'approved'"
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;

        let refused: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE created_at >= ? AND approval_status = 'refused'"
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;

        let expired: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE created_at >= ? AND approval_status = 'expired'"
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;

        let auto_approved: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM action_audit_log WHERE created_at >= ? AND approval_status = 'auto_approved'"
        )
        .bind(&since_str)
        .fetch_one(&self.pool)
        .await?;

        let by_action_type = sqlx::query_as::<_, ActionTypeCount>(
            "SELECT action_type, COUNT(*) as count FROM action_audit_log WHERE created_at >= ? GROUP BY action_type ORDER BY count DESC"
        )
        .bind(&since_str)
        .fetch_all(&self.pool)
        .await?;

        Ok(AuditStats {
            period: format!("{}d", period_days),
            total_actions: total,
            approved,
            refused,
            auto_approved,
            expired,
            by_action_type,
        })
    }

    /// Aplica migrations SQL (statements individuais para sqlx SQLite)
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS action_audit_log (
                id TEXT PRIMARY KEY,
                bot_id TEXT NOT NULL,
                action_type TEXT NOT NULL,
                target TEXT,
                approval_status TEXT NOT NULL,
                approval_decision TEXT,
                arguments_hash TEXT NOT NULL,
                result_status TEXT,
                result_summary TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                decided_at TEXT,
                executed_at TEXT
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_bot_date ON action_audit_log(bot_id, created_at DESC)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_approval ON action_audit_log(approval_status)"
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Audit log migrations applied successfully");
        Ok(())
    }
}

/// Hash SHA-256 dos argumentos JSON
fn hash_arguments(arguments_json: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(arguments_json.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_store() -> AuditStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = AuditStore::new(pool);
        store.run_migrations().await.unwrap();
        store
    }

    fn sample_entry() -> CreateAuditEntry {
        CreateAuditEntry {
            bot_id: "bot-1".into(),
            action_type: "send_email".into(),
            target: Some("client@example.com".into()),
            approval_status: "approved".into(),
            approval_decision: Some("allow_once".into()),
            arguments_json: r#"{"to":"client@example.com","subject":"Test"}"#.into(),
            result_status: Some("success".into()),
            result_summary: Some("Email sent successfully".into()),
        }
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let store = setup_test_store().await;
        let id = store.create(sample_entry()).await.unwrap();

        let response = store.list(AuditFilter::default()).await.unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.entries[0].id, id);
        assert_eq!(response.entries[0].bot_id, "bot-1");
        assert_eq!(response.entries[0].action_type, "send_email");
    }

    #[tokio::test]
    async fn test_arguments_hashed_not_plaintext() {
        let store = setup_test_store().await;
        let _id = store.create(sample_entry()).await.unwrap();

        let response = store.list(AuditFilter::default()).await.unwrap();
        let entry = &response.entries[0];

        // arguments_hash deve ser SHA-256 hex (64 chars)
        assert_eq!(entry.arguments_hash.len(), 64);
        // Não deve conter plaintext dos argumentos
        assert!(!entry.arguments_hash.contains("client@example.com"));
        assert!(!entry.arguments_hash.contains("Test"));
    }

    #[tokio::test]
    async fn test_filter_by_bot_id() {
        let store = setup_test_store().await;
        store.create(sample_entry()).await.unwrap();

        let mut entry2 = sample_entry();
        entry2.bot_id = "bot-2".into();
        store.create(entry2).await.unwrap();

        let filter = AuditFilter {
            bot_id: Some("bot-1".into()),
            ..Default::default()
        };
        let response = store.list(filter).await.unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.entries[0].bot_id, "bot-1");
    }

    #[tokio::test]
    async fn test_stats_aggregation() {
        let store = setup_test_store().await;

        // Criar entradas com diferentes status
        for status in ["approved", "approved", "refused", "expired"] {
            let mut entry = sample_entry();
            entry.approval_status = status.into();
            store.create(entry).await.unwrap();
        }

        let stats = store.stats(7).await.unwrap();
        assert_eq!(stats.total_actions, 4);
        assert_eq!(stats.approved, 2);
        assert_eq!(stats.refused, 1);
        assert_eq!(stats.expired, 1);
        assert_eq!(stats.period, "7d");
    }

    #[tokio::test]
    async fn test_pagination() {
        let store = setup_test_store().await;

        for i in 0..10 {
            let mut entry = sample_entry();
            entry.action_type = format!("action_{}", i);
            store.create(entry).await.unwrap();
        }

        let filter = AuditFilter {
            limit: Some(3),
            offset: Some(0),
            ..Default::default()
        };
        let page1 = store.list(filter).await.unwrap();
        assert_eq!(page1.entries.len(), 3);
        assert_eq!(page1.total, 10);

        let filter = AuditFilter {
            limit: Some(3),
            offset: Some(3),
            ..Default::default()
        };
        let page2 = store.list(filter).await.unwrap();
        assert_eq!(page2.entries.len(), 3);
        // Páginas devem ter entradas diferentes
        assert_ne!(page1.entries[0].id, page2.entries[0].id);
    }

    #[tokio::test]
    async fn test_hash_deterministic() {
        let json = r#"{"password":"secret123"}"#;
        let hash1 = hash_arguments(json);
        let hash2 = hash_arguments(json);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex = 64 chars
    }

    #[tokio::test]
    async fn test_hash_different_inputs() {
        let hash1 = hash_arguments(r#"{"a":1}"#);
        let hash2 = hash_arguments(r#"{"b":2}"#);
        assert_ne!(hash1, hash2);
    }
}