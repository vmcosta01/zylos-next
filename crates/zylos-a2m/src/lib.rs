//! zylos-a2m: Agent-to-Agent messaging with file transfers.
//!
//! Provides a message bus for inter-agent communication supporting text
//! messages, file transfers (up to 50MB), reactions, reply threading, and
//! delivery receipts. Messages are persisted in SQLite for offline agents
//! and delivered on reconnect.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tracing::info;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum A2mError {
    #[error("agent not found: {0}")]
    AgentNotFound(String),

    #[error("message not found: {0}")]
    MessageNotFound(String),

    #[error("file too large: {size} bytes (max {max})")]
    FileTooLarge { size: u64, max: u64 },

    #[error("file integrity check failed: expected {expected}, got {actual}")]
    IntegrityCheck { expected: String, actual: String },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, A2mError>;

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for an agent.
pub type AgentId = String;

/// Unique identifier for a message.
pub type MessageId = String;

/// Delivery status for receipt tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeliveryStatus {
    Sent,
    Delivered,
    Read,
}

/// An A2M protocol message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum A2mMessage {
    Text {
        from: AgentId,
        to: AgentId,
        body: String,
        reply_to: Option<MessageId>,
    },
    File {
        from: AgentId,
        to: AgentId,
        filename: String,
        content_hash: String,
        size: u64,
    },
    Reaction {
        from: AgentId,
        message_id: MessageId,
        emoji: String,
    },
    Receipt {
        message_id: MessageId,
        status: DeliveryStatus,
    },
}

impl A2mMessage {
    /// Extract the sender of this message.
    pub fn sender(&self) -> Option<&AgentId> {
        match self {
            A2mMessage::Text { from, .. } => Some(from),
            A2mMessage::File { from, .. } => Some(from),
            A2mMessage::Reaction { from, .. } => Some(from),
            A2mMessage::Receipt { .. } => None,
        }
    }

    /// Extract the recipient of this message (if applicable).
    pub fn recipient(&self) -> Option<&AgentId> {
        match self {
            A2mMessage::Text { to, .. } => Some(to),
            A2mMessage::File { to, .. } => Some(to),
            _ => None,
        }
    }
}

/// Metadata for a sent message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEnvelope {
    pub id: MessageId,
    pub message: A2mMessage,
    pub timestamp: DateTime<Utc>,
    pub delivered: bool,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the A2M bus.
#[derive(Debug, Clone)]
pub struct A2mConfig {
    /// Maximum file transfer size in bytes (default: 50MB).
    pub max_file_size: u64,
    /// Directory for storing transferred files.
    pub transfer_dir: PathBuf,
}

impl Default for A2mConfig {
    fn default() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB
            transfer_dir: PathBuf::from("/data/a2m-transfers"),
        }
    }
}

// ---------------------------------------------------------------------------
// A2M Bus
// ---------------------------------------------------------------------------

/// The agent-to-agent message bus.
pub struct A2mBus {
    config: A2mConfig,
    db: Arc<RwLock<Connection>>,
    subscribers: Arc<RwLock<std::collections::HashMap<AgentId, broadcast::Sender<A2mMessage>>>>,
}

impl A2mBus {
    /// Create a new A2M bus with the given configuration.
    pub fn new(config: A2mConfig) -> Result<Self> {
        let db_path = config.transfer_dir.join("a2m.db");
        std::fs::create_dir_all(&config.transfer_dir)?;

        let db = Connection::open(&db_path)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                from_agent TEXT,
                to_agent TEXT,
                payload TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                delivered INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_messages_to ON messages(to_agent, delivered);
            CREATE INDEX IF NOT EXISTS idx_messages_from ON messages(from_agent);",
        )?;

        info!(path = ?db_path, "A2M bus initialized");

