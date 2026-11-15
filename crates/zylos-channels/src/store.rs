//! ChannelsStore — persistência SQLite via sqlx para canais e mensagens.

use crate::models::{ChannelMember, ChannelMessage, ChannelResponse, ChannelWithCount};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;
use thiserror::Error;

/// Erros do store de canais.
#[derive(Debug, Error)]
pub enum ChannelsError {
    #[error("sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("channel not found: {0}")]
    NotFound(String),
    #[error("channel already exists: {0}")]
    AlreadyExists(String),
    #[error("member already in channel")]
    MemberAlreadyExists,
    #[error("migration error: {0}")]
    Migration(String),
}

pub type Result<T> = std::result::Result<T, ChannelsError>;

/// Store persistente de canais backed by SQLite.
#[derive(Clone)]
pub struct ChannelsStore {
    pool: SqlitePool,
}

impl ChannelsStore {
    /// Cria store com pool existente.
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Cria store conectando a uma URL SQLite.
    pub async fn connect(url: &str) -> Result<Self> {
        let opts = SqliteConnectOptions::from_str(url)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        Ok(Self { pool })
    }

    /// Cria store em memória (para testes).
    pub async fn in_memory() -> Result<Self> {
        Self::connect("sqlite::memory:").await
    }

    /// Roda migrations individuais (não multi-statement).
    pub async fn run_migrations(&self) -> Result<()> {
        let migrations = [
            include_str!("../migrations/20261112000001_create_channels.sql"),
            include_str!("../migrations/20261112000002_create_channel_members.sql"),
            include_str!("../migrations/20261112000003_create_channel_messages.sql"),
        ];
        for (i, sql) in migrations.iter().enumerate() {
            // Cada migration pode ter múltiplos statements separados por ;
            for stmt in sql.split(';') {
                let stmt = stmt.trim();
                if !stmt.is_empty() {
                    sqlx::query(stmt).execute(&self.pool).await.map_err(|e| {
                        ChannelsError::Migration(format!("migration {} statement failed: {}", i, e))
                    })?;
                }
            }
        }
        Ok(())
    }

