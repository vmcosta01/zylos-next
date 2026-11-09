//! # zylos-swarm-orchestrator
//!
//! Multi-agent swarm orchestration for zylos-gateway:
//! - Task decomposition into atomic work units
//! - Dependency DAG with topological scheduling
//! - Agent assignment and load balancing
//! - Execution state machine with retry/backoff
//! - SQLite WAL-backed persistence for crash recovery

use chrono::{DateTime, Utc};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// Unique identifier for a task in the swarm.
pub type TaskId = String;

/// Unique identifier for an agent in the swarm.
pub type AgentId = String;

/// Current state of a task in the execution lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Waiting for dependencies to complete.
    Pending,
    /// All dependencies met; eligible for scheduling.
    Ready,
    /// Assigned to an agent and executing.
    Running,
    /// Completed successfully.
    Done,
    /// Failed after exhausting retries.
    Failed,
    /// Cancelled by user or system.
    Cancelled,
    /// Blocked waiting for external input.
    Blocked,
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Ready => write!(f, "ready"),
            Self::Running => write!(f, "running"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

impl FromStr for TaskState {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "ready" => Ok(Self::Ready),
            "running" => Ok(Self::Running),
            "done" => Ok(Self::Done),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "blocked" => Ok(Self::Blocked),
            _ => Err(format!("Unknown task state: {}", s)),
        }
    }
}

/// Priority level for task scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Normal
    }
}

/// A single unit of work in the swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task.
    pub id: TaskId,
    /// Human-readable name for this task.
    pub name: String,
    /// Detailed description of the work to be done.
    pub description: String,
    /// Current lifecycle state of this task.
    pub state: TaskState,
    /// Scheduling priority level.
    pub priority: Priority,
    /// IDs of tasks that must complete before this one can start.
    pub dependencies: Vec<TaskId>,
    /// The agent currently assigned to this task (if any).
    pub assigned_agent: Option<AgentId>,
    /// Required capabilities/tags for agent matching.
    pub required_capabilities: Vec<String>,
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Current retry count.
    pub retry_count: u32,
    /// Timeout in seconds; None = no timeout.
    pub timeout_seconds: Option<u64>,
    /// Arbitrary payload data for the agent.
    pub payload: serde_json::Value,
    /// Result data after completion.
    pub result: Option<serde_json::Value>,
    /// Error message if failed.
    pub error: Option<String>,
    /// Timestamp when this task was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last modification.
    pub updated_at: DateTime<Utc>,
    /// Timestamp when execution started, if applicable.
    pub started_at: Option<DateTime<Utc>>,
    /// Timestamp when the task reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,
    /// Parent task ID for hierarchical decomposition.
    pub parent_id: Option<TaskId>,
    /// Tags for filtering and grouping.
    pub tags: Vec<String>,
}

impl Task {
    /// Creates a new task with the given name and description.
    ///
    /// The task starts in [`TaskState::Pending`] with default priority,
    /// 3 max retries, and a freshly generated UUID.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            state: TaskState::Pending,
            priority: Priority::Normal,
            dependencies: Vec::new(),
            assigned_agent: None,
            required_capabilities: Vec::new(),
            max_retries: 3,
            retry_count: 0,
            timeout_seconds: None,
            payload: serde_json::Value::Null,
            result: None,
            error: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            parent_id: None,
            tags: Vec::new(),
        }
    }

    /// Sets the dependency list for this task (builder pattern).
    pub fn with_dependencies(mut self, deps: Vec<TaskId>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Sets the scheduling priority for this task (builder pattern).
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the required agent capabilities for this task (builder pattern).
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.required_capabilities = caps;
        self
    }

    /// Attaches an arbitrary JSON payload to this task (builder pattern).
    pub fn with_payload(mut self, payload: serde_json::Value) -> Self {
        self.payload = payload;
        self
    }

    /// Sets the parent task ID for hierarchical decomposition (builder pattern).
    pub fn with_parent(mut self, parent_id: TaskId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Returns true if this task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TaskState::Done | TaskState::Failed | TaskState::Cancelled
        )
    }

    /// Computes a fingerprint for change detection.
    pub fn fingerprint(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.name.as_bytes());
        hasher.update(self.state.to_string().as_bytes());
        hasher.update(self.retry_count.to_le_bytes());
        hex::encode(hasher.finalize())
    }
}

/// An agent participating in the swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmAgent {
    /// Unique identifier for this agent.
    pub id: AgentId,
    /// Human-readable name for this agent.
    pub name: String,
    /// List of capability tags this agent provides.
    pub capabilities: Vec<String>,
    /// Maximum number of tasks this agent can run simultaneously.
    pub max_concurrent_tasks: usize,
    /// Number of tasks currently assigned to this agent.
    pub current_load: usize,
    /// Whether this agent is currently available for scheduling.
    pub active: bool,
    /// Timestamp of the last heartbeat received from this agent.
    pub last_heartbeat: DateTime<Utc>,
    /// Arbitrary key-value metadata attached to this agent.
    pub metadata: HashMap<String, String>,
}

