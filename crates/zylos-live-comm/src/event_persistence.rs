//! Append-only SQLite event log with replay, TTL and dead-letter queue.

use crate::event_types::EventEnvelope;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, warn};

/// Persistent event store backed by SQLite.
///
/// Tables:
/// - `events`: append-only log indexed by (topic, timestamp, sequence)
/// - `dead_letters`: failed deliveries after N retries
pub struct EventStore {
    conn: Mutex<Connection>,
}

impl EventStore {
    /// Open or create the store at `path`. Pass `":memory:"` for tests.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                sequence    INTEGER PRIMARY KEY AUTOINCREMENT,
                id          TEXT NOT NULL,
                topic       TEXT NOT NULL,
                timestamp   TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                payload     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_topic_ts
                ON events(topic, timestamp, sequence);
            CREATE TABLE IF NOT EXISTS dead_letters (
                id          TEXT PRIMARY KEY,
                topic       TEXT NOT NULL,
                payload     TEXT NOT NULL,
                error       TEXT NOT NULL,
                retries     INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Append an event. Returns the assigned sequence number.
    pub fn append(&self, envelope: &mut EventEnvelope) -> Result<u64, rusqlite::Error> {
        let payload_json = serde_json::to_string(&envelope.payload)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO events (id, topic, timestamp, schema_version, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                envelope.id,
                envelope.topic,
                envelope.timestamp.to_rfc3339(),
                envelope.schema_version,
                payload_json,
            ],
        )?;
        let seq = conn.last_insert_rowid() as u64;
        envelope.sequence = seq;
        debug!(seq, topic = %envelope.topic, "event persisted");
        Ok(seq)
    }

    /// Replay events for a topic starting after `last_event_id` (sequence).
    pub fn replay(
        &self,
        topic: &str,
        last_event_id: u64,
        limit: u64,
    ) -> Result<Vec<EventEnvelope>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT sequence, id, topic, timestamp, schema_version, payload
             FROM events
             WHERE topic = ?1 AND sequence > ?2
             ORDER BY sequence ASC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![topic, last_event_id as i64, limit as i64], |row| {
            let seq: i64 = row.get(0)?;
            let id: String = row.get(1)?;
            let topic: String = row.get(2)?;
            let ts: String = row.get(3)?;
            let sv: i64 = row.get(4)?;
            let payload_raw: String = row.get(5)?;
            Ok((seq, id, topic, ts, sv, payload_raw))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (seq, id, topic, ts, sv, payload_raw) = row?;
            let payload: crate::event_types::ZylosEvent = serde_json::from_str(&payload_raw)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            let timestamp = DateTime::parse_from_rfc3339(&ts)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            out.push(EventEnvelope {
                id,
                topic,
                timestamp,
                sequence: seq as u64,
                schema_version: sv as u32,
                payload,
            });
        }
        Ok(out)
    }

    /// Purge events older than `ttl_hours` for a given topic (or all topics if None).
    pub fn purge_expired(
        &self,
        ttl_hours: i64,
        topic: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let cutoff = Utc::now() - chrono::Duration::hours(ttl_hours);
        let affected = if let Some(t) = topic {
            conn.execute(
                "DELETE FROM events WHERE topic = ?1 AND timestamp < ?2",
                params![t, cutoff.to_rfc3339()],
            )?
        } else {
            conn.execute(
                "DELETE FROM events WHERE timestamp < ?1",
                params![cutoff.to_rfc3339()],
            )?
        };
        if affected > 0 {
            debug!(affected, ttl_hours, "purged expired events");
        }
        Ok(affected)
    }

    // -----------------------------------------------------------------------
    // Dead Letter Queue
    // -----------------------------------------------------------------------

    /// Move a failed delivery to the DLQ.
    pub fn enqueue_dead_letter(
        &self,
        envelope: &EventEnvelope,
        error: &str,
        retries: u32,
    ) -> Result<(), rusqlite::Error> {
        let payload_json = serde_json::to_string(&envelope.payload)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO dead_letters (id, topic, payload, error, retries, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                envelope.id,
                envelope.topic,
                payload_json,
                error,
                retries,
                Utc::now().to_rfc3339(),
            ],
        )?;
        warn!(id = %envelope.id, error, retries, "event moved to DLQ");
        Ok(())
    }

    /// List dead letters (most recent first).
    pub fn list_dead_letters(&self, limit: u32) -> Result<Vec<DeadLetterEntry>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, topic, payload, error, retries, created_at
             FROM dead_letters ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(DeadLetterEntry {
                id: row.get(0)?,
                topic: row.get(1)?,
                payload: row.get(2)?,
                error: row.get(3)?,
                retries: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    /// Remove a dead letter by id (after manual replay / ack).
    pub fn ack_dead_letter(&self, id: &str) -> Result<bool, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM dead_letters WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeadLetterEntry {
    pub id: String,
    pub topic: String,
    pub payload: String,
    pub error: String,
    pub retries: u32,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_types::{AgentStateChanged, ZylosEvent};

    #[test]
    fn test_append_and_replay() {
        let store = EventStore::open(":memory:").unwrap();
        let mut env = EventEnvelope::new(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "idle".into(),
                new_state: "running".into(),
                reason: None,
            }),
            1,
        );
        let seq = store.append(&mut env).unwrap();
        assert_eq!(seq, 1);

        let replayed = store.replay("agents.a1.state", 0, 10).unwrap();
        assert_eq!(replayed.len(), 1);
        assert_eq!(replayed[0].sequence, 1);
    }

    #[test]
    fn test_replay_from_offset() {
        let store = EventStore::open(":memory:").unwrap();
        for i in 0..5 {
            let mut env = EventEnvelope::new(
                "tasks.t1.progress",
                ZylosEvent::DagProgressChanged(crate::event_types::DagProgressChanged {
                    dag_id: "d1".into(),
                    node_id: format!("n{}", i),
                    status: "done".into(),
                    completed_nodes: i + 1,
                    total_nodes: 5,
                }),
                1,
            );
            store.append(&mut env).unwrap();
        }
        let from_3 = store.replay("tasks.t1.progress", 3, 10).unwrap();
        assert_eq!(from_3.len(), 2);
        assert_eq!(from_3[0].sequence, 4);
    }

    #[test]
    fn test_dead_letter_roundtrip() {
        let store = EventStore::open(":memory:").unwrap();
        let env = EventEnvelope::new(
            "agents.a1.state",
            ZylosEvent::AgentStateChanged(AgentStateChanged {
                agent_id: "a1".into(),
                previous_state: "idle".into(),
                new_state: "error".into(),
                reason: Some("timeout".into()),
            }),
            1,
        );
        store.enqueue_dead_letter(&env, "delivery failed", 3).unwrap();
        let dl = store.list_dead_letters(10).unwrap();
        assert_eq!(dl.len(), 1);
        assert_eq!(dl[0].retries, 3);

        assert!(store.ack_dead_letter(&env.id).unwrap());
        assert_eq!(store.list_dead_letters(10).unwrap().len(), 0);
    }
}