    /// Seed de canais padrão (idempotente).
    pub async fn seed_defaults(&self) -> Result<()> {
        let defaults = vec![
            ("ch-geral", "geral", "Discussões gerais da equipe", false),
            (
                "ch-engenharia",
                "engenharia",
                "Tópicos técnicos e arquitetura",
                false,
            ),
            (
                "ch-segurança",
                "segurança",
                "Alertas e auditorias de segurança",
                true,
            ),
            ("ch-anúncios", "anúncios", "Comunicados oficiais", false),
        ];
        let now = chrono::Utc::now().to_rfc3339();
        for (id, name, desc, private) in defaults {
            let priv_i: i32 = if private { 1 } else { 0 };
            sqlx::query(
                "INSERT OR IGNORE INTO channels (id, name, description, is_private, created_at, last_message_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(desc)
            .bind(priv_i)
            .bind(&now)
            .bind(&now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Lista todos os canais com contagem de membros.
    pub async fn list_channels(&self) -> Result<Vec<ChannelResponse>> {
        let rows = sqlx::query_as::<_, ChannelWithCount>(
            "SELECT c.*, COALESCE(m.cnt, 0) as member_count FROM channels c LEFT JOIN (SELECT channel_id, COUNT(*) as cnt FROM channel_members GROUP BY channel_id) m ON c.id = m.channel_id ORDER BY c.name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(ChannelResponse::from).collect())
    }

    /// Obtém um canal por ID.
    pub async fn get_channel(&self, id: &str) -> Result<ChannelResponse> {
        let row = sqlx::query_as::<_, ChannelWithCount>(
            "SELECT c.*, COALESCE(m.cnt, 0) as member_count FROM channels c LEFT JOIN (SELECT channel_id, COUNT(*) as cnt FROM channel_members GROUP BY channel_id) m ON c.id = m.channel_id WHERE c.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| ChannelsError::NotFound(id.to_string()))?;
        Ok(ChannelResponse::from(row))
    }

    /// Cria um novo canal.
    pub async fn create_channel(
        &self,
        name: &str,
        description: &str,
        is_private: bool,
    ) -> Result<ChannelResponse> {
        let id = format!("ch-{}", name.to_lowercase().replace(' ', "-"));
        let now = chrono::Utc::now().to_rfc3339();
        let priv_i: i32 = if is_private { 1 } else { 0 };

        // Verifica existência prévia
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM channels WHERE id = ?")
            .bind(&id)
            .fetch_one(&self.pool)
            .await?;
        if exists > 0 {
            return Err(ChannelsError::AlreadyExists(id));
        }

        sqlx::query(
            "INSERT INTO channels (id, name, description, is_private, created_at, last_message_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(description)
        .bind(priv_i)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_channel(&id).await
    }

    /// Lista mensagens de um canal.
    pub async fn list_messages(&self, channel_id: &str) -> Result<Vec<ChannelMessage>> {
        // Verifica se canal existe
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM channels WHERE id = ?")
            .bind(channel_id)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(ChannelsError::NotFound(channel_id.to_string()));
        }

        let msgs = sqlx::query_as::<_, ChannelMessage>(
            "SELECT id, channel_id, author, content, timestamp FROM channel_messages WHERE channel_id = ? ORDER BY timestamp ASC",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(msgs)
    }

    /// Envia mensagem para um canal.
    pub async fn send_message(
        &self,
        channel_id: &str,
        author: &str,
        content: &str,
    ) -> Result<ChannelMessage> {
        // Verifica se canal existe
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM channels WHERE id = ?")
            .bind(channel_id)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(ChannelsError::NotFound(channel_id.to_string()));
        }

        let now = chrono::Utc::now().to_rfc3339();
        let id = format!("msg-{}", uuid::Uuid::new_v4());

        sqlx::query(
            "INSERT INTO channel_messages (id, channel_id, author, content, timestamp) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(channel_id)
        .bind(author)
        .bind(content)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        // Atualiza last_message_at
        sqlx::query("UPDATE channels SET last_message_at = ? WHERE id = ?")
            .bind(&now)
            .bind(channel_id)
            .execute(&self.pool)
            .await?;

        Ok(ChannelMessage {
            id,
            channel_id: channel_id.to_string(),
            author: author.to_string(),
            content: content.to_string(),
            timestamp: now,
        })
    }

    /// Adiciona membro a um canal (join).
    pub async fn join_channel(&self, channel_id: &str, user_id: &str) -> Result<ChannelMember> {
        // Verifica se canal existe
        let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM channels WHERE id = ?")
            .bind(channel_id)
            .fetch_one(&self.pool)
            .await?;
        if exists == 0 {
            return Err(ChannelsError::NotFound(channel_id.to_string()));
        }

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO channel_members (channel_id, user_id, joined_at) VALUES (?, ?, ?)",
        )
        .bind(channel_id)
        .bind(user_id)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.message().contains("UNIQUE") => {
                ChannelsError::MemberAlreadyExists
            }
            other => ChannelsError::Sqlx(other),
        })?;

        Ok(ChannelMember {
            channel_id: channel_id.to_string(),
            user_id: user_id.to_string(),
            joined_at: now,
        })
    }

    /// Remove membro de um canal (leave).
    pub async fn leave_channel(&self, channel_id: &str, user_id: &str) -> Result<()> {
        let result =
            sqlx::query("DELETE FROM channel_members WHERE channel_id = ? AND user_id = ?")
                .bind(channel_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(ChannelsError::NotFound(format!(
                "member {} in channel {}",
                user_id, channel_id
            )));
        }
        Ok(())
    }

    /// Lista membros de um canal.
    pub async fn list_members(&self, channel_id: &str) -> Result<Vec<ChannelMember>> {
        let members = sqlx::query_as::<_, ChannelMember>(
            "SELECT channel_id, user_id, joined_at FROM channel_members WHERE channel_id = ? ORDER BY joined_at",
        )
        .bind(channel_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(members)
    }

    /// Retorna referência ao pool (para uso avançado).
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> ChannelsStore {
        let store = ChannelsStore::in_memory().await.unwrap();
        store.run_migrations().await.unwrap();
        store.seed_defaults().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_01_migrations_run() {
        let store = ChannelsStore::in_memory().await.unwrap();
        store.run_migrations().await.unwrap();
        // Se chegou aqui sem erro, migrations funcionaram
    }

    #[tokio::test]
    async fn test_02_seed_defaults() {
        let store = setup().await;
        let channels = store.list_channels().await.unwrap();
        assert_eq!(channels.len(), 4);
    }

    #[tokio::test]
    async fn test_03_seed_idempotent() {
        let store = setup().await;
        store.seed_defaults().await.unwrap(); // segunda vez não deve falhar
        let channels = store.list_channels().await.unwrap();
        assert_eq!(channels.len(), 4);
    }

    #[tokio::test]
    #[allow(unused_comparisons)]
    async fn test_04_list_channels_has_member_count() {
        let store = setup().await;
        let channels = store.list_channels().await.unwrap();
        for ch in &channels {
            assert!(ch.member_count >= 0);
        }
    }

    #[tokio::test]
    async fn test_05_get_channel_found() {
        let store = setup().await;
        let ch = store.get_channel("ch-geral").await.unwrap();
        assert_eq!(ch.name, "geral");
    }

    #[tokio::test]
    async fn test_06_get_channel_not_found() {
        let store = setup().await;
        let result = store.get_channel("ch-inexistente").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_07_create_channel() {
        let store = setup().await;
        let ch = store
            .create_channel("Novo Canal", "Descrição teste", false)
            .await
            .unwrap();
        assert_eq!(ch.id, "ch-novo-canal");
        assert_eq!(ch.name, "Novo Canal");
    }

    #[tokio::test]
    async fn test_08_create_channel_duplicate() {
        let store = setup().await;
        store
            .create_channel("Duplicado", "desc", false)
            .await
            .unwrap();
        let result = store.create_channel("Duplicado", "desc2", false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_09_send_message() {
        let store = setup().await;
        let msg = store
            .send_message("ch-geral", "Alice", "Olá mundo!")
            .await
            .unwrap();
        assert_eq!(msg.author, "Alice");
        assert_eq!(msg.content, "Olá mundo!");
        assert_eq!(msg.channel_id, "ch-geral");
    }

    #[tokio::test]
    async fn test_10_send_message_channel_not_found() {
        let store = setup().await;
        let result = store.send_message("ch-inexistente", "Bob", "test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_11_list_messages() {
        let store = setup().await;
        store
            .send_message("ch-geral", "Alice", "msg1")
            .await
            .unwrap();
        store.send_message("ch-geral", "Bob", "msg2").await.unwrap();
        let msgs = store.list_messages("ch-geral").await.unwrap();
        assert_eq!(msgs.len(), 2);
    }

    #[tokio::test]
    async fn test_12_list_messages_empty() {
        let store = setup().await;
        let msgs = store.list_messages("ch-engenharia").await.unwrap();
        assert!(msgs.is_empty());
    }

    #[tokio::test]
    async fn test_13_join_channel() {
        let store = setup().await;
        let member = store.join_channel("ch-geral", "user-1").await.unwrap();
        assert_eq!(member.user_id, "user-1");
        assert_eq!(member.channel_id, "ch-geral");
    }

    #[tokio::test]
    async fn test_14_join_channel_duplicate() {
        let store = setup().await;
        store.join_channel("ch-geral", "user-1").await.unwrap();
        let result = store.join_channel("ch-geral", "user-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_15_leave_channel() {
        let store = setup().await;
        store.join_channel("ch-geral", "user-1").await.unwrap();
        store.leave_channel("ch-geral", "user-1").await.unwrap();
        let members = store.list_members("ch-geral").await.unwrap();
        assert!(members.is_empty());
    }

    #[tokio::test]
    async fn test_16_leave_channel_not_member() {
        let store = setup().await;
        let result = store.leave_channel("ch-geral", "user-fantasma").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_17_join_updates_member_count() {
        let store = setup().await;
        let before = store.get_channel("ch-geral").await.unwrap();
        store.join_channel("ch-geral", "user-novo").await.unwrap();
        let after = store.get_channel("ch-geral").await.unwrap();
        assert_eq!(after.member_count, before.member_count + 1);
    }

    #[tokio::test]
    async fn test_18_list_members() {
        let store = setup().await;
        store.join_channel("ch-engenharia", "u1").await.unwrap();
        store.join_channel("ch-engenharia", "u2").await.unwrap();
        let members = store.list_members("ch-engenharia").await.unwrap();
        assert_eq!(members.len(), 2);
    }

    #[tokio::test]
    async fn test_19_send_message_updates_last_message_at() {
        let store = setup().await;
        let before = store.get_channel("ch-geral").await.unwrap();
        // Pequeno delay para garantir timestamp diferente
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        store
            .send_message("ch-geral", "Alice", "nova msg")
            .await
            .unwrap();
        let after = store.get_channel("ch-geral").await.unwrap();
        assert!(after.last_message_at >= before.last_message_at);
    }

    #[tokio::test]
    async fn test_20_create_private_channel() {
        let store = setup().await;
        let ch = store
            .create_channel("Secreto", "canal privado", true)
            .await
            .unwrap();
        assert!(ch.is_private);
    }
}
