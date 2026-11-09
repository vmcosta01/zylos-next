//! # zylos-memory-sync
//!
//! Cross-harness memory synchronization for zylos-gateway.
//!
//! Provides a unified memory layer that synchronizes agent memory across
//! different frameworks (ADK, Claude SDK, CrewAI, LangGraph, LlamaIndex)
//! using content-addressed storage with SHA-256 deduplication and
//! conflict-free replicated data types (CRDTs) for eventual consistency.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqliteRow};
use sqlx::Row;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// A single memory entry, content-addressed by SHA-256 hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Content-addressed ID (SHA-256 of namespace + key + content).
    pub id: String,
    /// Logical namespace (e.g., "agent:claude-sdk:researcher").
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The stored content (UTF-8 text or JSON string).
    pub content: String,
    /// Content type hint ("text/plain", "application/json").
    pub content_type: String,
    /// Monotonic version counter for CRDT merge.
    pub version: u64,
    /// Last writer wins timestamp.
    pub updated_at: DateTime<Utc>,
    /// Origin harness that produced this entry.
    pub source_harness: String,
    /// Optional tags for filtering.
    pub tags: Vec<String>,
    /// TTL in seconds; None = permanent.
    pub ttl_seconds: Option<u64>,
}

impl MemoryEntry {
    /// Computes the content-addressed ID from namespace, key, and content.
    pub fn compute_id(namespace: &str, key: &str, content: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(namespace.as_bytes());
        hasher.update(b"\x00");
        hasher.update(key.as_bytes());
        hasher.update(b"\x00");
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Creates a new entry with auto-computed ID.
    pub fn new(
        namespace: impl Into<String>,
        key: impl Into<String>,
        content: impl Into<String>,
        source_harness: impl Into<String>,
    ) -> Self {
        let ns = namespace.into();
        let k = key.into();
        let c = content.into();
        let id = Self::compute_id(&ns, &k, &c);
        Self {
            id,
            namespace: ns,
            key: k,
            content: c,
            content_type: "text/plain".to_string(),
            version: 1,
            updated_at: Utc::now(),
            source_harness: source_harness.into(),
            tags: Vec::new(),
            ttl_seconds: None,
        }
    }

    /// Returns true if this entry has expired based on its TTL.
    pub fn is_expired(&self) -> bool {
        if let Some(ttl) = self.ttl_seconds {
            let elapsed = Utc::now()
                .signed_duration_since(self.updated_at)
                .num_seconds();
            elapsed > ttl as i64
        } else {
            false
        }
    }
}

/// A sync operation representing a change to be propagated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncOp {
    /// Unique identifier for this sync operation (UUID v4).
    pub op_id: String,
    /// The content-addressed ID of the memory entry this operation targets.
    pub entry_id: String,
    /// The type of sync operation (upsert, delete, or touch).
    pub op_type: SyncOpType,
    /// When this operation was recorded.
    pub timestamp: DateTime<Utc>,
    /// The harness that originated this operation.
    pub source_harness: String,
}

/// The type of a sync operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOpType {
    /// Insert or update a memory entry.
    Upsert,
    /// Delete a memory entry.
    Delete,
    /// Touch (bump version/timestamp without changing content).
    Touch,
}

/// Result of a sync reconciliation between two harnesses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    /// Number of entries inserted or updated during sync.
    pub entries_upserted: usize,
    /// Number of entries deleted during sync.
    pub entries_deleted: usize,
    /// Number of version conflicts resolved via last-writer-wins.
    pub conflicts_resolved: usize,
    /// Non-fatal errors encountered during sync.
    pub errors: Vec<String>,
}

/// Query filter for memory lookups.
#[derive(Debug, Clone, Default)]
pub struct MemoryQuery {
    /// Filter by exact namespace match.
    pub namespace: Option<String>,
    /// Filter by key prefix (SQL LIKE pattern).
    pub key_prefix: Option<String>,
    /// Filter by tag substring match.
    pub tag: Option<String>,
    /// Maximum number of results to return.
    pub limit: Option<usize>,
    /// Number of results to skip (for pagination).
    pub offset: Option<usize>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors that can occur during memory sync operations.
#[derive(Debug, thiserror::Error)]
pub enum MemorySyncError {
    /// Underlying SQLite database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    /// JSON serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// Requested entry does not exist.
    #[error("Entry not found: {0}")]
    NotFound(String),
    /// Version conflict detected during merge.
    #[error("Conflict: entry {0} has divergent versions")]
    Conflict(String),
    /// Filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Catch-all for internal invariant violations.
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Memory Store
// ---------------------------------------------------------------------------

/// SQLite WAL-backed memory store with content-addressed deduplication.
#[derive(Clone)]
pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    /// Opens or creates the memory store at the given path.
    pub async fn open(path: &str) -> Result<Self, MemorySyncError> {
        let options = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await?;
        let store = Self { pool };
        store.run_migrations().await?;
        Ok(store)
    }

