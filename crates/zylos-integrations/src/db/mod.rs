//! SQLite-backed encrypted token storage with audit logging.

use crate::error::{IntegrationError, Result};
use crate::oauth::TokenBundle;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use chrono::Utc;
use rand::RngCore;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::{info, warn};
use zeroize::Zeroizing;

const DB_FILENAME: &str = "integration_tokens.db";

/// Encrypted token store backed by SQLite WAL.
#[derive(Clone)]
pub struct TokenStore {
    pool: SqlitePool,
    encryption_key: Zeroizing<[u8; 32]>,
}

impl TokenStore {
    /// Open or create the token database.
    /// The encryption key is derived from the ZYLOS_VAULT_KEY env var.
    pub async fn new() -> Result<Self> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let options = SqliteConnectOptions::from_str(&url)
            .map_err(|e| IntegrationError::Database(sqlx::Error::Configuration(e.into())))?;

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;

        // Enable WAL mode
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await?;

        Self::run_migrations(&pool).await?;

        let encryption_key = Self::load_encryption_key()?;

        Ok(Self {
            pool,
            encryption_key,
        })
    }

    fn db_path() -> PathBuf {
        let data_dir = std::env::var("ZYLOS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("zylos")
            });
        data_dir.join(DB_FILENAME)
    }

    fn load_encryption_key() -> Result<Zeroizing<[u8; 32]>> {
        let key_hex = std::env::var("ZYLOS_VAULT_KEY").unwrap_or_else(|_| {
            warn!("ZYLOS_VAULT_KEY not set, using development key");
            "0000000000000000000000000000000000000000000000000000000000000000".to_string()
        });

        let key_bytes = hex::decode(&key_hex)
            .map_err(|e| IntegrationError::Encryption(format!("Invalid hex key: {}", e)))?;

        if key_bytes.len() != 32 {
            return Err(IntegrationError::Encryption(
                "Key must be exactly 32 bytes (64 hex chars)".into(),
            ));
        }

        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&key_bytes);
        Ok(key)
    }

    async fn run_migrations(pool: &SqlitePool) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS integration_tokens (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                provider_slug TEXT NOT NULL,
                access_token_encrypted BLOB NOT NULL,
                refresh_token_encrypted BLOB,
                expires_at INTEGER NOT NULL,
                scopes TEXT NOT NULL,
                token_type TEXT NOT NULL DEFAULT 'Bearer',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                UNIQUE(user_id, provider_slug)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS integration_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id TEXT NOT NULL,
                provider_slug TEXT NOT NULL,
                action TEXT NOT NULL,
                decided_by TEXT NOT NULL,
                decision_hash TEXT NOT NULL,
                status TEXT NOT NULL,
                executed_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_tokens_user_provider ON integration_tokens(user_id, provider_slug)",
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_audit_user_provider ON integration_audit_log(user_id, provider_slug)",
        )
        .execute(pool)
        .await?;

        info!("Integration token database migrations complete");
        Ok(())
    }

    /// Store encrypted tokens for a user+provider.
    pub async fn store_tokens(&self, user_id: &str, bundle: &TokenBundle) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();

        let access_encrypted = self.encrypt(bundle.access_token.as_bytes())?;
        let refresh_encrypted = bundle
            .refresh_token
            .as_ref()
            .map(|t| self.encrypt(t.as_bytes()))
            .transpose()?;

        let scopes_json = serde_json::to_string(&bundle.scopes)?;

        sqlx::query(
            r#"
            INSERT INTO integration_tokens 
                (id, user_id, provider_slug, access_token_encrypted, refresh_token_encrypted,
                 expires_at, scopes, token_type, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, provider_slug) DO UPDATE SET
                access_token_encrypted = excluded.access_token_encrypted,
                refresh_token_encrypted = excluded.refresh_token_encrypted,
                expires_at = excluded.expires_at,
                scopes = excluded.scopes,
                token_type = excluded.token_type,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(user_id)
        .bind(&bundle.provider_slug)
        .bind(&access_encrypted)
        .bind(&refresh_encrypted)
        .bind(bundle.expires_at.timestamp())
        .bind(&scopes_json)
        .bind(&bundle.token_type)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Retrieve and decrypt tokens for a user+provider.
    pub async fn get_tokens(&self, user_id: &str, slug: &str) -> Result<Option<TokenBundle>> {
        let row = sqlx::query(
            "SELECT access_token_encrypted, refresh_token_encrypted, expires_at, scopes, token_type FROM integration_tokens WHERE user_id = ? AND provider_slug = ?",
        )
        .bind(user_id)
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let access_encrypted: Vec<u8> = row.get("access_token_encrypted");
                let access_token = String::from_utf8(self.decrypt(&access_encrypted)?)
                    .map_err(|e| IntegrationError::Encryption(e.to_string()))?;

                let refresh_token = if let Some(refresh_encrypted) =
                    row.try_get::<Option<Vec<u8>>, _>("refresh_token_encrypted")?
                {
                    Some(
                        String::from_utf8(self.decrypt(&refresh_encrypted)?)
                            .map_err(|e| IntegrationError::Encryption(e.to_string()))?,
                    )
                } else {
                    None
                };

                let expires_at_ts: i64 = row.get("expires_at");
                let scopes_json: String = row.get("scopes");
                let token_type: String = row.get("token_type");

                let scopes: Vec<String> = serde_json::from_str(&scopes_json)?;

                Ok(Some(TokenBundle {
                    provider_slug: slug.to_string(),
                    access_token,
                    refresh_token,
                    expires_at: chrono::DateTime::from_timestamp(expires_at_ts, 0)
                        .unwrap_or_default(),
                    scopes,
                    token_type,
                }))
            }
            None => Ok(None),
        }
    }

    /// Delete stored tokens for a user+provider.
    pub async fn delete_tokens(&self, user_id: &str, slug: &str) -> Result<()> {
        sqlx::query("DELETE FROM integration_tokens WHERE user_id = ? AND provider_slug = ?")
            .bind(user_id)
            .bind(slug)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Log an integration action to the audit trail.
    pub async fn log_action(
        &self,
        user_id: &str,
        slug: &str,
        action: &str,
        decided_by: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        use sha2::{Digest, Sha256};

        let hash_input = format!("{}:{}:{}", action, slug, result);
        let hash = hex::encode(Sha256::digest(hash_input.as_bytes()));

        sqlx::query(
            r#"
            INSERT INTO integration_audit_log 
                (user_id, provider_slug, action, decided_by, decision_hash, status, executed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(slug)
        .bind(action)
        .bind(decided_by)
        .bind(&hash)
        .bind("approved")
        .bind(Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Expose the underlying pool for integration tests that need direct SQL access.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(&*self.encryption_key)
            .map_err(|e| IntegrationError::Encryption(e.to_string()))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| IntegrationError::Encryption(e.to_string()))?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            return Err(IntegrationError::Encryption(
                "Ciphertext too short for nonce".into(),
            ));
        }

        let cipher = Aes256Gcm::new_from_slice(&*self.encryption_key)
            .map_err(|e| IntegrationError::Encryption(e.to_string()))?;

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| IntegrationError::Encryption(format!("Decryption failed: {}", e)))
    }
}
