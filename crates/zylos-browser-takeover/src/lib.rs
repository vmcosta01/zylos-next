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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverSession {
    pub id: String,
    pub agent_id: String,
    pub browser_target_url: String,
    pub cdp_ws_url: String,
    pub status: TakeoverStatus,
    pub initiated_by: Initiator,
    pub approved_by: Option<String>,
    pub reason: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub message_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeoverStatus {
    PendingApproval,
    Active,
    Rejected,
    Completed,
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
    pub fn from_str(s: &str) -> Result<Self, TakeoverError> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Initiator {
    Agent,
    Human,
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
    pub fn from_str(s: &str) -> Result<Self, TakeoverError> {
        match s {
            "agent" => Ok(Self::Agent),
            "human" => Ok(Self::Human),
            "system" => Ok(Self::System),
            _ => Err(TakeoverError::InvalidInitiator(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    Incoming,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditedMessage {
    pub id: String,
    pub session_id: String,
    pub direction: MessageDirection,
    pub method: Option<String>,
    pub payload_size: usize,
    pub timestamp: DateTime<Utc>,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverRequest {
    pub agent_id: String,
    pub browser_target_url: String,
    pub cdp_ws_url: String,
    pub reason: String,
    pub initiated_by: Initiator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverDecision {
    pub session_id: String,
    pub approved: bool,
    pub approver: String,
    pub comment: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum TakeoverError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Invalid status: {0}")]
    InvalidStatus(String),
    #[error("Invalid initiator: {0}")]
    InvalidInitiator(String),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Session already active: {0}")]
    AlreadyActive(String),
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

pub struct TakeoverStore {
    tx: mpsc::Sender<DbCommand>,
}

impl TakeoverStore {
    pub fn open(path: &Path) -> Result<Self, TakeoverError> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        run_migrations(&conn)?;
        Ok(Self::spawn_writer(conn))
    }

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
        self.tx.send(cmd).await.map_err(|_| TakeoverError::ChannelClosed)
    }

    // -- Sessions --

    pub async fn create_session(&self, req: &TakeoverRequest) -> Result<TakeoverSession, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::CreateSession { req: req.clone(), reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn approve_session(&self, decision: &TakeoverDecision) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ApproveSession { decision: decision.clone(), reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn end_session(&self, session_id: &str, status: TakeoverStatus) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::EndSession { session_id: session_id.to_string(), status, reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn get_session(&self, id: &str) -> Result<TakeoverSession, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetSession { id: id.to_string(), reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn list_active_sessions(&self) -> Result<Vec<TakeoverSession>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ListByStatus { status: TakeoverStatus::Active, reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn list_pending_sessions(&self) -> Result<Vec<TakeoverSession>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::ListByStatus { status: TakeoverStatus::PendingApproval, reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    // -- Messages --

    pub async fn record_message(&self, msg: &AuditedMessage) -> Result<(), TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::RecordMessage { msg: msg.clone(), reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn get_messages(&self, session_id: &str, limit: usize) -> Result<Vec<AuditedMessage>, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetMessages { session_id: session_id.to_string(), limit, reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    pub async fn get_message_count(&self, session_id: &str) -> Result<u64, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::GetMessageCount { session_id: session_id.to_string(), reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }

    // -- Dashboard --

    pub async fn dashboard_summary(&self) -> Result<TakeoverDashboard, TakeoverError> {
        let (reply, rx) = oneshot::channel();
        self.send(DbCommand::DashboardSummary { reply }).await?;
        rx.await.map_err(|_| TakeoverError::ChannelClosed)?
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeoverDashboard {
    pub total_sessions: usize,
    pub active_sessions: usize,
    pub pending_sessions: usize,
    pub completed_sessions: usize,
    pub rejected_sessions: usize,
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
            DbCommand::EndSession { session_id, status, reply } => {
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
            DbCommand::GetMessages { session_id, limit, reply } => {
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

fn sync_create_session(conn: &Connection, req: &TakeoverRequest) -> Result<TakeoverSession, TakeoverError> {
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

fn sync_approve_session(conn: &Connection, decision: &TakeoverDecision) -> Result<(), TakeoverError> {
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

fn sync_end_session(conn: &Connection, session_id: &str, status: TakeoverStatus) -> Result<(), TakeoverError> {
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
        |row| row_to_session(row),
    )
    .optional()?
    .ok_or_else(|| TakeoverError::NotFound(id.to_string()))
}

fn sync_list_by_status(conn: &Connection, status: TakeoverStatus) -> Result<Vec<TakeoverSession>, TakeoverError> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, browser_target_url, cdp_ws_url, status, initiated_by, approved_by, reason, started_at, ended_at, message_count FROM takeover_sessions WHERE status = ?1 ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map(params![status.to_string()], |row| row_to_session(row))?;
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

fn sync_get_messages(conn: &Connection, session_id: &str, limit: usize) -> Result<Vec<AuditedMessage>, TakeoverError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, direction, method, payload_size, timestamp, payload FROM takeover_messages WHERE session_id = ?1 ORDER BY timestamp ASC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![session_id, limit as i64], |row| row_to_message(row))?;
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
    let total_sessions: usize = conn.query_row("SELECT COUNT(*) FROM takeover_sessions", [], |r| r.get(0))?;
    let active_sessions: usize = conn.query_row("SELECT COUNT(*) FROM takeover_sessions WHERE status = 'active'", [], |r| r.get(0))?;
    let pending_sessions: usize = conn.query_row("SELECT COUNT(*) FROM takeover_sessions WHERE status = 'pending_approval'", [], |r| r.get(0))?;
    let completed_sessions: usize = conn.query_row("SELECT COUNT(*) FROM takeover_sessions WHERE status = 'completed'", [], |r| r.get(0))?;
    let rejected_sessions: usize = conn.query_row("SELECT COUNT(*) FROM takeover_sessions WHERE status = 'rejected'", [], |r| r.get(0))?;
    let total_messages: u64 = conn.query_row("SELECT COALESCE(SUM(message_count), 0) FROM takeover_sessions", [], |r| r.get(0))?;
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

pub struct CdpProxy {
    store: std::sync::Arc<TakeoverStore>,
    session_id: String,
    live_tx: broadcast::Sender<AuditedMessage>,
}

impl CdpProxy {
    pub fn new(store: std::sync::Arc<TakeoverStore>, session_id: String) -> Self {
        let (live_tx, _) = broadcast::channel(1024);
        Self {
            store,
            session_id,
            live_tx,
        }
    }

    pub fn subscribe_live(&self) -> broadcast::Receiver<AuditedMessage> {
        self.live_tx.subscribe()
    }

    pub async fn run(
        self,
        client_ws: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
        upstream_url: &str,
    ) -> Result<(), TakeoverError> {
        use tokio_tungstenite::connect_async;

        let (upstream_ws, _) = connect_async(upstream_url).await?;
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

        let _ = self.store.end_session(&self.session_id, TakeoverStatus::Completed).await;
        Ok(())
    }
}

fn extract_cdp_method(json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("method").and_then(|m| m.as_str().map(|s| s.to_string())))
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
        status: TakeoverStatus::from_str(&row.get::<_, String>(4)?).unwrap_or(TakeoverStatus::Terminated),
        initiated_by: Initiator::from_str(&row.get::<_, String>(5)?).unwrap_or(Initiator::System),
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
            payload: r#"{"id":1,"method":"Page.navigate","params":{"url":"https://example.com"}}"#.into(),
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