impl SwarmAgent {
    /// Creates a new active agent with the given ID and name.
    ///
    /// Defaults to 1 max concurrent task and zero current load.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            capabilities: Vec::new(),
            max_concurrent_tasks: 1,
            current_load: 0,
            active: true,
            last_heartbeat: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Returns how many more tasks this agent can accept right now.
    ///
    /// Returns 0 if the agent is inactive or at capacity.
    pub fn available_capacity(&self) -> usize {
        if !self.active {
            return 0;
        }
        self.max_concurrent_tasks.saturating_sub(self.current_load)
    }

    /// Checks whether this agent has the capabilities and capacity to handle
    /// the given task.
    pub fn can_handle(&self, task: &Task) -> bool {
        if !self.active || self.available_capacity() == 0 {
            return false;
        }
        if task.required_capabilities.is_empty() {
            return true;
        }
        task.required_capabilities
            .iter()
            .all(|cap| self.capabilities.contains(cap))
    }
}

/// Result of scheduling a batch of tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleResult {
    /// Pairs of (task_id, agent_id) that were successfully assigned.
    pub assignments: Vec<(TaskId, AgentId)>,
    /// Tasks that could not be scheduled because dependencies are unmet.
    pub blocked_tasks: Vec<TaskId>,
    /// Tasks whose required capabilities match no registered agent.
    pub unassignable_tasks: Vec<TaskId>,
    /// Non-fatal errors encountered during scheduling.
    pub errors: Vec<String>,
}

/// Aggregate statistics about the swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmStats {
    /// Total number of tasks across all states.
    pub total_tasks: usize,
    /// Tasks waiting for dependencies to resolve.
    pub pending_tasks: usize,
    /// Tasks eligible for scheduling (all dependencies met).
    pub ready_tasks: usize,
    /// Tasks currently assigned to an agent and executing.
    pub running_tasks: usize,
    /// Tasks that completed successfully.
    pub done_tasks: usize,
    /// Tasks that failed after exhausting retries.
    pub failed_tasks: usize,
    /// Tasks blocked waiting for external input.
    pub blocked_tasks: usize,
    /// Tasks cancelled by user or system.
    pub cancelled_tasks: usize,
    /// Total number of registered agents.
    pub total_agents: usize,
    /// Number of agents currently marked active.
    pub active_agents: usize,
    /// Sum of max_concurrent_tasks across all active agents.
    pub total_capacity: usize,
    /// Sum of current_load across all agents.
    pub used_capacity: usize,
}

/// Query filter for task lookups.
#[derive(Debug, Clone, Default)]
pub struct TaskQuery {
    /// Filter by exact task state.
    pub state: Option<TaskState>,
    /// Filter by assigned agent ID.
    pub assigned_agent: Option<AgentId>,
    /// Filter by parent task ID.
    pub parent_id: Option<TaskId>,
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

#[derive(Debug, thiserror::Error)]
/// Errors produced by swarm orchestrator operations.
pub enum SwarmError {
    /// Wraps a sqlx database error.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    /// Wraps a serde_json serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A referenced task does not exist in the store.
    #[error("Task not found: {0}")]
    TaskNotFound(String),
    /// A referenced agent does not exist in the registry.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    /// The task dependency graph contains a cycle.
    #[error("Cycle detected in task dependencies")]
    CycleDetected,
    /// An attempted state transition violates the state machine rules.
    #[error("Invalid state transition: {0} -> {1}")]
    InvalidTransition(String, String),
    /// Wraps a std::io::Error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Catch-all for internal orchestrator errors.
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Swarm Orchestrator
// ---------------------------------------------------------------------------

/// Main orchestrator for multi-agent swarm coordination.
#[derive(Clone)]
pub struct SwarmOrchestrator {
    pool: SqlitePool,
}

impl SwarmOrchestrator {
    /// Opens or creates the swarm orchestrator store at the given path.
    pub async fn open(path: &str) -> Result<Self, SwarmError> {
        let options = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await?;
        let orch = Self { pool };
        orch.run_migrations().await?;
        Ok(orch)
    }

