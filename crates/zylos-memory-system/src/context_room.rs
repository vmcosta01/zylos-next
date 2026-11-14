//! # ContextRoom — Bundling semântico de contexto (T8.2.3)
//!
//! Agrupa fontes, decisões, tasks e entradas de memória em uma unidade
//! coesa ("sala de contexto") que pode ser versionada, serializada e
//! compartilhada entre agentes dentro do mesmo tenant.
//!
//! ## Persistência
//! Tabela SQLite `context_rooms` com JSON column para o bundle completo.
//! Cada room tem isolamento por `tenant_id`.
//!
//! ## Snapshot/Versioning
//! Cada chamada a `snapshot()` cria um ponto-in-time imutável do room.
//! Snapshots são armazenados na tabela `context_room_snapshots`.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::{MemoryError, Result};

/// Identificador único de um ContextRoom.
pub type RoomId = String;

/// Identificador de snapshot.
pub type SnapshotId = String;

/// Referência a um nó no ContextGraph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeRef {
    pub node_id: String,
    pub label: Option<String>,
}

/// Referência a uma prova/decisão no EvidenceLedger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProofRef {
    pub proof_id: String,
    pub summary: Option<String>,
}

/// Referência a uma task no SwarmOrchestrator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskRef {
    pub task_id: String,
    pub name: Option<String>,
}

/// Entrada de memória dentro do room.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    pub entry_id: String,
    pub content_preview: String,
    pub scope: String,
}

/// Bundle completo de um ContextRoom.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RoomBundle {
    /// Fontes/documentos associados ao room.
    pub sources: Vec<NodeRef>,
    /// Decisões/provas registradas.
    pub decisions: Vec<ProofRef>,
    /// Tasks associadas ao room.
    pub tasks: Vec<TaskRef>,
    /// Entradas de memória relevantes.
    pub memory: Vec<MemoryEntry>,
}

/// Um ContextRoom persistente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextRoom {
    /// ID único do room.
    pub id: RoomId,
    /// Nome legível do room.
    pub name: String,
    /// Descrição do propósito do room.
    pub description: String,
    /// Tenant ID para isolamento multi-tenant.
    pub tenant_id: String,
    /// Bundle atual do room.
    pub bundle: RoomBundle,
    /// Metadata arbitrária.
    pub metadata: serde_json::Value,
    /// Timestamp de criação (ISO 8601).
    pub created_at: String,
    /// Timestamp da última atualização (ISO 8601).
    pub updated_at: String,
}

