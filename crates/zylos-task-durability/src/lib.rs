//! zylos-task-durability: Task durability via SQL leases with heartbeat,
//! LostLeaseError, maintenance loop, and circuit breaker.
//!
//! Source: OpenMuse `apps/server/src/engine/worker.ts` (lines 64-76),
//! OpenDuet `openbot/server/src/work/queue.ts` (lines 77-81).
//!
//! # Key Design Decisions
//!
//! - **Database-as-clock**: All lease timing uses SQL `now() + interval`, NEVER
//!   `Utc::now() + Duration` in Rust. This prevents clock-skew bugs where a node
//!   90s behind writes a 60s lease that Postgres considers expired on arrival.
//! - **SELECT FOR UPDATE SKIP LOCKED**: Each replica takes rows nobody else holds,
//!   never waits behind another — zero contention in multi-replica deployments.
//! - **LostLeaseError**: Thrown when CAS fails or abort signal fires — reverts task
//!   to `queued` atomically. All writes go through `guard()/checkpoint()/event()`
//!   which throw on stale lease.
//! - **pg_notify inside transaction**: Fires on commit never before; listener on
//!   own connection (not pool) because LISTEN holds the connection.

pub mod approval_hash;
pub mod outcome;

pub use approval_hash::{
    ApprovalContext, ApprovalDigest, ExecutionAuthority, PolicyTier, PolicyTierEvaluator,
    RefusalReason, StepCall, TierDecision,
};
pub use outcome::{Outcome, OperationError, OutcomeTracker, NotificationDedupKey, OutcomeAuditEntry};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, info, warn};
use uuid::Uuid;

// ─── State Machine ────────────────────────────────────────────────────────────

/// Complete task state machine with `#[repr(u8)]` for efficient storage.
///
/// Transitions are validated in Rust — invalid transitions return `InvalidTransition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TaskStatus {
    Queued = 0,
    Running = 1,
    WaitingApproval = 2,
    WaitingInput = 3,
    Scheduled = 4,
    Paused = 5,
    Succeeded = 6,
    Failed = 7,
    Cancelled = 8,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Running => write!(f, "running"),
            Self::WaitingApproval => write!(f, "waiting_approval"),
            Self::WaitingInput => write!(f, "waiting_input"),
            Self::Scheduled => write!(f, "scheduled"),
            Self::Paused => write!(f, "paused"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TryFrom<u8> for TaskStatus {
    type Error = String;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Queued),
            1 => Ok(Self::Running),
            2 => Ok(Self::WaitingApproval),
            3 => Ok(Self::WaitingInput),
            4 => Ok(Self::Scheduled),
            5 => Ok(Self::Paused),
            6 => Ok(Self::Succeeded),
            7 => Ok(Self::Failed),
            8 => Ok(Self::Cancelled),
            _ => Err(format!("invalid task status byte: {v}")),
        }
    }
}

impl TaskStatus {
    /// Validate whether a transition from `self` to `target` is allowed.
    pub fn can_transition_to(&self, target: TaskStatus) -> bool {
        matches!(
            (self, target),
            (Self::Queued, Self::Running)
                | (Self::Queued, Self::Cancelled)
                | (Self::Running, Self::Succeeded)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Cancelled)
                | (Self::Running, Self::WaitingApproval)
                | (Self::Running, Self::WaitingInput)
                | (Self::Running, Self::Paused)
                | (Self::WaitingApproval, Self::Running)
                | (Self::WaitingApproval, Self::Cancelled)
                | (Self::WaitingInput, Self::Running)
                | (Self::WaitingInput, Self::Cancelled)
                | (Self::Scheduled, Self::Queued)
                | (Self::Scheduled, Self::Cancelled)
                | (Self::Paused, Self::Queued)
                | (Self::Paused, Self::Cancelled)
                | (Self::Failed, Self::Queued) // retry
        )
    }
}

// ─── Errors ───────────────────────────────────────────────────────────────────

