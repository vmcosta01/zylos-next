//! # zylos-goals
//!
//! Goal tracking system for zylos-gateway with:
//! - Hierarchical goals → milestones → key results (OKR model)
//! - Cron-based progress watchers that auto-evaluate metrics
//! - SQLite WAL persistence with full audit trail
//! - Integration hooks for skills, playbooks, and integrations

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Domain Types
// ---------------------------------------------------------------------------

/// A high-level objective (e.g., "Launch MVP by Q2")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    /// Unique identifier for this goal.
    pub id: String,
    /// Short title for this goal.
    pub title: String,
    /// Optional longer description.
    pub description: Option<String>,
    /// Person or team responsible for this goal.
    pub owner: Option<String>,
    /// Current lifecycle status.
    pub status: GoalStatus,
    /// Importance level for scheduling and display.
    pub priority: Priority,
    /// Timestamp when this goal was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last modification.
    pub updated_at: DateTime<Utc>,
    /// Optional target completion date.
    pub due_date: Option<DateTime<Utc>>,
    /// Parent goal ID for hierarchical nesting.
    pub parent_goal_id: Option<String>,
    /// Free-form tags for filtering and grouping.
    pub tags: Vec<String>,
}

/// Lifecycle status of a goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Draft,
    Active,
    OnTrack,
    AtRisk,
    Blocked,
    Completed,
    Cancelled,
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Active => write!(f, "active"),
            Self::OnTrack => write!(f, "on_track"),
            Self::AtRisk => write!(f, "at_risk"),
            Self::Blocked => write!(f, "blocked"),
            Self::Completed => write!(f, "completed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl GoalStatus {
    /// Parse a lowercase string into a [`GoalStatus`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "on_track" => Ok(Self::OnTrack),
            "at_risk" => Ok(Self::AtRisk),
            "blocked" => Ok(Self::Blocked),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(GoalsError::InvalidStatus(s.to_string())),
        }
    }
}

/// Task priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl Priority {
    /// Parse a lowercase string into a [`Priority`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(GoalsError::InvalidPriority(s.to_string())),
        }
    }
}

/// A measurable outcome tied to a goal (OKR key result)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyResult {
    /// Unique identifier for this key result.
    pub id: String,
    /// ID of the goal this key result belongs to.
    pub goal_id: String,
    /// Short title describing the measurable outcome.
    pub title: String,
    /// Kind of metric being tracked.
    pub metric_type: MetricType,
    /// Target value that constitutes achievement.
    pub target_value: f64,
    /// Current measured value.
    pub current_value: f64,
    /// Display unit (e.g. "users", "%", "USD").
    pub unit: Option<String>,
    /// Completion status, auto-updated based on current vs target value.
    pub status: KrStatus,
    /// Timestamp when this key result was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last progress update.
    pub updated_at: DateTime<Utc>,
}

/// The kind of metric a key result tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricType {
    Percentage,
    Count,
    Currency,
    Duration,
    Boolean,
    Custom,
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Percentage => write!(f, "percentage"),
            Self::Count => write!(f, "count"),
            Self::Currency => write!(f, "currency"),
            Self::Duration => write!(f, "duration"),
            Self::Boolean => write!(f, "boolean"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl MetricType {
    /// Parse a lowercase string into a [`MetricType`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "percentage" => Ok(Self::Percentage),
            "count" => Ok(Self::Count),
            "currency" => Ok(Self::Currency),
            "duration" => Ok(Self::Duration),
            "boolean" => Ok(Self::Boolean),
            "custom" => Ok(Self::Custom),
            _ => Err(GoalsError::InvalidMetricType(s.to_string())),
        }
    }
}

/// Completion status of a key result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KrStatus {
    NotStarted,
    InProgress,
    Achieved,
    Missed,
}

impl std::fmt::Display for KrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStarted => write!(f, "not_started"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Achieved => write!(f, "achieved"),
            Self::Missed => write!(f, "missed"),
        }
    }
}

impl KrStatus {
    /// Parse a lowercase string into a [`KrStatus`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "not_started" => Ok(Self::NotStarted),
            "in_progress" => Ok(Self::InProgress),
            "achieved" => Ok(Self::Achieved),
            "missed" => Ok(Self::Missed),
            _ => Err(GoalsError::InvalidKrStatus(s.to_string())),
        }
    }
}

