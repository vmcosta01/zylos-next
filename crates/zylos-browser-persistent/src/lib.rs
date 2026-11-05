//! # zylos-browser-persistent
//!
//! Per-agent persistent browser sessions with cookie/storage isolation.
//! Each agent gets its own browser profile directory with isolated cookies,
//! localStorage, sessionStorage, and IndexedDB. Sessions survive restarts
//! via SQLite metadata tracking.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum BrowserError {
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("profile directory creation failed: {0}")]
    ProfileCreation(String),
    #[error("session expired: {0}")]
    SessionExpired(String),
}

/// Metadata for a persistent browser session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserSession {
    pub session_id: String,
    pub agent_id: String,
    pub profile_dir: PathBuf,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
    pub user_agent: Option<String>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub extra_headers: HashMap<String, String>,
    pub tags: Vec<String>,
}

/// Cookie entry stored in SQLite
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub expires: Option<DateTime<Utc>>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: String,
}

/// LocalStorage / SessionStorage entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageEntry {
    pub origin: String,
    pub key: String,
    pub value: String,
    pub storage_type: StorageType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    Local,
    Session,
    IndexedDb,
}

/// Configuration for the browser session manager
#[derive(Debug, Clone)]
pub struct BrowserManagerConfig {
    /// Base directory for browser profiles
    pub profiles_base_dir: PathBuf,
    /// Maximum number of concurrent sessions per agent
    pub max_sessions_per_agent: usize,
    /// Session TTL in seconds (0 = no expiry)
    pub session_ttl_secs: u64,
    /// Default viewport width
    pub default_viewport_width: u32,
    /// Default viewport height
    pub default_viewport_height: u32,
}

impl Default for BrowserManagerConfig {
    fn default() -> Self {
        let profiles_base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("zylos")
            .join("browser-profiles");
        Self {
            profiles_base_dir: profiles_base,
            max_sessions_per_agent: 5,
            session_ttl_secs: 0,
            default_viewport_width: 1280,
            default_viewport_height: 720,
        }
    }
}

/// Manages persistent browser sessions with per-agent isolation
pub struct BrowserSessionManager {
    pool: SqlitePool,
    config: BrowserManagerConfig,
}

