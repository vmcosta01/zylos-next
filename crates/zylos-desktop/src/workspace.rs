//! Per-bot workspace management.
//!
//! Each bot gets an isolated directory under `~/Zylos/Agents/{bot_id}/`
//! with separate memory, skills, browser profiles, and logs. This module
//! handles creation, listing, and path resolution for these workspaces.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Metadata about a bot workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    /// Unique bot identifier (alphanumeric, hyphens, underscores; 1-128 chars).
    pub bot_id: String,
    /// Human-readable display name for the workspace.
    pub name: String,
    /// Root directory of this workspace on disk (e.g., `~/Zylos/Agents/<bot_id>/`).
    pub path: PathBuf,
    /// When the workspace was created (Unix epoch seconds).
    pub created_at: u64,
    /// Last access time (Unix epoch seconds); updated by [`WorkspaceManager::touch`].
    pub last_accessed: u64,
}

/// Errors from workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Filesystem I/O error during workspace operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Bot ID contains invalid characters or is outside the 1-128 char range.
    #[error("Invalid bot ID: {0}")]
    InvalidBotId(String),
    /// A workspace already exists for the given bot ID.
    #[error("Workspace already exists: {0}")]
    AlreadyExists(String),
    /// No workspace found for the given bot ID.
    #[error("Workspace not found: {0}")]
    NotFound(String),
    /// JSON serialization/deserialization error for workspace metadata.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Manages per-bot workspace directories.
#[derive(Clone)]
pub struct WorkspaceManager {
    /// Root directory for all agent workspaces
    agents_root: PathBuf,
}

impl WorkspaceManager {
    /// Create a new workspace manager with the default root.
    pub fn new() -> Self {
        let root = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Zylos")
            .join("Agents");
        Self { agents_root: root }
    }

    /// Create a workspace manager with a custom root (for testing).
    pub fn with_root(root: PathBuf) -> Self {
        Self { agents_root: root }
    }

    /// Get the root directory for all agent workspaces.
    pub fn agents_root(&self) -> &Path {
        &self.agents_root
    }