/// A checkpoint within a goal's timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    /// Unique identifier for this milestone.
    pub id: String,
    /// ID of the goal this key result belongs to.
    pub goal_id: String,
    /// Short title for the milestone.
    pub title: String,
    /// Optional description of what this checkpoint represents.
    pub description: Option<String>,
    /// Target date for this milestone.
    pub due_date: DateTime<Utc>,
    /// Actual completion timestamp, if completed.
    pub completed_at: Option<DateTime<Utc>>,
    /// Current completion status.
    pub status: MilestoneStatus,
    /// Timestamp when this milestone was created.
    pub created_at: DateTime<Utc>,
}

/// Completion status of a milestone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Pending,
    Completed,
    Overdue,
    Skipped,
}

impl std::fmt::Display for MilestoneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Completed => write!(f, "completed"),
            Self::Overdue => write!(f, "overdue"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

impl MilestoneStatus {
    /// Parse a lowercase string into a [`MilestoneStatus`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            "overdue" => Ok(Self::Overdue),
            "skipped" => Ok(Self::Skipped),
            _ => Err(GoalsError::InvalidMilestoneStatus(s.to_string())),
        }
    }
}

/// A cron-scheduled watcher that auto-evaluates a key result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressWatcher {
    /// Unique identifier for this watcher.
    pub id: String,
    /// ID of the key result this watcher monitors.
    pub key_result_id: String,
    /// Cron expression defining the evaluation schedule.
    pub cron_expr: String,
    /// Kind of data source used to collect the metric.
    pub source_type: WatcherSourceType,
    /// Source-specific configuration (query, URL, skill name, etc.).
    pub source_config: serde_json::Value,
    /// Whether this watcher is currently active.
    pub enabled: bool,
    /// Timestamp of the last evaluation run.
    pub last_run_at: Option<DateTime<Utc>>,
    /// Numeric value returned by the last evaluation.
    pub last_result: Option<f64>,
    /// Error message from the last evaluation, if it failed.
    pub last_error: Option<String>,
    /// Timestamp when this watcher was created.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Source type for a progress watcher.
pub enum WatcherSourceType {
    /// Execute a skill and parse its numeric output as the metric value.
    SkillExecution,
    /// Query an integration action (e.g., GitHub issue count).
    IntegrationAction,
    /// Run a SQL query against the local database.
    SqlQuery,
    /// Poll an HTTP endpoint that returns JSON with a numeric field.
    HttpEndpoint,
    /// Accept manual progress updates through the API.
    Manual,
}

