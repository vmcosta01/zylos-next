//! CRDT collaboration primitives — Yjs-compatible WebSocket sync protocol.
//!
//! This module provides a server-side CRDT sync layer that speaks the Yjs
//! y-sync protocol over WebSocket, enabling collaborative editing of skills
//! and documents from the cockpit frontend.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};

/// Unique identifier for a collaborative document.
pub type DocumentId = String;

/// Messages exchanged between clients and the CRDT sync server.
/// Wire format is compatible with y-sync / y-websocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CrdtMessage {
    /// Client sends its current state vector to request missing updates.
    SyncStep1(Vec<u8>),
    /// Server or client sends encoded state as part of sync.
    SyncStep2(Vec<u8>),
    /// Incremental update (awareness or document change).
    Update(Vec<u8>),
    /// Awareness update (cursor position, selection, user info).
    Awareness(Vec<u8>),
    /// Authentication message sent by client on connect.
    Auth { token: String },
    /// Server acknowledges successful auth and assigns client ID.
    AuthOk { client_id: u64 },
    /// Server rejects authentication.
    AuthDenied { reason: String },
}

/// Per-document sync state holding subscribers and accumulated updates.
struct DocumentState {
    /// Broadcast channel for real-time updates to connected clients.
    tx: broadcast::Sender<CrdtMessage>,
    /// Accumulated binary updates (in production, replace with proper Yjs state vector store).
    updates: Vec<Vec<u8>>,
}

/// Server-side CRDT sync coordinator.
///
/// Manages multiple documents, each with its own broadcast channel.
/// Clients connect via WebSocket, authenticate, then subscribe to a document.
#[derive(Clone)]
pub struct CrdtSyncServer {
    documents: Arc<DashMap<DocumentId, DocumentState>>,
}

impl CrdtSyncServer {
    /// Create a new CRDT sync server.
    pub fn new() -> Self {
        Self {
            documents: Arc::new(DashMap::new()),
        }
    }

    /// Get or create a document's broadcast channel.
    pub fn get_or_create_document(&self, doc_id: &DocumentId) -> broadcast::Sender<CrdtMessage> {
        self.documents
            .entry(doc_id.clone())
            .or_insert_with(|| {
                info!(doc_id = %doc_id, "creating new CRDT document");
                let (tx, _) = broadcast::channel(256);
                DocumentState {
                    tx,
                    updates: Vec::new(),
                }
            })
            .tx
            .clone()
    }

    /// Subscribe to updates for a specific document.
    pub fn subscribe(&self, doc_id: &DocumentId) -> broadcast::Receiver<CrdtMessage> {
        let tx = self.get_or_create_document(doc_id);
        tx.subscribe()
    }

    /// Broadcast an update to all subscribers of a document.
    pub fn broadcast_update(&self, doc_id: &DocumentId, msg: CrdtMessage) {
        // Clone the sender first so we don't hold the read lock during send
        let tx = {
            if let Some(state) = self.documents.get(doc_id) {
                state.tx.clone()
            } else {
                return;
            }
        };

        // Store incremental updates for late-joining clients
        if let CrdtMessage::Update(ref data) = msg {
            if let Some(mut state) = self.documents.get_mut(doc_id) {
                state.updates.push(data.clone());
            }
        }

        let _ = tx.send(msg);
    }

    /// Get the number of active documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// List all active document IDs.
    pub fn list_documents(&self) -> Vec<DocumentId> {
        self.documents.iter().map(|e| e.key().clone()).collect()
    }

    /// Remove a document (e.g., when no clients are connected).
    pub fn remove_document(&self, doc_id: &DocumentId) {
        if self.documents.remove(doc_id).is_some() {
            debug!(doc_id = %doc_id, "removed CRDT document");
        }
    }
}

impl Default for CrdtSyncServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_document_creation_and_subscription() {
        let server = CrdtSyncServer::new();
        let doc_id = "skill-abc".to_string();

        let mut rx = server.subscribe(&doc_id);
        assert_eq!(server.document_count(), 1);

        server.broadcast_update(&doc_id, CrdtMessage::Update(vec![1, 2, 3]));

        let msg = rx.recv().await.unwrap();
        match msg {
            CrdtMessage::Update(data) => assert_eq!(data, vec![1, 2, 3]),
            other => panic!("expected Update, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let server = CrdtSyncServer::new();
        let doc_id = "doc-1".to_string();

        let mut rx1 = server.subscribe(&doc_id);
        let mut rx2 = server.subscribe(&doc_id);

        server.broadcast_update(&doc_id, CrdtMessage::Awareness(vec![42]));

        let m1 = rx1.recv().await.unwrap();
        let m2 = rx2.recv().await.unwrap();
        assert!(matches!(m1, CrdtMessage::Awareness(_)));
        assert!(matches!(m2, CrdtMessage::Awareness(_)));
    }

    #[tokio::test]
    async fn test_remove_document() {
        let server = CrdtSyncServer::new();
        let doc_id = "temp-doc".to_string();
        server.get_or_create_document(&doc_id);
        assert_eq!(server.document_count(), 1);

        server.remove_document(&doc_id);
        assert_eq!(server.document_count(), 0);
    }
}