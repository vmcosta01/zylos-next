//! Presence tracking for collaborative editing sessions.
//!
//! Tracks which users are currently viewing/editing a document,
//! their cursor positions, and last-seen timestamps for stale detection.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Unique identifier for a connected client session.
pub type ClientId = u64;

/// A presence event emitted when a user joins, leaves, or updates their state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum PresenceEvent {
    /// A new user joined the document session.
    Joined {
        client_id: ClientId,
        user_id: String,
        name: String,
        color: String,
    },
    /// A user left the document session.
    Left {
        client_id: ClientId,
        user_id: String,
    },
    /// A user updated their cursor/selection state.
    CursorUpdate {
        client_id: ClientId,
        user_id: String,
        /// Cursor position as byte offset or line:col.
        position: String,
        /// Optional selected range.
        selection: Option<String>,
    },
}

/// Per-client presence state.
#[derive(Debug, Clone)]
struct ClientPresence {
    user_id: String,
    name: String,
    color: String,
    position: Option<String>,
    last_seen_ms: u64,
}

/// Tracks presence state for all connected clients across documents.
#[derive(Clone)]
pub struct PresenceTracker {
    /// Map from document_id -> (client_id -> ClientPresence)
    documents: DashMap<String, DashMap<ClientId, ClientPresence>>,
}

impl PresenceTracker {
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
        }
    }

    /// Register a new client joining a document session.
    pub fn join(
        &self,
        doc_id: &str,
        client_id: ClientId,
        user_id: String,
        name: String,
        color: String,
    ) -> PresenceEvent {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let entry = ClientPresence {
            user_id: user_id.clone(),
            name: name.clone(),
            color: color.clone(),
            position: None,
            last_seen_ms: now_ms,
        };

        self.documents
            .entry(doc_id.to_string())
            .or_default()
            .insert(client_id, entry);

        tracing::debug!(doc_id, client_id, user_id, "presence: joined");
        PresenceEvent::Joined {
            client_id,
            user_id,
            name,
            color,
        }
    }

    /// Remove a client from a document session.
    pub fn leave(&self, doc_id: &str, client_id: ClientId) -> Option<PresenceEvent> {
        if let Some(doc_map) = self.documents.get(doc_id) {
            if let Some((_, entry)) = doc_map.remove(&client_id) {
                tracing::debug!(doc_id, client_id, user_id = %entry.user_id, "presence: left");
                return Some(PresenceEvent::Left {
                    client_id,
                    user_id: entry.user_id,
                });
            }
        }
        None
    }

    /// Update cursor/selection state for a client.
    pub fn update_cursor(
        &self,
        doc_id: &str,
        client_id: ClientId,
        position: String,
        selection: Option<String>,
    ) -> Option<PresenceEvent> {
        if let Some(doc_map) = self.documents.get(doc_id) {
            if let Some(mut entry) = doc_map.get_mut(&client_id) {
                entry.position = Some(position.clone());
                entry.last_seen_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                return Some(PresenceEvent::CursorUpdate {
                    client_id,
                    user_id: entry.user_id.clone(),
                    position,
                    selection,
                });
            }
        }
        None
    }

    /// Get all active users in a document session.
    pub fn active_users(&self, doc_id: &str) -> Vec<(ClientId, String, String, String)> {
        if let Some(doc_map) = self.documents.get(doc_id) {
            doc_map
                .iter()
                .map(|e| (*e.key(), e.user_id.clone(), e.name.clone(), e.color.clone()))
                .collect()
        } else {
            vec![]
        }
    }

    /// Remove stale clients not seen within the given timeout (ms).
    pub fn prune_stale(&self, timeout_ms: u64) -> Vec<(String, ClientId)> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut pruned = vec![];
        for doc_entry in self.documents.iter() {
            let doc_id = doc_entry.key().clone();
            let stale_ids: Vec<ClientId> = doc_entry
                .value()
                .iter()
                .filter(|e| now_ms.saturating_sub(e.last_seen_ms) > timeout_ms)
                .map(|e| *e.key())
                .collect();

            for cid in stale_ids {
                doc_entry.value().remove(&cid);
                pruned.push((doc_id.clone(), cid));
            }
        }
        pruned
    }
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_and_active_users() {
        let tracker = PresenceTracker::new();
        let event = tracker.join(
            "doc-1",
            100,
            "user-a".into(),
            "Alice".into(),
            "#ff0000".into(),
        );
        assert!(matches!(event, PresenceEvent::Joined { .. }));

        let users = tracker.active_users("doc-1");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].1, "user-a");
    }

    #[test]
    fn test_leave_removes_user() {
        let tracker = PresenceTracker::new();
        tracker.join(
            "doc-1",
            200,
            "user-b".into(),
            "Bob".into(),
            "#00ff00".into(),
        );
        let event = tracker.leave("doc-1", 200);
        assert!(event.is_some());
        assert!(tracker.active_users("doc-1").is_empty());
    }

    #[test]
    fn test_cursor_update() {
        let tracker = PresenceTracker::new();
        tracker.join(
            "doc-1",
            300,
            "user-c".into(),
            "Carol".into(),
            "#0000ff".into(),
        );
        let event = tracker.update_cursor("doc-1", 300, "line:5,col:10".into(), None);
        assert!(matches!(event, Some(PresenceEvent::CursorUpdate { .. })));
    }
}