        Ok(Self {
            config,
            db: Arc::new(RwLock::new(db)),
            subscribers: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Send a message through the bus. Returns the message ID.
    pub async fn send(&self, msg: A2mMessage) -> Result<MessageId> {
        let id = Uuid::new_v4().to_string();
        let envelope = MessageEnvelope {
            id: id.clone(),
            message: msg.clone(),
            timestamp: Utc::now(),
            delivered: false,
        };

        // Persist to SQLite
        let payload = serde_json::to_string(&msg)
            .map_err(|e| A2mError::Serialization(e.to_string()))?;

        let db = self.db.read().await;
        db.execute(
            "INSERT INTO messages (id, from_agent, to_agent, payload, timestamp, delivered) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
            params![
                &id,
                msg.sender(),
                msg.recipient(),
                &payload,
                &envelope.timestamp.to_rfc3339(),
            ],
        )?;
        drop(db);

        // Deliver to subscriber if online
        if let Some(recipient) = msg.recipient() {
            let subs = self.subscribers.read().await;
            if let Some(tx) = subs.get(recipient) {
                if tx.send(msg.clone()).is_ok() {
                    // Mark as delivered
                    let db = self.db.read().await;
                    let _ = db.execute("UPDATE messages SET delivered = 1 WHERE id = ?1", params![&id]);
                }
            }
        }

        info!(msg_id = %id, "Message sent");
        Ok(id)
    }

    /// Subscribe to messages for a specific agent. Returns a stream receiver.
    pub async fn subscribe(&self, agent_id: &AgentId) -> broadcast::Receiver<A2mMessage> {
        let mut subs = self.subscribers.write().await;
        let tx = subs
            .entry(agent_id.clone())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(256);
                tx
            });
        let rx = tx.subscribe();

        // Deliver any pending offline messages
        let db = self.db.read().await;
        if let Ok(mut stmt) = db.prepare(
            "SELECT payload FROM messages WHERE to_agent = ?1 AND delivered = 0 ORDER BY timestamp ASC",
        ) {
            if let Ok(rows) = stmt.query_map(params![agent_id], |row| {
                row.get::<_, String>(0)
            }) {
                for row in rows.flatten() {
                    if let Ok(msg) = serde_json::from_str::<A2mMessage>(&row) {
                        let _ = tx.send(msg);
                    }
                }
            }
            // Mark all as delivered
            let _ = db.execute(
                "UPDATE messages SET delivered = 1 WHERE to_agent = ?1 AND delivered = 0",
                params![agent_id],
            );
        }

        info!(agent = %agent_id, "Agent subscribed to A2M bus");
        rx
    }

    /// Transfer a file between agents. Returns the message ID.
    pub async fn transfer_file(
        &self,
        from: &AgentId,
        to: &AgentId,
        path: &Path,
    ) -> Result<MessageId> {
        let metadata = tokio::fs::metadata(path).await?;
        let size = metadata.len();

        if size > self.config.max_file_size {
            return Err(A2mError::FileTooLarge {
                size,
                max: self.config.max_file_size,
            });
        }

        // Compute SHA-256 hash
        let contents = tokio::fs::read(path).await?;
        let hash = hex::encode(Sha256::digest(&contents));

        // Copy file to transfer directory with content-addressed name
        let dest = self.config.transfer_dir.join(&hash);
        tokio::fs::write(&dest, &contents).await?;

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let msg = A2mMessage::File {
            from: from.clone(),
            to: to.clone(),
            filename,
            content_hash: hash,
            size,
        };

        self.send(msg).await
    }

    /// Retrieve a transferred file by its content hash, verifying integrity.
    pub async fn retrieve_file(&self, content_hash: &str) -> Result<Vec<u8>> {
        let path = self.config.transfer_dir.join(content_hash);
        let contents = tokio::fs::read(&path).await?;

        // Verify integrity
        let actual_hash = hex::encode(Sha256::digest(&contents));
        if actual_hash != content_hash {
            return Err(A2mError::IntegrityCheck {
                expected: content_hash.to_string(),
                actual: actual_hash,
            });
        }

        Ok(contents)
    }

