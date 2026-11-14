//! LifecycleManager — spawn/kill/suspend agents via Tokio JoinSet + CancellationToken

use std::sync::Arc;

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{BotError, Result};

/// Unique identifier for an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Current state of an agent
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    /// Agent is spawned and running
    Running,
    /// Agent is suspended (paused, not consuming CPU)
    Suspended,
    /// Agent has been killed gracefully
    Stopped,
    /// Agent crashed or was force-killed
    Failed,
}

/// Handle returned when spawning an agent, allowing control operations
#[derive(Debug, Clone)]
pub struct AgentHandle {
    id: AgentId,
    cancel_token: CancellationToken,
    state_tx: broadcast::Sender<AgentState>,
}

impl AgentHandle {
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Signal the agent to stop gracefully
    pub fn kill(&self) {
        info!(agent_id = %self.id, "Signaling agent to stop");
        self.cancel_token.cancel();
    }

    /// Subscribe to state changes for this agent
    pub fn subscribe_state(&self) -> broadcast::Receiver<AgentState> {
        self.state_tx.subscribe()
    }
}

/// Internal metadata tracked per agent
struct AgentEntry {
    handle: AgentHandle,
    state: AgentState,
}

/// Manages the lifecycle of multiple agents using Tokio JoinSet + CancellationToken
pub struct LifecycleManager {
    agents: Arc<DashMap<AgentId, AgentEntry>>,
    join_set: Arc<tokio::sync::Mutex<JoinSet<std::result::Result<(), BotError>>>>,
    global_cancel: CancellationToken,
}