impl ContextRoom {
    /// Cria um novo ContextRoom vazio.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        tenant_id: impl Into<String>,
    ) -> Self {
        let now = chrono_now_stub();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            description: description.into(),
            tenant_id: tenant_id.into(),
            bundle: RoomBundle::default(),
            metadata: serde_json::Value::Null,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    /// Builder: define bundle customizado.
    pub fn with_bundle(mut self, bundle: RoomBundle) -> Self {
        self.bundle = bundle;
        self
    }

    /// Builder: anexa metadata.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Serializa o bundle para JSON.
    pub fn bundle_to_json(&self) -> std::result::Result<String, serde_json::Error> {
        serde_json::to_string(&self.bundle)
    }

    /// Desserializa bundle de JSON.
    pub fn bundle_from_json(json: &str) -> std::result::Result<RoomBundle, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Snapshot imutável de um ContextRoom.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSnapshot {
    /// ID único do snapshot.
    pub id: SnapshotId,
    /// ID do room originário.
    pub room_id: RoomId,
    /// Bundle congelado no momento do snapshot.
    pub bundle: RoomBundle,
    /// Timestamp do snapshot (ISO 8601).
    pub created_at: String,
    /// Tag opcional para identificação humana.
    pub tag: Option<String>,
}

/// Store para ContextRooms com persistência SQLite.
pub struct ContextRoomStore {
    conn: Arc<Mutex<Connection>>,
}

impl ContextRoomStore {
    /// Cria um novo store com conexão SQLite existente.
    pub async fn new(conn: Arc<Mutex<Connection>>) -> Result<Self> {
        let store = Self { conn };
        store.initialize_schema().await?;
        Ok(store)
    }

    /// Cria um store in-memory para testes.
    pub async fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.initialize_schema().await?;
        Ok(store)
    }

    async fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().await;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS context_rooms (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                tenant_id TEXT NOT NULL,
                bundle_json TEXT NOT NULL DEFAULT '{}',
                metadata TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_context_rooms_tenant ON context_rooms(tenant_id);

            CREATE TABLE IF NOT EXISTS context_room_snapshots (
                id TEXT PRIMARY KEY,
                room_id TEXT NOT NULL,
                bundle_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                tag TEXT,
                FOREIGN KEY (room_id) REFERENCES context_rooms(id)
            );
            CREATE INDEX IF NOT EXISTS idx_snapshots_room ON context_room_snapshots(room_id);",
        )?;
        Ok(())
    }

    /// Insere um novo ContextRoom.
    pub async fn insert(&self, room: &ContextRoom) -> Result<()> {
        let conn = self.conn.lock().await;
        let bundle_json = room.bundle_to_json().map_err(MemoryError::Serialization)?;
        let metadata_str =
            serde_json::to_string(&room.metadata).map_err(MemoryError::Serialization)?;

        conn.execute(
            "INSERT INTO context_rooms (id, name, description, tenant_id, bundle_json, metadata, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                room.id,
                room.name,
                room.description,
                room.tenant_id,
                bundle_json,
                metadata_str,
                room.created_at,
                room.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Busca um room por ID com isolamento de tenant.
    pub async fn get(&self, room_id: &str, tenant_id: &str) -> Result<Option<ContextRoom>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, tenant_id, bundle_json, metadata, created_at, updated_at
             FROM context_rooms WHERE id = ?1 AND tenant_id = ?2",
        )?;

        let result = stmt
            .query_row(params![room_id, tenant_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .optional()?;

        match result {
            Some((id, name, desc, tid, bundle_json, meta_str, created, updated)) => {
                let bundle = ContextRoom::bundle_from_json(&bundle_json)
                    .map_err(MemoryError::Serialization)?;
                let metadata: serde_json::Value =
                    serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
                Ok(Some(ContextRoom {
                    id,
                    name,
                    description: desc,
                    tenant_id: tid,
                    bundle,
                    metadata,
                    created_at: created,
                    updated_at: updated,
                }))
            }
            None => Ok(None),
        }
    }

    /// Lista todos os rooms de um tenant.
    pub async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<ContextRoom>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, tenant_id, bundle_json, metadata, created_at, updated_at
             FROM context_rooms WHERE tenant_id = ?1 ORDER BY updated_at DESC",
        )?;

        let rows = stmt
            .query_map(params![tenant_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let rooms = rows
            .into_iter()
            .filter_map(
                |(id, name, desc, tid, bundle_json, meta_str, created, updated)| {
                    let bundle = ContextRoom::bundle_from_json(&bundle_json).ok()?;
                    let metadata: serde_json::Value =
                        serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
                    Some(ContextRoom {
                        id,
                        name,
                        description: desc,
                        tenant_id: tid,
                        bundle,
                        metadata,
                        created_at: created,
                        updated_at: updated,
                    })
                },
            )
            .collect();

        Ok(rooms)
    }

    /// Atualiza o bundle de um room existente.
    pub async fn update_bundle(
        &self,
        room_id: &str,
        tenant_id: &str,
        bundle: &RoomBundle,
    ) -> Result<bool> {
        let conn = self.conn.lock().await;
        let bundle_json = serde_json::to_string(bundle).map_err(MemoryError::Serialization)?;
        let now = chrono_now_stub();

        let affected = conn.execute(
            "UPDATE context_rooms SET bundle_json = ?1, updated_at = ?2 WHERE id = ?3 AND tenant_id = ?4",
            params![bundle_json, now, room_id, tenant_id],
        )?;
        Ok(affected > 0)
    }

    /// Cria um snapshot imutável do room atual.
    pub async fn snapshot(
        &self,
        room_id: &str,
        tenant_id: &str,
        tag: Option<&str>,
    ) -> Result<RoomSnapshot> {
        let room = self
            .get(room_id, tenant_id)
            .await?
            .ok_or_else(|| MemoryError::NotFound(format!("Room {} not found", room_id)))?;

        let snap = RoomSnapshot {
            id: Uuid::new_v4().to_string(),
            room_id: room_id.to_string(),
            bundle: room.bundle.clone(),
            created_at: chrono_now_stub(),
            tag: tag.map(|t| t.to_string()),
        };

        let conn = self.conn.lock().await;
        let bundle_json =
            serde_json::to_string(&snap.bundle).map_err(MemoryError::Serialization)?;
        conn.execute(
            "INSERT INTO context_room_snapshots (id, room_id, bundle_json, created_at, tag)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                snap.id,
                snap.room_id,
                bundle_json,
                snap.created_at,
                snap.tag
            ],
        )?;

        Ok(snap)
    }

    /// Lista snapshots de um room.
    pub async fn list_snapshots(&self, room_id: &str) -> Result<Vec<RoomSnapshot>> {
        let conn = self.conn.lock().await;
        let mut stmt = conn.prepare(
            "SELECT id, room_id, bundle_json, created_at, tag
             FROM context_room_snapshots WHERE room_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map(params![room_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let snapshots = rows
            .into_iter()
            .filter_map(|(id, rid, bundle_json, created, tag)| {
                let bundle = ContextRoom::bundle_from_json(&bundle_json).ok()?;
                Some(RoomSnapshot {
                    id,
                    room_id: rid,
                    bundle,
                    created_at: created,
                    tag,
                })
            })
            .collect();

        Ok(snapshots)
    }

    /// Remove um room.
    pub async fn delete(&self, room_id: &str, tenant_id: &str) -> Result<bool> {
        let conn = self.conn.lock().await;
        // Remover snapshots primeiro (FK constraint)
        conn.execute(
            "DELETE FROM context_room_snapshots WHERE room_id = ?1",
            params![room_id],
        )?;
        let affected = conn.execute(
            "DELETE FROM context_rooms WHERE id = ?1 AND tenant_id = ?2",
            params![room_id, tenant_id],
        )?;
        Ok(affected > 0)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn chrono_now_stub() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}Z", duration.as_secs())
}

trait OptionalExt<T> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error>;
}

impl<T> OptionalExt<T> for std::result::Result<T, rusqlite::Error> {
    fn optional(self) -> std::result::Result<Option<T>, rusqlite::Error> {
        match self {
            Ok(val) => Ok(Some(val)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Sprint 42", "Context for sprint 42", "tenant-a").with_bundle(
            RoomBundle {
                sources: vec![NodeRef {
                    node_id: "node-1".into(),
                    label: Some("PRD".into()),
                }],
                decisions: vec![ProofRef {
                    proof_id: "proof-1".into(),
                    summary: Some("Use PostgreSQL".into()),
                }],
                tasks: vec![TaskRef {
                    task_id: "task-1".into(),
                    name: Some("Implement auth".into()),
                }],
                memory: vec![MemoryEntry {
                    entry_id: "mem-1".into(),
                    content_preview: "Auth uses JWT".into(),
                    scope: "agent".into(),
                }],
            },
        );

        store.insert(&room).await.unwrap();

        let retrieved = store.get(&room.id, "tenant-a").await.unwrap();
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert_eq!(r.name, "Sprint 42");
        assert_eq!(r.bundle.sources.len(), 1);
        assert_eq!(r.bundle.decisions.len(), 1);
        assert_eq!(r.bundle.tasks.len(), 1);
        assert_eq!(r.bundle.memory.len(), 1);
    }

    #[tokio::test]
    async fn test_tenant_isolation() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Private Room", "Secret", "tenant-a");
        store.insert(&room).await.unwrap();

        // tenant-b não deve ver room do tenant-a
        let result = store.get(&room.id, "tenant-b").await.unwrap();
        assert!(result.is_none());

        let list_b = store.list_by_tenant("tenant-b").await.unwrap();
        assert!(list_b.is_empty());
    }

    #[tokio::test]
    async fn test_update_bundle() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Updatable", "Test updates", "tenant-a");
        store.insert(&room).await.unwrap();

        let new_bundle = RoomBundle {
            sources: vec![
                NodeRef {
                    node_id: "s1".into(),
                    label: None,
                },
                NodeRef {
                    node_id: "s2".into(),
                    label: None,
                },
            ],
            ..RoomBundle::default()
        };

        let updated = store
            .update_bundle(&room.id, "tenant-a", &new_bundle)
            .await
            .unwrap();
        assert!(updated);

        let r = store.get(&room.id, "tenant-a").await.unwrap().unwrap();
        assert_eq!(r.bundle.sources.len(), 2);
    }

    #[tokio::test]
    async fn test_snapshot_and_list() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Snapshottable", "Test snapshots", "tenant-a").with_bundle(
            RoomBundle {
                sources: vec![NodeRef {
                    node_id: "v1-source".into(),
                    label: None,
                }],
                ..RoomBundle::default()
            },
        );
        store.insert(&room).await.unwrap();

        // Criar snapshot v1
        let snap1 = store
            .snapshot(&room.id, "tenant-a", Some("v1"))
            .await
            .unwrap();
        assert_eq!(snap1.bundle.sources.len(), 1);
        assert_eq!(snap1.tag, Some("v1".into()));

        // Atualizar room
        let new_bundle = RoomBundle {
            sources: vec![
                NodeRef {
                    node_id: "v2-s1".into(),
                    label: None,
                },
                NodeRef {
                    node_id: "v2-s2".into(),
                    label: None,
                },
            ],
            ..RoomBundle::default()
        };
        store
            .update_bundle(&room.id, "tenant-a", &new_bundle)
            .await
            .unwrap();

        // Criar snapshot v2
        let snap2 = store
            .snapshot(&room.id, "tenant-a", Some("v2"))
            .await
            .unwrap();
        assert_eq!(snap2.bundle.sources.len(), 2);

        // Listar snapshots
        let snaps = store.list_snapshots(&room.id).await.unwrap();
        assert_eq!(snaps.len(), 2);

        // Encontrar snapshots por tag (ordem pode variar se timestamps forem iguais)
        let snap_v1 = snaps
            .iter()
            .find(|s| s.tag == Some("v1".into()))
            .expect("v1 snapshot not found");
        let snap_v2 = snaps
            .iter()
            .find(|s| s.tag == Some("v2".into()))
            .expect("v2 snapshot not found");

        // Snapshot v1 deve preservar estado original (1 source)
        assert_eq!(snap_v1.bundle.sources.len(), 1);
        assert_eq!(snap_v1.bundle.sources[0].node_id, "v1-source");

        // Snapshot v2 deve ter estado atualizado (2 sources)
        assert_eq!(snap_v2.bundle.sources.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_cascades_snapshots() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Deletable", "Test delete", "tenant-a");
        store.insert(&room).await.unwrap();
        store.snapshot(&room.id, "tenant-a", None).await.unwrap();

        let deleted = store.delete(&room.id, "tenant-a").await.unwrap();
        assert!(deleted);

        assert!(store.get(&room.id, "tenant-a").await.unwrap().is_none());
        assert!(store.list_snapshots(&room.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_by_tenant() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        for i in 0..3 {
            let room = ContextRoom::new(format!("Room {}", i), "desc", "tenant-x");
            store.insert(&room).await.unwrap();
        }
        // Outro tenant
        let other = ContextRoom::new("Other", "desc", "tenant-y");
        store.insert(&other).await.unwrap();

        let list = store.list_by_tenant("tenant-x").await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_bundle_serialization_roundtrip() {
        let bundle = RoomBundle {
            sources: vec![NodeRef {
                node_id: "n1".into(),
                label: Some("L".into()),
            }],
            decisions: vec![ProofRef {
                proof_id: "p1".into(),
                summary: None,
            }],
            tasks: vec![TaskRef {
                task_id: "t1".into(),
                name: Some("T".into()),
            }],
            memory: vec![MemoryEntry {
                entry_id: "m1".into(),
                content_preview: "preview".into(),
                scope: "global".into(),
            }],
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let restored: RoomBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(bundle, restored);
    }

    #[tokio::test]
    async fn test_empty_room() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Empty", "No content yet", "tenant-a");
        store.insert(&room).await.unwrap();

        let r = store.get(&room.id, "tenant-a").await.unwrap().unwrap();
        assert!(r.bundle.sources.is_empty());
        assert!(r.bundle.decisions.is_empty());
        assert!(r.bundle.tasks.is_empty());
        assert!(r.bundle.memory.is_empty());
    }

    #[tokio::test]
    async fn test_room_with_metadata() {
        let store = ContextRoomStore::in_memory().await.unwrap();

        let room = ContextRoom::new("Rich", "With metadata", "tenant-a").with_metadata(
            serde_json::json!({
                "owner": "team-alpha",
                "priority": "high",
                "tags": ["sprint-42", "auth"]
            }),
        );
        store.insert(&room).await.unwrap();

        let r = store.get(&room.id, "tenant-a").await.unwrap().unwrap();
        assert_eq!(r.metadata["owner"], "team-alpha");
        assert_eq!(r.metadata["priority"], "high");
    }
}