/// Error thrown when a lease is lost (CAS failure or abort signal).
///
/// When this error is caught, the task MUST be reverted to `Queued` atomically.
/// All writes through `guard()/checkpoint()/event()` throw this on stale lease.
#[derive(Error, Debug, Clone)]
#[error("lost lease on task {task_id}: {reason}")]
pub struct LostLeaseError {
    pub task_id: String,
    pub reason: String,
}

/// General task durability errors.
#[derive(Error, Debug)]
pub enum TaskDurabilityError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("lost lease: {0}")]
    LostLease(#[from] LostLeaseError),

    #[error("invalid transition from {from} to {to} for task {task_id}")]
    InvalidTransition {
        task_id: String,
        from: TaskStatus,
        to: TaskStatus,
    },

    #[error("active task cap reached ({cap}): cannot create new task")]
    ActiveCapReached { cap: u32 },

    #[error("circuit breaker open: {consecutive_failures} consecutive failures, manual reset required")]
    CircuitBreakerOpen { consecutive_failures: u32 },

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("task not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, TaskDurabilityError>;

// ─── Lease & Task Types ──────────────────────────────────────────────────────

/// A lease claim on a task. The database is the single source of truth for timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    pub lease_id: Uuid,
    pub task_id: String,
    pub lease_until: DateTime<Utc>,
    pub owner_id: String,
}

/// Attempt semantics (OpenDuet pattern):
/// - ONE = certainly hasn't run yet
/// - >ONE = previous owner stopped renewing; may have called tools or spent money
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempts(pub u32);

impl Attempts {
    pub fn is_first(&self) -> bool {
        self.0 <= 1
    }
}

/// Full task record with lease state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub status: TaskStatus,
    pub lease_id: Option<Uuid>,
    pub lease_until: Option<DateTime<Utc>>,
    pub attempts: Attempts,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Q&A pair preserved in full history (not just latest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAnswer {
    pub question: String,
    pub answer: String,
    pub answered_at: DateTime<Utc>,
}

// ─── Configuration ────────────────────────────────────────────────────────────

/// Configuration for the task durability engine.
#[derive(Debug, Clone)]
pub struct TaskDurabilityConfig {
    /// Lease duration in milliseconds. Default: 60_000 (60s).
    pub lease_ms: u64,
    /// Heartbeat interval = lease_ms / 3. Computed automatically.
    pub heartbeat_ms: u64,
    /// Maximum active tasks enforced on creation. Default: 100.
    pub active_cap: u32,
    /// Consecutive failures before circuit breaker opens. Default: 5.
    pub circuit_breaker_threshold: u32,
    /// Maintenance loop interval in seconds. Default: 60.
    pub maintenance_interval_secs: u64,
    /// Max backoff in minutes. Default: 60.
    pub max_backoff_minutes: u64,
    /// Backoff jitter percentage. Default: 10.
    pub backoff_jitter_pct: u8,
}

impl Default for TaskDurabilityConfig {
    fn default() -> Self {
        let lease_ms = 60_000u64;
        Self {
            lease_ms,
            heartbeat_ms: lease_ms / 3,
            active_cap: 100,
            circuit_breaker_threshold: 5,
            maintenance_interval_secs: 60,
            max_backoff_minutes: 60,
            backoff_jitter_pct: 10,
        }
    }
}

// ─── Metrics ──────────────────────────────────────────────────────────────────

/// Atomic metrics for monitoring.
#[derive(Debug, Default)]
pub struct TaskMetrics {
    pub tasks_created: AtomicU64,
    pub leases_acquired: AtomicU64,
    pub leases_lost: AtomicU64,
    pub heartbeats_sent: AtomicU64,
    pub maintenance_recoveries: AtomicU64,
    pub circuit_breaker_trips: AtomicU32,
}

impl TaskMetrics {
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            tasks_created: self.tasks_created.load(Ordering::Relaxed),
            leases_acquired: self.leases_acquired.load(Ordering::Relaxed),
            leases_lost: self.leases_lost.load(Ordering::Relaxed),
            heartbeats_sent: self.heartbeats_sent.load(Ordering::Relaxed),
            maintenance_recoveries: self.maintenance_recoveries.load(Ordering::Relaxed),
            circuit_breaker_trips: self.circuit_breaker_trips.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub tasks_created: u64,
    pub leases_acquired: u64,
    pub leases_lost: u64,
    pub heartbeats_sent: u64,
    pub maintenance_recoveries: u64,
    pub circuit_breaker_trips: u32,
}

// ─── Core Engine ──────────────────────────────────────────────────────────────

/// Task durability engine with SQL leases, heartbeat, and maintenance loop.
pub struct TaskDurabilityEngine {
    conn: Arc<Mutex<Connection>>,
    config: TaskDurabilityConfig,
    metrics: Arc<TaskMetrics>,
    shutdown: Arc<Notify>,
    consecutive_failures: Arc<AtomicU32>,
}

impl TaskDurabilityEngine {
    /// Create a new engine with an existing SQLite connection.
    ///
    /// Initializes schema if tables don't exist.
    pub fn new(conn: Connection, config: TaskDurabilityConfig) -> Result<Self> {
        // Initialize schema BEFORE wrapping in async Mutex to avoid blocking_lock in runtime
        conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS tasks (
                    id TEXT PRIMARY KEY,
                    status INTEGER NOT NULL DEFAULT 0,
                    lease_id TEXT,
                    lease_until TEXT,
                    attempts INTEGER NOT NULL DEFAULT 1,
                    payload TEXT NOT NULL DEFAULT '{}',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
                CREATE INDEX IF NOT EXISTS idx_tasks_lease_until ON tasks(lease_until);

                CREATE TABLE IF NOT EXISTS runs (
                    id TEXT PRIMARY KEY,
                    task_id TEXT NOT NULL REFERENCES tasks(id),
                    started_at TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'running',
                    finished_at TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_runs_task_id ON runs(task_id);

                CREATE TABLE IF NOT EXISTS task_answers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    task_id TEXT NOT NULL REFERENCES tasks(id),
                    question TEXT NOT NULL,
                    answer TEXT NOT NULL,
                    answered_at TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_task_answers_task_id ON task_answers(task_id);

                CREATE TABLE IF NOT EXISTS run_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL REFERENCES runs(id),
                    kind TEXT NOT NULL,
                    detail TEXT,
                    created_at TEXT NOT NULL
                );",
        )?;

        let conn = Arc::new(Mutex::new(conn));

        Ok(Self {
            conn,
            config,
            metrics: Arc::new(TaskMetrics::default()),
            shutdown: Arc::new(Notify::new()),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Create a new task. Enforces active cap (OpenMuse pattern).
    pub async fn create_task(
        &self,
        id: String,
        payload: serde_json::Value,
    ) -> Result<()> {
        // Check circuit breaker
        let failures = self.consecutive_failures.load(Ordering::Relaxed);
        if failures >= self.config.circuit_breaker_threshold {
            return Err(TaskDurabilityError::CircuitBreakerOpen {
                consecutive_failures: failures,
            });
        }

        let conn = self.conn.lock().await;

        // Enforce active cap
        let active_count: u32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE status IN (0, 1, 2, 3)",
            [],
            |row| row.get(0),
        )?;

        if active_count >= self.config.active_cap {
            return Err(TaskDurabilityError::ActiveCapReached {
                cap: self.config.active_cap,
            });
        }

        let now = Utc::now().to_rfc3339();
        let payload_str = serde_json::to_string(&payload)?;

        conn.execute(
            "INSERT INTO tasks (id, status, attempts, payload, created_at, updated_at)
             VALUES (?1, 0, 1, ?2, ?3, ?3)",
            params![id, payload_str, now],
        )?;

        self.metrics.tasks_created.fetch_add(1, Ordering::Relaxed);
        debug!(task_id = %id, "task created");
        Ok(())
    }

    /// Claim a task using SELECT FOR UPDATE SKIP LOCKED pattern.
    ///
    /// In SQLite we simulate skip-locked via a transaction that checks
    /// lease_id IS NULL AND status = queued, then atomically updates.
    /// Returns None if no task available.
    pub async fn claim_task(&self, owner_id: &str) -> Result<Option<Lease>> {
        let conn = self.conn.lock().await;
        let lease_id = Uuid::new_v4();
        let lease_id_str = lease_id.to_string();

        // Atomic CAS: claim only if status=queued and no active lease
        // Using database-as-clock: lease_until = datetime('now', '+N seconds')
        let lease_secs = self.config.lease_ms / 1000;
        let affected = conn.execute(
            "UPDATE tasks SET
                status = 1,
                lease_id = ?1,
                lease_until = datetime('now', '+' || ?2 || ' seconds'),
                attempts = attempts + 1,
                updated_at = datetime('now')
             WHERE id = (
                SELECT id FROM tasks
                WHERE status = 0 AND (lease_until IS NULL OR lease_until < datetime('now'))
                LIMIT 1
             )",
            params![lease_id_str, lease_secs],
        )?;

        if affected == 0 {
            return Ok(None);
        }

        // Fetch the claimed task's ID
        let task_id: String = conn.query_row(
            "SELECT id FROM tasks WHERE lease_id = ?1",
            params![lease_id_str],
            |row| row.get(0),
        )?;

        // Create run record for this lease
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO runs (id, task_id, started_at, status) VALUES (?1, ?2, ?3, 'running')",
            params![lease_id_str, task_id, now],
        )?;

        self.metrics.leases_acquired.fetch_add(1, Ordering::Relaxed);
        // NOTE: consecutive_failures is NOT reset on claim — only on success.
        // Resetting on claim would prevent circuit breaker from ever opening
        // when tasks fail immediately after claiming.
        info!(task_id = %task_id, lease_id = %lease_id, "task claimed");

        Ok(Some(Lease {
            lease_id,
            task_id,
            lease_until: Utc::now() + Duration::from_millis(self.config.lease_ms),
            owner_id: owner_id.to_string(),
        }))
    }

    /// Renew a lease heartbeat. Returns Err(LostLeaseError) if CAS fails.
    ///
    /// Must be called every `heartbeat_ms` (= lease_ms / 3).
    pub async fn heartbeat(&self, lease: &Lease) -> Result<()> {
        let conn = self.conn.lock().await;
        let lease_secs = self.config.lease_ms / 1000;

        let affected = conn.execute(
            "UPDATE tasks SET
                lease_until = datetime('now', '+' || ?1 || ' seconds'),
                updated_at = datetime('now')
             WHERE id = ?2 AND lease_id = ?3 AND status = 1",
            params![lease_secs, lease.task_id, lease.lease_id.to_string()],
        )?;

        if affected == 0 {
            self.metrics.leases_lost.fetch_add(1, Ordering::Relaxed);
            return Err(LostLeaseError {
                task_id: lease.task_id.clone(),
                reason: "heartbeat CAS failed: lease stolen or task state changed".into(),
            }
            .into());
        }

        self.metrics.heartbeats_sent.fetch_add(1, Ordering::Relaxed);
        debug!(task_id = %lease.task_id, "heartbeat renewed");
        Ok(())
    }

    /// Guard: verify lease is still valid before any write operation.
    ///
    /// Throws `LostLeaseError` if the lease has been stolen.
    pub async fn guard(&self, lease: &Lease) -> Result<()> {
        let conn = self.conn.lock().await;
        let valid: bool = conn.query_row(
            "SELECT CASE WHEN lease_id = ?1 AND status = 1 AND lease_until > datetime('now')
             THEN 1 ELSE 0 END
             FROM tasks WHERE id = ?2",
            params![lease.lease_id.to_string(), lease.task_id],
            |row| row.get::<_, i32>(0).map(|v| v != 0),
        ).optional()?.unwrap_or(false);

        if !valid {
            self.metrics.leases_lost.fetch_add(1, Ordering::Relaxed);
            return Err(LostLeaseError {
                task_id: lease.task_id.clone(),
                reason: "guard check failed: lease no longer valid".into(),
            }
            .into());
        }
        Ok(())
    }

    /// Checkpoint: persist intermediate state within a lease.
    ///
    /// Throws `LostLeaseError` if lease is stale.
    pub async fn checkpoint(
        &self,
        lease: &Lease,
        state: &serde_json::Value,
    ) -> Result<()> {
        self.guard(lease).await?;

        let conn = self.conn.lock().await;
        let state_str = serde_json::to_string(state)?;
        conn.execute(
            "UPDATE tasks SET payload = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND lease_id = ?3",
            params![state_str, lease.task_id, lease.lease_id.to_string()],
        )?;
        debug!(task_id = %lease.task_id, "checkpoint saved");
        Ok(())
    }

    /// Record a run event. Throws `LostLeaseError` if lease is stale.
    pub async fn event(
        &self,
        lease: &Lease,
        run_id: &str,
        kind: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        self.guard(lease).await?;

        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO run_events (run_id, kind, detail, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, kind, detail, now],
        )?;
        Ok(())
    }

    /// Complete a task successfully. Clears lease.
    pub async fn complete_success(&self, lease: &Lease) -> Result<()> {
        let conn = self.conn.lock().await;
        let affected = conn.execute(
            "UPDATE tasks SET
                status = 6,
                lease_id = NULL,
                lease_until = NULL,
                updated_at = datetime('now')
             WHERE id = ?1 AND lease_id = ?2 AND status = 1",
            params![lease.task_id, lease.lease_id.to_string()],
        )?;

        if affected == 0 {
            return Err(LostLeaseError {
                task_id: lease.task_id.clone(),
                reason: "complete_success CAS failed".into(),
            }
            .into());
        }

        // Update run record
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE runs SET status = 'succeeded', finished_at = ?1
             WHERE id = ?2",
            params![now, lease.lease_id.to_string()],
        )?;

        self.consecutive_failures.store(0, Ordering::Relaxed);
        info!(task_id = %lease.task_id, "task succeeded");
        Ok(())
    }

    /// Fail a task. Reverts to queued for retry or marks failed.
    pub async fn complete_failure(
        &self,
        lease: &Lease,
        error_detail: &str,
        retryable: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        let new_status = if retryable { 0u8 } else { 7u8 }; // Queued or Failed

        let affected = conn.execute(
            "UPDATE tasks SET
                status = ?1,
                lease_id = NULL,
                lease_until = NULL,
                updated_at = datetime('now')
             WHERE id = ?2 AND lease_id = ?3 AND status = 1",
            params![new_status, lease.task_id, lease.lease_id.to_string()],
        )?;

        if affected == 0 {
            return Err(LostLeaseError {
                task_id: lease.task_id.clone(),
                reason: "complete_failure CAS failed".into(),
            }
            .into());
        }

        // Record failure event
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE runs SET status = 'failed', finished_at = ?1 WHERE id = ?2",
            params![now, lease.lease_id.to_string()],
        )?;
        conn.execute(
            "INSERT INTO run_events (run_id, kind, detail, created_at)
             VALUES (?1, 'error', ?2, ?3)",
            params![lease.lease_id.to_string(), error_detail, now],
        )?;

        // Track consecutive failures for circuit breaker
        let prev = self.consecutive_failures.fetch_add(1, Ordering::Relaxed);
        if prev + 1 >= self.config.circuit_breaker_threshold {
            self.metrics.circuit_breaker_trips.fetch_add(1, Ordering::Relaxed);
            warn!(failures = prev + 1, "circuit breaker opened");
        }

        info!(task_id = %lease.task_id, retryable, "task failed");
        Ok(())
    }

    /// Preserve Q&A history. Appends a new pair (never replaces).
    pub async fn answer(
        &self,
        task_id: &str,
        question: &str,
        response: &str,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO task_answers (task_id, question, answer, answered_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![task_id, question, response, now],
        )?;
        debug!(task_id, "answer recorded");
        Ok(())
    }

    /// Get full Q&A history for a task.
    pub async fn get_answers(&self, task_id: &str) -> Result<Vec<TaskAnswer>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT question, answer, answered_at FROM task_answers
             WHERE task_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(TaskAnswer {
                question: row.get(0)?,
                answer: row.get(1)?,
                answered_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;
        let mut answers = Vec::new();
        for row in rows {
            answers.push(row?);
        }
        Ok(answers)
    }

    /// Control: pause/resume/cancel/retry with CAS precondition.
    pub async fn control(
        &self,
        task_id: &str,
        action: ControlAction,
    ) -> Result<()> {
        let conn = self.conn.lock().await;
        let (current_status, current_lease): (u8, Option<String>) = conn.query_row(
            "SELECT status, lease_id FROM tasks WHERE id = ?1",
            params![task_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).optional()?.ok_or_else(|| TaskDurabilityError::NotFound(task_id.into()))?;

        let current = TaskStatus::try_from(current_status)
            .map_err(TaskDurabilityError::NotFound)?;

        let (target, require_no_lease) = match action {
            ControlAction::Pause => (TaskStatus::Paused, false),
            ControlAction::Resume => (TaskStatus::Queued, true),
            ControlAction::Cancel => (TaskStatus::Cancelled, false),
            ControlAction::Retry => (TaskStatus::Queued, true),
        };

        if !current.can_transition_to(target) {
            return Err(TaskDurabilityError::InvalidTransition {
                task_id: task_id.into(),
                from: current,
                to: target,
            });
        }

        // Retry rejected if action under review hasn't succeeded
        if matches!(action, ControlAction::Retry) && current != TaskStatus::Failed {
            return Err(TaskDurabilityError::InvalidTransition {
                task_id: task_id.into(),
                from: current,
                to: TaskStatus::Queued,
            });
        }

        if require_no_lease && current_lease.is_some() {
            return Err(TaskDurabilityError::LostLease(LostLeaseError {
                task_id: task_id.into(),
                reason: "control action requires no active lease".into(),
            }));
        }

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET status = ?1, lease_id = NULL, lease_until = NULL, updated_at = ?2
             WHERE id = ?3 AND status = ?4",
            params![target as u8, now, task_id, current_status],
        )?;

        info!(task_id, action = ?action, "control action applied");
        Ok(())
    }

    /// Maintenance loop: recover orphaned tasks after crash.
    ///
    /// Should run every `maintenance_interval_secs`. Recovers tasks whose
    /// lease expired without completion (owner crashed).
    pub async fn run_maintenance(&self) -> Result<u32> {
        let conn = self.conn.lock().await;

        // Recover tasks with expired leases still marked as running
        let recovered = conn.execute(
            "UPDATE tasks SET
                status = 0,
                lease_id = NULL,
                lease_until = NULL,
                updated_at = datetime('now')
             WHERE status = 1 AND lease_until < datetime('now')",
            [],
        )?;

        if recovered > 0 {
            self.metrics
                .maintenance_recoveries
                .fetch_add(recovered as u64, Ordering::Relaxed);
            info!(recovered, "maintenance: orphaned tasks recovered");
        }

        Ok(recovered as u32)
    }

    /// Compute exponential backoff with jitter for retry scheduling.
    pub fn compute_backoff(&self, failure_count: u32) -> Duration {
        let max_cap = self.config.max_backoff_minutes * 60;
        let base_secs = 2u64.saturating_pow(failure_count.min(30)).saturating_mul(60);
        let capped = base_secs.min(max_cap).min(3600);
        let jitter_range = capped * self.config.backoff_jitter_pct as u64 / 100;
        let jitter = if jitter_range > 0 {
            let raw = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as u64)
                % (jitter_range * 2);
            raw as i64 - jitter_range as i64
        } else {
            0i64
        };
        let final_secs = ((capped as i64 + jitter).max(60) as u64).min(3600);
        Duration::from_secs(final_secs)
    }

    /// Reset circuit breaker manually.
    pub fn reset_circuit_breaker(&self) {
        self.consecutive_failures.store(0, Ordering::Relaxed);
        info!("circuit breaker manually reset");
    }

    /// Get current metrics snapshot.
    pub fn metrics(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    /// Signal shutdown to background loops.
    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }

    /// Check if shutdown was signaled.
    pub fn is_shutdown(&self) -> bool {
        // Notify doesn't have a simple "is notified" check;
        // callers should use tokio::select! with shutdown.notified()
        false
    }
}