    async fn run_migrations(&self) -> Result<(), SwarmError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS swarm_tasks (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT 'pending',
                priority INTEGER NOT NULL DEFAULT 1,
                dependencies TEXT NOT NULL DEFAULT '[]',
                assigned_agent TEXT,
                required_capabilities TEXT NOT NULL DEFAULT '[]',
                max_retries INTEGER NOT NULL DEFAULT 3,
                retry_count INTEGER NOT NULL DEFAULT 0,
                timeout_seconds INTEGER,
                payload TEXT NOT NULL DEFAULT 'null',
                result TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT,
                parent_id TEXT,
                tags TEXT NOT NULL DEFAULT '[]'
            );
            CREATE INDEX IF NOT EXISTS idx_swarm_tasks_state ON swarm_tasks(state);
            CREATE INDEX IF NOT EXISTS idx_swarm_tasks_agent ON swarm_tasks(assigned_agent);
            CREATE INDEX IF NOT EXISTS idx_swarm_tasks_parent ON swarm_tasks(parent_id);
            CREATE INDEX IF NOT EXISTS idx_swarm_tasks_priority ON swarm_tasks(priority DESC);

            CREATE TABLE IF NOT EXISTS swarm_agents (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                capabilities TEXT NOT NULL DEFAULT '[]',
                max_concurrent_tasks INTEGER NOT NULL DEFAULT 1,
                current_load INTEGER NOT NULL DEFAULT 0,
                active INTEGER NOT NULL DEFAULT 1,
                last_heartbeat TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}'
            );
            CREATE INDEX IF NOT EXISTS idx_swarm_agents_active ON swarm_agents(active);
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Task CRUD --