    async fn run_migrations(&self) -> Result<(), MemorySyncError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memory_entries (
                id TEXT PRIMARY KEY,
                namespace TEXT NOT NULL,
                key TEXT NOT NULL,
                content TEXT NOT NULL,
                content_type TEXT NOT NULL DEFAULT 'text/plain',
                version INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL,
                source_harness TEXT NOT NULL,
                tags TEXT NOT NULL DEFAULT '[]',
                ttl_seconds INTEGER,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_memory_ns_key ON memory_entries(namespace, key);
            CREATE INDEX IF NOT EXISTS idx_memory_tags ON memory_entries(tags);
            CREATE INDEX IF NOT EXISTS idx_memory_updated ON memory_entries(updated_at);

            CREATE TABLE IF NOT EXISTS sync_ops (
                op_id TEXT PRIMARY KEY,
                entry_id TEXT NOT NULL,
                op_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                source_harness TEXT NOT NULL,
                applied INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_sync_applied ON sync_ops(applied, timestamp);

            CREATE TABLE IF NOT EXISTS sync_cursors (
                harness_id TEXT PRIMARY KEY,
                last_synced_op TEXT,
                last_synced_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Inserts or updates a memory entry. Uses content-addressed dedup:
    /// if an entry with the same (namespace, key) exists but different content,
    /// the version is bumped and the old content is replaced.
    pub async fn upsert(&self, entry: &MemoryEntry) -> Result<bool, MemorySyncError> {
        let tags_json = serde_json::to_string(&entry.tags)?;
        let existing = sqlx::query(
            "SELECT id, version, content FROM memory_entries WHERE namespace = ? AND key = ?",
        )
        .bind(&entry.namespace)
        .bind(&entry.key)
        .fetch_optional(&self.pool)
        .await?;

        match existing {
            Some(row) => {
                let old_content: String = row.get("content");
                let old_version: u64 = row.get::<i64, _>("version") as u64;
                if old_content == entry.content {
                    // Same content, just touch
                    sqlx::query(
                        "UPDATE memory_entries SET updated_at = ?, version = version + 1 WHERE namespace = ? AND key = ?",
                    )
                    .bind(Utc::now().to_rfc3339())
                    .bind(&entry.namespace)
                    .bind(&entry.key)
                    .execute(&self.pool)
                    .await?;
                    return Ok(false);
                }
                // Different content: bump version
                let new_version = old_version + 1;
                sqlx::query(
                    r#"UPDATE memory_entries SET
                        id = ?, content = ?, content_type = ?, version = ?,
                        updated_at = ?, source_harness = ?, tags = ?, ttl_seconds = ?
                    WHERE namespace = ? AND key = ?"#,
                )
                .bind(&entry.id)
                .bind(&entry.content)
                .bind(&entry.content_type)
                .bind(new_version as i64)
                .bind(entry.updated_at.to_rfc3339())
                .bind(&entry.source_harness)
                .bind(&tags_json)
                .bind(entry.ttl_seconds.map(|t| t as i64))
                .bind(&entry.namespace)
                .bind(&entry.key)
                .execute(&self.pool)
                .await?;
                Ok(true)
            }
            None => {
                sqlx::query(
                    r#"INSERT INTO memory_entries
                        (id, namespace, key, content, content_type, version, updated_at, source_harness, tags, ttl_seconds)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
                )
                .bind(&entry.id)
                .bind(&entry.namespace)
                .bind(&entry.key)
                .bind(&entry.content)
                .bind(&entry.content_type)
                .bind(entry.version as i64)
                .bind(entry.updated_at.to_rfc3339())
                .bind(&entry.source_harness)
                .bind(&tags_json)
                .bind(entry.ttl_seconds.map(|t| t as i64))
                .execute(&self.pool)
                .await?;
                Ok(true)
            }
        }
    }

    /// Retrieves a memory entry by namespace and key.
    pub async fn get(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<MemoryEntry>, MemorySyncError> {
        let row = sqlx::query(
            "SELECT * FROM memory_entries WHERE namespace = ? AND key = ?",
        )
        .bind(namespace)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(row_to_entry(&r))),
            None => Ok(None),
        }
    }

    /// Queries memory entries with optional filters.
    pub async fn query(&self, q: &MemoryQuery) -> Result<Vec<MemoryEntry>, MemorySyncError> {
        let mut sql = String::from("SELECT * FROM memory_entries WHERE 1=1");
        let mut conditions = Vec::new();

        if q.namespace.is_some() {
            conditions.push("namespace = ?");
        }
        if q.key_prefix.is_some() {
            conditions.push("key LIKE ?");
        }
        if q.tag.is_some() {
            conditions.push("tags LIKE ?");
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY updated_at DESC");
        if q.limit.is_some() {
            sql.push_str(" LIMIT ?");
        }
        if q.offset.is_some() {
            sql.push_str(" OFFSET ?");
        }

        let mut query = sqlx::query(&sql);
        if let Some(ref ns) = q.namespace {
            query = query.bind(ns);
        }
        if let Some(ref kp) = q.key_prefix {
            query = query.bind(format!("{}%", kp));
        }
        if let Some(ref tag) = q.tag {
            query = query.bind(format!("%{}%", tag));
        }
        if let Some(limit) = q.limit {
            query = query.bind(limit as i64);
        }
        if let Some(offset) = q.offset {
            query = query.bind(offset as i64);
        }

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(row_to_entry).collect())
    }

    /// Deletes a memory entry by namespace and key.
    pub async fn delete(&self, namespace: &str, key: &str) -> Result<bool, MemorySyncError> {
        let result = sqlx::query("DELETE FROM memory_entries WHERE namespace = ? AND key = ?")
            .bind(namespace)
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Purges expired entries based on TTL.
    pub async fn purge_expired(&self) -> Result<usize, MemorySyncError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            r#"DELETE FROM memory_entries
            WHERE ttl_seconds IS NOT NULL
            AND datetime(updated_at, '+' || ttl_seconds || ' seconds') < ?"#,
        )
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as usize)
    }