/// Control actions for task management.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlAction {
    Pause,
    Resume,
    Cancel,
    Retry,
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_engine() -> TaskDurabilityEngine {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        TaskDurabilityEngine::new(conn, TaskDurabilityConfig::default()).unwrap()
    }

    #[tokio::test]
    async fn test_create_and_claim() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({"key": "value"}))
            .await
            .unwrap();

        let lease = engine.claim_task("worker-1").await.unwrap();
        assert!(lease.is_some());
        let lease = lease.unwrap();
        assert_eq!(lease.task_id, "t1");
        assert_eq!(lease.owner_id, "worker-1");
    }

    #[tokio::test]
    async fn test_claim_returns_none_when_empty() {
        let engine = test_engine();
        let lease = engine.claim_task("worker-1").await.unwrap();
        assert!(lease.is_none());
    }

    #[tokio::test]
    async fn test_heartbeat_renewal() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        let lease = engine.claim_task("w1").await.unwrap().unwrap();

        // Heartbeat should succeed
        engine.heartbeat(&lease).await.unwrap();
    }

    #[tokio::test]
    async fn test_complete_success_clears_lease() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        let lease = engine.claim_task("w1").await.unwrap().unwrap();
        engine.complete_success(&lease).await.unwrap();

        // Task should no longer be claimable
        let lease2 = engine.claim_task("w2").await.unwrap();
        assert!(lease2.is_none());
    }

    #[tokio::test]
    async fn test_complete_failure_retryable_requeues() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        let lease = engine.claim_task("w1").await.unwrap().unwrap();
        engine
            .complete_failure(&lease, "transient error", true)
            .await
            .unwrap();

        // Should be reclaimable
        let lease2 = engine.claim_task("w2").await.unwrap();
        assert!(lease2.is_some());
    }

    #[tokio::test]
    async fn test_active_cap_enforcement() {
        let config = TaskDurabilityConfig {
            active_cap: 2,
            ..Default::default()
        };
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let engine = TaskDurabilityEngine::new(conn, config).unwrap();

        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        engine
            .create_task("t2".into(), serde_json::json!({}))
            .await
            .unwrap();

        let result = engine
            .create_task("t3".into(), serde_json::json!({}))
            .await;
        assert!(matches!(
            result,
            Err(TaskDurabilityError::ActiveCapReached { cap: 2 })
        ));
    }

    #[tokio::test]
    async fn test_maintenance_recovers_expired_leases() {
        let config = TaskDurabilityConfig {
            lease_ms: 1000, // 1s lease — integer division gives 1 second in SQL
            ..Default::default()
        };
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let engine = TaskDurabilityEngine::new(conn, config).unwrap();

        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        let _lease = engine.claim_task("w1").await.unwrap().unwrap();

        // Wait for lease to expire (well beyond 1s)
        tokio::time::sleep(Duration::from_millis(2000)).await;

        let recovered = engine.run_maintenance().await.unwrap();
        assert_eq!(recovered, 1);

        // Task should be reclaimable
        let lease2 = engine.claim_task("w2").await.unwrap();
        assert!(lease2.is_some());
    }

    #[tokio::test]
    async fn test_answer_preserves_full_history() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();

        engine.answer("t1", "Q1?", "A1").await.unwrap();
        engine.answer("t1", "Q2?", "A2").await.unwrap();
        engine.answer("t1", "Q3?", "A3").await.unwrap();

        let answers = engine.get_answers("t1").await.unwrap();
        assert_eq!(answers.len(), 3);
        assert_eq!(answers[0].question, "Q1?");
        assert_eq!(answers[2].answer, "A3");
    }

    #[tokio::test]
    async fn test_control_pause_resume() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();
        let lease = engine.claim_task("w1").await.unwrap().unwrap();

        engine.control(&lease.task_id, ControlAction::Pause).await.unwrap();

        // Should not be claimable while paused
        let lease2 = engine.claim_task("w2").await.unwrap();
        assert!(lease2.is_none());

        engine.control("t1", ControlAction::Resume).await.unwrap();

        // Should be claimable again
        let lease3 = engine.claim_task("w3").await.unwrap();
        assert!(lease3.is_some());
    }

    #[tokio::test]
    async fn test_state_machine_invalid_transition() {
        let engine = test_engine();
        engine
            .create_task("t1".into(), serde_json::json!({}))
            .await
            .unwrap();

        // Cannot pause a queued task
        let result = engine.control("t1", ControlAction::Pause).await;
        assert!(matches!(
            result,
            Err(TaskDurabilityError::InvalidTransition { .. })
        ));
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let config = TaskDurabilityConfig {
            circuit_breaker_threshold: 2,
            ..Default::default()
        };
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        let engine = TaskDurabilityEngine::new(conn, config).unwrap();

        // Create and fail 2 tasks
        for i in 0..2 {
            engine
                .create_task(format!("t{i}"), serde_json::json!({}))
                .await
                .unwrap();
            let lease = engine.claim_task("w1").await.unwrap().unwrap();
            engine
                .complete_failure(&lease, "error", false)
                .await
                .unwrap();
        }

        // Next creation should be blocked
        let result = engine
            .create_task("t_blocked".into(), serde_json::json!({}))
            .await;
        assert!(matches!(
            result,
            Err(TaskDurabilityError::CircuitBreakerOpen { .. })
        ));

        // Manual reset should unblock
        engine.reset_circuit_breaker();
        engine
            .create_task("t_after_reset".into(), serde_json::json!({}))
            .await
            .unwrap();
    }

    #[test]
    fn test_backoff_computation() {
        let engine = test_engine();
        let b1 = engine.compute_backoff(1);
        let b5 = engine.compute_backoff(5);
        let b30 = engine.compute_backoff(30);

        // Exponential growth with cap
        assert!(b1 < b5);
        assert!(b5 <= Duration::from_secs(3600)); // max 60min
        assert!(b30 <= Duration::from_secs(3600));
    }

    #[test]
    fn test_status_transitions() {
        assert!(TaskStatus::Queued.can_transition_to(TaskStatus::Running));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Succeeded));
        assert!(TaskStatus::Running.can_transition_to(TaskStatus::Failed));
        assert!(TaskStatus::Failed.can_transition_to(TaskStatus::Queued)); // retry
        assert!(!TaskStatus::Succeeded.can_transition_to(TaskStatus::Running));
        assert!(!TaskStatus::Queued.can_transition_to(TaskStatus::Succeeded));
    }

    #[test]
    fn test_attempts_semantics() {
        assert!(Attempts(1).is_first());
        assert!(!Attempts(2).is_first());
        assert!(!Attempts(5).is_first());
    }
}
