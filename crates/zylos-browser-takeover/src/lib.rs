//! # zylos-browser-takeover
//!
//! Browser takeover console for zylos-gateway:
//! - Live CDP WebSocket proxy with full message audit
//! - Session handoff protocol (agent → human, human → agent)
//! - SQLite-backed audit trail for every CDP message
//! - Takeover request/approval workflow with HITL gates
//! - Replay capability for post-incident review

use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Domain Types
// ---------------------------------------------------------------------------

/// A browser takeover session representing a handoff between an AI agent and a human operator.
///
/// Tracks the full lifecycle from creation through approval/rejection to completion,
/// including audit metadata such as message counts and timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverSession {
    /// Unique session identifier (UUID v4).
    pub id: String,
    /// Identifier of the AI agent that owns or requested this session.
    pub agent_id: String,
    /// URL of the browser target being taken over.
    pub browser_target_url: String,
    /// Chrome DevTools Protocol WebSocket endpoint for the target page.
    pub cdp_ws_url: String,
    /// Current lifecycle status of the session.
    pub status: TakeoverStatus,
    /// Who initiated the takeover request.
    pub initiated_by: Initiator,
    /// Human approver identifier, set once the session is approved or rejected.
    pub approved_by: Option<String>,
    /// Free-text reason explaining why the takeover was requested.
    pub reason: String,
    /// Timestamp when the session was created.
    pub started_at: DateTime<Utc>,
    /// Timestamp when the session ended (completed, rejected, or terminated).
    pub ended_at: Option<DateTime<Utc>>,
    /// Running count of CDP messages recorded during this session.
    pub message_count: u64,
}

/// Lifecycle status of a [`TakeoverSession`].
///
/// Sessions transition through these states: `PendingApproval` → `Active` (on approval)
/// or `Rejected` (on denial). Active sessions may end as `Completed` or `Terminated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeoverStatus {
    /// Awaiting human approval before the session can become active.
    PendingApproval,
    /// Approved and currently proxying CDP traffic.
    Active,
    /// Approval was denied; the session is closed.
    Rejected,
    /// Session ended normally after the handoff completed.
    Completed,
    /// Session was forcibly terminated (e.g. timeout or system shutdown).
    Terminated,
}

impl std::fmt::Display for TakeoverStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PendingApproval => write!(f, "pending_approval"),
            Self::Active => write!(f, "active"),
            Self::Rejected => write!(f, "rejected"),
            Self::Completed => write!(f, "completed"),
            Self::Terminated => write!(f, "terminated"),
        }
    }
}

impl TakeoverStatus {
    /// Parses a status string into a [`TakeoverStatus`] variant.
    ///
    /// Accepts the snake_case representations produced by [`Display`](std::fmt::Display)
    /// and serde serialization (`pending_approval`, `active`, etc.).
    pub fn parse(s: &str) -> Result<Self, TakeoverError> {
        match s {
            "pending_approval" => Ok(Self::PendingApproval),
            "active" => Ok(Self::Active),
            "rejected" => Ok(Self::Rejected),
            "completed" => Ok(Self::Completed),
            "terminated" => Ok(Self::Terminated),
            _ => Err(TakeoverError::InvalidStatus(s.to_string())),
        }
    }
}

/// Identifies who initiated a takeover request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Initiator {
    /// The AI agent requested the handoff.
    Agent,
    /// A human operator requested the handoff.
    Human,
    /// The system triggered the handoff automatically.
    System,
}

impl std::fmt::Display for Initiator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Agent => write!(f, "agent"),
            Self::Human => write!(f, "human"),
            Self::System => write!(f, "system"),
        }
    }
}

impl Initiator {
    /// Parses an initiator string into an [`Initiator`] variant.
    ///
    /// Accepts `agent`, `human`, or `system`.
    pub fn parse(s: &str) -> Result<Self, TakeoverError> {
        match s {
            "agent" => Ok(Self::Agent),
            "human" => Ok(Self::Human),
            "system" => Ok(Self::System),
            _ => Err(TakeoverError::InvalidInitiator(s.to_string())),
        }
    }
}

