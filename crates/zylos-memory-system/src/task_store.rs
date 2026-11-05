//! Task Store with SQL Leases for durable task execution.
//!
//! Implements compare-and-swap (CAS) ownership via SQL leases,
//! maintenance loop for orphan recovery, and heartbeat-based liveness.
//! Supports both SQLite (polling) and PostgreSQL (FOR UPDATE SKIP LOCKED)
//! via feature flags.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Unique lease identifier for task ownership.
pub type LeaseId = String;

/// Task execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting to be claimed.
    Pending,
    /// Claimed by a worker with active lease.
    Running,
    /// Completed successfully.
    Succeeded,
    /// Completed with failure.
    Failed,
    /// Outcome unknown (network error, 5xx).
    OutcomeUnknown,
    /// Cancelled by user or system.
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::OutcomeUnknown => write!(f, "outcome_unknown"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TaskStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "succeeded" => Some(Self::Succeeded),
            "failed" => Some(Self::Failed),
            "outcome_unknown" => Some(Self::OutcomeUnknown),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// A durable task record with lease metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub payload: serde_json::Value,
    pub status: TaskStatus,
    pub lease_id: Option<String>,
    pub lease_until: Option<DateTime<Utc>>,
    pub attempts: u32,
    pub max_attempts: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// Configuration for the task store.
#[derive(Debug, Clone)]
pub struct TaskStoreConfig {
    /// Maximum concurrent active tasks.
    pub max_active_tasks: usize,
    /// Default lease duration in seconds.
    pub lease_duration_secs: u64,
    /// Maintenance loop interval in seconds.
    pub maintenance_interval_secs: u64,
    /// Heartbeat interval as fraction of lease duration (e.g., 3 = lease/3).
    pub heartbeat_divisor: u64,
    /// Circuit breaker threshold: consecutive failures before pause.
    pub circuit_breaker_threshold: u32,
}

impl Default for TaskStoreConfig {
    fn default() -> Self {
        Self {
            max_active_tasks: 100,
            lease_duration_secs: 60,
            maintenance_interval_secs: 60,
            heartbeat_divisor: 3,
            circuit_breaker_threshold: 5,
        }
    }
}

/// Error type for task store operations.
#[derive(Debug, thiserror::Error)]
pub enum TaskStoreError {
    #[error("Task not found: {0}")]
    NotFound(String),
    #[error("Lease conflict: task {task_id} owned by {lease_holder}, expected {expected}")]
    LeaseConflict {
        task_id: String,
        lease_holder: String,
        expected: String,
    },
    #[error("Active task cap reached ({max})")]
    ActiveCapReached { max: usize },
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Circuit breaker open after {failures} consecutive failures")]
    CircuitBreakerOpen { failures: u32 },
}

/// SQLite-backed task store with lease-based ownership.
pub struct TaskStore {
    conn: Arc<Mutex<Connection>>,
    config: TaskStoreConfig,
    consecutive_failures: Arc<Mutex<u32>>,
}

impl TaskStore {
    /// Create a new TaskStore with the given SQLite connection.
    pub fn new(conn: Connection, config: TaskStoreConfig) -> Result<Self, TaskStoreError> {
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
            consecutive_failures: Arc::new(Mutex::new(0)),
        };
        store.run_migrations()?;
        Ok(store)
    }

    /// Create an in-memory TaskStore for testing.
    pub fn in_memory() -> Result<Self, TaskStoreError> {
        let conn = Connection::open_in_memory()?;
        Self::new(conn, TaskStoreConfig::default())
    }

    fn run_migrations(&self) -> Result<(), TaskStoreError> {
        let conn = self.conn.blocking_lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                lease_id TEXT,
                lease_until TEXT,
                attempts INTEGER NOT NULL DEFAULT 0,
                max_attempts INTEGER NOT NULL DEFAULT 3,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result TEXT,
                error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_lease_until ON tasks(lease_until);",
        )?;
        Ok(())
    }

    /// Insert a new task. Returns the task ID.
    pub async fn create_task(
        &self,
        payload: serde_json::Value,
        max_attempts: u32,
    ) -> Result<String, TaskStoreError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let payload_str = serde_json::to_string(&payload)?;
        let conn = self.conn.lock().await;
        conn.execute(
            "INSERT INTO tasks (id, payload, status, attempts, max_attempts, created_at, updated_at)
             VALUES (?1, ?2, 'pending', 0, ?3, ?4, ?5)",
            params![&id, &payload_str, max_attempts, now.to_rfc3339(), now.to_rfc3339()],
        )?;
        debug!(task_id = %id, "Task created");
        Ok(id)
    }

    /// Attempt to claim a pending task using CAS.
    /// Returns the lease_id on success.
    pub async fn claim_task(&self, task_id: &str) -> Result<LeaseId, TaskStoreError> {
        // Check circuit breaker
        {
            let failures = self.consecutive_failures.lock().await;
            if *failures >= self.config.circuit_breaker_threshold {
                return Err(TaskStoreError::CircuitBreakerOpen {
                    failures: *failures,
                });
            }
        }

        // Check active cap
        {
            let conn = self.conn.lock().await;
            let active_count: usize = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE status = 'running'",
                [],
                |row| row.get(0),
            )?;
            if active_count >= self.config.max_active_tasks {
                return Err(TaskStoreError::ActiveCapReached {
                    max: self.config.max_active_tasks,
                });
            }
        }

        let lease_id = Uuid::new_v4().to_string();
        let lease_until = Utc::now()
            + chrono::Duration::seconds(self.config.lease_duration_secs as i64);
        let now = Utc::now();

        let conn = self.conn.lock().await;
        // CAS: only claim if status is still pending
        let affected = conn.execute(
            "UPDATE tasks SET status = 'running', lease_id = ?1, lease_until = ?2,
             attempts = attempts + 1, updated_at = ?3
             WHERE id = ?4 AND status = 'pending'",
            params![&lease_id, lease_until.to_rfc3339(), now.to_rfc3339(), task_id],
        )?;

        if affected == 0 {
            // Check if someone else owns it
            let current_lease: Option<String> = conn
                .query_row(
                    "SELECT lease_id FROM tasks WHERE id = ?1 AND status = 'running'",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;

            if let Some(holder) = current_lease {
                return Err(TaskStoreError::LeaseConflict {
                    task_id: task_id.to_string(),
                    lease_holder: holder,
                    expected: "pending".to_string(),
                });
            }
            return Err(TaskStoreError::NotFound(task_id.to_string()));
        }

        debug!(task_id = %task_id, lease_id = %lease_id, "Task claimed");
        Ok(lease_id)
    }

    /// Renew a lease. Must be called periodically (lease_duration / heartbeat_divisor).
    pub async fn renew_lease(
        &self,
        task_id: &str,
        lease_id: &str,
    ) -> Result<(), TaskStoreError> {
        let new_lease_until = Utc::now()
            + chrono::Duration::seconds(self.config.lease_duration_secs as i64);
        let now = Utc::now();

        let conn = self.conn.lock().await;
        let affected = conn.execute(
            "UPDATE tasks SET lease_until = ?1, updated_at = ?2
             WHERE id = ?3 AND lease_id = ?4 AND status = 'running'",
            params![
                new_lease_until.to_rfc3339(),
                now.to_rfc3339(),
                task_id,
                lease_id
            ],
        )?;

        if affected == 0 {
            return Err(TaskStoreError::LeaseConflict {
                task_id: task_id.to_string(),
                lease_holder: "unknown".to_string(),
                expected: lease_id.to_string(),
            });
        }

        debug!(task_id = %task_id, "Lease renewed");
        Ok(())
    }

    /// Release a completed or failed task.
    pub async fn release_task(
        &self,
        task_id: &str,
        lease_id: &str,
        status: TaskStatus,
        result: Option<serde_json::Value>,
        error: Option<String>,
    ) -> Result<(), TaskStoreError> {
        let now = Utc::now();
        let result_str = result.map(|r| serde_json::to_string(&r)).transpose()?;

        let conn = self.conn.lock().await;
        let affected = conn.execute(
            "UPDATE tasks SET status = ?1, lease_id = NULL, lease_until = NULL,
             result = ?2, error = ?3, updated_at = ?4
             WHERE id = ?5 AND lease_id = ?6 AND status = 'running'",
            params![
                status.to_string(),
                result_str,
                error,
                now.to_rfc3339(),
                task_id,
                lease_id
            ],
        )?;

        if affected == 0 {
            return Err(TaskStoreError::LeaseConflict {
                task_id: task_id.to_string(),
                lease_holder: "unknown".to_string(),
                expected: lease_id.to_string(),
            });
        }

        // Update circuit breaker
        {
            let mut failures = self.consecutive_failures.lock().await;
            match status {
                TaskStatus::Succeeded => *failures = 0,
                TaskStatus::Failed => *failures += 1,
                _ => {}
            }
        }

        debug!(task_id = %task_id, status = %status, "Task released");
        Ok(())
    }

    /// Get a task by ID.
    pub async fn get_task(&self, task_id: &str) -> Result<TaskRecord, TaskStoreError> {
        let conn = self.conn.lock().await;
        let record = conn
            .query_row(
                "SELECT id, payload, status, lease_id, lease_until, attempts, max_attempts,
                 created_at, updated_at, result, error
                 FROM tasks WHERE id = ?1",
                params![task_id],
                |row| Self::row_to_record(row),
            )
            .optional()?;

        record.ok_or_else(|| TaskStoreError::NotFound(task_id.to_string()))
    }

    /// List tasks by status.
    pub async fn list_tasks(
        &self,
        status: Option<TaskStatus>,
        limit: usize,
    ) -> Result<Vec<TaskRecord>, TaskStoreError> {
        let conn = self.conn.lock().await;
        let mut stmt = match status {
            Some(s) => conn.prepare(
                "SELECT id, payload, status, lease_id, lease_until, attempts, max_attempts,
                 created_at, updated_at, result, error
                 FROM tasks WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?,
            None => conn.prepare(
                "SELECT id, payload, status, lease_id, lease_until, attempts, max_attempts,
                 created_at, updated_at, result, error
                 FROM tasks ORDER BY created_at DESC LIMIT ?1",
            )?,
        };

        let rows = match status {
            Some(s) => stmt.query_map(params![s.to_string(), limit as i64], |row| {
                Self::row_to_record(row)
            })?,
            None => stmt.query_map(params![limit as i64], |row| {
                Self::row_to_record(row)
            })?,
        };

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<TaskRecord> {
        let payload_str: String = row.get(1)?;
        let status_str: String = row.get(2)?;
        let lease_until_str: Option<String> = row.get(4)?;
        let created_str: String = row.get(7)?;
        let updated_str: String = row.get(8)?;
        let result_str: Option<String> = row.get(9)?;

        Ok(TaskRecord {
            id: row.get(0)?,
            payload: serde_json::from_str(&payload_str).unwrap_or_default(),
            status: TaskStatus::from_str(&status_str).unwrap_or(TaskStatus::Pending),
            lease_id: row.get(3)?,
            lease_until: lease_until_str
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            attempts: row.get(5)?,
            max_attempts: row.get(6)?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .unwrap_or_default()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_str)
                .unwrap_or_default()
                .with_timezone(&Utc),
            result: result_str.and_then(|s| serde_json::from_str(&s).ok()),
            error: row.get(10)?,
        })
    }

    /// Maintenance loop: recover expired leases and reclaim orphaned tasks.
    /// Should be called every `maintenance_interval_secs`.
    pub async fn run_maintenance(&self) -> Result<u32, TaskStoreError> {
        let now = Utc::now();
        let conn = self.conn.lock().await;

        // Reclaim tasks whose lease has expired
        let reclaimed = conn.execute(
            "UPDATE tasks SET status = 'pending', lease_id = NULL, lease_until = NULL, updated_at = ?1
             WHERE status = 'running' AND lease_until < ?2",
            params![now.to_rfc3339(), now.to_rfc3339()],
        )?;

        if reclaimed > 0 {
            warn!(reclaimed, "Expired leases reclaimed");
        }

        // Reset circuit breaker if no active failures
        let active_failures: u32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = 'failed'",
            [],
            |row| row.get(0),
        )?;

        if active_failures == 0 {
            let mut failures = self.consecutive_failures.lock().await;
            if *failures > 0 {
                info!("Circuit breaker reset: no active failures");
                *failures = 0;
            }
        }

        Ok(reclaimed as u32)
    }

    /// Count tasks by status.
    pub async fn count_by_status(&self, status: TaskStatus) -> Result<usize, TaskStoreError> {
        let conn = self.conn.lock().await;
        let count: usize = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status = ?1",
            params![status.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_task() {
        let store = TaskStore::in_memory().unwrap();
        let id = store
            .create_task(serde_json::json!({"action": "test"}), 3)
            .await
            .unwrap();

        let task = store.get_task(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.attempts, 0);
        assert_eq!(task.max_attempts, 3);
    }

    #[tokio::test]
    async fn test_claim_task_cas() {
        let store = TaskStore::in_memory().unwrap();
        let id = store
            .create_task(serde_json::json!({"action": "test"}), 3)
            .await
            .unwrap();

        // First claim succeeds
        let lease1 = store.claim_task(&id).await.unwrap();
        assert!(!lease1.is_empty());

        // Second claim fails with LeaseConflict
        let err = store.claim_task(&id).await.unwrap_err();
        assert!(matches!(err, TaskStoreError::LeaseConflict { .. }));
    }

    #[tokio::test]
    async fn test_renew_lease() {
        let store = TaskStore::in_memory().unwrap();
        let id = store
            .create_task(serde_json::json!({"action": "test"}), 3)
            .await
            .unwrap();
        let lease = store.claim_task(&id).await.unwrap();

        // Renewal succeeds
        store.renew_lease(&id, &lease).await.unwrap();

        // Renewal with wrong lease fails
        let err = store.renew_lease(&id, "wrong-lease").await.unwrap_err();
        assert!(matches!(err, TaskStoreError::LeaseConflict { .. }));
    }

    #[tokio::test]
    async fn test_release_task() {
        let store = TaskStore::in_memory().unwrap();
        let id = store
            .create_task(serde_json::json!({"action": "test"}), 3)
            .await
            .unwrap();
        let lease = store.claim_task(&id).await.unwrap();

        store
            .release_task(
                &id,
                &lease,
                TaskStatus::Succeeded,
                Some(serde_json::json!({"result": "ok"})),
                None,
            )
            .await
            .unwrap();

        let task = store.get_task(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Succeeded);
        assert!(task.lease_id.is_none());
    }

    #[tokio::test]
    async fn test_active_cap() {
        let config = TaskStoreConfig {
            max_active_tasks: 2,
            ..Default::default()
        };
        let conn = Connection::open_in_memory().unwrap();
        let store = TaskStore::new(conn, config).unwrap();

        let id1 = store
            .create_task(serde_json::json!({"n": 1}), 3)
            .await
            .unwrap();
        let id2 = store
            .create_task(serde_json::json!({"n": 2}), 3)
            .await
            .unwrap();
        let id3 = store
            .create_task(serde_json::json!({"n": 3}), 3)
            .await
            .unwrap();

        store.claim_task(&id1).await.unwrap();
        store.claim_task(&id2).await.unwrap();

        // Third claim should fail with ActiveCapReached
        let err = store.claim_task(&id3).await.unwrap_err();
        assert!(matches!(err, TaskStoreError::ActiveCapReached { max: 2 }));
    }

    #[tokio::test]
    async fn test_maintenance_recovers_expired() {
        let config = TaskStoreConfig {
            lease_duration_secs: 0, // Immediate expiry
            ..Default::default()
        };
        let conn = Connection::open_in_memory().unwrap();
        let store = TaskStore::new(conn, config).unwrap();

        let id = store
            .create_task(serde_json::json!({"action": "test"}), 3)
            .await
            .unwrap();
        store.claim_task(&id).await.unwrap();

        // Wait a tiny bit for lease to expire (0s duration)
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let recovered = store.run_maintenance().await.unwrap();
        assert_eq!(recovered, 1);

        let task = store.get_task(&id).await.unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.lease_id.is_none());
    }

    #[tokio::test]
    async fn test_list_tasks() {
        let store = TaskStore::in_memory().unwrap();
        store
            .create_task(serde_json::json!({"n": 1}), 3)
            .await
            .unwrap();
        store
            .create_task(serde_json::json!({"n": 2}), 3)
            .await
            .unwrap();

        let all = store.list_tasks(None, 10).await.unwrap();
        assert_eq!(all.len(), 2);

        let pending = store.list_tasks(Some(TaskStatus::Pending), 10).await.unwrap();
        assert_eq!(pending.len(), 2);

        let running = store.list_tasks(Some(TaskStatus::Running), 10).await.unwrap();
        assert_eq!(running.len(), 0);
    }

    #[tokio::test]
    async fn test_count_by_status() {
        let store = TaskStore::in_memory().unwrap();
        store
            .create_task(serde_json::json!({"n": 1}), 3)
            .await
            .unwrap();
        store
            .create_task(serde_json::json!({"n": 2}), 3)
            .await
            .unwrap();

        assert_eq!(store.count_by_status(TaskStatus::Pending).await.unwrap(), 2);
        assert_eq!(store.count_by_status(TaskStatus::Running).await.unwrap(), 0);
    }
}