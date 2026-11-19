//! ApprovalStore — persistência SQLite para approval gates HITL
//!
//! Todas as operações são async via sqlx com runtime tokio.
//! Migrations devem ser aplicadas separadamente (001_initial.sql).

use crate::error::ApprovalError;
use crate::schema::*;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Store para operações CRUD de approvals e regras
#[derive(Clone)]
pub struct ApprovalStore {
    pool: SqlitePool,
}

impl ApprovalStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Conecta ao banco SQLite no path fornecido (ex: `sqlite:data/approvals.db` ou `sqlite::memory:`).
    /// O chamador deve garantir que o diretório pai existe antes de chamar.
    pub async fn connect(path: &str) -> Result<Self, ApprovalError> {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
        use std::str::FromStr;

        let options = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);

        let pool = SqlitePool::connect_with(options).await?;

        Ok(Self { pool })
    }

    /// Cria nova requisição de aprovação pendente
    pub async fn create(&self, req: CreateApprovalRequest) -> Result<String, ApprovalError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        // Validar action_category
        if ActionCategory::parse(&req.action_category).is_none() {
            return Err(ApprovalError::InvalidCategory(req.action_category));
        }

        let expires_at = req.expires_in_seconds.map(|secs| {
            let expiry = chrono::Utc::now() + chrono::Duration::seconds(secs as i64);
            expiry.to_rfc3339()
        });

        sqlx::query(
            "INSERT INTO approval_requests (id, bot_id, tool_name, action_category, summary, arguments_json, status, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?)"
        )
        .bind(&id)
        .bind(&req.bot_id)
        .bind(&req.tool_name)
        .bind(&req.action_category)
        .bind(&req.summary)
        .bind(&req.arguments_json)
        .bind(&now)
        .bind(&expires_at)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            approval_id = %id,
            bot_id = %req.bot_id,
            tool = %req.tool_name,
            "Created approval request"
        );

        Ok(id)
    }

    /// Busca requisição por ID
    pub async fn get(&self, id: &str) -> Result<ApprovalRequest, ApprovalError> {
        let row =
            sqlx::query_as::<_, ApprovalRequest>("SELECT * FROM approval_requests WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;

        Ok(row)
    }

    /// Busca requisição pendente por ID (retorna erro se não estiver pending)
    pub async fn get_pending(&self, id: &str) -> Result<ApprovalRequest, ApprovalError> {
        let req = self.get(id).await?;

        if !req.is_pending() {
            return Err(ApprovalError::AlreadyDecided(id.to_string()));
        }

        if req.is_expired() {
            // Atualizar status para expired
            sqlx::query("UPDATE approval_requests SET status = 'expired' WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;
            return Err(ApprovalError::Expired(id.to_string()));
        }

        Ok(req)
    }

    /// Lista todas as requisições pendentes (não expiradas)
    pub async fn list_pending(&self) -> Result<Vec<ApprovalRequest>, ApprovalError> {
        let now = chrono::Utc::now().to_rfc3339();
        let rows = sqlx::query_as::<_, ApprovalRequest>(
            "SELECT * FROM approval_requests WHERE status = 'pending' AND (expires_at IS NULL OR expires_at > ?) ORDER BY created_at ASC"
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Registra decisão do usuário
    pub async fn decide(
        &self,
        id: &str,
        decision: ApprovalDecision,
        refuse_reason: Option<String>,
    ) -> Result<String, ApprovalError> {
        // Verificar se existe e está pendente
        let _req = self.get_pending(id).await?;

        let now = chrono::Utc::now().to_rfc3339();
        let status = match decision {
            ApprovalDecision::Refuse => "refused",
            _ => "approved",
        };

        sqlx::query(
            "UPDATE approval_requests SET status = ?, decision = ?, refuse_reason = ?, decided_at = ? WHERE id = ?"
        )
        .bind(status)
        .bind(decision.as_str())
        .bind(&refuse_reason)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Se allow_always, criar regra persistente
        if decision == ApprovalDecision::AllowAlways {
            let req = self.get(id).await?;
            let rule_id = Uuid::new_v4().to_string();
            let result = sqlx::query(
                "INSERT OR IGNORE INTO approval_rules (id, bot_id, tool_name, action_category, decision, created_at, created_by) VALUES (?, ?, ?, ?, ?, ?, 'user')"
            )
            .bind(&rule_id)
            .bind(&req.bot_id)
            .bind(&req.tool_name)
            .bind(&req.action_category)
            .bind(decision.as_str())
            .bind(&now)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    tracing::info!(
                        rule_id = %rule_id,
                        bot_id = %req.bot_id,
                        tool = %req.tool_name,
                        "Created persistent approval rule"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to create approval rule (may already exist)");
                }
            }
        }

        tracing::info!(
            approval_id = %id,
            decision = %decision.as_str(),
            "Approval decided"
        );

        Ok(now)
    }

    /// Lista regras persistentes para um bot
    pub async fn list_rules(&self, bot_id: &str) -> Result<Vec<ApprovalRule>, ApprovalError> {
        let rows = sqlx::query_as::<_, ApprovalRule>(
            "SELECT * FROM approval_rules WHERE bot_id = ? ORDER BY created_at DESC",
        )
        .bind(bot_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Verifica se existe regra allow-always para bot+tool+category
    pub async fn has_allow_always_rule(
        &self,
        bot_id: &str,
        tool_name: &str,
        action_category: &str,
    ) -> Result<bool, ApprovalError> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM approval_rules WHERE bot_id = ? AND tool_name = ? AND action_category = ? AND decision = 'allow_always'"
        )
        .bind(bot_id)
        .bind(tool_name)
        .bind(action_category)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.0 > 0)
    }

    /// Remove regra persistente
    pub async fn delete_rule(&self, rule_id: &str) -> Result<(), ApprovalError> {
        let result = sqlx::query("DELETE FROM approval_rules WHERE id = ?")
            .bind(rule_id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApprovalError::NotFound(format!("rule:{rule_id}")));
        }

        tracing::info!(rule_id = %rule_id, "Deleted approval rule");
        Ok(())
    }

    /// Salva receipt JSON para sobrevivência a refresh
    pub async fn save_receipt(&self, id: &str, receipt_json: &str) -> Result<(), ApprovalError> {
        sqlx::query("UPDATE approval_requests SET receipt_json = ? WHERE id = ?")
            .bind(receipt_json)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Aplica migrations SQL manualmente (statements individuais para sqlx SQLite)
    pub async fn run_migrations(&self) -> Result<(), ApprovalError> {
        // Statement 1: approval_requests table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS approval_requests (
                id TEXT PRIMARY KEY,
                bot_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                action_category TEXT NOT NULL,
                summary TEXT NOT NULL,
                arguments_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                decision TEXT,
                refuse_reason TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                decided_at TEXT,
                expires_at TEXT,
                receipt_json TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        // Statement 2: idx_approval_bot_status
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_approval_bot_status ON approval_requests(bot_id, status)"
        )
        .execute(&self.pool)
        .await?;

        // Statement 3: idx_approval_created
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_approval_created ON approval_requests(created_at)",
        )
        .execute(&self.pool)
        .await?;

        // Statement 4: idx_approval_pending (partial index)
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_approval_pending ON approval_requests(status) WHERE status = 'pending'"
        )
        .execute(&self.pool)
        .await?;

        // Statement 5: approval_rules table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS approval_rules (
                id TEXT PRIMARY KEY,
                bot_id TEXT NOT NULL,
                tool_name TEXT NOT NULL,
                action_category TEXT NOT NULL,
                arguments_pattern TEXT,
                decision TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                created_by TEXT NOT NULL DEFAULT 'user',
                UNIQUE(bot_id, tool_name, action_category, arguments_pattern)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Statement 6: idx_approval_rules_lookup
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_approval_rules_lookup ON approval_rules(bot_id, tool_name, action_category)"
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Approval migrations applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_store() -> ApprovalStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = ApprovalStore::new(pool);
        store.run_migrations().await.unwrap();
        store
    }

    fn sample_request() -> CreateApprovalRequest {
        CreateApprovalRequest {
            bot_id: "bot-1".into(),
            tool_name: "send_email".into(),
            action_category: "send_message".into(),
            summary: "Send email to client@example.com".into(),
            arguments_json: r#"{"to":"client@example.com","subject":"Test"}"#.into(),
            expires_in_seconds: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        let req = store.get(&id).await.unwrap();
        assert_eq!(req.bot_id, "bot-1");
        assert_eq!(req.tool_name, "send_email");
        assert_eq!(req.status, "pending");
        assert!(req.decision.is_none());
    }

    #[tokio::test]
    async fn test_decide_allow_once() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        let decided_at = store
            .decide(&id, ApprovalDecision::AllowOnce, None)
            .await
            .unwrap();

        let req = store.get(&id).await.unwrap();
        assert_eq!(req.status, "approved");
        assert_eq!(req.decision.as_deref(), Some("allow_once"));
        assert!(req.decided_at.is_some());
        assert!(!decided_at.is_empty());
    }

    #[tokio::test]
    async fn test_decide_refuse_with_reason() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        store
            .decide(&id, ApprovalDecision::Refuse, Some("Not authorized".into()))
            .await
            .unwrap();

        let req = store.get(&id).await.unwrap();
        assert_eq!(req.status, "refused");
        assert_eq!(req.decision.as_deref(), Some("refuse"));
        assert_eq!(req.refuse_reason.as_deref(), Some("Not authorized"));
    }

    #[tokio::test]
    async fn test_decide_allow_always_creates_rule() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        store
            .decide(&id, ApprovalDecision::AllowAlways, None)
            .await
            .unwrap();

        let rules = store.list_rules("bot-1").await.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].tool_name, "send_email");
        assert_eq!(rules[0].decision, "allow_always");
    }

    #[tokio::test]
    async fn test_double_decide_returns_error() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        store
            .decide(&id, ApprovalDecision::AllowOnce, None)
            .await
            .unwrap();

        let result = store.decide(&id, ApprovalDecision::AllowOnce, None).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already decided"));
    }

    #[tokio::test]
    async fn test_list_pending() {
        let store = setup_test_store().await;
        let id1 = store.create(sample_request()).await.unwrap();
        let _id2 = store.create(sample_request()).await.unwrap();

        let pending = store.list_pending().await.unwrap();
        assert_eq!(pending.len(), 2);

        // Decidir uma
        store
            .decide(&id1, ApprovalDecision::AllowOnce, None)
            .await
            .unwrap();

        let pending = store.list_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn test_expired_request() {
        let store = setup_test_store().await;
        let mut req = sample_request();
        req.expires_in_seconds = Some(0); // Expira imediatamente

        let id = store.create(req).await.unwrap();

        // Pequeno delay para garantir expiração
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = store.get_pending(&id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().is_expired());
    }

    #[tokio::test]
    async fn test_has_allow_always_rule() {
        let store = setup_test_store().await;

        assert!(!store
            .has_allow_always_rule("bot-1", "send_email", "send_message")
            .await
            .unwrap());

        let id = store.create(sample_request()).await.unwrap();
        store
            .decide(&id, ApprovalDecision::AllowAlways, None)
            .await
            .unwrap();

        assert!(store
            .has_allow_always_rule("bot-1", "send_email", "send_message")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_delete_rule() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();
        store
            .decide(&id, ApprovalDecision::AllowAlways, None)
            .await
            .unwrap();

        let rules = store.list_rules("bot-1").await.unwrap();
        assert_eq!(rules.len(), 1);

        store.delete_rule(&rules[0].id).await.unwrap();

        let rules = store.list_rules("bot-1").await.unwrap();
        assert_eq!(rules.len(), 0);
    }

    #[tokio::test]
    async fn test_invalid_category_rejected() {
        let store = setup_test_store().await;
        let mut req = sample_request();
        req.action_category = "invalid_category".into();

        let result = store.create(req).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid action category"));
    }

    #[tokio::test]
    async fn test_save_receipt() {
        let store = setup_test_store().await;
        let id = store.create(sample_request()).await.unwrap();

        let receipt = r#"{"version":1,"decision":"allow_once","timestamp":"2026-11-11T10:00:00Z"}"#;
        store.save_receipt(&id, receipt).await.unwrap();

        let req = store.get(&id).await.unwrap();
        assert_eq!(req.receipt_json.as_deref(), Some(receipt));
    }
}
