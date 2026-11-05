//! Kanban Bridge — Bidirectional sync between DAG execution and Hermes Kanban.
//!
//! Maps [`DagNode`] to Kanban task cards and propagates status changes in both
//! directions:
//! - **DAG → Kanban**: node status transitions (Pending/Running/Success/Failed)
//!   update the corresponding Kanban card column.
//! - **Kanban → DAG**: external card moves (e.g. human re-prioritises or blocks)
//!   are polled and reflected back into the DAG executor's scheduling.
//!
//! This module is transport-agnostic: it communicates with the Kanban backend
//! through the [`KanbanBackend`] trait, allowing in-process, HTTP, or SQLite
//! implementations.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::dag::{DagId, DagNode, NodeStatus};
use crate::error::{BotError, Result};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Opaque Kanban task identifier (matches Hermes kanban task id format).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Kanban column / status that mirrors [`NodeStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskColumn {
    Todo,
    Ready,
    Running,
    Review,
    Done,
    Blocked,
}

impl From<NodeStatus> for TaskColumn {
    fn from(s: NodeStatus) -> Self {
        match s {
            NodeStatus::Pending => TaskColumn::Todo,
            NodeStatus::Running => TaskColumn::Running,
            NodeStatus::Success => TaskColumn::Done,
            NodeStatus::Failed => TaskColumn::Blocked,
            NodeStatus::Skipped => TaskColumn::Done,
        }
    }
}

/// Mapping between a DAG node and its Kanban card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTaskMapping {
    pub dag_id: DagId,
    pub node_id: String,
    pub task_id: TaskId,
    pub last_synced_status: NodeStatus,
}

/// Summary of a sync cycle.
#[derive(Debug, Clone, Default)]
pub struct SyncReport {
    pub dag_to_kanban_updates: usize,
    pub kanban_to_dag_updates: usize,
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Backend trait
// ---------------------------------------------------------------------------

/// Abstraction over the Kanban storage / API layer.
///
/// Implementations may talk to the Hermes kanban kernel (in-process), an HTTP
/// gateway, or a local SQLite mirror. The bridge never assumes a specific
/// transport.
#[async_trait::async_trait]
pub trait KanbanBackend: Send + Sync {
    /// Create or update a task card. Returns the task id.
    async fn upsert_task(
        &self,
        task_id: Option<&TaskId>,
        title: &str,
        column: TaskColumn,
        metadata: &serde_json::Value,
    ) -> Result<TaskId>;

    /// Read the current column of a task.
    async fn get_task_column(&self, task_id: &TaskId) -> Result<TaskColumn>;

    /// Move a task to a new column.
    async fn move_task(&self, task_id: &TaskId, column: TaskColumn) -> Result<()>;

    /// List all tasks that belong to a given DAG (by metadata.dag_id).
    async fn list_tasks_for_dag(&self, dag_id: &DagId) -> Result<Vec<(TaskId, TaskColumn)>>;
}

// ---------------------------------------------------------------------------
// Bridge
// ---------------------------------------------------------------------------

/// Bidirectional sync engine between DAG execution and Kanban.
pub struct KanbanBridge {
    backend: Arc<dyn KanbanBackend>,
    mappings: Arc<RwLock<HashMap<(DagId, String), NodeTaskMapping>>>,
    cancel: CancellationToken,
    poll_interval: Duration,
}

impl KanbanBridge {
    /// Create a new bridge with the given backend and polling interval for
    /// Kanban → DAG sync.
    pub fn new(backend: Arc<dyn KanbanBackend>, poll_interval: Duration) -> Self {
        Self {
            backend,
            mappings: Arc::new(RwLock::new(HashMap::new())),
            cancel: CancellationToken::new(),
            poll_interval,
        }
    }

    /// Register a DAG node ↔ Kanban task mapping. If no task exists yet, one
    /// is created via the backend.
    pub async fn register_node(&self, dag_id: DagId, node: &DagNode) -> Result<TaskId> {
        let key = (dag_id, node.id.clone());
        let existing = {
            let map = self.mappings.read().await;
            map.get(&key).map(|m| m.task_id.clone())
        };

        let metadata = serde_json::json!({
            "dag_id": dag_id.to_string(),
            "node_id": node.id,
            "agent_name": node.agent_name,
        });

        let task_id = self
            .backend
            .upsert_task(existing.as_ref(), &node.id, TaskColumn::Todo, &metadata)
            .await?;

        let mapping = NodeTaskMapping {
            dag_id,
            node_id: node.id.clone(),
            task_id: task_id.clone(),
            last_synced_status: NodeStatus::Pending,
        };

        {
            let mut map = self.mappings.write().await;
            map.insert(key, mapping);
        }

        info!(dag_id = %dag_id, node = %node.id, task = %task_id, "Registered node↔task mapping");
        Ok(task_id)
    }

    /// Push a DAG node status change to Kanban.
    pub async fn push_status(&self, dag_id: DagId, node_id: &str, status: NodeStatus) -> Result<()> {
        let key = (dag_id, node_id.to_string());
        let mapping = {
            let map = self.mappings.read().await;
            map.get(&key).cloned()
        };

        let Some(mapping) = mapping else {
            warn!(dag_id = %dag_id, node = %node_id, "No mapping found for push_status");
            return Err(BotError::Internal(format!(
                "no kanban mapping for node {node_id} in dag {dag_id}"
            )));
        };

        if mapping.last_synced_status == status {
            return Ok(()); // no-op
        }

        let column = TaskColumn::from(status);
        self.backend.move_task(&mapping.task_id, column).await?;

        // Update cached status.
        {
            let mut map = self.mappings.write().await;
            if let Some(m) = map.get_mut(&key) {
                m.last_synced_status = status;
            }
        }

        debug!(
            dag_id = %dag_id,
            node = %node_id,
            task = %mapping.task_id,
            ?status,
            "Pushed status to Kanban"
        );
        Ok(())
    }

    /// Poll Kanban for external status changes and return any that differ
    /// from the cached state. The caller (typically the DAG executor) applies
    /// these back to the DAG.
    pub async fn poll_external_changes(&self) -> Result<Vec<(DagId, String, TaskColumn)>> {
        let mut changes = Vec::new();
        let map = self.mappings.read().await;

        for ((dag_id, node_id), mapping) in map.iter() {
            match self.backend.get_task_column(&mapping.task_id).await {
                Ok(col) => {
                    let expected: TaskColumn = mapping.last_synced_status.into();
                    if col != expected {
                        changes.push((*dag_id, node_id.clone(), col));
                    }
                }
                Err(e) => {
                    warn!(task = %mapping.task_id, error = %e, "Failed to poll task column");
                }
            }
        }

        Ok(changes)
    }

    /// Start a background loop that polls Kanban → DAG changes at the
    /// configured interval. Returns a cancellation token to stop it.
    pub fn start_poll_loop(self: &Arc<Self>) -> CancellationToken {
        let token = self.cancel.clone();
        let poll_token = token.clone();
        let this = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = poll_token.cancelled() => break,
                    _ = tokio::time::sleep(this.poll_interval) => {
                        match this.poll_external_changes().await {
                            Ok(changes) if !changes.is_empty() => {
                                info!(count = changes.len(), "Detected external Kanban changes");
                                // In a full implementation, these would be fed back
                                // into the DagExecutor via a channel.
                            }
                            Ok(_) => {}
                            Err(e) => error!(error = %e, "Poll loop error"),
                        }
                    }
                }
            }
        });
        token
    }

    /// Stop the background poll loop.
    pub fn stop(&self) {
        self.cancel.cancel();
    }

    /// Return a snapshot of all current mappings.
    pub async fn mappings_snapshot(&self) -> Vec<NodeTaskMapping> {
        self.mappings.read().await.values().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::DagNode;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use uuid::Uuid;

    /// In-memory test backend.
    struct MockBackend {
        tasks: Mutex<HashMap<String, TaskColumn>>,
        upsert_count: AtomicUsize,
    }

    impl MockBackend {
        fn new() -> Self {
            Self {
                tasks: Mutex::new(HashMap::new()),
                upsert_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl KanbanBackend for MockBackend {
        async fn upsert_task(
            &self,
            task_id: Option<&TaskId>,
            _title: &str,
            column: TaskColumn,
            _metadata: &serde_json::Value,
        ) -> Result<TaskId> {
            self.upsert_count.fetch_add(1, Ordering::Relaxed);
            let id = task_id
                .cloned()
                .unwrap_or_else(|| TaskId(Uuid::new_v4().to_string()));
            self.tasks.lock().unwrap().insert(id.0.clone(), column);
            Ok(id)
        }

        async fn get_task_column(&self, task_id: &TaskId) -> Result<TaskColumn> {
            self.tasks
                .lock()
                .unwrap()
                .get(&task_id.0)
                .copied()
                .ok_or_else(|| BotError::Internal("task not found".into()))
        }

        async fn move_task(&self, task_id: &TaskId, column: TaskColumn) -> Result<()> {
            self.tasks.lock().unwrap().insert(task_id.0.clone(), column);
            Ok(())
        }

        async fn list_tasks_for_dag(&self, _dag_id: &DagId) -> Result<Vec<(TaskId, TaskColumn)>> {
            Ok(self
                .tasks
                .lock()
                .unwrap()
                .iter()
                .map(|(k, v)| (TaskId(k.clone()), *v))
                .collect())
        }
    }

    fn make_node(id: &str) -> DagNode {
        DagNode {
            id: id.to_string(),
            agent_name: "test-agent".to_string(),
            task_spec: serde_json::json!({}),
            timeout_ms: 5000,
            max_retries: 1,
            backoff_ms: 100,
        }
    }

    #[tokio::test]
    async fn register_creates_task() {
        let backend = Arc::new(MockBackend::new());
        let bridge = KanbanBridge::new(backend.clone(), Duration::from_secs(60));
        let dag_id = DagId::new();
        let node = make_node("n1");

        let task_id = bridge.register_node(dag_id, &node).await.unwrap();
        assert!(!task_id.0.is_empty());
        assert_eq!(backend.upsert_count.load(Ordering::Relaxed), 1);

        let col = backend.get_task_column(&task_id).await.unwrap();
        assert_eq!(col, TaskColumn::Todo);
    }

    #[tokio::test]
    async fn push_status_updates_column() {
        let backend = Arc::new(MockBackend::new());
        let bridge = KanbanBridge::new(backend.clone(), Duration::from_secs(60));
        let dag_id = DagId::new();
        let node = make_node("n1");

        let task_id = bridge.register_node(dag_id, &node).await.unwrap();
        bridge
            .push_status(dag_id, "n1", NodeStatus::Running)
            .await
            .unwrap();

        let col = backend.get_task_column(&task_id).await.unwrap();
        assert_eq!(col, TaskColumn::Running);
    }

    #[tokio::test]
    async fn push_status_noop_when_unchanged() {
        let backend = Arc::new(MockBackend::new());
        let bridge = KanbanBridge::new(backend.clone(), Duration::from_secs(60));
        let dag_id = DagId::new();
        let node = make_node("n1");

        bridge.register_node(dag_id, &node).await.unwrap();
        // First push: Todo → Running
        bridge
            .push_status(dag_id, "n1", NodeStatus::Running)
            .await
            .unwrap();
        // Second push: same status, should be no-op (no error)
        bridge
            .push_status(dag_id, "n1", NodeStatus::Running)
            .await
            .unwrap();

        let col = backend.get_task_column(&TaskId(
            bridge
                .mappings_snapshot()
                .await
                .first()
                .unwrap()
                .task_id
                .0
                .clone(),
        ))
        .await
        .unwrap();
        assert_eq!(col, TaskColumn::Running);
    }

    #[tokio::test]
    async fn poll_detects_external_change() {
        let backend = Arc::new(MockBackend::new());
        let bridge = KanbanBridge::new(backend.clone(), Duration::from_secs(60));
        let dag_id = DagId::new();
        let node = make_node("n1");

        let task_id = bridge.register_node(dag_id, &node).await.unwrap();

        // Simulate external move to Blocked.
        backend
            .move_task(&task_id, TaskColumn::Blocked)
            .await
            .unwrap();

        let changes = bridge.poll_external_changes().await.unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].2, TaskColumn::Blocked);
    }

    #[tokio::test]
    async fn node_status_to_column_mapping() {
        assert_eq!(TaskColumn::from(NodeStatus::Pending), TaskColumn::Todo);
        assert_eq!(TaskColumn::from(NodeStatus::Running), TaskColumn::Running);
        assert_eq!(TaskColumn::from(NodeStatus::Success), TaskColumn::Done);
        assert_eq!(TaskColumn::from(NodeStatus::Failed), TaskColumn::Blocked);
        assert_eq!(TaskColumn::from(NodeStatus::Skipped), TaskColumn::Done);
    }

    #[tokio::test]
    async fn push_without_mapping_returns_error() {
        let backend = Arc::new(MockBackend::new());
        let bridge = KanbanBridge::new(backend, Duration::from_secs(60));
        let dag_id = DagId::new();

        let result = bridge.push_status(dag_id, "nonexistent", NodeStatus::Running).await;
        assert!(result.is_err());
    }
}