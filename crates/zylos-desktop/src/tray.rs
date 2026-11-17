//! System tray management for the Zylos desktop shell.
//!
//! Provides a persistent system tray icon with status indication and
//! quick action menu items. Works cross-platform via Tauri's tray API.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Current gateway health status shown in tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayStatus {
    /// Gateway is running and healthy
    Healthy,
    /// Gateway is starting or degraded
    Warning,
    /// Gateway is stopped or unreachable
    Error,
    /// Status unknown (initial state)
    Unknown,
}

impl TrayStatus {
    /// Get the tooltip text for this status.
    pub fn tooltip(&self) -> &'static str {
        match self {
            Self::Healthy => "Zylos Gateway — Running",
            Self::Warning => "Zylos Gateway — Starting...",
            Self::Error => "Zylos Gateway — Stopped",
            Self::Unknown => "Zylos Gateway — Connecting...",
        }
    }

    /// Get a color hint for the status indicator.
    pub fn color_hint(&self) -> &'static str {
        match self {
            Self::Healthy => "#22c55e", // green
            Self::Warning => "#eab308", // yellow
            Self::Error => "#ef4444",   // red
            Self::Unknown => "#6b7280", // gray
        }
    }
}

/// Menu item definition for the tray context menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayMenuItem {
    /// Unique identifier for this menu item (e.g., `"show"`, `"quit"`).
    pub id: String,
    /// Human-readable label displayed in the tray context menu.
    pub label: String,
    /// Whether this menu item is clickable (grayed out when `false`).
    pub enabled: bool,
    /// Whether to draw a separator line above this item in the menu.
    pub separator_before: bool,
}

/// Default tray menu structure.
pub fn default_menu() -> Vec<TrayMenuItem> {
    vec![
        TrayMenuItem {
            id: "show".into(),
            label: "Show Window".into(),
            enabled: true,
            separator_before: false,
        },
        TrayMenuItem {
            id: "dashboard".into(),
            label: "Open Dashboard".into(),
            enabled: true,
            separator_before: false,
        },
        TrayMenuItem {
            id: "new_chat".into(),
            label: "New Chat".into(),
            enabled: true,
            separator_before: false,
        },
        TrayMenuItem {
            id: "restart".into(),
            label: "Restart Gateway".into(),
            enabled: true,
            separator_before: true,
        },
        TrayMenuItem {
            id: "check_updates".into(),
            label: "Check for Updates".into(),
            enabled: true,
            separator_before: false,
        },
        TrayMenuItem {
            id: "quit".into(),
            label: "Quit".into(),
            enabled: true,
            separator_before: true,
        },
    ]
}

/// Thread-safe tray state manager.
#[derive(Clone)]
pub struct TrayManager {
    status: Arc<RwLock<TrayStatus>>,
    visible: Arc<RwLock<bool>>,
}

impl TrayManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(TrayStatus::Unknown)),
            visible: Arc::new(RwLock::new(true)),
        }
    }

    /// Update the tray status indicator.
    pub async fn set_status(&self, status: TrayStatus) {
        let mut s = self.status.write().await;
        if *s != status {
            info!("Tray status changed: {:?} → {:?}", *s, status);
            *s = status;
        }
    }

    /// Get current tray status.
    pub async fn get_status(&self) -> TrayStatus {
        *self.status.read().await
    }

    /// Show or hide the main window.
    pub async fn set_visible(&self, visible: bool) {
        *self.visible.write().await = visible;
    }

    /// Check if window should be visible.
    pub async fn is_visible(&self) -> bool {
        *self.visible.read().await
    }

    /// Toggle window visibility.
    pub async fn toggle_visible(&self) -> bool {
        let mut v = self.visible.write().await;
        *v = !*v;
        *v
    }

    /// Handle a tray menu action. Returns true if the app should quit.
    pub async fn handle_action(&self, action_id: &str) -> bool {
        match action_id {
            "show" => {
                self.set_visible(true).await;
                info!("Tray: show window");
                false
            }
            "dashboard" => {
                info!("Tray: open dashboard");
                false
            }
            "new_chat" => {
                info!("Tray: new chat");
                false
            }
            "restart" => {
                info!("Tray: restart gateway requested");
                false
            }
            "check_updates" => {
                info!("Tray: check updates");
                false
            }
            "quit" => {
                info!("Tray: quit requested");
                true
            }
            other => {
                warn!("Unknown tray action: {other}");
                false
            }
        }
    }
}

impl Default for TrayManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_status_tooltip() {
        assert_eq!(TrayStatus::Healthy.tooltip(), "Zylos Gateway — Running");
        assert_eq!(TrayStatus::Error.tooltip(), "Zylos Gateway — Stopped");
    }

    #[test]
    fn test_default_menu_has_quit() {
        let menu = default_menu();
        assert!(menu.iter().any(|item| item.id == "quit"));
    }

    #[tokio::test]
    async fn test_tray_manager_status_update() {
        let mgr = TrayManager::new();
        assert_eq!(mgr.get_status().await, TrayStatus::Unknown);
        mgr.set_status(TrayStatus::Healthy).await;
        assert_eq!(mgr.get_status().await, TrayStatus::Healthy);
    }

    #[tokio::test]
    async fn test_tray_manager_toggle() {
        let mgr = TrayManager::new();
        assert!(mgr.is_visible().await);
        let v = mgr.toggle_visible().await;
        assert!(!v);
        assert!(!mgr.is_visible().await);
    }

    #[tokio::test]
    async fn test_handle_quit_action() {
        let mgr = TrayManager::new();
        assert!(mgr.handle_action("quit").await);
    }

    #[tokio::test]
    async fn test_handle_show_action() {
        let mgr = TrayManager::new();
        mgr.set_visible(false).await;
        assert!(!mgr.handle_action("show").await);
        assert!(mgr.is_visible().await);
    }
}