    /// Get message history for an agent (last N messages).
    pub async fn history(&self, agent_id: &AgentId, limit: usize) -> Result<Vec<MessageEnvelope>> {
        let db = self.db.read().await;
        let mut stmt = db.prepare(
            "SELECT id, payload, timestamp, delivered FROM messages WHERE to_agent = ?1 OR from_agent = ?1 ORDER BY timestamp DESC LIMIT ?2",
        )?;

        let envelopes = stmt
            .query_map(params![agent_id, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id, payload, ts, delivered)| {
                let message = serde_json::from_str(&payload).ok()?;
                let timestamp = DateTime::parse_from_rfc3339(&ts).ok()?.with_timezone(&Utc);
                Some(MessageEnvelope {
                    id,
                    message,
                    timestamp,
                    delivered,
                })
            })
            .collect();

        Ok(envelopes)
    }

    /// Get the number of pending (undelivered) messages for an agent.
    pub async fn pending_count(&self, agent_id: &AgentId) -> Result<usize> {
        let db = self.db.read().await;
        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM messages WHERE to_agent = ?1 AND delivered = 0",
            params![agent_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get bus statistics.
    pub async fn stats(&self) -> A2mStats {
        let db = self.db.read().await;
        let total: i64 = db
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap_or(0);
        let delivered: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM messages WHERE delivered = 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let subs = self.subscribers.read().await;

        A2mStats {
            total_messages: total as u64,
            delivered_messages: delivered as u64,
            active_subscribers: subs.len(),
        }
    }
}

/// Bus statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2mStats {
    pub total_messages: u64,
    pub delivered_messages: u64,
    pub active_subscribers: usize,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(dir: &Path) -> A2mConfig {
        A2mConfig {
            max_file_size: 1024 * 1024, // 1MB for tests
            transfer_dir: dir.to_path_buf(),
        }
    }

    #[tokio::test]
    async fn test_send_text_message() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let msg = A2mMessage::Text {
            from: "agent-a".into(),
            to: "agent-b".into(),
            body: "Hello, agent B!".into(),
            reply_to: None,
        };

        let id = bus.send(msg).await.unwrap();
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn test_subscribe_and_receive() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let mut rx = bus.subscribe(&"agent-b".into()).await;

        let msg = A2mMessage::Text {
            from: "agent-a".into(),
            to: "agent-b".into(),
            body: "Test message".into(),
            reply_to: None,
        };

        bus.send(msg.clone()).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_offline_message_delivery() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        // Send message before recipient subscribes
        let msg = A2mMessage::Text {
            from: "agent-a".into(),
            to: "agent-c".into(),
            body: "Offline message".into(),
            reply_to: None,
        };
        bus.send(msg.clone()).await.unwrap();

        // Now subscribe — should receive pending message
        let mut rx = bus.subscribe(&"agent-c".into()).await;
        let received = rx.recv().await.unwrap();
        assert_eq!(received, msg);
    }

    #[tokio::test]
    async fn test_reply_threading() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let original = A2mMessage::Text {
            from: "agent-a".into(),
            to: "agent-b".into(),
            body: "Original".into(),
            reply_to: None,
        };
        let original_id = bus.send(original).await.unwrap();

        let reply = A2mMessage::Text {
            from: "agent-b".into(),
            to: "agent-a".into(),
            body: "Reply".into(),
            reply_to: Some(original_id.clone()),
        };
        let reply_id = bus.send(reply).await.unwrap();

        assert_ne!(original_id, reply_id);