/// Direction of a CDP message relative to the browser target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    /// Message sent from the client (agent or human) toward the browser.
    Incoming,
    /// Message received from the browser and forwarded to the client.
    Outgoing,
}

impl std::fmt::Display for MessageDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incoming => write!(f, "incoming"),
            Self::Outgoing => write!(f, "outgoing"),
        }
    }
}

/// A single audited CDP message captured during a takeover session.
///
/// Every message flowing through the [`CdpProxy`] is recorded in SQLite
/// and optionally broadcast to live subscribers for real-time monitoring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditedMessage {
    /// Unique message identifier (UUID v4).
    pub id: String,
    /// Session this message belongs to.
    pub session_id: String,
    /// Whether the message was incoming (client→browser) or outgoing (browser→client).
    pub direction: MessageDirection,
    /// CDP method name extracted from the JSON payload (e.g. `Page.navigate`).
    /// `None` for responses or binary frames.
    pub method: Option<String>,
    /// Size of the raw payload in bytes.
    pub payload_size: usize,
    /// Timestamp when the message was captured.
    pub timestamp: DateTime<Utc>,
    /// Raw JSON payload or a binary placeholder string.
    pub payload: String,
}

/// Request payload to initiate a new browser takeover session.
///
/// Submitted by an agent, human, or the system to request control handoff.
/// The session starts in [`TakeoverStatus::PendingApproval`] until a human reviews it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverRequest {
    /// Identifier of the AI agent requesting the takeover.
    pub agent_id: String,
    /// URL of the browser target to take over.
    pub browser_target_url: String,
    /// CDP WebSocket endpoint for the target page.
    pub cdp_ws_url: String,
    /// Human-readable reason for the takeover request.
    pub reason: String,
    /// Who initiated this request.
    pub initiated_by: Initiator,
}

/// Human decision on a pending takeover request.
///
/// Used to approve or reject a session that is in [`TakeoverStatus::PendingApproval`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverDecision {
    /// Session being approved or rejected.
    pub session_id: String,
    /// Whether the takeover was approved.
    pub approved: bool,
    /// Identifier of the human who made the decision.
    pub approver: String,
    /// Optional comment explaining the decision.
    pub comment: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors that can occur during takeover operations.
#[derive(Debug, thiserror::Error)]
pub enum TakeoverError {
    /// SQLite database error.
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    /// Requested session does not exist.
    #[error("Session not found: {0}")]
    NotFound(String),
    /// Invalid status string encountered during parsing.
    #[error("Invalid status: {0}")]
    InvalidStatus(String),
    /// Invalid initiator string encountered during parsing.
    #[error("Invalid initiator: {0}")]
    InvalidInitiator(String),
    /// WebSocket transport error.
    #[error("WebSocket error: {0}")]
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// File-system or I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// A session for the same agent is already active.
    #[error("Session already active: {0}")]
    AlreadyActive(String),
    /// Internal channel was closed unexpectedly.
    #[error("Channel closed")]
    ChannelClosed,
}

// ---------------------------------------------------------------------------
// Internal DB commands sent to the writer task
// ---------------------------------------------------------------------------

enum DbCommand {
    CreateSession {
        req: TakeoverRequest,
        reply: oneshot::Sender<Result<TakeoverSession, TakeoverError>>,
    },
    ApproveSession {
        decision: TakeoverDecision,
        reply: oneshot::Sender<Result<(), TakeoverError>>,
    },
    EndSession {
        session_id: String,
        status: TakeoverStatus,
        reply: oneshot::Sender<Result<(), TakeoverError>>,
    },
    GetSession {
        id: String,
        reply: oneshot::Sender<Result<TakeoverSession, TakeoverError>>,
    },
    ListByStatus {
        status: TakeoverStatus,
        reply: oneshot::Sender<Result<Vec<TakeoverSession>, TakeoverError>>,
    },
    RecordMessage {
        msg: AuditedMessage,
        reply: oneshot::Sender<Result<(), TakeoverError>>,
    },
    GetMessages {
        session_id: String,
        limit: usize,
        reply: oneshot::Sender<Result<Vec<AuditedMessage>, TakeoverError>>,
    },
    GetMessageCount {
        session_id: String,
        reply: oneshot::Sender<Result<u64, TakeoverError>>,
    },
    DashboardSummary {
        reply: oneshot::Sender<Result<TakeoverDashboard, TakeoverError>>,
    },
}

