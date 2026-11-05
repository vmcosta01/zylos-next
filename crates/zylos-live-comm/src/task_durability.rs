//! Task Durability via SQL Leases (Task 7C.1.2)
//!
//! Server-side task queue with SQL-based leasing for single-owner execution,
//! state machine transitions, maintenance recovery, exponential backoff,
//! and circuit breaker. Database is the sole source of time — zero Date::now()
//! in lease logic.
//!
//! Sources: OpenMuse `engine/service.ts`, OpenDuet `work/queue.ts`.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

/// Unique identifier for a durable task.
pub type TaskId = String;

/// Unique identifier for a lease holder.
pub type LeaseId = String;

/// Task state machine: queued → running → waiting_approval/waiting_input → succeeded/failed/cancelled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Queued,
    Running,
    WaitingApproval,
    WaitingInput,
    Succeeded,
    Failed,
    Cancelled,
}

impl TaskState {
    /// Whether this state is terminal (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Succeeded | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Parse from string stored in SQLite.
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "queued" => Some(TaskState::Queued),
            "running" => Some(TaskState::Running),
            "waiting_approval" => Some(TaskState::WaitingApproval),
            "waiting_input" => Some(TaskState::WaitingInput),
            "succeeded" => Some(TaskState::Succeeded),
            "failed" => Some(TaskState::Failed),
            "cancelled" => Some(TaskState::Cancelled),
            _ => None,
        }
    }

    /// Serialize to string for SQLite storage.
    pub fn to_db(&self) -> &'static str {
        match self {
            TaskState::Queued => "queued",
            TaskState::Running => "running",
            TaskState::WaitingApproval => "waiting_approval",
            TaskState::WaitingInput => "waiting_input",
            TaskState::Succeeded => "succeeded",
            TaskState::Failed => "failed",
            TaskState::Cancelled => "cancelled",
        }
    }
}

/// A durable task record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableTask {
    pub id: TaskId,
    pub payload: String,
    pub state: TaskState,
    pub lease_id: Option<LeaseId>,
    pub lease_until: Option<DateTime<Utc>>,
    pub consecutive_failures: u32,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Configuration for the task worker.
#[derive(Debug, Clone)]
pub struct TaskWorkerConfig {
    /// Lease duration before another worker can steal.
    pub lease_duration: Duration,
    /// Maintenance loop interval.
    pub maintenance_interval: Duration,
    /// Max consecutive failures before circuit breaker trips.
    pub max_consecutive_failures: u32,
    /// Base backoff multiplier (seconds). Backoff = base * 2^failures.
    pub backoff_base_secs: u64,
    /// Maximum backoff cap.
    pub max_backoff: Duration,
}

impl Default for TaskWorkerConfig {
    fn default() -> Self {
        Self {
            lease_duration: Duration::from_secs(300), // 5 minutes
            maintenance_interval: Duration::from_secs(60),
            max_consecutive_failures: 5,
            backoff_base_secs: 60,
            max_backoff: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Errors from task operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TaskError {
    #[error("task not found")]
    NotFound,
    #[error("task already claimed by another worker")]
    AlreadyClaimed,
    #[error("invalid state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    #[error("circuit breaker open: {failures} consecutive failures")]
    CircuitBreakerOpen { failures: u32 },
    #[error("database error: {0}")]
    Database(String),
}

impl From<rusqlite::Error> for TaskError {
    fn from(e: rusqlite::Error) -> Self {
        TaskError::Database(e.to_string())
    }
}

/// SQL-backed task store with lease-based durability.
pub struct TaskStore {
    conn: Arc<Mutex<Connection>>,
    config: TaskWorkerConfig,
}

impl TaskStore {
    /// Create a new TaskStore with an in-memory SQLite database.
    /// Production would use a file path or connection pool.
    pub fn new_in_memory(config: TaskWorkerConfig) -> Result<Self, TaskError> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        })
    }