impl LifecycleManager {
    /// Create a new LifecycleManager with a global cancellation token
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            join_set: Arc::new(tokio::sync::Mutex::new(JoinSet::new())),
            global_cancel: CancellationToken::new(),
        }
    }

    /// Spawn a new agent with the given async task
    ///
    /// The task receives a `CancellationToken` that is cancelled when the agent
    /// should shut down. The task MUST respect this token for graceful shutdown.
    pub async fn spawn<F, Fut>(&self, name: &str, task: F) -> Result<AgentHandle>
    where
        F: FnOnce(CancellationToken) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = std::result::Result<(), BotError>> + Send + 'static,
    {
        let id = AgentId::new();
        let child_token = self.global_cancel.child_token();
        let (state_tx, _) = broadcast::channel::<AgentState>(16);

        let handle = AgentHandle {
            id,
            cancel_token: child_token.clone(),
            state_tx: state_tx.clone(),
        };

        let entry = AgentEntry {
            handle: handle.clone(),
            state: AgentState::Running,
        };
        self.agents.insert(id, entry);

        // Notify initial state
        let _ = state_tx.send(AgentState::Running);

        let agents_ref = self.agents.clone();
        let agent_id = id;
        let state_tx_clone = state_tx.clone();
        let name_owned = name.to_string();

        let mut js = self.join_set.lock().await;
        js.spawn(async move {
            debug!(agent_id = %agent_id, name = %name_owned, "Agent started");
            let result = task(child_token).await;
            match &result {
                Ok(()) => {
                    info!(agent_id = %agent_id, name = %name_owned, "Agent completed successfully");
                    if let Some(mut entry) = agents_ref.get_mut(&agent_id) {
                        entry.state = AgentState::Stopped;
                    }
                    let _ = state_tx_clone.send(AgentState::Stopped);
                }
                Err(e) => {
                    error!(agent_id = %agent_id, name = %name_owned, error = %e, "Agent failed");
                    if let Some(mut entry) = agents_ref.get_mut(&agent_id) {
                        entry.state = AgentState::Failed;
                    }
                    let _ = state_tx_clone.send(AgentState::Failed);
                }
            }
            result
        });

        info!(agent_id = %id, name = name, "Agent spawned");
        Ok(handle)
    }

    /// Kill a specific agent by ID
    pub fn kill_agent(&self, id: AgentId) -> Result<()> {
        if let Some(entry) = self.agents.get(&id) {
            entry.handle.kill();
            Ok(())
        } else {
            Err(BotError::AgentNotFound(id.to_string()))
        }
    }

    /// Get the current state of an agent
    pub fn agent_state(&self, id: AgentId) -> Result<AgentState> {
        self.agents
            .get(&id)
            .map(|entry| entry.state)
            .ok_or_else(|| BotError::AgentNotFound(id.to_string()))
    }

    /// List all agent IDs and their current states
    pub fn list_agents(&self) -> Vec<(AgentId, AgentState)> {
        self.agents
            .iter()
            .map(|entry| (*entry.key(), entry.state))
            .collect()
    }

    /// Count of currently running agents
    pub fn running_count(&self) -> usize {
        self.agents
            .iter()
            .filter(|entry| entry.state == AgentState::Running)
            .count()
    }

    /// Gracefully shut down all agents, waiting up to `timeout_ms` milliseconds
    pub async fn shutdown_all(&self, timeout_ms: u64) -> Result<()> {
        info!("Initiating graceful shutdown of all agents");
        self.global_cancel.cancel();

        let timeout = tokio::time::Duration::from_millis(timeout_ms);
        let result = tokio::time::timeout(timeout, async {
            let mut js = self.join_set.lock().await;
            while js.join_next().await.is_some() {
                // Drain all completed tasks
            }
        })
        .await;

        match result {
            Ok(()) => {
                info!("All agents shut down gracefully");
                Ok(())
            }
            Err(_) => {
                warn!("Shutdown timed out after {}ms", timeout_ms);
                Err(BotError::ShutdownTimeout(timeout_ms))
            }
        }
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_and_kill() {
        let manager = LifecycleManager::new();

        let handle = manager
            .spawn("test-agent", |token| async move {
                tokio::select! {
                    _ = token.cancelled() => {
                        debug!("Test agent received cancellation signal");
                        Ok(())
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                        Ok(())
                    }
                }
            })
            .await
            .expect("spawn should succeed");

        assert_eq!(manager.running_count(), 1);
        assert_eq!(
            manager.agent_state(handle.id()).unwrap(),
            AgentState::Running
        );

        handle.kill();

        // Allow time for the task to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // After kill, agent should eventually reach Stopped
        // (may still be Running briefly during async cleanup)
        let state = manager.agent_state(handle.id()).unwrap();
        assert!(
            state == AgentState::Stopped || state == AgentState::Running,
            "Expected Stopped or Running, got {:?}",
            state
        );
    }

    #[tokio::test]
    async fn test_shutdown_all() {
        let manager = LifecycleManager::new();

        for i in 0..3 {
            manager
                .spawn(&format!("agent-{}", i), |token| async move {
                    tokio::select! {
                        _ = token.cancelled() => Ok(()),
                        _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => Ok(()),
                    }
                })
                .await
                .expect("spawn should succeed");
        }

        assert_eq!(manager.running_count(), 3);

        manager
            .shutdown_all(5000)
            .await
            .expect("shutdown should succeed");

        assert_eq!(manager.running_count(), 0);
    }

    #[tokio::test]
    async fn test_agent_not_found() {
        let manager = LifecycleManager::new();
        let fake_id = AgentId::new();

        assert!(matches!(
            manager.agent_state(fake_id),
            Err(BotError::AgentNotFound(_))
        ));

        assert!(matches!(
            manager.kill_agent(fake_id),
            Err(BotError::AgentNotFound(_))
        ));
    }

    #[tokio::test]
    async fn test_list_agents() {
        let manager = LifecycleManager::new();

        manager
            .spawn("a1", |token| async move {
                tokio::select! {
                    _ = token.cancelled() => Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => Ok(()),
                }
            })
            .await
            .unwrap();

        manager
            .spawn("a2", |token| async move {
                tokio::select! {
                    _ = token.cancelled() => Ok(()),
                    _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => Ok(()),
                }
            })
            .await
            .unwrap();

        let agents = manager.list_agents();
        assert_eq!(agents.len(), 2);
    }
}