impl BrowserSessionManager {
    /// Create a new BrowserSessionManager with the given SQLite pool
    pub async fn new(pool: SqlitePool, config: BrowserManagerConfig) -> Result<Self, BrowserError> {
        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS browser_sessions (
                session_id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                profile_dir TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_used_at TEXT NOT NULL,
                user_agent TEXT,
                viewport_width INTEGER NOT NULL DEFAULT 1280,
                viewport_height INTEGER NOT NULL DEFAULT 720,
                extra_headers TEXT NOT NULL DEFAULT '{}',
                tags TEXT NOT NULL DEFAULT '[]'
            );
            CREATE INDEX IF NOT EXISTS idx_browser_sessions_agent ON browser_sessions(agent_id);

            CREATE TABLE IF NOT EXISTS browser_cookies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES browser_sessions(session_id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                value TEXT NOT NULL,
                domain TEXT NOT NULL,
                path TEXT NOT NULL DEFAULT '/',
                expires TEXT,
                secure INTEGER NOT NULL DEFAULT 0,
                http_only INTEGER NOT NULL DEFAULT 0,
                same_site TEXT NOT NULL DEFAULT 'lax',
                UNIQUE(session_id, name, domain, path)
            );
            CREATE INDEX IF NOT EXISTS idx_browser_cookies_session ON browser_cookies(session_id);

            CREATE TABLE IF NOT EXISTS browser_storage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL REFERENCES browser_sessions(session_id) ON DELETE CASCADE,
                origin TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                storage_type TEXT NOT NULL DEFAULT 'local',
                UNIQUE(session_id, origin, key, storage_type)
            );
            CREATE INDEX IF NOT EXISTS idx_browser_storage_session ON browser_storage(session_id);
            "#,
        )
        .execute(&pool)
        .await?;

        // Ensure profiles base directory exists
        tokio::fs::create_dir_all(&config.profiles_base_dir).await?;

        Ok(Self { pool, config })
    }

    /// Create or resume a browser session for an agent
    pub async fn get_or_create_session(
        &self,
        agent_id: &str,
        tags: &[String],
    ) -> Result<BrowserSession, BrowserError> {
        // Check for existing active session
        let existing = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i32, i32, String, String)>(
            "SELECT session_id, agent_id, profile_dir, created_at, last_used_at, user_agent, viewport_width, viewport_height, extra_headers, tags FROM browser_sessions WHERE agent_id = ? ORDER BY last_used_at DESC LIMIT 1"
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = existing {
            let session = BrowserSession {
                session_id: row.0,
                agent_id: row.1,
                profile_dir: PathBuf::from(row.2),
                created_at: DateTime::parse_from_rfc3339(&row.3)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_used_at: DateTime::parse_from_rfc3339(&row.4)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                user_agent: row.5,
                viewport_width: row.6 as u32,
                viewport_height: row.7 as u32,
                extra_headers: serde_json::from_str(&row.8).unwrap_or_default(),
                tags: serde_json::from_str(&row.9).unwrap_or_default(),
            };

            // Check TTL
            if self.config.session_ttl_secs > 0 {
                let elapsed = Utc::now()
                    .signed_duration_since(session.last_used_at)
                    .num_seconds() as u64;
                if elapsed > self.config.session_ttl_secs {
                    // Session expired — create new one
                    return self.create_new_session(agent_id, tags).await;
                }
            }

            // Touch last_used_at
            sqlx::query("UPDATE browser_sessions SET last_used_at = ? WHERE session_id = ?")
                .bind(Utc::now().to_rfc3339())
                .bind(&session.session_id)
                .execute(&self.pool)
                .await?;

            return Ok(session);
        }

        self.create_new_session(agent_id, tags).await
    }

    async fn create_new_session(
        &self,
        agent_id: &str,
        tags: &[String],
    ) -> Result<BrowserSession, BrowserError> {
        // Enforce max sessions per agent
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM browser_sessions WHERE agent_id = ?",
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await?;

        if count.0 as usize >= self.config.max_sessions_per_agent {
            // Evict oldest session
            let oldest: Option<(String,)> = sqlx::query_as(
                "SELECT session_id FROM browser_sessions WHERE agent_id = ? ORDER BY last_used_at ASC LIMIT 1",
            )
            .bind(agent_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some((old_id,)) = oldest {
                self.destroy_session(&old_id).await?;
            }
        }

        let session_id = Uuid::new_v4().to_string();
        let profile_dir = self.config.profiles_base_dir.join(agent_id).join(&session_id);
        tokio::fs::create_dir_all(&profile_dir).await.map_err(|e| {
            BrowserError::ProfileCreation(format!("{}: {}", profile_dir.display(), e))
        })?;

        let now = Utc::now();
        let headers_json = serde_json::to_string(&HashMap::<String, String>::new())?;
        let tags_json = serde_json::to_string(tags)?;

        sqlx::query(
            r#"INSERT INTO browser_sessions
               (session_id, agent_id, profile_dir, created_at, last_used_at, user_agent, viewport_width, viewport_height, extra_headers, tags)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&session_id)
        .bind(agent_id)
        .bind(profile_dir.to_string_lossy().as_ref())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(None::<String>)
        .bind(self.config.default_viewport_width as i32)
        .bind(self.config.default_viewport_height as i32)
        .bind(&headers_json)
        .bind(&tags_json)
        .execute(&self.pool)
        .await?;

        Ok(BrowserSession {
            session_id,
            agent_id: agent_id.to_string(),
            profile_dir,
            created_at: now,
            last_used_at: now,
            user_agent: None,
            viewport_width: self.config.default_viewport_width,
            viewport_height: self.config.default_viewport_height,
            extra_headers: HashMap::new(),
            tags: tags.to_vec(),
        })
    }

    /// Store cookies for a session
    pub async fn store_cookies(
        &self,
        session_id: &str,
        cookies: &[StoredCookie],
    ) -> Result<(), BrowserError> {
        for cookie in cookies {
            sqlx::query(
                r#"INSERT OR REPLACE INTO browser_cookies
                   (session_id, name, value, domain, path, expires, secure, http_only, same_site)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            )
            .bind(session_id)
            .bind(&cookie.name)
            .bind(&cookie.value)
            .bind(&cookie.domain)
            .bind(&cookie.path)
            .bind(cookie.expires.map(|e| e.to_rfc3339()))
            .bind(cookie.secure)
            .bind(cookie.http_only)
            .bind(&cookie.same_site)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Retrieve all cookies for a session
    pub async fn get_cookies(&self, session_id: &str) -> Result<Vec<StoredCookie>, BrowserError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, bool, bool, String)>(
            "SELECT name, value, domain, path, expires, secure, http_only, same_site FROM browser_cookies WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| StoredCookie {
                name: r.0,
                value: r.1,
                domain: r.2,
                path: r.3,
                expires: r.4.and_then(|s| DateTime::parse_from_rfc3339(&s).ok()).map(|d| d.with_timezone(&Utc)),
                secure: r.5,
                http_only: r.6,
                same_site: r.7,
            })
            .collect())
    }

    /// Store a storage entry (localStorage / sessionStorage / IndexedDB)
    pub async fn store_storage_entry(
        &self,
        session_id: &str,
        entry: &StorageEntry,
    ) -> Result<(), BrowserError> {
        let stype = match entry.storage_type {
            StorageType::Local => "local",
            StorageType::Session => "session",
            StorageType::IndexedDb => "indexeddb",
        };
        sqlx::query(
            r#"INSERT OR REPLACE INTO browser_storage
               (session_id, origin, key, value, storage_type)
               VALUES (?, ?, ?, ?, ?)"#,
        )
        .bind(session_id)
        .bind(&entry.origin)
        .bind(&entry.key)
        .bind(&entry.value)
        .bind(stype)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all storage entries for a session
    pub async fn get_storage_entries(
        &self,
        session_id: &str,
    ) -> Result<Vec<StorageEntry>, BrowserError> {
        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT origin, key, value, storage_type FROM browser_storage WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| StorageEntry {
                origin: r.0,
                key: r.1,
                value: r.2,
                storage_type: match r.3.as_str() {
                    "session" => StorageType::Session,
                    "indexeddb" => StorageType::IndexedDb,
                    _ => StorageType::Local,
                },
            })
            .collect())
    }

    /// List all sessions for an agent
    pub async fn list_sessions(&self, agent_id: &str) -> Result<Vec<BrowserSession>, BrowserError> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i32, i32, String, String)>(
            "SELECT session_id, agent_id, profile_dir, created_at, last_used_at, user_agent, viewport_width, viewport_height, extra_headers, tags FROM browser_sessions WHERE agent_id = ? ORDER BY last_used_at DESC",
        )
        .bind(agent_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BrowserSession {
                session_id: r.0,
                agent_id: r.1,
                profile_dir: PathBuf::from(r.2),
                created_at: DateTime::parse_from_rfc3339(&r.3)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                last_used_at: DateTime::parse_from_rfc3339(&r.4)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                user_agent: r.5,
                viewport_width: r.6 as u32,
                viewport_height: r.7 as u32,
                extra_headers: serde_json::from_str(&r.8).unwrap_or_default(),
                tags: serde_json::from_str(&r.9).unwrap_or_default(),
            })
            .collect())
    }

    /// Destroy a session and clean up its profile directory
    pub async fn destroy_session(&self, session_id: &str) -> Result<(), BrowserError> {
        // Get profile dir before deleting
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT profile_dir FROM browser_sessions WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        // Delete from DB (cascades to cookies and storage)
        sqlx::query("DELETE FROM browser_sessions WHERE session_id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;

        // Clean up profile directory
        if let Some((dir,)) = row {
            let path = Path::new(&dir);
            if path.exists() {
                tokio::fs::remove_dir_all(path).await.ok();
            }
        }

        Ok(())
    }

    /// Update session viewport and user agent
    pub async fn update_session_config(
        &self,
        session_id: &str,
        viewport_width: Option<u32>,
        viewport_height: Option<u32>,
        user_agent: Option<String>,
    ) -> Result<(), BrowserError> {
        let mut sets = Vec::new();
        if viewport_width.is_some() {
            sets.push("viewport_width = ?");
        }
        if viewport_height.is_some() {
            sets.push("viewport_height = ?");
        }
        if user_agent.is_some() {
            sets.push("user_agent = ?");
        }
        sets.push("last_used_at = ?");

        let query = format!(
            "UPDATE browser_sessions SET {} WHERE session_id = ?",
            sets.join(", ")
        );

        let mut q = sqlx::query(&query);
        if let Some(w) = viewport_width {
            q = q.bind(w as i32);
        }
        if let Some(h) = viewport_height {
            q = q.bind(h as i32);
        }
        if let Some(ref ua) = user_agent {
            q = q.bind(ua);
        }
        q = q.bind(Utc::now().to_rfc3339());
        q = q.bind(session_id);
        q.execute(&self.pool).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_retrieve_session() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let config = BrowserManagerConfig {
            profiles_base_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mgr = BrowserSessionManager::new(pool, config).await.unwrap();

        let session = mgr
            .get_or_create_session("agent-test", &["web".to_string()])
            .await
            .unwrap();
        assert_eq!(session.agent_id, "agent-test");
        assert!(session.profile_dir.exists());

        // Retrieve same session
        let session2 = mgr
            .get_or_create_session("agent-test", &[])
            .await
            .unwrap();
        assert_eq!(session.session_id, session2.session_id);
    }

    #[tokio::test]
    async fn test_cookie_roundtrip() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let config = BrowserManagerConfig {
            profiles_base_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mgr = BrowserSessionManager::new(pool, config).await.unwrap();

        let session = mgr
            .get_or_create_session("agent-cookies", &[])
            .await
            .unwrap();

        let cookies = vec![StoredCookie {
            name: "session_id".into(),
            value: "abc123".into(),
            domain: ".example.com".into(),
            path: "/".into(),
            expires: None,
            secure: true,
            http_only: true,
            same_site: "strict".into(),
        }];

        mgr.store_cookies(&session.session_id, &cookies).await.unwrap();
        let retrieved = mgr.get_cookies(&session.session_id).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].name, "session_id");
        assert_eq!(retrieved[0].value, "abc123");
    }

    #[tokio::test]
    async fn test_destroy_session_cleans_profile() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let config = BrowserManagerConfig {
            profiles_base_dir: tmp.path().to_path_buf(),
            ..Default::default()
        };
        let mgr = BrowserSessionManager::new(pool, config).await.unwrap();

        let session = mgr
            .get_or_create_session("agent-destroy", &[])
            .await
            .unwrap();
        assert!(session.profile_dir.exists());

        mgr.destroy_session(&session.session_id).await.unwrap();
        assert!(!session.profile_dir.exists());
    }
}