    /// Validate a bot ID (alphanumeric, hyphens, underscores only).
    fn validate_bot_id(bot_id: &str) -> Result<(), WorkspaceError> {
        if bot_id.is_empty() || bot_id.len() > 128 {
            return Err(WorkspaceError::InvalidBotId(
                "must be 1-128 characters".into(),
            ));
        }
        if !bot_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(WorkspaceError::InvalidBotId(
                "only alphanumeric, hyphens, underscores allowed".into(),
            ));
        }
        Ok(())
    }

    /// Get the workspace path for a bot.
    pub fn workspace_path(&self, bot_id: &str) -> PathBuf {
        self.agents_root.join(bot_id)
    }

    /// Create a new workspace for a bot.
    pub fn create_workspace(&self, bot_id: &str, name: &str) -> Result<Workspace, WorkspaceError> {
        Self::validate_bot_id(bot_id)?;

        let path = self.workspace_path(bot_id);
        if path.exists() {
            return Err(WorkspaceError::AlreadyExists(bot_id.into()));
        }

        // Create directory structure
        std::fs::create_dir_all(path.join("memory"))?;
        std::fs::create_dir_all(path.join("skills"))?;
        std::fs::create_dir_all(path.join("browser"))?;
        std::fs::create_dir_all(path.join("logs"))?;
        std::fs::create_dir_all(path.join("config"))?;

        let now = chrono::Utc::now().timestamp() as u64;
        let ws = Workspace {
            bot_id: bot_id.into(),
            name: name.into(),
            path,
            created_at: now,
            last_accessed: now,
        };

        // Write metadata
        let meta_path = ws.path.join("workspace.json");
        let json = serde_json::to_string_pretty(&ws)?;
        std::fs::write(meta_path, json)?;

        info!(
            "Created workspace for bot '{bot_id}' at {}",
            ws.path.display()
        );
        Ok(ws)
    }

    /// List all existing workspaces.
    pub fn list_workspaces(&self) -> Result<Vec<Workspace>, WorkspaceError> {
        if !self.agents_root.exists() {
            return Ok(vec![]);
        }

        let mut workspaces = Vec::new();
        for entry in std::fs::read_dir(&self.agents_root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let meta_path = entry.path().join("workspace.json");
            if meta_path.exists() {
                match std::fs::read_to_string(&meta_path) {
                    Ok(content) => match serde_json::from_str::<Workspace>(&content) {
                        Ok(ws) => workspaces.push(ws),
                        Err(e) => {
                            warn!("Failed to parse workspace.json in {:?}: {e}", entry.path())
                        }
                    },
                    Err(e) => warn!("Failed to read workspace.json in {:?}: {e}", entry.path()),
                }
            }
        }

        workspaces.sort_by_key(|w| w.name.clone());
        Ok(workspaces)
    }

    /// Get a specific workspace by bot ID.
    pub fn get_workspace(&self, bot_id: &str) -> Result<Workspace, WorkspaceError> {
        let path = self.workspace_path(bot_id);
        let meta_path = path.join("workspace.json");

        if !meta_path.exists() {
            return Err(WorkspaceError::NotFound(bot_id.into()));
        }

        let content = std::fs::read_to_string(&meta_path)?;
        let ws: Workspace = serde_json::from_str(&content)?;
        Ok(ws)
    }

    /// Update the last accessed timestamp.
    pub fn touch(&self, bot_id: &str) -> Result<(), WorkspaceError> {
        let path = self.workspace_path(bot_id);
        let meta_path = path.join("workspace.json");

        if !meta_path.exists() {
            return Err(WorkspaceError::NotFound(bot_id.into()));
        }

        let content = std::fs::read_to_string(&meta_path)?;
        let mut ws: Workspace = serde_json::from_str(&content)?;
        ws.last_accessed = chrono::Utc::now().timestamp() as u64;

        let json = serde_json::to_string_pretty(&ws)?;
        std::fs::write(meta_path, json)?;
        Ok(())
    }

    /// Delete a workspace and all its contents.
    pub fn delete_workspace(&self, bot_id: &str) -> Result<(), WorkspaceError> {
        let path = self.workspace_path(bot_id);
        if !path.exists() {
            return Err(WorkspaceError::NotFound(bot_id.into()));
        }

        std::fs::remove_dir_all(&path)?;
        info!("Deleted workspace for bot '{bot_id}'");
        Ok(())
    }

    /// Get subdirectory paths within a workspace.
    pub fn memory_dir(&self, bot_id: &str) -> PathBuf {
        self.workspace_path(bot_id).join("memory")
    }

    pub fn skills_dir(&self, bot_id: &str) -> PathBuf {
        self.workspace_path(bot_id).join("skills")
    }

    pub fn browser_dir(&self, bot_id: &str) -> PathBuf {
        self.workspace_path(bot_id).join("browser")
    }

    pub fn logs_dir(&self, bot_id: &str) -> PathBuf {
        self.workspace_path(bot_id).join("logs")
    }

    pub fn config_dir(&self, bot_id: &str) -> PathBuf {
        self.workspace_path(bot_id).join("config")
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_manager() -> (WorkspaceManager, TempDir) {
        let tmp = TempDir::new().unwrap();
        let mgr = WorkspaceManager::with_root(tmp.path().to_path_buf());
        (mgr, tmp)
    }

    #[test]
    fn test_validate_bot_id_valid() {
        assert!(WorkspaceManager::validate_bot_id("my-bot_01").is_ok());
    }

    #[test]
    fn test_validate_bot_id_empty() {
        assert!(WorkspaceManager::validate_bot_id("").is_err());
    }

    #[test]
    fn test_validate_bot_id_special_chars() {
        assert!(WorkspaceManager::validate_bot_id("bot@home").is_err());
        assert!(WorkspaceManager::validate_bot_id("bot/home").is_err());
    }

    #[tokio::test]
    async fn test_create_and_list_workspace() {
        let (mgr, _tmp) = test_manager();

        let ws = mgr.create_workspace("test-bot", "Test Bot").unwrap();
        assert_eq!(ws.bot_id, "test-bot");
        assert_eq!(ws.name, "Test Bot");
        assert!(ws.path.exists());
        assert!(ws.path.join("memory").exists());
        assert!(ws.path.join("skills").exists());
        assert!(ws.path.join("browser").exists());

        let list = mgr.list_workspaces().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].bot_id, "test-bot");
    }

    #[tokio::test]
    async fn test_create_duplicate_workspace() {
        let (mgr, _tmp) = test_manager();
        mgr.create_workspace("dup-bot", "Dup").unwrap();
        let result = mgr.create_workspace("dup-bot", "Dup Again");
        assert!(matches!(result, Err(WorkspaceError::AlreadyExists(_))));
    }

    #[tokio::test]
    async fn test_get_workspace() {
        let (mgr, _tmp) = test_manager();
        mgr.create_workspace("get-bot", "Get Bot").unwrap();
        let ws = mgr.get_workspace("get-bot").unwrap();
        assert_eq!(ws.name, "Get Bot");
    }

    #[tokio::test]
    async fn test_delete_workspace() {
        let (mgr, _tmp) = test_manager();
        mgr.create_workspace("del-bot", "Del Bot").unwrap();
        assert!(mgr.workspace_path("del-bot").exists());
        mgr.delete_workspace("del-bot").unwrap();
        assert!(!mgr.workspace_path("del-bot").exists());
    }

    #[test]
    fn test_subdirectory_paths() {
        let (mgr, _tmp) = test_manager();
        assert_eq!(
            mgr.memory_dir("b1"),
            mgr.workspace_path("b1").join("memory")
        );
        assert_eq!(
            mgr.skills_dir("b1"),
            mgr.workspace_path("b1").join("skills")
        );
    }
}
