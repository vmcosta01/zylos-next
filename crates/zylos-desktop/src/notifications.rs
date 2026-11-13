//! OS-native notification bridge.
//!
//! Provides a cross-platform notification interface that works with Tauri's
//! notification plugin on desktop platforms. Supports action callbacks for
//! HITL (Human-in-the-Loop) approval requests.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Notification urgency level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Urgency {
    /// Informational notification (auto-dismiss)
    Info,
    /// Warning that may need attention
    Warning,
    /// Critical notification requiring user action
    Critical,
}

/// A notification to display to the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    /// Unique notification ID (UUID v4 or `hitl-<request_id>`).
    pub id: String,
    /// Title text displayed in the notification banner.
    pub title: String,
    /// Body text with additional details.
    pub body: String,
    /// Urgency level controlling display priority and auto-dismiss behavior.
    pub urgency: Urgency,
    /// Optional action button label shown to the user (e.g., `"Review & Approve"`).
    pub action_label: Option<String>,
    /// Optional action identifier used for callback routing when the user clicks the action button.
    pub action_id: Option<String>,
    /// Timestamp when the notification was created (Unix epoch seconds).
    pub created_at: u64,
}

impl Notification {
    /// Create a simple info notification.
    pub fn info(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            body: body.into(),
            urgency: Urgency::Info,
            action_label: None,
            action_id: None,
            created_at: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Create a warning notification.
    pub fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            body: body.into(),
            urgency: Urgency::Warning,
            action_label: None,
            action_id: None,
            created_at: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Create a critical notification with an action button.
    pub fn critical(
        title: impl Into<String>,
        body: impl Into<String>,
        action_label: impl Into<String>,
        action_id: impl Into<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            body: body.into(),
            urgency: Urgency::Critical,
            action_label: Some(action_label.into()),
            action_id: Some(action_id.into()),
            created_at: chrono::Utc::now().timestamp() as u64,
        }
    }

    /// Create a HITL approval request notification.
    pub fn hitl_request(request_id: &str, summary: &str, urgency_text: &str) -> Self {
        let urgency = match urgency_text.to_lowercase().as_str() {
            "critical" | "high" => Urgency::Critical,
            "warning" | "medium" => Urgency::Warning,
            _ => Urgency::Info,
        };

        Self {
            id: format!("hitl-{request_id}"),
            title: "Approval Required".into(),
            body: summary.into(),
            urgency,
            action_label: Some("Review & Approve".into()),
            action_id: Some(format!("approve:{request_id}")),
            created_at: chrono::Utc::now().timestamp() as u64,
        }
    }
}

/// Manages notification delivery and history.
#[derive(Clone)]
pub struct NotificationManager {
    /// Recent notifications (bounded ring buffer)
    history: Arc<RwLock<Vec<Notification>>>,
    /// Maximum history size
    max_history: usize,
    /// Whether notifications are enabled
    enabled: Arc<RwLock<bool>>,
    /// Optional Tauri app handle for native notifications (only with `gui` feature)
    #[cfg(feature = "gui")]
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            history: Arc::new(RwLock::new(Vec::new())),
            max_history: 100,
            enabled: Arc::new(RwLock::new(true)),
            #[cfg(feature = "gui")]
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the Tauri app handle for native OS notifications.
    #[cfg(feature = "gui")]
    pub async fn set_app_handle(&self, handle: tauri::AppHandle) {
        *self.app_handle.write().await = Some(handle);
    }

    /// Enable or disable notifications.
    pub async fn set_enabled(&self, enabled: bool) {
        *self.enabled.write().await = enabled;
    }