// ---------------------------------------------------------------------------
// TakeoverStore — channel-based SQLite WAL backend
// ---------------------------------------------------------------------------

/// SQLite-backed store for takeover sessions and audited messages.
///
/// Uses a channel-based writer pattern: all database operations are serialized
/// on a dedicated blocking thread, keeping the async runtime free. The backing
/// connection uses WAL mode for concurrent read access during proxy operation.
pub struct TakeoverStore {
    tx: mpsc::Sender<DbCommand>,
}

impl TakeoverStore {
    /// Opens a persistent SQLite-backed store at the given path.
    ///
    /// Enables WAL journal mode and runs migrations automatically.
    pub fn open(path: &Path) -> Result<Self, TakeoverError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        run_migrations(&conn)?;
        Ok(Self::spawn_writer(conn))
    }

    /// Creates an in-memory store for testing purposes.
    ///
    /// Data is lost when the store is dropped. Foreign keys are enabled
    /// but WAL mode is not (unnecessary for in-memory connections).
    pub fn in_memory() -> Result<Self, TakeoverError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        run_migrations(&conn)?;
        Ok(Self::spawn_writer(conn))
    }

    fn spawn_writer(conn: Connection) -> Self {
        let (tx, rx) = mpsc::channel::<DbCommand>(256);
        tokio::task::spawn_blocking(move || writer_loop(conn, rx));
        Self { tx }
    }

    async fn send(&self, cmd: DbCommand) -> Result<(), TakeoverError> {
        self.tx
            .send(cmd)
            .await
            .map_err(|_| TakeoverError::ChannelClosed)
    }

    // -- Sessions --

    /// Creates a new takeover session in [`TakeoverStatus::PendingApproval`] state.
    ///
    /// Returns the created session with a generated UUID and current timestamp.
    pub async fn create_session(
        &self,
        req: &TakeoverRequest,
    ) -> Result<TakeoverSession, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::CreateSession {
            req: req.clone(),
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Approves or rejects a pending takeover session.
    ///
    /// On approval the session transitions to [`TakeoverStatus::Active`].
    /// On rejection it transitions to [`TakeoverStatus::Rejected`] and `ended_at` is set.
    pub async fn approve_session(&self, decision: &TakeoverDecision) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ApproveSession {
            decision: decision.clone(),
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Ends an active session with the given final status.
    ///
    /// Sets `ended_at` to the current time. Only sessions in [`TakeoverStatus::Active`]
    /// can be ended; returns [`TakeoverError::NotFound`] otherwise.
    pub async fn end_session(
        &self,
        session_id: &str,
        status: TakeoverStatus,
    ) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::EndSession {
            session_id: session_id.to_string(),
            status,
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Retrieves a session by its identifier.
    ///
    /// Returns [`TakeoverError::NotFound`] if no session exists with the given ID.
    pub async fn get_session(&self, id: &str) -> Result<TakeoverSession, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetSession {
            id: id.to_string(),
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Lists all sessions currently in [`TakeoverStatus::Active`] state.
    pub async fn list_active_sessions(&self) -> Result<Vec<TakeoverSession>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ListByStatus {
            status: TakeoverStatus::Active,
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Lists all sessions currently in [`TakeoverStatus::PendingApproval`] state.
    pub async fn list_pending_sessions(&self) -> Result<Vec<TakeoverSession>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ListByStatus {
            status: TakeoverStatus::PendingApproval,
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    // -- Messages --

    /// Records an audited CDP message and increments the session's message counter.
    pub async fn record_message(&self, msg: &AuditedMessage) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::RecordMessage {
            msg: msg.clone(),
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Retrieves audited messages for a session, ordered by timestamp ascending.
    ///
    /// Returns at most `limit` messages.
    pub async fn get_messages(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<AuditedMessage>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetMessages {
            session_id: session_id.to_string(),
            limit,
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    /// Returns the total number of audited messages recorded for a session.
    pub async fn get_message_count(&self, session_id: &str) -> Result<u64, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetMessageCount {
            session_id: session_id.to_string(),
            reply,
        })
        .await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    // -- Dashboard --

    /// Returns an aggregate summary of all takeover sessions and messages.
    pub async fn dashboard_summary(&self) -> Result<TakeoverDashboard, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::DashboardSummary { reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }
}

/// Aggregate statistics for the takeover dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverDashboard {
    /// Total number of sessions across all statuses.
    pub total_sessions: usize,
    /// Number of sessions currently in [`TakeoverStatus::Active`] state.
    pub active_sessions: usize,
    /// Number of sessions awaiting approval.
    pub pending_sessions: usize,
    /// Number of sessions that completed normally.
    pub completed_sessions: usize,
    /// Number of sessions that were rejected.
    pub rejected_sessions: usize,
    /// Total CDP messages recorded across all sessions.
    pub total_messages: u64,
}

// ---------------------------------------------------------------------------
// Writer loop — runs on a dedicated blocking thread
// ---------------------------------------------------------------------------

fn writer_loop(conn: Connection, mut rx: mpsc::Receiver<DbCommand>) {
    while let Some(cmd) = rx.blocking_recv() {
        match cmd {
            DbCommand::CreateSession { req, reply } => {
                let result = sync_create_session(&conn, &req);
                let _ = reply.send(result);
            }
            DbCommand::ApproveSession { decision, reply } => {
                let result = sync_approve_session(&conn, &decision);
                let _ = reply.send(result);
            }
            DbCommand::EndSession {
                session_id,
                status,
                reply,
            } => {
                let result = sync_end_session(&conn, &session_id, status);
                let _ = reply.send(result);
            }
            DbCommand::GetSession { id, reply } => {
                let result = sync_get_session(&conn, &id);
                let _ = reply.send(result);
            }
            DbCommand::ListByStatus { status, reply } => {
                let result = sync_list_by_status(&conn, status);
                let _ = reply.send(result);
            }
            DbCommand::RecordMessage { msg, reply } => {
                let result = sync_record_message(&conn, &msg);
                let _ = reply.send(result);
            }
            DbCommand::GetMessages {
                session_id,
                limit,
                reply,
            } => {
                let result = sync_get_messages(&conn, &session_id, limit);
                let _ = reply.send(result);
            }
            DbCommand::GetMessageCount { session_id, reply } => {
                let result = sync_get_message_count(&conn, &session_id);
                let _ = reply.send(result);
            }
            DbCommand::DashboardSummary { reply } => {
                let result = sync_dashboard_summary(&conn);
                let _ = reply.send(result);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Synchronous DB operations (called from writer thread only)
// ---------------------------------------------------------------------------

fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS takeover_sessions (
            id TEXT PRIMARY KEY,
            agent_id TEXT NOT NULL,
            browser_target_url TEXT NOT NULL,
            cdp_ws_url TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending_approval',
            initiated_by TEXT NOT NULL,
            approved_by TEXT,
            reason TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            message_count INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS takeover_messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES takeover_sessions(id) ON DELETE CASCADE,
            direction TEXT NOT NULL,
            method TEXT,
            payload_size INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            payload TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_takeover_sessions_agent ON takeover_sessions(agent_id);
        CREATE INDEX IF NOT EXISTS idx_takeover_sessions_status ON takeover_sessions(status);
        CREATE INDEX IF NOT EXISTS idx_takeover_messages_session ON takeover_messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_takeover_messages_ts ON takeover_messages(timestamp);
        ",
    )?;
    Ok(())
}

fn sync_create_session(
    conn: &Connection,
    req: &TakeoverRequest,
) -> Result<TakeoverSession, TakeoverError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let session = TakeoverSession {
        id: id.clone(),
        agent_id: req.agent_id.clone(),
        browser_target_url: req.browser_target_url.clone(),
        cdp_ws_url: req.cdp_ws_url.clone(),
        status: TakeoverStatus::PendingApproval,
        initiated_by: req.initiated_by,
        approved_by: None,
        reason: req.reason.clone(),
        started_at: now,
        ended_at: None,
        message_count: 0,
    };
    conn.execute(
        "INSERT INTO takeover_sessions (id, agent_id, browser_target_url, cdp_ws_url, status, initiated_by, approved_by, reason, started_at, ended_at, message_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            session.id,
            session.agent_id,
            session.browser_target_url,
            session.cdp_ws_url,
            session.status.to_string(),
            session.initiated_by.to_string(),
            session.approved_by,
            session.reason,
            session.started_at.to_rfc3339(),
            session.ended_at.as_ref().map(|d| d.to_rfc3339()),
            session.message_count as i64,
        ],
    )?;
    Ok(session)
}

fn sync_approve_session(
    conn: &Connection,
    decision: &TakeoverDecision,
) -> Result<(), TakeoverError> {
    let status = if decision.approved {
        TakeoverStatus::Active
    } else {
        TakeoverStatus::Rejected
    };
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE takeover_sessions SET status = ?1, approved_by = ?2 WHERE id = ?3 AND status = 'pending_approval'",
        params![status.to_string(), decision.approver, decision.session_id],
    )?;
    if affected == 0 {
        return Err(TakeoverError::NotFound(decision.session_id.clone()));
    }
    if !decision.approved {
        conn.execute(
            "UPDATE takeover_sessions SET ended_at = ?1 WHERE id = ?2",
            params![now, decision.session_id],
        )?;
    }
    Ok(())
}

fn sync_end_session(
    conn: &Connection,
    session_id: &str,
    status: TakeoverStatus,
) -> Result<(), TakeoverError> {
    let now = Utc::now().to_rfc3339();
    let affected = conn.execute(
        "UPDATE takeover_sessions SET status = ?1, ended_at = ?2 WHERE id = ?3 AND status = 'active'",
        params![status.to_string(), now, session_id],
    )?;
    if affected == 0 {
        return Err(TakeoverError::NotFound(session_id.to_string()));
    }
    Ok(())
}

fn sync_get_session(conn: &Connection, id: &str) -> Result<TakeoverSession, TakeoverError> {
    conn.query_row(
        "SELECT id, agent_id, browser_target_url, cdp_ws_url, status, initiated_by, approved_by, reason, started_at, ended_at, message_count FROM takeover_sessions WHERE id = ?1",
        params![id],
        row_to_session,
    )
    .optional()?
    .ok_or_else(|| TakeoverError::NotFound(id.to_string()))
}

fn sync_list_by_status(
    conn: &Connection,
    status: TakeoverStatus,
) -> Result<Vec<TakeoverSession>, TakeoverError> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, browser_target_url, cdp_ws_url, status, initiated_by, approved_by, reason, started_at, ended_at, message_count FROM takeover_sessions WHERE status = ?1 ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map(params![status.to_string()], row_to_session)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn sync_record_message(conn: &Connection, msg: &AuditedMessage) -> Result<(), TakeoverError> {
    conn.execute(
        "INSERT INTO takeover_messages (id, session_id, direction, method, payload_size, timestamp, payload) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            msg.id,
            msg.session_id,
            msg.direction.to_string(),
            msg.method,
            msg.payload_size as i64,
            msg.timestamp.to_rfc3339(),
            msg.payload,
        ],
    )?;
    conn.execute(
        "UPDATE takeover_sessions SET message_count = message_count + 1 WHERE id = ?1",
        params![msg.session_id],
    )?;
    Ok(())
}

fn sync_get_messages(
    conn: &Connection,
    session_id: &str,
    limit: usize,
) -> Result<Vec<AuditedMessage>, TakeoverError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, direction, method, payload_size, timestamp, payload FROM takeover_messages WHERE session_id = ?1 ORDER BY timestamp ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![session_id, limit as i64], row_to_message)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn sync_get_message_count(conn: &Connection, session_id: &str) -> Result<u64, TakeoverError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM takeover_messages WHERE session_id = ?1",
        params![session_id],
        |r| r.get(0),
    )?;
    Ok(count as u64)
}

fn sync_dashboard_summary(conn: &Connection) -> Result<TakeoverDashboard, TakeoverError> {
    let total_sessions: usize =
        conn.query_row("SELECT COUNT(*) FROM takeover_sessions", [], |r| r.get(0))?;
    let active_sessions: usize = conn.query_row(
        "SELECT COUNT(*) FROM takeover_sessions WHERE status = 'active'",
        [],
        |r| r.get(0),
    )?;
    let pending_sessions: usize = conn.query_row(
        "SELECT COUNT(*) FROM takeover_sessions WHERE status = 'pending_approval'",
        [],
        |r| r.get(0),
    )?;
    let completed_sessions: usize = conn.query_row(
        "SELECT COUNT(*) FROM takeover_sessions WHERE status = 'completed'",
        [],
        |r| r.get(0),
    )?;
    let rejected_sessions: usize = conn.query_row(
        "SELECT COUNT(*) FROM takeover_sessions WHERE status = 'rejected'",
        [],
        |r| r.get(0),
    )?;
    let total_messages: u64 = conn.query_row(
        "SELECT COALESCE(SUM(message_count), 0) FROM takeover_sessions",
        [],
        |r| r.get(0),
    )?;
    Ok(TakeoverDashboard {
        total_sessions,
        active_sessions,
        pending_sessions,
        completed_sessions,
        rejected_sessions,
        total_messages,
    })
}

// ---------------------------------------------------------------------------
// CDP Proxy
// ---------------------------------------------------------------------------

/// Bidirectional CDP WebSocket proxy that audits every message in transit.
///
/// Sits between a client (agent or human) and the upstream browser target,
/// recording all traffic to the [`TakeoverStore`] and broadcasting live
/// messages to subscribers via a broadcast channel.
pub struct CdpProxy {
    store: std::sync::Arc<TakeoverStore>,
    session_id: String,
    live_tx: broadcast::Sender<AuditedMessage>,
}

impl CdpProxy {
    /// Creates a new proxy instance for the given session.
    ///
    /// The proxy does not start forwarding until [`run`](Self::run) is called.
    pub fn new(store: std::sync::Arc<TakeoverStore>, session_id: String) -> Self {
        let (live_tx, _) = broadcast::channel(1024);
        Self {
            store,
            session_id,
            live_tx,
        }
    }

    /// Subscribes to the live broadcast channel for real-time CDP message monitoring.
    ///
    /// Returns a receiver that yields [`AuditedMessage`] values as they flow through the proxy.
    pub fn subscribe_live(&self) -> broadcast::Receiver<AuditedMessage> {
        self.live_tx.subscribe()
    }

    /// Runs the bidirectional CDP proxy until one side disconnects.
    ///
    /// Connects to `upstream_url`, then forwards messages between `client_ws` and
    /// the upstream browser while auditing every frame to the store and live
    /// broadcast channel. When either side closes, the session is automatically
    /// ended with [`TakeoverStatus::Completed`].
    pub async fn run(
        self,
        client_ws: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        upstream_url: &str,
    ) -> Result<(), TakeoverError> {
        use tokio_tungstenite::connect_async;

        let (upstream_ws, _) = connect_async(upstream_url)
            .await
            .map_err(|e| TakeoverError::WebSocket(Box::new(e)))?;
        let (mut client_sink, mut client_stream) = client_ws.split();
        let (mut upstream_sink, mut upstream_stream) = upstream_ws.split();

        let store_in = self.store.clone();
        let session_in = self.session_id.clone();
        let tx_in = self.live_tx.clone();

        let incoming_handle = tokio::spawn(async move {
            while let Some(msg) = client_stream.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        let method = extract_cdp_method(&text);
                        let audited = AuditedMessage {
                            id: Uuid::new_v4().to_string(),
                            session_id: session_in.clone(),
                            direction: MessageDirection::Incoming,
                            method,
                            payload_size: text.len(),
                            timestamp: Utc::now(),
                            payload: text.clone(),
                        };
                        let _ = store_in.record_message(&audited).await;
                        let _ = tx_in.send(audited);
                        if upstream_sink.send(WsMessage::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Ok(WsMessage::Binary(data)) => {
                        let audited = AuditedMessage {
                            id: Uuid::new_v4().to_string(),
                            session_id: session_in.clone(),
                            direction: MessageDirection::Incoming,
                            method: None,
                            payload_size: data.len(),
                            timestamp: Utc::now(),
                            payload: format!("<binary {} bytes>", data.len()),
                        };
                        let _ = store_in.record_message(&audited).await;
                        let _ = tx_in.send(audited);
                        if upstream_sink.send(WsMessage::Binary(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(WsMessage::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        let store_out = self.store.clone();
        let session_out = self.session_id.clone();
        let tx_out = self.live_tx.clone();

        let outgoing_handle = tokio::spawn(async move {
            while let Some(msg) = upstream_stream.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        let method = extract_cdp_method(&text);
                        let audited = AuditedMessage {
                            id: Uuid::new_v4().to_string(),
                            session_id: session_out.clone(),
                            direction: MessageDirection::Outgoing,
                            method,
                            payload_size: text.len(),
                            timestamp: Utc::now(),
                            payload: text.clone(),
                        };
                        let _ = store_out.record_message(&audited).await;
                        let _ = tx_out.send(audited);
                        if client_sink.send(WsMessage::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Ok(WsMessage::Binary(data)) => {
                        let audited = AuditedMessage {
                            id: Uuid::new_v4().to_string(),
                            session_id: session_out.clone(),
                            direction: MessageDirection::Outgoing,
                            method: None,
                            payload_size: data.len(),
                            timestamp: Utc::now(),
                            payload: format!("<binary {} bytes>", data.len()),
                        };
                        let _ = store_out.record_message(&audited).await;
                        let _ = tx_out.send(audited);
                        if client_sink.send(WsMessage::Binary(data)).await.is_err() {
                            break;
                        }
                    }
                    Ok(WsMessage::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        tokio::select! {
            _ = incoming_handle => {},
            _ = outgoing_handle => {},
        }

        let _ = self
            .store
            .end_session(&self.session_id, TakeoverStatus::Completed)
            .await;
        Ok(())
    }
}

fn extract_cdp_method(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| {
            v.get("method")
                .and_then(|m| m.as_str().map(|s| s.to_string()))
        })
}

// ---------------------------------------------------------------------------
// Row mapping helpers
// ---------------------------------------------------------------------------

fn row_to_session(row: &rusqlite::Row) -> Result<TakeoverSession, rusqlite::Error> {
    let ended_raw: Option<String> = row.get(9)?;
    let ended_at = ended_raw
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    Ok(TakeoverSession {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        browser_target_url: row.get(2)?,
        cdp_ws_url: row.get(3)?,
        status: TakeoverStatus::parse(&row.get::<_, String>(4)?)
            .unwrap_or(TakeoverStatus::Terminated),
        initiated_by: Initiator::parse(&row.get::<_, String>(5)?).unwrap_or(Initiator::System),
        approved_by: row.get(6)?,
        reason: row.get(7)?,
        started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        ended_at,
        message_count: row.get::<_, i64>(10)? as u64,
    })
}

fn row_to_message(row: &rusqlite::Row) -> Result<AuditedMessage, rusqlite::Error> {
    Ok(AuditedMessage {
        id: row.get(0)?,
        session_id: row.get(1)?,
        direction: match row.get::<_, String>(2)?.as_str() {
            "incoming" => MessageDirection::Incoming,
            _ => MessageDirection::Outgoing,
        },
        method: row.get(3)?,
        payload_size: row.get::<_, i64>(4)? as usize,
        timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
            .unwrap_or_default()
            .with_timezone(&Utc),
        payload: row.get(6)?,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_approve_session() {
        let store = TakeoverStore::in_memory().unwrap();
        let req = TakeoverRequest {
            agent_id: "agent-1".into(),
            browser_target_url: "https://example.com".into(),
            cdp_ws_url: "ws://localhost:9222/devtools/page/abc".into(),
            reason: "Payment form requires human verification".into(),
            initiated_by: Initiator::Agent,
        };
        let session = store.create_session(&req).await.unwrap();
        assert_eq!(session.status, TakeoverStatus::PendingApproval);

        let decision = TakeoverDecision {
            session_id: session.id.clone(),
            approved: true,
            approver: "alice".into(),
            comment: None,
        };
        store.approve_session(&decision).await.unwrap();
        let updated = store.get_session(&session.id).await.unwrap();
        assert_eq!(updated.status, TakeoverStatus::Active);
        assert_eq!(updated.approved_by, Some("alice".into()));
    }

    #[tokio::test]
    async fn test_reject_session() {
        let store = TakeoverStore::in_memory().unwrap();
        let req = TakeoverRequest {
            agent_id: "agent-2".into(),
            browser_target_url: "https://bank.example.com".into(),
            cdp_ws_url: "ws://localhost:9222/devtools/page/def".into(),
            reason: "Suspicious activity".into(),
            initiated_by: Initiator::Agent,
        };
        let session = store.create_session(&req).await.unwrap();
        let decision = TakeoverDecision {
            session_id: session.id.clone(),
            approved: false,
            approver: "bob".into(),
            comment: Some("Denied: insufficient justification".into()),
        };
        store.approve_session(&decision).await.unwrap();
        let updated = store.get_session(&session.id).await.unwrap();
        assert_eq!(updated.status, TakeoverStatus::Rejected);
        assert!(updated.ended_at.is_some());
    }

    #[tokio::test]
    async fn test_record_and_retrieve_messages() {
        let store = TakeoverStore::in_memory().unwrap();
        let req = TakeoverRequest {
            agent_id: "agent-3".into(),
            browser_target_url: "https://example.com".into(),
            cdp_ws_url: "ws://localhost:9222/devtools/page/ghi".into(),
            reason: "Test".into(),
            initiated_by: Initiator::Human,
        };
        let session = store.create_session(&req).await.unwrap();

        let msg = AuditedMessage {
            id: "msg-1".into(),
            session_id: session.id.clone(),
            direction: MessageDirection::Incoming,
            method: Some("Page.navigate".into()),
            payload_size: 128,
            timestamp: Utc::now(),
            payload: r#"{"id":1,"method":"Page.navigate","params":{"url":"https://example.com"}}"#
                .into(),
        };
        store.record_message(&msg).await.unwrap();

        let messages = store.get_messages(&session.id, 100).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].method, Some("Page.navigate".into()));
        assert_eq!(messages[0].direction, MessageDirection::Incoming);

        let count = store.get_message_count(&session.id).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_extract_cdp_method() {
        let json = r#"{"id":1,"method":"Runtime.evaluate","params":{"expression":"1+1"}}"#;
        assert_eq!(extract_cdp_method(json), Some("Runtime.evaluate".into()));

        let response = r#"{"id":1,"result":{"value":2}}"#;
        assert_eq!(extract_cdp_method(response), None);
    }

    #[tokio::test]
    async fn test_dashboard_summary() {
        let store = TakeoverStore::in_memory().unwrap();
        let summary = store.dashboard_summary().await.unwrap();
        assert_eq!(summary.total_sessions, 0);
        assert_eq!(summary.active_sessions, 0);
        assert_eq!(summary.total_messages, 0);
    }
}