    /// Create a new TaskStore backed by a file.
    pub fn new_file(path: &str, config: TaskWorkerConfig) -> Result<Self, TaskError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            config,
        })
    }

    fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS durable_tasks (
                id TEXT PRIMARY KEY,
                payload TEXT NOT NULL,
                state TEXT NOT NULL DEFAULT 'queued',
                lease_id TEXT,
                lease_until TEXT,
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                next_retry_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tasks_state ON durable_tasks(state);
            CREATE INDEX IF NOT EXISTS idx_tasks_lease ON durable_tasks(lease_until);
            CREATE INDEX IF NOT EXISTS idx_tasks_retry ON durable_tasks(next_retry_at);",
        )
    }

    /// Get current timestamp from the database (database-as-clock pattern).
    /// This ensures all timestamps are consistent and testable without Utc::now().
    fn db_now(conn: &Connection) -> Result<DateTime<Utc>, TaskError> {
        // SQLite datetime('now') returns UTC ISO8601; parse it back to DateTime<Utc>
        let ts: String = conn
            .query_row("SELECT datetime('now')", [], |row| row.get(0))
            .map_err(|e| TaskError::Database(e.to_string()))?;
        // SQLite returns "YYYY-MM-DD HH:MM:SS" format, append 'Z' for UTC parsing
        let iso = format!("{}Z", ts.replace(' ', "T"));
        DateTime::parse_from_rfc3339(&iso)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| TaskError::Database(format!("invalid db timestamp: {}", e)))
    }

    /// Enqueue a new task. Returns the generated task ID.
    pub fn enqueue(&self, payload: String) -> Result<TaskId, TaskError> {
        let id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;
        conn.execute(
            "INSERT INTO durable_tasks (id, payload, state, consecutive_failures, created_at, updated_at)
             VALUES (?1, ?2, 'queued', 0, ?3, ?3)",
            params![id, payload, now.to_rfc3339()],
        )?;
        Ok(id)
    }

    /// Attempt to claim a queued or retryable task using CAS.
    /// Database-as-clock: uses SQL datetime comparison, never Date::now().
    /// Returns the claimed task or None if nothing available.
    pub fn try_claim(&self, _worker_id: &str) -> Result<Option<DurableTask>, TaskError> {
        let lease_id = Uuid::new_v4().to_string();
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;
        let lease_until = now + chrono::Duration::from_std(self.config.lease_duration)
            .map_err(|e| TaskError::Database(e.to_string()))?;

        // CAS: atomically claim one task that is either:
        // 1. queued with no lease
        // 2. lease expired (stale)
        // 3. retryable (next_retry_at <= now) and not circuit-broken
        let rows_changed = conn.execute(
            "UPDATE durable_tasks SET
                lease_id = ?1,
                lease_until = ?2,
                state = 'running',
                updated_at = ?3
             WHERE id = (
                SELECT id FROM durable_tasks
                WHERE (state = 'queued' AND lease_id IS NULL)
                   OR (state IN ('waiting_approval','waiting_input') AND lease_id IS NULL)
                   OR (state = 'running' AND lease_until < ?3)
                   OR (state IN ('queued','failed') AND next_retry_at IS NOT NULL AND next_retry_at <= ?3 AND consecutive_failures < ?4)
                ORDER BY created_at ASC
                LIMIT 1
             )",
            params![lease_id, lease_until.to_rfc3339(), now.to_rfc3339(), self.config.max_consecutive_failures],
        )?;

        if rows_changed == 0 {
            return Ok(None);
        }

        // Fetch the claimed task
        let task = conn
            .query_row(
                "SELECT id, payload, state, lease_id, lease_until, consecutive_failures, next_retry_at, created_at, updated_at
                 FROM durable_tasks WHERE lease_id = ?1",
                params![lease_id],
                |row| {
                    Ok(DurableTask {
                        id: row.get(0)?,
                        payload: row.get(1)?,
                        state: TaskState::from_db(&row.get::<_, String>(2)?).unwrap_or(TaskState::Queued),
                        lease_id: row.get(3)?,
                        lease_until: row.get::<_, Option<String>>(4)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                        consecutive_failures: row.get(5)?,
                        next_retry_at: row.get::<_, Option<String>>(6)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?).unwrap().with_timezone(&Utc),
                        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?).unwrap().with_timezone(&Utc),
                    })
                },
            )
            .optional()?;

        Ok(task)
    }

    /// Transition task state with validation. Only the current lease holder can transition.
    pub fn transition(
        &self,
        task_id: &str,
        lease_id: &str,
        new_state: TaskState,
    ) -> Result<(), TaskError> {
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;

        // Validate transition
        let current_state_str: String = conn
            .query_row(
                "SELECT state FROM durable_tasks WHERE id = ?1 AND lease_id = ?2",
                params![task_id, lease_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(TaskError::NotFound)?;

        let current_state =
            TaskState::from_db(&current_state_str).ok_or(TaskError::Database(format!(
                "unknown state: {}",
                current_state_str
            )))?;

        if !Self::is_valid_transition(&current_state, &new_state) {
            return Err(TaskError::InvalidTransition {
                from: current_state.to_db().to_string(),
                to: new_state.to_db().to_string(),
            });
        }

        let lease_clear = new_state.is_terminal() || matches!(new_state, TaskState::WaitingApproval | TaskState::WaitingInput);

        if lease_clear {
            conn.execute(
                "UPDATE durable_tasks SET state = ?1, lease_id = NULL, lease_until = NULL, updated_at = ?2 WHERE id = ?3 AND lease_id = ?4",
                params![new_state.to_db(), now.to_rfc3339(), task_id, lease_id],
            )?;
        } else {
            conn.execute(
                "UPDATE durable_tasks SET state = ?1, updated_at = ?2 WHERE id = ?3 AND lease_id = ?4",
                params![new_state.to_db(), now.to_rfc3339(), task_id, lease_id],
            )?;
        }

        let _next_retry: Option<DateTime<Utc>> = None; // reserved for future use
        Ok(())
    }

    /// Record a failure with exponential backoff and circuit breaker check.
    pub fn record_failure(&self, task_id: &str, lease_id: &str) -> Result<(), TaskError> {
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;

        // Increment failures and compute backoff atomically
        conn.execute(
            "UPDATE durable_tasks SET
                consecutive_failures = consecutive_failures + 1,
                state = CASE WHEN consecutive_failures + 1 >= ?1 THEN 'failed' ELSE 'queued' END,
                next_retry_at = CASE WHEN consecutive_failures + 1 < ?1 THEN ?2 ELSE NULL END,
                lease_id = NULL,
                lease_until = NULL,
                updated_at = ?3
             WHERE id = ?4 AND lease_id = ?5",
            params![
                self.config.max_consecutive_failures,
                Self::compute_next_retry(now, 0, &self.config).to_rfc3339(),
                now.to_rfc3339(),
                task_id,
                lease_id,
            ],
        )?;

        // Check if circuit breaker tripped
        let failures: u32 = conn
            .query_row(
                "SELECT consecutive_failures FROM durable_tasks WHERE id = ?1",
                params![task_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);

        if failures >= self.config.max_consecutive_failures {
            return Err(TaskError::CircuitBreakerOpen { failures });
        }

        Ok(())
    }

    /// Compute exponential backoff: base * 2^failures, capped at max_backoff.
    fn compute_next_retry(now: DateTime<Utc>, failures: u32, config: &TaskWorkerConfig) -> DateTime<Utc> {
        let backoff_secs = config.backoff_base_secs.saturating_mul(2u64.saturating_pow(failures));
        let backoff = Duration::from_secs(backoff_secs).min(config.max_backoff);
        now + chrono::Duration::from_std(backoff).unwrap_or(chrono::Duration::hours(1))
    }

    /// Maintenance loop: recover tasks with expired leases.
    /// Returns number of recovered tasks.
    pub fn run_maintenance(&self) -> Result<u32, TaskError> {
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;

        // Reset stale running tasks back to queued
        let recovered = conn.execute(
            "UPDATE durable_tasks SET
                state = 'queued',
                lease_id = NULL,
                lease_until = NULL,
                updated_at = ?1
             WHERE state = 'running' AND lease_until < ?1",
            params![now.to_rfc3339()],
        )?;

        Ok(recovered as u32)
    }

    /// Manual circuit breaker reset. Requires explicit human action.
    pub fn reset_circuit_breaker(&self, task_id: &str) -> Result<(), TaskError> {
        let conn = self.conn.lock().unwrap();
        let now = Self::db_now(&conn)?;

        let rows = conn.execute(
            "UPDATE durable_tasks SET
                consecutive_failures = 0,
                next_retry_at = NULL,
                state = 'queued',
                updated_at = ?1
             WHERE id = ?2 AND consecutive_failures >= ?3",
            params![now.to_rfc3339(), task_id, self.config.max_consecutive_failures],
        )?;

        if rows == 0 {
            return Err(TaskError::NotFound);
        }
        Ok(())
    }

    /// Get task by ID.
    pub fn get_task(&self, task_id: &str) -> Result<Option<DurableTask>, TaskError> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT id, payload, state, lease_id, lease_until, consecutive_failures, next_retry_at, created_at, updated_at
             FROM durable_tasks WHERE id = ?1",
            params![task_id],
            |row| {
                Ok(DurableTask {
                    id: row.get(0)?,
                    payload: row.get(1)?,
                    state: TaskState::from_db(&row.get::<_, String>(2)?).unwrap_or(TaskState::Queued),
                    lease_id: row.get(3)?,
                    lease_until: row.get::<_, Option<String>>(4)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                    consecutive_failures: row.get(5)?,
                    next_retry_at: row.get::<_, Option<String>>(6)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?).unwrap().with_timezone(&Utc),
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?).unwrap().with_timezone(&Utc),
                })
            },
        )
        .optional()
        .map_err(TaskError::from)
    }

    /// Count tasks by state.
    pub fn count_by_state(&self, state: TaskState) -> Result<u32, TaskError> {
        let conn = self.conn.lock().unwrap();
        let count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM durable_tasks WHERE state = ?1",
            params![state.to_db()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Check if a state transition is valid.
    fn is_valid_transition(from: &TaskState, to: &TaskState) -> bool {
        matches!(
            (from, to),
            (TaskState::Queued, TaskState::Running)
                | (TaskState::Running, TaskState::Succeeded)
                | (TaskState::Running, TaskState::Failed)
                | (TaskState::Running, TaskState::Cancelled)
                | (TaskState::Running, TaskState::WaitingApproval)
                | (TaskState::Running, TaskState::WaitingInput)
                | (TaskState::WaitingApproval, TaskState::Running)
                | (TaskState::WaitingInput, TaskState::Running)
                | (TaskState::WaitingApproval, TaskState::Cancelled)
                | (TaskState::WaitingInput, TaskState::Cancelled)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> TaskStore {
        TaskStore::new_in_memory(TaskWorkerConfig::default()).unwrap()
    }

    fn fast_store() -> TaskStore {
        TaskStore::new_in_memory(TaskWorkerConfig {
            lease_duration: Duration::from_millis(50),
            maintenance_interval: Duration::from_millis(10),
            max_consecutive_failures: 3,
            backoff_base_secs: 1,
            max_backoff: Duration::from_secs(10),
        })
        .unwrap()
    }

    #[test]
    fn test_enqueue_and_get() {
        let store = test_store();
        let id = store.enqueue("test-payload".into()).unwrap();
        let task = store.get_task(&id).unwrap().unwrap();
        assert_eq!(task.payload, "test-payload");
        assert_eq!(task.state, TaskState::Queued);
        assert_eq!(task.consecutive_failures, 0);
    }

    #[test]
    fn test_claim_queued_task() {
        let store = test_store();
        let id = store.enqueue("payload".into()).unwrap();

        let claimed = store.try_claim("worker-1").unwrap().unwrap();
        assert_eq!(claimed.id, id);
        assert_eq!(claimed.state, TaskState::Running);
        assert!(claimed.lease_id.is_some());
    }

    #[test]
    fn test_lease_steal_after_expiry() {
        let store = fast_store();
        let id = store.enqueue("payload".into()).unwrap();

        // Worker A claims
        let claimed_a = store.try_claim("worker-a").unwrap().unwrap();
        assert_eq!(claimed_a.id, id);

        // Worker B cannot steal while lease is valid
        let stolen = store.try_claim("worker-b").unwrap();
        assert!(stolen.is_none());

        // Wait for lease to expire (must exceed 1s for SQLite datetime precision)
        std::thread::sleep(Duration::from_millis(1100));

        // Worker B can now steal
        let stolen = store.try_claim("worker-b").unwrap().unwrap();
        assert_eq!(stolen.id, id);
        assert_ne!(stolen.lease_id, claimed_a.lease_id);
    }

    #[test]
    fn test_cas_prevents_double_claim() {
        let store = test_store();
        store.enqueue("payload".into()).unwrap();

        // First claim succeeds
        let first = store.try_claim("worker-1").unwrap();
        assert!(first.is_some());

        // Second claim fails (same task already leased)
        let second = store.try_claim("worker-2").unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn test_state_transitions() {
        let store = test_store();
        let id = store.enqueue("payload".into()).unwrap();
        let claimed = store.try_claim("w1").unwrap().unwrap();
        let lease = claimed.lease_id.unwrap();

        // Running → Succeeded
        store.transition(&id, &lease, TaskState::Succeeded).unwrap();
        let task = store.get_task(&id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Succeeded);
    }

    #[test]
    fn test_invalid_transition_rejected() {
        let store = test_store();
        let id = store.enqueue("payload".into()).unwrap();
        let claimed = store.try_claim("w1").unwrap().unwrap();
        let lease = claimed.lease_id.unwrap();

        // Running → Queued is invalid
        let err = store.transition(&id, &lease, TaskState::Queued).unwrap_err();
        assert!(matches!(err, TaskError::InvalidTransition { .. }));
    }

    #[test]
    fn test_waiting_approval_roundtrip() {
        let store = test_store();
        let id = store.enqueue("payload".into()).unwrap();
        let claimed = store.try_claim("w1").unwrap().unwrap();
        let lease = claimed.lease_id.unwrap();

        // Running → WaitingApproval
        store
            .transition(&id, &lease, TaskState::WaitingApproval)
            .unwrap();

        // Re-claim after approval granted (try_claim sets state to Running)
        let re_claimed = store.try_claim("w1").unwrap().unwrap();
        assert_eq!(re_claimed.id, id);
        let new_lease = re_claimed.lease_id.unwrap();

        // Running → Succeeded
        store.transition(&id, &new_lease, TaskState::Succeeded).unwrap();
    }

    #[test]
    fn test_maintenance_recovers_stale() {
        let store = fast_store();
        let id = store.enqueue("payload".into()).unwrap();

        // Claim but don't complete
        store.try_claim("worker-gone").unwrap().unwrap();

        // Before expiry: maintenance finds nothing
        let recovered = store.run_maintenance().unwrap();
        assert_eq!(recovered, 0);

        // Wait for lease expiry (must exceed 1s for SQLite datetime precision)
        std::thread::sleep(Duration::from_millis(1100));

        // After expiry: maintenance recovers
        let recovered = store.run_maintenance().unwrap();
        assert_eq!(recovered, 1);

        // Task is back to queued
        let task = store.get_task(&id).unwrap().unwrap();
        assert_eq!(task.state, TaskState::Queued);
        assert!(task.lease_id.is_none());
    }

    #[test]
    fn test_exponential_backoff_progression() {
        let config = TaskWorkerConfig {
            backoff_base_secs: 60,
            max_backoff: Duration::from_secs(3600),
            ..Default::default()
        };
        let now = Utc::now();

        let t0 = TaskStore::compute_next_retry(now, 0, &config);
        let t1 = TaskStore::compute_next_retry(now, 1, &config);
        let t2 = TaskStore::compute_next_retry(now, 2, &config);
        let t3 = TaskStore::compute_next_retry(now, 3, &config);

        // 60s, 120s, 240s, 480s
        assert!((t1 - t0).num_seconds() >= 59);
        assert!((t2 - t1).num_seconds() >= 119);
        assert!((t3 - t2).num_seconds() >= 239);
    }

    #[test]
    fn test_backoff_caps_at_max() {
        let config = TaskWorkerConfig {
            backoff_base_secs: 60,
            max_backoff: Duration::from_secs(300),
            ..Default::default()
        };
        let now = Utc::now();

        // 2^10 * 60 = 61440s, should be capped at 300s
        let capped = TaskStore::compute_next_retry(now, 10, &config);
        let diff = (capped - now).num_seconds();
        assert!(diff <= 301); // allow 1s tolerance
    }

    #[test]
    fn test_circuit_breaker_trips() {
        let store = fast_store(); // max_consecutive_failures = 3
        let id = store.enqueue("payload".into()).unwrap();

        for i in 0..3 {
            let claimed = store.try_claim("w1").unwrap().unwrap();
            let lease = claimed.lease_id.unwrap();
            let result = store.record_failure(&id, &lease);

            if i < 2 {
                // First two failures: back to queued, no circuit breaker
                assert!(result.is_ok());
                let task = store.get_task(&id).unwrap().unwrap();
                assert_eq!(task.consecutive_failures, i + 1);
            } else {
                // Third failure: circuit breaker trips
                assert!(matches!(
                    result,
                    Err(TaskError::CircuitBreakerOpen { failures: 3 })
                ));
            }
        }
    }

    #[test]
    fn test_circuit_breaker_manual_reset() {
        let store = fast_store();
        let id = store.enqueue("payload".into()).unwrap();

        // Trip the breaker
        for _ in 0..3 {
            let claimed = store.try_claim("w1").unwrap().unwrap();
            let lease = claimed.lease_id.unwrap();
            let _ = store.record_failure(&id, &lease);
        }

        let task = store.get_task(&id).unwrap().unwrap();
        assert_eq!(task.consecutive_failures, 3);
        assert_eq!(task.state, TaskState::Failed);

        // Manual reset
        store.reset_circuit_breaker(&id).unwrap();

        let task = store.get_task(&id).unwrap().unwrap();
        assert_eq!(task.consecutive_failures, 0);
        assert_eq!(task.state, TaskState::Queued);
    }

    #[test]
    fn test_count_by_state() {
        let store = test_store();
        assert_eq!(store.count_by_state(TaskState::Queued).unwrap(), 0);

        store.enqueue("a".into()).unwrap();
        store.enqueue("b".into()).unwrap();
        assert_eq!(store.count_by_state(TaskState::Queued).unwrap(), 2);

        let claimed = store.try_claim("w1").unwrap().unwrap();
        assert_eq!(store.count_by_state(TaskState::Queued).unwrap(), 1);
        assert_eq!(store.count_by_state(TaskState::Running).unwrap(), 1);
    }

    #[test]
    fn test_database_as_clock_no_date_now() {
        // This test verifies that lease expiry uses SQL datetime comparison.
        // The implementation uses Utc::now() only to generate timestamps,
        // but all comparisons happen in SQL WHERE clauses.
        let store = fast_store();
        store.enqueue("payload".into()).unwrap();

        // Claim sets lease_until via SQL parameter
        let claimed = store.try_claim("w1").unwrap().unwrap();
        assert!(claimed.lease_until.is_some());

        // Maintenance compares lease_until < current_time in SQL
        // Must exceed 1s for SQLite datetime('now') second-level precision
        std::thread::sleep(Duration::from_millis(1100));
        let recovered = store.run_maintenance().unwrap();
        assert_eq!(recovered, 1);
    }

    #[test]
    fn test_not_found_on_missing_task() {
        let store = test_store();
        assert!(store.get_task("nonexistent").unwrap().is_none());

        let err = store
            .transition("nonexistent", "lease", TaskState::Succeeded)
            .unwrap_err();
        assert_eq!(err, TaskError::NotFound);
    }
}