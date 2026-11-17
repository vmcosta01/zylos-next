//! BriefStore — persistência SQLite para Morning Briefs
//!
//! Armazena briefs gerados para evitar regeneração no mesmo dia.
//! Suporta histórico e marcação de leitura.

use crate::schema::*;
use sqlx::SqlitePool;

/// Store para operações CRUD de morning briefs
#[derive(Clone)]
pub struct BriefStore {
    pool: SqlitePool,
}

impl BriefStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Salva um brief gerado (upsert por user_id + date)
    pub async fn save(&self, brief: &MorningBrief) -> Result<(), sqlx::Error> {
        let content_json =
            serde_json::to_string(brief).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        sqlx::query(
            "INSERT INTO morning_briefs (id, user_id, date, content_json, delivered_at, items_count, needs_you_count)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(user_id, date) DO UPDATE SET
                content_json = excluded.content_json,
                delivered_at = excluded.delivered_at,
                items_count = excluded.items_count,
                needs_you_count = excluded.needs_you_count"
        )
        .bind(&brief.id)
        .bind(&brief.user_id)
        .bind(&brief.date)
        .bind(&content_json)
        .bind(&brief.generated_at)
        .bind(brief.total_items as i64)
        .bind(brief.needs_you_count as i64)
        .execute(&self.pool)
        .await?;

        tracing::info!(
            brief_id = %brief.id,
            user_id = %brief.user_id,
            date = %brief.date,
            items = brief.total_items,
            needs_you = brief.needs_you_count,
            "Morning brief saved"
        );

        Ok(())
    }

    /// Busca brief do dia para um usuário
    pub async fn get_today(
        &self,
        user_id: &str,
        date: &str,
    ) -> Result<Option<MorningBrief>, sqlx::Error> {
        let row = sqlx::query_as::<_, BriefRow>(
            "SELECT * FROM morning_briefs WHERE user_id = ? AND date = ?",
        )
        .bind(user_id)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => {
                let brief: MorningBrief = serde_json::from_str(&r.content_json)
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
                Ok(Some(brief))
            }
            None => Ok(None),
        }
    }

    /// Marca brief como lido
    pub async fn mark_read(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE morning_briefs SET read_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Lista histórico de briefs (mais recentes primeiro)
    pub async fn history(&self, user_id: &str, limit: i64) -> Result<Vec<BriefRow>, sqlx::Error> {
        let rows = sqlx::query_as::<_, BriefRow>(
            "SELECT * FROM morning_briefs WHERE user_id = ? ORDER BY date DESC LIMIT ?",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Deleta brief específico (para regeneração forçada)
    pub async fn delete(&self, user_id: &str, date: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM morning_briefs WHERE user_id = ? AND date = ?")
            .bind(user_id)
            .bind(date)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Aplica migrations SQL (statements individuais para sqlx SQLite)
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS morning_briefs (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                date TEXT NOT NULL,
                content_json TEXT NOT NULL,
                delivered_at TEXT NOT NULL,
                read_at TEXT,
                items_count INTEGER NOT NULL DEFAULT 0,
                needs_you_count INTEGER NOT NULL DEFAULT 0,
                UNIQUE(user_id, date)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_brief_user_date ON morning_briefs(user_id, date DESC)",
        )
        .execute(&self.pool)
        .await?;

        tracing::info!("Morning brief migrations applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_store() -> BriefStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = BriefStore::new(pool);
        store.run_migrations().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_save_and_get_today() {
        let store = setup_test_store().await;
        let brief = MorningBrief::empty("user-1", "2026-11-11");

        store.save(&brief).await.unwrap();

        let retrieved = store.get_today("user-1", "2026-11-11").await.unwrap();
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert_eq!(r.user_id, "user-1");
        assert_eq!(r.date, "2026-11-11");
        assert_eq!(r.sections.len(), 4);
    }

    #[tokio::test]
    async fn test_get_today_returns_none_when_missing() {
        let store = setup_test_store().await;
        let result = store.get_today("user-1", "2026-11-11").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_upsert_overwrites_existing() {
        let store = setup_test_store().await;
        let mut brief = MorningBrief::empty("user-1", "2026-11-11");
        store.save(&brief).await.unwrap();

        // Modificar e salvar novamente
        brief.sections[0].items.push(BriefItem {
            id: "new-item".into(),
            title: "New task".into(),
            description: None,
            priority: "high".into(),
            source: "task".into(),
            action_url: None,
            metadata: None,
        });
        brief.finalize();
        store.save(&brief).await.unwrap();

        let retrieved = store
            .get_today("user-1", "2026-11-11")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.total_items, 1);
        assert_eq!(retrieved.needs_you_count, 1);
    }

    #[tokio::test]
    async fn test_mark_read() {
        let store = setup_test_store().await;
        let brief = MorningBrief::empty("user-1", "2026-11-11");
        store.save(&brief).await.unwrap();

        store.mark_read(&brief.id).await.unwrap();

        let history = store.history("user-1", 10).await.unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0].read_at.is_some());
    }

    #[tokio::test]
    async fn test_history_ordered_by_date_desc() {
        let store = setup_test_store().await;

        for day in ["2026-11-09", "2026-11-10", "2026-11-11"] {
            let brief = MorningBrief::empty("user-1", day);
            store.save(&brief).await.unwrap();
        }

        let history = store.history("user-1", 10).await.unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].date, "2026-11-11");
        assert_eq!(history[1].date, "2026-11-10");
        assert_eq!(history[2].date, "2026-11-09");
    }

    #[tokio::test]
    async fn test_delete_for_regeneration() {
        let store = setup_test_store().await;
        let brief = MorningBrief::empty("user-1", "2026-11-11");
        store.save(&brief).await.unwrap();

        store.delete("user-1", "2026-11-11").await.unwrap();

        let result = store.get_today("user-1", "2026-11-11").await.unwrap();
        assert!(result.is_none());
    }
}