        let history = bus.history(&"agent-a".into(), 10).await.unwrap();
        assert!(history.len() >= 2);
    }

    #[tokio::test]
    async fn test_reaction() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let msg = A2mMessage::Text {
            from: "agent-a".into(),
            to: "agent-b".into(),
            body: "Something".into(),
            reply_to: None,
        };
        let msg_id = bus.send(msg).await.unwrap();

        let reaction = A2mMessage::Reaction {
            from: "agent-b".into(),
            message_id: msg_id,
            emoji: "👍".into(),
        };
        let reaction_id = bus.send(reaction).await.unwrap();
        assert!(!reaction_id.is_empty());
    }

    #[tokio::test]
    async fn test_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let receipt = A2mMessage::Receipt {
            message_id: "msg-123".into(),
            status: DeliveryStatus::Read,
        };
        let id = bus.send(receipt).await.unwrap();
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn test_file_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        // Create a test file
        let test_file = dir.path().join("test.txt");
        tokio::fs::write(&test_file, b"file contents here").await.unwrap();

        let msg_id = bus
            .transfer_file(&"agent-a".into(), &"agent-b".into(), &test_file)
            .await
            .unwrap();
        assert!(!msg_id.is_empty());

        // Verify file was stored
        let hash = hex::encode(Sha256::digest(b"file contents here"));
        let stored = dir.path().join(&hash);
        assert!(stored.exists());
    }

    #[tokio::test]
    async fn test_file_too_large() {
        let dir = tempfile::tempdir().unwrap();
        let config = A2mConfig {
            max_file_size: 100, // Very small limit
            transfer_dir: dir.path().to_path_buf(),
        };
        let bus = A2mBus::new(config).unwrap();

        let test_file = dir.path().join("big.bin");
        tokio::fs::write(&test_file, vec![0u8; 200]).await.unwrap();

        let result = bus
            .transfer_file(&"agent-a".into(), &"agent-b".into(), &test_file)
            .await;
        assert!(matches!(result, Err(A2mError::FileTooLarge { .. })));
    }

    #[tokio::test]
    async fn test_file_integrity_verification() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let data = b"integrity test data";
        let hash = hex::encode(Sha256::digest(data));

        // Store file manually
        tokio::fs::write(dir.path().join(&hash), data).await.unwrap();

        let retrieved = bus.retrieve_file(&hash).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_file_integrity_failure() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        let hash = hex::encode(Sha256::digest(b"original"));
        // Write different content under the same hash
        tokio::fs::write(dir.path().join(&hash), b"tampered")
            .await
            .unwrap();

        let result = bus.retrieve_file(&hash).await;
        assert!(matches!(result, Err(A2mError::IntegrityCheck { .. })));
    }

    #[tokio::test]
    async fn test_message_history() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        for i in 0..5 {
            bus.send(A2mMessage::Text {
                from: "agent-a".into(),
                to: "agent-b".into(),
                body: format!("Message {}", i),
                reply_to: None,
            })
            .await
            .unwrap();
        }

        let history = bus.history(&"agent-b".into(), 10).await.unwrap();
        assert_eq!(history.len(), 5);
    }

    #[tokio::test]
    async fn test_pending_count() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        // Send messages without subscriber
        for _ in 0..3 {
            bus.send(A2mMessage::Text {
                from: "agent-a".into(),
                to: "agent-d".into(),
                body: "Pending".into(),
                reply_to: None,
            })
            .await
            .unwrap();
        }

        let count = bus.pending_count(&"agent-d".into()).await.unwrap();
        assert_eq!(count, 3);

        // Subscribe clears pending
        let _rx = bus.subscribe(&"agent-d".into()).await;
        let count_after = bus.pending_count(&"agent-d".into()).await.unwrap();
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn test_bus_stats() {
        let dir = tempfile::tempdir().unwrap();
        let bus = A2mBus::new(test_config(dir.path())).unwrap();

        bus.send(A2mMessage::Text {
            from: "a".into(),
            to: "b".into(),
            body: "x".into(),
            reply_to: None,
        })
        .await
        .unwrap();

        let stats = bus.stats().await;
        assert_eq!(stats.total_messages, 1);
    }

    #[tokio::test]
    async fn test_message_sender_recipient() {
        let text = A2mMessage::Text {
            from: "a".into(),
            to: "b".into(),
            body: "hi".into(),
            reply_to: None,
        };
        assert_eq!(text.sender(), Some(&"a".to_string()));
        assert_eq!(text.recipient(), Some(&"b".to_string()));

        let receipt = A2mMessage::Receipt {
            message_id: "x".into(),
            status: DeliveryStatus::Sent,
        };
        assert_eq!(receipt.sender(), None);
        assert_eq!(receipt.recipient(), None);
    }
}