impl std::fmt::Display for WatcherSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkillExecution => write!(f, "skill_execution"),
            Self::IntegrationAction => write!(f, "integration_action"),
            Self::SqlQuery => write!(f, "sql_query"),
            Self::HttpEndpoint => write!(f, "http_endpoint"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl WatcherSourceType {
    /// Parse a lowercase string into a [`WatcherSourceType`].
    pub fn from_str(s: &str) -> Result<Self, GoalsError> {
        match s {
            "skill_execution" => Ok(Self::SkillExecution),
            "integration_action" => Ok(Self::IntegrationAction),
            "sql_query" => Ok(Self::SqlQuery),
            "http_endpoint" => Ok(Self::HttpEndpoint),
            "manual" => Ok(Self::Manual),
            _ => Err(GoalsError::InvalidWatcherSource(s.to_string())),
        }
    }
}

/// Audit log entry for goal changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAuditEntry {
    /// Unique identifier for this audit entry.
    pub id: String,
    /// Type of entity mutated ("goal", "key_result", "milestone", "watcher").
    pub entity_type: String,
    /// ID of the mutated entity.
    pub entity_id: String,
    /// Action performed (e.g. "created", "status_changed").
    pub action: String,
    /// Previous value before the mutation, if applicable.
    pub old_value: Option<String>,
    /// New value after the mutation, if applicable.
    pub new_value: Option<String>,
    /// Identity of the actor who performed the mutation.
    pub actor: Option<String>,
    /// Timestamp when the mutation occurred.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors returned by goal-store operations.
#[derive(Debug, thiserror::Error)]
pub enum GoalsError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Goal not found: {0}")]
    NotFound(String),
    #[error("Invalid goal status: {0}")]
    InvalidStatus(String),
    #[error("Invalid priority: {0}")]
    InvalidPriority(String),
    #[error("Invalid metric type: {0}")]
    InvalidMetricType(String),
    #[error("Invalid KR status: {0}")]
    InvalidKrStatus(String),
    #[error("Invalid milestone status: {0}")]
    InvalidMilestoneStatus(String),
    #[error("Invalid watcher source: {0}")]
    InvalidWatcherSource(String),
    #[error("Invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// GoalStore — SQLite WAL backend
// ---------------------------------------------------------------------------

/// SQLite WAL-backed store for goals, key results, milestones, watchers, and audit logs.
///
/// All public methods are async-safe via an internal [`RwLock`] around the connection.
pub struct GoalStore {
    conn: Arc<RwLock<Connection>>,
}

impl GoalStore {
    /// Open or create the goals database at the given path
    pub fn open(path: &Path) -> Result<Self, GoalsError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Self::run_migrations(&conn)?;
        Ok(Self {
            conn: Arc::new(RwLock::new(conn)),
        })
    }

    /// Create an in-memory store for testing
    pub fn in_memory() -> Result<Self, GoalsError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Self::run_migrations(&conn)?;
        Ok(Self {
            conn: Arc::new(RwLock::new(conn)),
        })
    }

    fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS goals (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                owner TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                priority TEXT NOT NULL DEFAULT 'medium',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                due_date TEXT,
                parent_goal_id TEXT REFERENCES goals(id),
                tags TEXT NOT NULL DEFAULT '[]'
            );

            CREATE TABLE IF NOT EXISTS key_results (
                id TEXT PRIMARY KEY,
                goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                metric_type TEXT NOT NULL,
                target_value REAL NOT NULL,
                current_value REAL NOT NULL DEFAULT 0.0,
                unit TEXT,
                status TEXT NOT NULL DEFAULT 'not_started',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS milestones (
                id TEXT PRIMARY KEY,
                goal_id TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
                title TEXT NOT NULL,
                description TEXT,
                due_date TEXT NOT NULL,
                completed_at TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS progress_watchers (
                id TEXT PRIMARY KEY,
                key_result_id TEXT NOT NULL REFERENCES key_results(id) ON DELETE CASCADE,
                cron_expr TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_config TEXT NOT NULL DEFAULT '{}',
                enabled INTEGER NOT NULL DEFAULT 1,
                last_run_at TEXT,
                last_result REAL,
                last_error TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS goal_audit_log (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                action TEXT NOT NULL,
                old_value TEXT,
                new_value TEXT,
                actor TEXT,
                timestamp TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_goals_parent ON goals(parent_goal_id);
            CREATE INDEX IF NOT EXISTS idx_kr_goal ON key_results(goal_id);
            CREATE INDEX IF NOT EXISTS idx_milestones_goal ON milestones(goal_id);
            CREATE INDEX IF NOT EXISTS idx_watchers_kr ON progress_watchers(key_result_id);
            CREATE INDEX IF NOT EXISTS idx_audit_entity ON goal_audit_log(entity_type, entity_id);
            ",
        )?;
        Ok(())
    }

    // -- Goals CRUD --

    /// Insert a new goal and write an audit-log entry.
    pub async fn create_goal(&self, goal: &Goal) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let tags_json = serde_json::to_string(&goal.tags)?;
        conn.execute(
            "INSERT INTO goals (id, title, description, owner, status, priority, created_at, updated_at, due_date, parent_goal_id, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                goal.id,
                goal.title,
                goal.description,
                goal.owner,
                goal.status.to_string(),
                goal.priority.to_string(),
                goal.created_at.to_rfc3339(),
                goal.updated_at.to_rfc3339(),
                goal.due_date.map(|d| d.to_rfc3339()),
                goal.parent_goal_id,
                tags_json,
            ],
        )?;
        self.log_audit_sync(&conn, "goal", &goal.id, "created", None, Some(&goal.title), None)?;
        Ok(())
    }

    /// Fetch a single goal by ID. Returns [`GoalsError::NotFound`] if missing.
    pub async fn get_goal(&self, id: &str) -> Result<Goal, GoalsError> {
        let conn = self.conn.read().await;
        conn.query_row(
            "SELECT id, title, description, owner, status, priority, created_at, updated_at, due_date, parent_goal_id, tags FROM goals WHERE id = ?1",
            params![id],
            |row| row_to_goal(row),
        )
        .optional()?
        .ok_or_else(|| GoalsError::NotFound(id.to_string()))
    }

    /// List goals, optionally filtered by parent. Pass `None` for top-level goals.
    pub async fn list_goals(&self, parent_id: Option<&str>) -> Result<Vec<Goal>, GoalsError> {
        let conn = self.conn.read().await;
        let mut stmt = match parent_id {
            Some(pid) => {
                let mut s = conn.prepare("SELECT id, title, description, owner, status, priority, created_at, updated_at, due_date, parent_goal_id, tags FROM goals WHERE parent_goal_id = ?1 ORDER BY created_at DESC")?;
                let rows = s.query_map(params![pid], |row| row_to_goal(row))?;
                return rows.collect::<Result<Vec<_>, _>>().map_err(Into::into);
            }
            None => conn.prepare("SELECT id, title, description, owner, status, priority, created_at, updated_at, due_date, parent_goal_id, tags FROM goals ORDER BY created_at DESC")?,
        };
        let rows = stmt.query_map([], |row| row_to_goal(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Transition a goal to a new status and record the change in the audit log.
    pub async fn update_goal_status(&self, id: &str, status: GoalStatus, actor: Option<&str>) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let old: Option<String> = conn
            .query_row("SELECT status FROM goals WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| GoalsError::NotFound(id.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE goals SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )?;
        self.log_audit_sync(&conn, "goal", id, "status_changed", old.as_deref(), Some(&status.to_string()), actor)?;
        Ok(())
    }

    /// Delete a goal and its cascaded children. Returns [`GoalsError::NotFound`] if missing.
    pub async fn delete_goal(&self, id: &str) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let affected = conn.execute("DELETE FROM goals WHERE id = ?1", params![id])?;
        if affected == 0 {
            return Err(GoalsError::NotFound(id.to_string()));
        }
        self.log_audit_sync(&conn, "goal", id, "deleted", None, None, None)?;
        Ok(())
    }

    // -- Key Results CRUD --

    /// Attach a new key result to an existing goal.
    pub async fn create_key_result(&self, kr: &KeyResult) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        conn.execute(
            "INSERT INTO key_results (id, goal_id, title, metric_type, target_value, current_value, unit, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                kr.id,
                kr.goal_id,
                kr.title,
                kr.metric_type.to_string(),
                kr.target_value,
                kr.current_value,
                kr.unit,
                kr.status.to_string(),
                kr.created_at.to_rfc3339(),
                kr.updated_at.to_rfc3339(),
            ],
        )?;
        self.log_audit_sync(&conn, "key_result", &kr.id, "created", None, Some(&kr.title), None)?;
        Ok(())
    }

    /// Update a key result's current value. Automatically transitions status to
    /// `Achieved` when the value reaches or exceeds the target.
    pub async fn update_kr_progress(&self, id: &str, value: f64, actor: Option<&str>) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let old: Option<f64> = conn
            .query_row("SELECT current_value FROM key_results WHERE id = ?1", params![id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| GoalsError::NotFound(id.to_string()))?;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE key_results SET current_value = ?1, updated_at = ?2, status = CASE WHEN ?1 >= (SELECT target_value FROM key_results WHERE id = ?3) THEN 'achieved' ELSE 'in_progress' END WHERE id = ?3",
            params![value, now, id],
        )?;
        self.log_audit_sync(
            &conn,
            "key_result",
            id,
            "progress_updated",
            old.map(|v| v.to_string()).as_deref(),
            Some(&value.to_string()),
            actor,
        )?;
        Ok(())
    }

    /// List all key results belonging to a goal, ordered by creation time.
    pub async fn list_key_results(&self, goal_id: &str) -> Result<Vec<KeyResult>, GoalsError> {
        let conn = self.conn.read().await;
        let mut stmt = conn.prepare(
            "SELECT id, goal_id, title, metric_type, target_value, current_value, unit, status, created_at, updated_at FROM key_results WHERE goal_id = ?1 ORDER BY created_at",
        )?;
        let rows = stmt.query_map(params![goal_id], |row| row_to_kr(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // -- Milestones CRUD --

    /// Add a checkpoint milestone to a goal's timeline.
    pub async fn create_milestone(&self, ms: &Milestone) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        conn.execute(
            "INSERT INTO milestones (id, goal_id, title, description, due_date, completed_at, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                ms.id,
                ms.goal_id,
                ms.title,
                ms.description,
                ms.due_date.to_rfc3339(),
                ms.completed_at.map(|d| d.to_rfc3339()),
                ms.status.to_string(),
                ms.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Mark a milestone as completed with the current timestamp.
    pub async fn complete_milestone(&self, id: &str) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let now = Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE milestones SET status = 'completed', completed_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        if affected == 0 {
            return Err(GoalsError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// List milestones for a goal, ordered by due date ascending.
    pub async fn list_milestones(&self, goal_id: &str) -> Result<Vec<Milestone>, GoalsError> {
        let conn = self.conn.read().await;
        let mut stmt = conn.prepare(
            "SELECT id, goal_id, title, description, due_date, completed_at, status, created_at FROM milestones WHERE goal_id = ?1 ORDER BY due_date",
        )?;
        let rows = stmt.query_map(params![goal_id], |row| row_to_milestone(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // -- Progress Watchers CRUD --

    /// Register a cron-scheduled watcher for a key result. Validates the cron expression
    /// before persisting.
    pub async fn create_watcher(&self, w: &ProgressWatcher) -> Result<(), GoalsError> {
        // Validate cron expression
        cron::Schedule::from_str(&w.cron_expr)
            .map_err(|e| GoalsError::InvalidCron(e.to_string()))?;
        let conn = self.conn.write().await;
        let config_json = serde_json::to_string(&w.source_config)?;
        conn.execute(
            "INSERT INTO progress_watchers (id, key_result_id, cron_expr, source_type, source_config, enabled, last_run_at, last_result, last_error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                w.id,
                w.key_result_id,
                w.cron_expr,
                w.source_type.to_string(),
                config_json,
                w.enabled as i32,
                w.last_run_at.map(|d| d.to_rfc3339()),
                w.last_result,
                w.last_error,
                w.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Return all watchers that are currently enabled.
    pub async fn list_enabled_watchers(&self) -> Result<Vec<ProgressWatcher>, GoalsError> {
        let conn = self.conn.read().await;
        let mut stmt = conn.prepare(
            "SELECT id, key_result_id, cron_expr, source_type, source_config, enabled, last_run_at, last_result, last_error, created_at FROM progress_watchers WHERE enabled = 1",
        )?;
        let rows = stmt.query_map([], |row| row_to_watcher(row))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Record the outcome of a watcher execution (result value or error message).
    pub async fn record_watcher_run(
        &self,
        id: &str,
        result: Option<f64>,
        error: Option<&str>,
    ) -> Result<(), GoalsError> {
        let conn = self.conn.write().await;
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE progress_watchers SET last_run_at = ?1, last_result = ?2, last_error = ?3 WHERE id = ?4",
            params![now, result, error, id],
        )?;
        Ok(())
    }

    // -- Audit Log --

    fn log_audit_sync(
        &self,
        conn: &Connection,
        entity_type: &str,
        entity_id: &str,
        action: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        actor: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO goal_audit_log (id, entity_type, entity_id, action, old_value, new_value, actor, timestamp) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, entity_type, entity_id, action, old_value, new_value, actor, now],
        )?;
        Ok(())
    }

    /// Retrieve the most recent audit-log entries for an entity, newest first.
    pub async fn get_audit_log(&self, entity_id: &str, limit: usize) -> Result<Vec<GoalAuditEntry>, GoalsError> {
        let conn = self.conn.read().await;
        let mut stmt = conn.prepare(
            "SELECT id, entity_type, entity_id, action, old_value, new_value, actor, timestamp FROM goal_audit_log WHERE entity_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![entity_id, limit as i64], |row| {
            Ok(GoalAuditEntry {
                id: row.get(0)?,
                entity_type: row.get(1)?,
                entity_id: row.get(2)?,
                action: row.get(3)?,
                old_value: row.get(4)?,
                new_value: row.get(5)?,
                actor: row.get(6)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .unwrap_or_default()
                    .with_timezone(&Utc),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    // -- Dashboard / Summary --

    /// Compute an aggregate dashboard snapshot across all goals, KRs, milestones, and watchers.
    pub async fn dashboard_summary(&self) -> Result<DashboardSummary, GoalsError> {
        let conn = self.conn.read().await;
        let total_goals: usize = conn.query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))?;
        let active_goals: usize = conn.query_row("SELECT COUNT(*) FROM goals WHERE status IN ('active', 'on_track', 'at_risk', 'blocked')", [], |r| r.get(0))?;
        let completed_goals: usize = conn.query_row("SELECT COUNT(*) FROM goals WHERE status = 'completed'", [], |r| r.get(0))?;
        let at_risk: usize = conn.query_row("SELECT COUNT(*) FROM goals WHERE status = 'at_risk'", [], |r| r.get(0))?;
        let overdue_milestones: usize = conn.query_row(
            "SELECT COUNT(*) FROM milestones WHERE status = 'pending' AND due_date < ?1",
            params![Utc::now().to_rfc3339()],
            |r| r.get(0),
        )?;
        let active_watchers: usize = conn.query_row("SELECT COUNT(*) FROM progress_watchers WHERE enabled = 1", [], |r| r.get(0))?;
        let avg_kr_progress: f64 = conn.query_row(
            "SELECT COALESCE(AVG(CASE WHEN target_value > 0 THEN (current_value / target_value) * 100.0 ELSE 0 END), 0) FROM key_results",
            [],
            |r| r.get(0),
        )?;

        Ok(DashboardSummary {
            total_goals,
            active_goals,
            completed_goals,
            at_risk_goals: at_risk,
            overdue_milestones,
            active_watchers,
            avg_kr_progress_pct: avg_kr_progress,
        })
    }
}

/// Aggregate metrics for a goals dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    /// Total number of goals in the store.
    pub total_goals: usize,
    /// Goals with an active lifecycle status (active, on_track, at_risk, blocked).
    pub active_goals: usize,
    /// Goals marked as completed.
    pub completed_goals: usize,
    /// Goals currently flagged as at-risk.
    pub at_risk_goals: usize,
    /// Pending milestones whose due date is in the past.
    pub overdue_milestones: usize,
    /// Number of enabled progress watchers.
    pub active_watchers: usize,
    /// Average key-result progress as a percentage (0–100).
    pub avg_kr_progress_pct: f64,
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

fn row_to_goal(row: &rusqlite::Row) -> Result<Goal, rusqlite::Error> {
    let tags_raw: String = row.get(10)?;
    let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
    let due_raw: Option<String> = row.get(8)?;
    let due_date = due_raw
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    Ok(Goal {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        owner: row.get(3)?,
        status: GoalStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(GoalStatus::Draft),
        priority: Priority::from_str(&row.get::<_, String>(5)?).unwrap_or(Priority::Medium),
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        due_date,
        parent_goal_id: row.get(9)?,
        tags,
    })
}

fn row_to_kr(row: &rusqlite::Row) -> Result<KeyResult, rusqlite::Error> {
    Ok(KeyResult {
        id: row.get(0)?,
        goal_id: row.get(1)?,
        title: row.get(2)?,
        metric_type: MetricType::from_str(&row.get::<_, String>(3)?).unwrap_or(MetricType::Custom),
        target_value: row.get(4)?,
        current_value: row.get(5)?,
        unit: row.get(6)?,
        status: KrStatus::from_str(&row.get::<_, String>(7)?).unwrap_or(KrStatus::NotStarted),
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
    })
}

fn row_to_milestone(row: &rusqlite::Row) -> Result<Milestone, rusqlite::Error> {
    let completed_raw: Option<String> = row.get(5)?;
    let completed_at = completed_raw
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    Ok(Milestone {
        id: row.get(0)?,
        goal_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        due_date: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        completed_at,
        status: MilestoneStatus::from_str(&row.get::<_, String>(6)?).unwrap_or(MilestoneStatus::Pending),
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
    })
}

fn row_to_watcher(row: &rusqlite::Row) -> Result<ProgressWatcher, rusqlite::Error> {
    let config_raw: String = row.get(4)?;
    let source_config: serde_json::Value = serde_json::from_str(&config_raw).unwrap_or_default();
    let last_run_raw: Option<String> = row.get(6)?;
    let last_run_at = last_run_raw
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    Ok(ProgressWatcher {
        id: row.get(0)?,
        key_result_id: row.get(1)?,
        cron_expr: row.get(2)?,
        source_type: WatcherSourceType::from_str(&row.get::<_, String>(3)?).unwrap_or(WatcherSourceType::Manual),
        source_config,
        enabled: row.get::<_, i32>(5)? != 0,
        last_run_at,
        last_result: row.get(7)?,
        last_error: row.get(8)?,
        created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_goal() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();
        let goal = Goal {
            id: "g1".into(),
            title: "Launch MVP".into(),
            description: Some("Q2 target".into()),
            owner: Some("alice".into()),
            status: GoalStatus::Active,
            priority: Priority::High,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec!["product".into()],
        };
        store.create_goal(&goal).await.unwrap();
        let fetched = store.get_goal("g1").await.unwrap();
        assert_eq!(fetched.title, "Launch MVP");
        assert_eq!(fetched.priority, Priority::High);
        assert_eq!(fetched.tags, vec!["product"]);
    }

    #[tokio::test]
    async fn test_kr_progress_auto_status() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();
        let goal = Goal {
            id: "g1".into(),
            title: "Test".into(),
            description: None,
            owner: None,
            status: GoalStatus::Active,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&goal).await.unwrap();
        let kr = KeyResult {
            id: "kr1".into(),
            goal_id: "g1".into(),
            title: "Reach 100 users".into(),
            metric_type: MetricType::Count,
            target_value: 100.0,
            current_value: 0.0,
            unit: Some("users".into()),
            status: KrStatus::NotStarted,
            created_at: now,
            updated_at: now,
        };
        store.create_key_result(&kr).await.unwrap();
        store.update_kr_progress("kr1", 100.0, None).await.unwrap();
        let krs = store.list_key_results("g1").await.unwrap();
        assert_eq!(krs[0].status, KrStatus::Achieved);
        assert_eq!(krs[0].current_value, 100.0);
    }

    #[tokio::test]
    async fn test_dashboard_summary() {
        let store = GoalStore::in_memory().unwrap();
        let summary = store.dashboard_summary().await.unwrap();
        assert_eq!(summary.total_goals, 0);
        assert_eq!(summary.avg_kr_progress_pct, 0.0);
    }

    #[tokio::test]
    async fn test_invalid_cron_rejected() {
        let store = GoalStore::in_memory().unwrap();
        let w = ProgressWatcher {
            id: "w1".into(),
            key_result_id: "kr1".into(),
            cron_expr: "not a cron".into(),
            source_type: WatcherSourceType::Manual,
            source_config: serde_json::json!({}),
            enabled: true,
            last_run_at: None,
            last_result: None,
            last_error: None,
            created_at: Utc::now(),
        };
        assert!(store.create_watcher(&w).await.is_err());
    }

    #[tokio::test]
    async fn test_milestone_lifecycle() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();
        let goal = Goal {
            id: "g1".into(),
            title: "Milestone Test".into(),
            description: None,
            owner: None,
            status: GoalStatus::Active,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&goal).await.unwrap();

        let ms = Milestone {
            id: "ms1".into(),
            goal_id: "g1".into(),
            title: "Beta release".into(),
            description: Some("Feature-complete build".into()),
            due_date: now + chrono::Duration::days(30),
            completed_at: None,
            status: MilestoneStatus::Pending,
            created_at: now,
        };
        store.create_milestone(&ms).await.unwrap();

        let milestones = store.list_milestones("g1").await.unwrap();
        assert_eq!(milestones.len(), 1);
        assert_eq!(milestones[0].status, MilestoneStatus::Pending);

        store.complete_milestone("ms1").await.unwrap();
        let milestones = store.list_milestones("g1").await.unwrap();
        assert_eq!(milestones[0].status, MilestoneStatus::Completed);
        assert!(milestones[0].completed_at.is_some());
    }

    #[tokio::test]
    async fn test_complete_nonexistent_milestone() {
        let store = GoalStore::in_memory().unwrap();
        let result = store.complete_milestone("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_goal_cascades() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();
        let goal = Goal {
            id: "g1".into(),
            title: "Delete Me".into(),
            description: None,
            owner: None,
            status: GoalStatus::Draft,
            priority: Priority::Low,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&goal).await.unwrap();

        let kr = KeyResult {
            id: "kr1".into(),
            goal_id: "g1".into(),
            title: "KR to cascade".into(),
            metric_type: MetricType::Percentage,
            target_value: 100.0,
            current_value: 0.0,
            unit: None,
            status: KrStatus::NotStarted,
            created_at: now,
            updated_at: now,
        };
        store.create_key_result(&kr).await.unwrap();

        store.delete_goal("g1").await.unwrap();
        assert!(store.get_goal("g1").await.is_err());
        // KR should be gone via CASCADE
        let krs = store.list_key_results("g1").await.unwrap();
        assert!(krs.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_goal() {
        let store = GoalStore::in_memory().unwrap();
        assert!(store.delete_goal("nope").await.is_err());
    }

    #[tokio::test]
    async fn test_list_goals_by_parent() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();

        let parent = Goal {
            id: "parent".into(),
            title: "Parent Goal".into(),
            description: None,
            owner: None,
            status: GoalStatus::Active,
            priority: Priority::High,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&parent).await.unwrap();

        let child = Goal {
            id: "child1".into(),
            title: "Child Goal".into(),
            description: None,
            owner: None,
            status: GoalStatus::Active,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: Some("parent".into()),
            tags: vec![],
        };
        store.create_goal(&child).await.unwrap();

        let top_level = store.list_goals(None).await.unwrap();
        assert_eq!(top_level.len(), 2); // both exist at top level query

        let children = store.list_goals(Some("parent")).await.unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, "child1");

        let empty = store.list_goals(Some("child1")).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_update_goal_status_and_audit() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();
        let goal = Goal {
            id: "g1".into(),
            title: "Audit Test".into(),
            description: None,
            owner: None,
            status: GoalStatus::Draft,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&goal).await.unwrap();

        store.update_goal_status("g1", GoalStatus::Active, Some("alice")).await.unwrap();
        let fetched = store.get_goal("g1").await.unwrap();
        assert_eq!(fetched.status, GoalStatus::Active);

        let audit = store.get_audit_log("g1", 10).await.unwrap();
        // At least: created + status_changed
        assert!(audit.len() >= 2);
        let status_entry = audit.iter().find(|e| e.action == "status_changed").unwrap();
        assert_eq!(status_entry.old_value.as_deref(), Some("draft"));
        assert_eq!(status_entry.new_value.as_deref(), Some("active"));
        assert_eq!(status_entry.actor.as_deref(), Some("alice"));
    }

    #[tokio::test]
    async fn test_watcher_create_and_record_run() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();

        // Need a goal + KR first for FK constraints
        let goal = Goal {
            id: "g1".into(),
            title: "Watcher Test".into(),
            description: None,
            owner: None,
            status: GoalStatus::Active,
            priority: Priority::Medium,
            created_at: now,
            updated_at: now,
            due_date: None,
            parent_goal_id: None,
            tags: vec![],
        };
        store.create_goal(&goal).await.unwrap();
        let kr = KeyResult {
            id: "kr1".into(),
            goal_id: "g1".into(),
            title: "Metric".into(),
            metric_type: MetricType::Count,
            target_value: 50.0,
            current_value: 0.0,
            unit: None,
            status: KrStatus::NotStarted,
            created_at: now,
            updated_at: now,
        };
        store.create_key_result(&kr).await.unwrap();

        let w = ProgressWatcher {
            id: "w1".into(),
            key_result_id: "kr1".into(),
            cron_expr: "0 */6 * * * *".into(), // every 6 hours
            source_type: WatcherSourceType::Manual,
            source_config: serde_json::json!({}),
            enabled: true,
            last_run_at: None,
            last_result: None,
            last_error: None,
            created_at: now,
        };
        store.create_watcher(&w).await.unwrap();

        let watchers = store.list_enabled_watchers().await.unwrap();
        assert_eq!(watchers.len(), 1);
        assert_eq!(watchers[0].cron_expr, "0 */6 * * * *");

        store.record_watcher_run("w1", Some(42.0), None).await.unwrap();
        let watchers = store.list_enabled_watchers().await.unwrap();
        assert_eq!(watchers[0].last_result, Some(42.0));
        assert!(watchers[0].last_run_at.is_some());
        assert!(watchers[0].last_error.is_none());

        store.record_watcher_run("w1", None, Some("timeout")).await.unwrap();
        let watchers = store.list_enabled_watchers().await.unwrap();
        assert_eq!(watchers[0].last_error.as_deref(), Some("timeout"));
    }

    #[tokio::test]
    async fn test_dashboard_summary_with_data() {
        let store = GoalStore::in_memory().unwrap();
        let now = Utc::now();

        // Create two goals: one active, one completed
        for (id, status) in [("g1", GoalStatus::Active), ("g2", GoalStatus::Completed)] {
            store.create_goal(&Goal {
                id: id.into(),
                title: format!("Goal {}", id),
                description: None,
                owner: None,
                status,
                priority: Priority::Medium,
                created_at: now,
                updated_at: now,
                due_date: None,
                parent_goal_id: None,
                tags: vec![],
            }).await.unwrap();
        }

        // One KR at 50% progress
        store.create_key_result(&KeyResult {
            id: "kr1".into(),
            goal_id: "g1".into(),
            title: "Half done".into(),
            metric_type: MetricType::Percentage,
            target_value: 100.0,
            current_value: 50.0,
            unit: Some("%".into()),
            status: KrStatus::InProgress,
            created_at: now,
            updated_at: now,
        }).await.unwrap();

        let summary = store.dashboard_summary().await.unwrap();
        assert_eq!(summary.total_goals, 2);
        assert_eq!(summary.active_goals, 1);
        assert_eq!(summary.completed_goals, 1);
        assert!((summary.avg_kr_progress_pct - 50.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_enum_from_str_roundtrip() {
        // GoalStatus
        for s in &["draft", "active", "on_track", "at_risk", "blocked", "completed", "cancelled"] {
            let parsed = GoalStatus::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }
        assert!(GoalStatus::from_str("bogus").is_err());

        // Priority
        for s in &["low", "medium", "high", "critical"] {
            let parsed = Priority::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }
        assert!(Priority::from_str("ultra").is_err());

        // MetricType
        for s in &["percentage", "count", "currency", "duration", "boolean", "custom"] {
            let parsed = MetricType::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }

        // KrStatus
        for s in &["not_started", "in_progress", "achieved", "missed"] {
            let parsed = KrStatus::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }

        // MilestoneStatus
        for s in &["pending", "completed", "overdue", "skipped"] {
            let parsed = MilestoneStatus::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }

        // WatcherSourceType
        for s in &["skill_execution", "integration_action", "sql_query", "http_endpoint", "manual"] {
            let parsed = WatcherSourceType::from_str(s).unwrap();
            assert_eq!(parsed.to_string(), *s);
        }
    }
}