    /// Adds a new task to the swarm.
    pub async fn add_task(&self, task: &Task) -> Result<(), SwarmError> {
        let deps_json = serde_json::to_string(&task.dependencies)?;
        let caps_json = serde_json::to_string(&task.required_capabilities)?;
        let payload_json = serde_json::to_string(&task.payload)?;
        let result_json = task.result.as_ref().map(|r| serde_json::to_string(r)).transpose()?;
        let tags_json = serde_json::to_string(&task.tags)?;

        sqlx::query(
            r#"INSERT INTO swarm_tasks
                (id, name, description, state, priority, dependencies, assigned_agent,
                 required_capabilities, max_retries, retry_count, timeout_seconds,
                 payload, result, error, created_at, updated_at, started_at, completed_at,
                 parent_id, tags)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&task.id)
        .bind(&task.name)
        .bind(&task.description)
        .bind(task.state.to_string())
        .bind(task.priority as i32)
        .bind(&deps_json)
        .bind(&task.assigned_agent)
        .bind(&caps_json)
        .bind(task.max_retries as i32)
        .bind(task.retry_count as i32)
        .bind(task.timeout_seconds.map(|t| t as i64))
        .bind(&payload_json)
        .bind(&result_json)
        .bind(&task.error)
        .bind(task.created_at.to_rfc3339())
        .bind(task.updated_at.to_rfc3339())
        .bind(task.started_at.map(|t| t.to_rfc3339()))
        .bind(task.completed_at.map(|t| t.to_rfc3339()))
        .bind(&task.parent_id)
        .bind(&tags_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Retrieves a task by ID.
    pub async fn get_task(&self, id: &str) -> Result<Option<Task>, SwarmError> {
        let row = sqlx::query("SELECT * FROM swarm_tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_task(&r))),
            None => Ok(None),
        }
    }

    /// Updates an existing task.
    pub async fn update_task(&self, task: &Task) -> Result<bool, SwarmError> {
        let deps_json = serde_json::to_string(&task.dependencies)?;
        let caps_json = serde_json::to_string(&task.required_capabilities)?;
        let payload_json = serde_json::to_string(&task.payload)?;
        let result_json = task.result.as_ref().map(|r| serde_json::to_string(r)).transpose()?;
        let tags_json = serde_json::to_string(&task.tags)?;

        let result = sqlx::query(
            r#"UPDATE swarm_tasks SET
                name = ?, description = ?, state = ?, priority = ?, dependencies = ?,
                assigned_agent = ?, required_capabilities = ?, max_retries = ?,
                retry_count = ?, timeout_seconds = ?, payload = ?, result = ?, error = ?,
                updated_at = ?, started_at = ?, completed_at = ?, parent_id = ?, tags = ?
            WHERE id = ?"#,
        )
        .bind(&task.name)
        .bind(&task.description)
        .bind(task.state.to_string())
        .bind(task.priority as i32)
        .bind(&deps_json)
        .bind(&task.assigned_agent)
        .bind(&caps_json)
        .bind(task.max_retries as i32)
        .bind(task.retry_count as i32)
        .bind(task.timeout_seconds.map(|t| t as i64))
        .bind(&payload_json)
        .bind(&result_json)
        .bind(&task.error)
        .bind(Utc::now().to_rfc3339())
        .bind(task.started_at.map(|t| t.to_rfc3339()))
        .bind(task.completed_at.map(|t| t.to_rfc3339()))
        .bind(&task.parent_id)
        .bind(&tags_json)
        .bind(&task.id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Queries tasks with optional filters.
    pub async fn query_tasks(&self, q: &TaskQuery) -> Result<Vec<Task>, SwarmError> {
        let mut sql = String::from("SELECT * FROM swarm_tasks WHERE 1=1");
        let mut conditions = Vec::new();

        if q.state.is_some() {
            conditions.push("state = ?");
        }
        if q.assigned_agent.is_some() {
            conditions.push("assigned_agent = ?");
        }
        if q.parent_id.is_some() {
            conditions.push("parent_id = ?");
        }
        if q.tag.is_some() {
            conditions.push("tags LIKE ?");
        }

        if !conditions.is_empty() {
            sql.push_str(" AND ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY priority DESC, created_at ASC");
        if q.limit.is_some() {
            sql.push_str(" LIMIT ?");
        }
        if q.offset.is_some() {
            sql.push_str(" OFFSET ?");
        }

        let mut query = sqlx::query(&sql);
        if let Some(ref state) = q.state {
            query = query.bind(state.to_string());
        }
        if let Some(ref agent) = q.assigned_agent {
            query = query.bind(agent);
        }
        if let Some(ref parent) = q.parent_id {
            query = query.bind(parent);
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
        Ok(rows.iter().map(row_to_task).collect())
    }

    // -- State Transitions --

    /// Transitions a task to a new state with validation.
    pub async fn transition_task(
        &self,
        task_id: &str,
        new_state: TaskState,
        error: Option<String>,
    ) -> Result<Task, SwarmError> {
        let mut task = self
            .get_task(task_id)
            .await?
            .ok_or_else(|| SwarmError::TaskNotFound(task_id.to_string()))?;

        validate_transition(task.state, new_state)?;

        let now = Utc::now();
        task.state = new_state;
        task.updated_at = now;

        match new_state {
            TaskState::Running => {
                task.started_at = Some(now);
            }
            TaskState::Done | TaskState::Failed | TaskState::Cancelled => {
                task.completed_at = Some(now);
                if new_state == TaskState::Failed {
                    task.error = error;
                }
            }
            _ => {}
        }

        self.update_task(&task).await?;
        Ok(task)
    }

    /// Marks a task as completed with a result.
    pub async fn complete_task(
        &self,
        task_id: &str,
        result: serde_json::Value,
    ) -> Result<Task, SwarmError> {
        let mut task = self
            .get_task(task_id)
            .await?
            .ok_or_else(|| SwarmError::TaskNotFound(task_id.to_string()))?;

        validate_transition(task.state, TaskState::Done)?;

        let now = Utc::now();
        task.state = TaskState::Done;
        task.result = Some(result);
        task.completed_at = Some(now);
        task.updated_at = now;

        self.update_task(&task).await?;

        // Promote dependent tasks to Ready if all their deps are now Done
        self.promote_ready_dependents(task_id).await?;

        Ok(task)
    }

    /// Retries a failed task if retries remain.
    pub async fn retry_task(&self, task_id: &str) -> Result<Task, SwarmError> {
        let mut task = self
            .get_task(task_id)
            .await?
            .ok_or_else(|| SwarmError::TaskNotFound(task_id.to_string()))?;

        if task.state != TaskState::Failed {
            return Err(SwarmError::InvalidTransition(
                task.state.to_string(),
                "retry".to_string(),
            ));
        }

        if task.retry_count >= task.max_retries {
            return Err(SwarmError::Internal(format!(
                "Task {} exhausted all {} retries",
                task_id, task.max_retries
            )));
        }

        task.retry_count += 1;
        task.state = TaskState::Ready;
        task.error = None;
        task.assigned_agent = None;
        task.started_at = None;
        task.completed_at = None;
        task.updated_at = Utc::now();

        self.update_task(&task).await?;
        Ok(task)
    }

    // -- Dependency Resolution --

    /// Checks for cycles in the task dependency graph.
    pub async fn validate_dag(&self) -> Result<(), SwarmError> {
        let tasks = self
            .query_tasks(&TaskQuery::default())
            .await?;
        let graph = build_dependency_graph(&tasks);
        toposort(&graph, None).map_err(|_| SwarmError::CycleDetected)?;
        Ok(())
    }

    /// Promotes tasks from Pending to Ready when all dependencies are Done.
    pub async fn promote_ready_dependents(&self, completed_task_id: &str) -> Result<usize, SwarmError> {
        // Find all tasks that depend on the completed task
        let all_tasks = self.query_tasks(&TaskQuery {
            state: Some(TaskState::Pending),
            ..Default::default()
        }).await?;

        let mut promoted = 0;
        for task in &all_tasks {
            if !task.dependencies.contains(&completed_task_id.to_string()) {
                continue;
            }

            // Check if ALL dependencies are Done
            let mut all_done = true;
            for dep_id in &task.dependencies {
                if let Some(dep) = self.get_task(dep_id).await? {
                    if dep.state != TaskState::Done {
                        all_done = false;
                        break;
                    }
                } else {
                    all_done = false;
                    break;
                }
            }

            if all_done {
                let mut updated = task.clone();
                updated.state = TaskState::Ready;
                updated.updated_at = Utc::now();
                self.update_task(&updated).await?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }

    /// Initializes task states: tasks with no pending dependencies become Ready.
    pub async fn initialize_ready_tasks(&self) -> Result<usize, SwarmError> {
        let pending = self.query_tasks(&TaskQuery {
            state: Some(TaskState::Pending),
            ..Default::default()
        }).await?;

        let mut promoted = 0;
        for task in &pending {
            if task.dependencies.is_empty() {
                let mut updated = task.clone();
                updated.state = TaskState::Ready;
                updated.updated_at = Utc::now();
                self.update_task(&updated).await?;
                promoted += 1;
                continue;
            }

            let mut all_done = true;
            for dep_id in &task.dependencies {
                if let Some(dep) = self.get_task(dep_id).await? {
                    if dep.state != TaskState::Done {
                        all_done = false;
                        break;
                    }
                } else {
                    all_done = false;
                    break;
                }
            }

            if all_done {
                let mut updated = task.clone();
                updated.state = TaskState::Ready;
                updated.updated_at = Utc::now();
                self.update_task(&updated).await?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }

    // -- Agent Management --

    /// Registers or updates an agent in the swarm.
    pub async fn register_agent(&self, agent: &SwarmAgent) -> Result<(), SwarmError> {
        let caps_json = serde_json::to_string(&agent.capabilities)?;
        let meta_json = serde_json::to_string(&agent.metadata)?;

        sqlx::query(
            r#"INSERT INTO swarm_agents (id, name, capabilities, max_concurrent_tasks,
                current_load, active, last_heartbeat, metadata)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                capabilities = excluded.capabilities,
                max_concurrent_tasks = excluded.max_concurrent_tasks,
                current_load = excluded.current_load,
                active = excluded.active,
                last_heartbeat = excluded.last_heartbeat,
                metadata = excluded.metadata"#,
        )
        .bind(&agent.id)
        .bind(&agent.name)
        .bind(&caps_json)
        .bind(agent.max_concurrent_tasks as i32)
        .bind(agent.current_load as i32)
        .bind(agent.active as i32)
        .bind(agent.last_heartbeat.to_rfc3339())
        .bind(&meta_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Retrieves an agent by ID.
    pub async fn get_agent(&self, id: &str) -> Result<Option<SwarmAgent>, SwarmError> {
        let row = sqlx::query("SELECT * FROM swarm_agents WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_agent(&r))),
            None => Ok(None),
        }
    }

    /// Lists all registered agents.
    pub async fn list_agents(&self) -> Result<Vec<SwarmAgent>, SwarmError> {
        let rows = sqlx::query("SELECT * FROM swarm_agents ORDER BY name")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(row_to_agent).collect())
    }

    /// Updates agent heartbeat timestamp.
    pub async fn heartbeat(&self, agent_id: &str) -> Result<(), SwarmError> {
        let result = sqlx::query("UPDATE swarm_agents SET last_heartbeat = ? WHERE id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(SwarmError::AgentNotFound(agent_id.to_string()));
        }
        Ok(())
    }

    // -- Scheduling --

    /// Assigns ready tasks to available agents using capability matching and priority ordering.
    pub async fn schedule(&self) -> Result<ScheduleResult, SwarmError> {
        let ready_tasks = self.query_tasks(&TaskQuery {
            state: Some(TaskState::Ready),
            ..Default::default()
        }).await?;

        let agents = self.list_agents().await?;
        let mut agent_loads: HashMap<AgentId, usize> = agents
            .iter()
            .map(|a| (a.id.clone(), a.current_load))
            .collect();
        let _agent_map: HashMap<&str, &SwarmAgent> = agents.iter().map(|a| (a.id.as_str(), a)).collect();

        let mut assignments = Vec::new();
        let mut unassignable = Vec::new();

        // Sort by priority descending
        let mut sorted_tasks = ready_tasks;
        sorted_tasks.sort_by(|a, b| b.priority.cmp(&a.priority));

        for task in &sorted_tasks {
            // Find best-fit agent
            let best_agent = agents
                .iter()
                .filter(|a| {
                    a.active
                        && agent_loads.get(&a.id).copied().unwrap_or(0) < a.max_concurrent_tasks
                        && a.can_handle(task)
                })
                .min_by_key(|a| agent_loads.get(&a.id).copied().unwrap_or(0));

            if let Some(agent) = best_agent {
                // Assign task
                let mut updated_task = task.clone();
                updated_task.state = TaskState::Running;
                updated_task.assigned_agent = Some(agent.id.clone());
                updated_task.started_at = Some(Utc::now());
                updated_task.updated_at = Utc::now();
                self.update_task(&updated_task).await?;

                *agent_loads.entry(agent.id.clone()).or_insert(0) += 1;

                // Update agent load in DB
                let new_load = agent_loads[&agent.id];
                sqlx::query("UPDATE swarm_agents SET current_load = ? WHERE id = ?")
                    .bind(new_load as i32)
                    .bind(&agent.id)
                    .execute(&self.pool)
                    .await?;

                assignments.push((task.id.clone(), agent.id.clone()));
            } else {
                unassignable.push(task.id.clone());
            }
        }

        Ok(ScheduleResult {
            assignments,
            blocked_tasks: Vec::new(),
            unassignable_tasks: unassignable,
            errors: Vec::new(),
        })
    }

    /// Releases a completed/failed task's slot on its assigned agent.
    pub async fn release_agent_slot(&self, task_id: &str) -> Result<(), SwarmError> {
        let task = self
            .get_task(task_id)
            .await?
            .ok_or_else(|| SwarmError::TaskNotFound(task_id.to_string()))?;

        if let Some(agent_id) = &task.assigned_agent {
            sqlx::query(
                "UPDATE swarm_agents SET current_load = MAX(0, current_load - 1) WHERE id = ?",
            )
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // -- Stats --

    /// Returns aggregate swarm statistics.
    pub async fn stats(&self) -> Result<SwarmStats, SwarmError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks")
            .fetch_one(&self.pool)
            .await?;
        let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'pending'")
            .fetch_one(&self.pool)
            .await?;
        let ready: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'ready'")
            .fetch_one(&self.pool)
            .await?;
        let running: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'running'")
            .fetch_one(&self.pool)
            .await?;
        let done: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'done'")
            .fetch_one(&self.pool)
            .await?;
        let failed: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'failed'")
            .fetch_one(&self.pool)
            .await?;
        let blocked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'blocked'")
            .fetch_one(&self.pool)
            .await?;
        let cancelled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE state = 'cancelled'")
            .fetch_one(&self.pool)
            .await?;

        let total_agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_agents")
            .fetch_one(&self.pool)
            .await?;
        let active_agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_agents WHERE active = 1")
            .fetch_one(&self.pool)
            .await?;
        let total_capacity: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(max_concurrent_tasks), 0) FROM swarm_agents WHERE active = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        let used_capacity: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(current_load), 0) FROM swarm_agents",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(SwarmStats {
            total_tasks: total as usize,
            pending_tasks: pending as usize,
            ready_tasks: ready as usize,
            running_tasks: running as usize,
            done_tasks: done as usize,
            failed_tasks: failed as usize,
            blocked_tasks: blocked as usize,
            cancelled_tasks: cancelled as usize,
            total_agents: total_agents as usize,
            active_agents: active_agents as usize,
            total_capacity: total_capacity as usize,
            used_capacity: used_capacity as usize,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_transition(from: TaskState, to: TaskState) -> Result<(), SwarmError> {
    let valid = matches!(
        (from, to),
        (TaskState::Pending, TaskState::Ready)
            | (TaskState::Pending, TaskState::Cancelled)
            | (TaskState::Ready, TaskState::Running)
            | (TaskState::Ready, TaskState::Cancelled)
            | (TaskState::Ready, TaskState::Blocked)
            | (TaskState::Running, TaskState::Done)
            | (TaskState::Running, TaskState::Failed)
            | (TaskState::Running, TaskState::Cancelled)
            | (TaskState::Running, TaskState::Blocked)
            | (TaskState::Failed, TaskState::Ready) // retry
            | (TaskState::Blocked, TaskState::Ready)
            | (TaskState::Blocked, TaskState::Cancelled)
    );
    if valid {
        Ok(())
    } else {
        Err(SwarmError::InvalidTransition(from.to_string(), to.to_string()))
    }
}

fn build_dependency_graph(tasks: &[Task]) -> DiGraph<TaskId, ()> {
    let mut graph = DiGraph::new();
    let mut node_map: HashMap<&str, NodeIndex> = HashMap::new();

    for task in tasks {
        let idx = graph.add_node(task.id.clone());
        node_map.insert(&task.id, idx);
    }

    for task in tasks {
        if let Some(&to_idx) = node_map.get(task.id.as_str()) {
            for dep_id in &task.dependencies {
                if let Some(&from_idx) = node_map.get(dep_id.as_str()) {
                    graph.add_edge(from_idx, to_idx, ());
                }
            }
        }
    }

    graph
}

fn row_to_task(row: &sqlx::sqlite::SqliteRow) -> Task {
    let deps_str: String = row.get("dependencies");
    let caps_str: String = row.get("required_capabilities");
    let payload_str: String = row.get("payload");
    let result_str: Option<String> = row.get("result");
    let tags_str: String = row.get("tags");
    let state_str: String = row.get("state");
    let priority_int: i32 = row.get("priority");

    Task {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        state: TaskState::from_str(&state_str).unwrap_or(TaskState::Pending),
        priority: match priority_int {
            0 => Priority::Low,
            2 => Priority::High,
            3 => Priority::Critical,
            _ => Priority::Normal,
        },
        dependencies: serde_json::from_str(&deps_str).unwrap_or_default(),
        assigned_agent: row.get("assigned_agent"),
        required_capabilities: serde_json::from_str(&caps_str).unwrap_or_default(),
        max_retries: row.get::<i32, _>("max_retries") as u32,
        retry_count: row.get::<i32, _>("retry_count") as u32,
        timeout_seconds: row.get::<Option<i64>, _>("timeout_seconds").map(|t| t as u64),
        payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
        result: result_str.and_then(|s| serde_json::from_str(&s).ok()),
        error: row.get("error"),
        created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        started_at: row.get::<Option<String>, _>("started_at")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        completed_at: row.get::<Option<String>, _>("completed_at")
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc)),
        parent_id: row.get("parent_id"),
        tags: serde_json::from_str(&tags_str).unwrap_or_default(),
    }
}

fn row_to_agent(row: &sqlx::sqlite::SqliteRow) -> SwarmAgent {
    let caps_str: String = row.get("capabilities");
    let meta_str: String = row.get("metadata");

    SwarmAgent {
        id: row.get("id"),
        name: row.get("name"),
        capabilities: serde_json::from_str(&caps_str).unwrap_or_default(),
        max_concurrent_tasks: row.get::<i32, _>("max_concurrent_tasks") as usize,
        current_load: row.get::<i32, _>("current_load") as usize,
        active: row.get::<i32, _>("active") != 0,
        last_heartbeat: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_heartbeat"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        metadata: serde_json::from_str(&meta_str).unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_orchestrator() -> SwarmOrchestrator {
        SwarmOrchestrator::open(":memory:").await.unwrap()
    }

    #[tokio::test]
    async fn test_add_and_get_task() {
        let orch = test_orchestrator().await;
        let task = Task::new("Build frontend", "Compile React app");
        orch.add_task(&task).await.unwrap();

        let retrieved = orch.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(retrieved.name, "Build frontend");
        assert_eq!(retrieved.state, TaskState::Pending);
    }

    #[tokio::test]
    async fn test_state_transitions() {
        let orch = test_orchestrator().await;
        let task = Task::new("Test task", "Testing transitions");
        orch.add_task(&task).await.unwrap();

        // Pending -> Ready
        let t = orch.transition_task(&task.id, TaskState::Ready, None).await.unwrap();
        assert_eq!(t.state, TaskState::Ready);

        // Ready -> Running
        let t = orch.transition_task(&task.id, TaskState::Running, None).await.unwrap();
        assert_eq!(t.state, TaskState::Running);
        assert!(t.started_at.is_some());

        // Running -> Done
        let t = orch.complete_task(&task.id, serde_json::json!({"status": "ok"})).await.unwrap();
        assert_eq!(t.state, TaskState::Done);
        assert!(t.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_invalid_transition_rejected() {
        let orch = test_orchestrator().await;
        let task = Task::new("Bad transition", "Should fail");
        orch.add_task(&task).await.unwrap();

        // Pending -> Done is invalid
        let err = orch.transition_task(&task.id, TaskState::Done, None).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_retry_exhaustion() {
        let orch = test_orchestrator().await;
        let mut task = Task::new("Retry test", "Will fail");
        task.max_retries = 1;
        orch.add_task(&task).await.unwrap();

        // Move to Failed
        orch.transition_task(&task.id, TaskState::Ready, None).await.unwrap();
        orch.transition_task(&task.id, TaskState::Running, None).await.unwrap();
        orch.transition_task(&task.id, TaskState::Failed, Some("oops".into())).await.unwrap();

        // First retry succeeds
        let retried = orch.retry_task(&task.id).await.unwrap();
        assert_eq!(retried.retry_count, 1);
        assert_eq!(retried.state, TaskState::Ready);

        // Fail again
        orch.transition_task(&task.id, TaskState::Running, None).await.unwrap();
        orch.transition_task(&task.id, TaskState::Failed, Some("oops again".into())).await.unwrap();

        // Second retry should fail (exhausted)
        let err = orch.retry_task(&task.id).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_dependency_promotion() {
        let orch = test_orchestrator().await;

        let parent = Task::new("Parent", "Must complete first");
        orch.add_task(&parent).await.unwrap();

        let child = Task::new("Child", "Depends on parent")
            .with_dependencies(vec![parent.id.clone()]);
        orch.add_task(&child).await.unwrap();

        // Initialize: parent has no deps, becomes Ready
        let promoted = orch.initialize_ready_tasks().await.unwrap();
        assert_eq!(promoted, 1);

        let p = orch.get_task(&parent.id).await.unwrap().unwrap();
        assert_eq!(p.state, TaskState::Ready);

        let c = orch.get_task(&child.id).await.unwrap().unwrap();
        assert_eq!(c.state, TaskState::Pending);

        // Complete parent
        orch.transition_task(&parent.id, TaskState::Running, None).await.unwrap();
        orch.complete_task(&parent.id, serde_json::json!(null)).await.unwrap();

        // Child should now be Ready
        let c = orch.get_task(&child.id).await.unwrap().unwrap();
        assert_eq!(c.state, TaskState::Ready);
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        let orch = test_orchestrator().await;

        let a = Task::new("A", "Task A");
        let b = Task::new("B", "Task B").with_dependencies(vec![a.id.clone()]);
        let c = Task::new("C", "Task C").with_dependencies(vec![b.id.clone()]);

        orch.add_task(&a).await.unwrap();
        orch.add_task(&b).await.unwrap();
        orch.add_task(&c).await.unwrap();

        // No cycle: should pass
        orch.validate_dag().await.unwrap();
    }

    #[tokio::test]
    async fn test_agent_registration_and_scheduling() {
        let orch = test_orchestrator().await;

        let agent = SwarmAgent::new("agent-1", "Worker 1");
        orch.register_agent(&agent).await.unwrap();

        let task = Task::new("Simple task", "No deps");
        orch.add_task(&task).await.unwrap();
        orch.initialize_ready_tasks().await.unwrap();

        let result = orch.schedule().await.unwrap();
        assert_eq!(result.assignments.len(), 1);
        assert_eq!(result.assignments[0].0, task.id);
        assert_eq!(result.assignments[0].1, "agent-1");

        let updated = orch.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(updated.state, TaskState::Running);
        assert_eq!(updated.assigned_agent, Some("agent-1".into()));
    }

    #[tokio::test]
    async fn test_capability_matching() {
        let orch = test_orchestrator().await;

        let mut agent = SwarmAgent::new("rust-dev", "Rust Developer");
        agent.capabilities = vec!["rust".into(), "backend".into()];
        orch.register_agent(&agent).await.unwrap();

        let task = Task::new("Write Rust code", "Needs rust capability")
            .with_capabilities(vec!["rust".into()]);
        orch.add_task(&task).await.unwrap();
        orch.initialize_ready_tasks().await.unwrap();

        let result = orch.schedule().await.unwrap();
        assert_eq!(result.assignments.len(), 1);

        // Task requiring python should be unassignable
        let py_task = Task::new("Write Python", "Needs python")
            .with_capabilities(vec!["python".into()]);
        orch.add_task(&py_task).await.unwrap();
        orch.initialize_ready_tasks().await.unwrap();

        let result2 = orch.schedule().await.unwrap();
        assert_eq!(result2.unassignable_tasks.len(), 1);
        assert_eq!(result2.unassignable_tasks[0], py_task.id);
    }

    #[tokio::test]
    async fn test_swarm_stats() {
        let orch = test_orchestrator().await;

        orch.add_task(&Task::new("T1", "Done")).await.unwrap();
        orch.add_task(&Task::new("T2", "Pending")).await.unwrap();

        let agent = SwarmAgent::new("a1", "Agent 1");
        orch.register_agent(&agent).await.unwrap();

        let stats = orch.stats().await.unwrap();
        assert_eq!(stats.total_tasks, 2);
        assert_eq!(stats.total_agents, 1);
        assert_eq!(stats.active_agents, 1);
    }

    #[tokio::test]
    async fn test_release_agent_slot() {
        let orch = test_orchestrator().await;

        let agent = SwarmAgent::new("a1", "Agent 1");
        orch.register_agent(&agent).await.unwrap();

        let task = Task::new("Slot test", "Test release");
        orch.add_task(&task).await.unwrap();
        orch.initialize_ready_tasks().await.unwrap();
        orch.schedule().await.unwrap();

        let a = orch.get_agent("a1").await.unwrap().unwrap();
        assert_eq!(a.current_load, 1);

        orch.release_agent_slot(&task.id).await.unwrap();
        let a = orch.get_agent("a1").await.unwrap().unwrap();
        assert_eq!(a.current_load, 0);
    }
}