    /// Records a sync operation for propagation to other harnesses.
    pub async fn record_sync_op(
        &self,
        entry_id: &str,
        op_type: SyncOpType,
        source_harness: &str,
    ) -> Result<String, MemorySyncError> {
        let op_id = Uuid::new_v4().to_string();
        let op_type_str = match op_type {
            SyncOpType::Upsert => "upsert",
            SyncOpType::Delete => "delete",
            SyncOpType::Touch => "touch",
        };
        sqlx::query(
            "INSERT INTO sync_ops (op_id, entry_id, op_type, timestamp, source_harness) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&op_id)
        .bind(entry_id)
        .bind(op_type_str)
        .bind(Utc::now().to_rfc3339())
        .bind(source_harness)
        .execute(&self.pool)
        .await?;
        Ok(op_id)
    }

    /// Retrieves unapplied sync operations since the given cursor.
    pub async fn get_pending_ops(
        &self,
        since: Option<&str>,
        exclude_harness: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SyncOp>, MemorySyncError> {
        let mut sql = String::from(
            "SELECT * FROM sync_ops WHERE applied = 0",
        );
        if since.is_some() {
            sql.push_str(" AND timestamp > ?");
        }
        if exclude_harness.is_some() {
            sql.push_str(" AND source_harness != ?");
        }
        sql.push_str(" ORDER BY timestamp ASC LIMIT ?");

        let mut query = sqlx::query(&sql);
        if let Some(s) = since {
            query = query.bind(s);
        }
        if let Some(h) = exclude_harness {
            query = query.bind(h);
        }
        query = query.bind(limit as i64);

        let rows = query.fetch_all(&self.pool).await?;
        let ops = rows
            .iter()
            .map(|r| {
                let op_type_str: String = r.get("op_type");
                let op_type = match op_type_str.as_str() {
                    "upsert" => SyncOpType::Upsert,
                    "delete" => SyncOpType::Delete,
                    "touch" => SyncOpType::Touch,
                    _ => SyncOpType::Upsert,
                };
                SyncOp {
                    op_id: r.get("op_id"),
                    entry_id: r.get("entry_id"),
                    op_type,
                    timestamp: DateTime::parse_from_rfc3339(&r.get::<String, _>("timestamp"))
                        .unwrap_or_else(|_| Utc::now().into())
                        .with_timezone(&Utc),
                    source_harness: r.get("source_harness"),
                }
            })
            .collect();
        Ok(ops)
    }

    /// Marks sync operations as applied up to the given op_id.
    pub async fn mark_ops_applied(&self, up_to_op_id: &str) -> Result<usize, MemorySyncError> {
        let result = sqlx::query(
            "UPDATE sync_ops SET applied = 1 WHERE op_id <= ? AND applied = 0",
        )
        .bind(up_to_op_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as usize)
    }

    /// Updates the sync cursor for a harness.
    pub async fn update_cursor(
        &self,
        harness_id: &str,
        last_op_id: &str,
    ) -> Result<(), MemorySyncError> {
        sqlx::query(
            r#"INSERT INTO sync_cursors (harness_id, last_synced_op, last_synced_at)
            VALUES (?, ?, ?)
            ON CONFLICT(harness_id) DO UPDATE SET
                last_synced_op = excluded.last_synced_op,
                last_synced_at = excluded.last_synced_at"#,
        )
        .bind(harness_id)
        .bind(last_op_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Gets the current sync cursor for a harness.
    pub async fn get_cursor(&self, harness_id: &str) -> Result<Option<String>, MemorySyncError> {
        let row = sqlx::query("SELECT last_synced_op FROM sync_cursors WHERE harness_id = ?")
            .bind(harness_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get("last_synced_op")))
    }

    /// Returns total entry count and namespace breakdown.
    pub async fn stats(&self) -> Result<MemoryStats, MemorySyncError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_entries")
            .fetch_one(&self.pool)
            .await?;
        let pending_ops: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sync_ops WHERE applied = 0")
                .fetch_one(&self.pool)
                .await?;
        let namespaces: Vec<(String, i64)> = sqlx::query(
            "SELECT namespace, COUNT(*) as cnt FROM memory_entries GROUP BY namespace ORDER BY cnt DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| (r.get("namespace"), r.get("cnt")))
        .collect();

        Ok(MemoryStats {
            total_entries: total as usize,
            pending_sync_ops: pending_ops as usize,
            namespace_counts: namespaces
                .into_iter()
                .map(|(ns, cnt)| (ns, cnt as usize))
                .collect(),
        })
    }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

/// Aggregate statistics about the memory store contents and sync state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total number of memory entries across all namespaces.
    pub total_entries: usize,
    /// Number of sync operations not yet applied by downstream harnesses.
    pub pending_sync_ops: usize,
    /// Entry count broken down by namespace.
    pub namespace_counts: HashMap<String, usize>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_entry(row: &SqliteRow) -> MemoryEntry {
    let tags_str: String = row.get("tags");
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
    let ttl: Option<i64> = row.get("ttl_seconds");
    MemoryEntry {
        id: row.get("id"),
        namespace: row.get("namespace"),
        key: row.get("key"),
        content: row.get("content"),
        content_type: row.get("content_type"),
        version: row.get::<i64, _>("version") as u64,
        updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        source_harness: row.get("source_harness"),
        tags,
        ttl_seconds: ttl.map(|t| t as u64),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_address_deterministic() {
        let id1 = MemoryEntry::compute_id("ns", "key", "hello");
        let id2 = MemoryEntry::compute_id("ns", "key", "hello");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_content_address_changes_with_content() {
        let id1 = MemoryEntry::compute_id("ns", "key", "hello");
        let id2 = MemoryEntry::compute_id("ns", "key", "world");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_ttl_expiry() {
        let mut entry = MemoryEntry::new("ns", "k", "v", "test");
        entry.ttl_seconds = Some(0);
        entry.updated_at = Utc::now() - chrono::Duration::seconds(10);
        assert!(entry.is_expired());
    }

    #[test]
    fn test_no_ttl_never_expires() {
        let entry = MemoryEntry::new("ns", "k", "v", "test");
        assert!(!entry.is_expired());
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let store = MemoryStore::open(":memory:").await.unwrap();
        let entry = MemoryEntry::new("test-ns", "test-key", "test-content", "unit-test");
        let changed = store.upsert(&entry).await.unwrap();
        assert!(changed);

        let retrieved = store.get("test-ns", "test-key").await.unwrap().unwrap();
        assert_eq!(retrieved.content, "test-content");
        assert_eq!(retrieved.version, 1);
    }

    #[tokio::test]
    async fn test_upsert_same_content_bumps_version() {
        let store = MemoryStore::open(":memory:").await.unwrap();
        let entry = MemoryEntry::new("ns", "k", "same", "test");
        store.upsert(&entry).await.unwrap();
        let changed = store.upsert(&entry).await.unwrap();
        assert!(!changed); // same content = no change

        let retrieved = store.get("ns", "k").await.unwrap().unwrap();
        assert_eq!(retrieved.version, 2); // version still bumped
    }

    #[tokio::test]
    async fn test_delete() {
        let store = MemoryStore::open(":memory:").await.unwrap();
        let entry = MemoryEntry::new("ns", "k", "v", "test");
        store.upsert(&entry).await.unwrap();
        let deleted = store.delete("ns", "k").await.unwrap();
        assert!(deleted);
        assert!(store.get("ns", "k").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_stats() {
        let store = MemoryStore::open(":memory:").await.unwrap();
        store
            .upsert(&MemoryEntry::new("a", "k1", "v1", "test"))
            .await
            .unwrap();
        store
            .upsert(&MemoryEntry::new("b", "k2", "v2", "test"))
            .await
            .unwrap();
        let stats = store.stats().await.unwrap();
        assert_eq!(stats.total_entries, 2);
        assert_eq!(stats.namespace_counts.len(), 2);
    }
}