    /// Check if notifications are enabled.
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    /// Send a notification. Returns the notification ID if delivered.
    pub async fn send(&self, notification: Notification) -> Option<String> {
        if !self.is_enabled().await {
            warn!("Notifications disabled, dropping: {}", notification.title);
            return None;
        }

        let id = notification.id.clone();

        // Add to history
        {
            let mut history = self.history.write().await;
            if history.len() >= self.max_history {
                history.remove(0);
            }
            history.push(notification.clone());
        }

        info!(
         "Notification sent: [{}] {} (urgency={:?})",
         notification.id, notification.title, notification.urgency
        );

        // Send native OS notification via Tauri plugin (gui feature only)
        #[cfg(feature = "gui")]
        if let Some(handle) = self.app_handle.read().await.as_ref() {
            use tauri_plugin_notification::NotificationExt;
            let body_text = if let Some(ref label) = notification.action_label {
                format!("{}\n\n[{}]", notification.body, label)
            } else {
                notification.body.clone()
            };

            if let Err(e) = handle.notification()
                .builder()
                .title(&notification.title)
                .body(&body_text)
                .show()
            {
                warn!("Failed to send native notification: {e}");
            }
        }

        Some(id)
    }

    /// Get notification history (most recent first).
    pub async fn get_history(&self, limit: usize) -> Vec<Notification> {
        let history = self.history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Clear notification history.
    pub async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    /// Get unread count (all notifications since last clear).
    pub async fn count(&self) -> usize {
        self.history.read().await.len()
    }

    /// Remove a specific notification from history.
    pub async fn dismiss(&self, notification_id: &str) -> bool {
        let mut history = self.history.write().await;
        let before = history.len();
        history.retain(|n| n.id != notification_id);
        history.len() < before
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_notification() {
        let n = Notification::info("Test", "Body text");
        assert_eq!(n.title, "Test");
        assert_eq!(n.body, "Body text");
        assert_eq!(n.urgency, Urgency::Info);
        assert!(n.action_label.is_none());
    }

    #[test]
    fn test_critical_notification() {
        let n = Notification::critical("Alert", "Something broke", "Fix Now", "fix-123");
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.action_label.as_deref(), Some("Fix Now"));
        assert_eq!(n.action_id.as_deref(), Some("fix-123"));
    }

    #[test]
    fn test_hitl_request() {
        let n = Notification::hitl_request("req-42", "Deploy to prod?", "critical");
        assert!(n.id.starts_with("hitl-"));
        assert_eq!(n.urgency, Urgency::Critical);
        assert_eq!(n.action_label.as_deref(), Some("Review & Approve"));
        assert_eq!(n.action_id.as_deref(), Some("approve:req-42"));
    }

    #[tokio::test]
    async fn test_send_and_history() {
        let mgr = NotificationManager::new();
        let n = Notification::info("Hello", "World");
        let id = mgr.send(n).await;
        assert!(id.is_some());

        let history = mgr.get_history(10).await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "Hello");
    }

    #[tokio::test]
    async fn test_disabled_notifications() {
        let mgr = NotificationManager::new();
        mgr.set_enabled(false).await;
        let n = Notification::info("Dropped", "Should not appear");
        let id = mgr.send(n).await;
        assert!(id.is_none());
        assert_eq!(mgr.count().await, 0);
    }

    #[tokio::test]
    async fn test_dismiss_notification() {
        let mgr = NotificationManager::new();
        let n = Notification::info("Dismiss Me", "Please");
        let id = mgr.send(n).await.unwrap();
        assert_eq!(mgr.count().await, 1);
        assert!(mgr.dismiss(&id).await);
        assert_eq!(mgr.count().await, 0);
    }

    #[tokio::test]
    async fn test_clear_history() {
        let mgr = NotificationManager::new();
        for i in 0..5 {
            mgr.send(Notification::info(format!("N{i}"), "body")).await;
        }
        assert_eq!(mgr.count().await, 5);
        mgr.clear_history().await;
        assert_eq!(mgr.count().await, 0);
    }

    #[tokio::test]
    async fn test_history_bounded() {
        let mgr = NotificationManager::new();
        for i in 0..150 {
            mgr.send(Notification::info(format!("N{i}"), "body")).await;
        }
        // max_history is 100
        assert_eq!(mgr.count().await, 100);
    }
}
