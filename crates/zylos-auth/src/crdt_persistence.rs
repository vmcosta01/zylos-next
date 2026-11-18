//! Persistência de estado CRDT (Yjs/yrs) em SQLite — Fase 11.9 (T11.9.6).
//!
//! Modelo de armazenamento:
//! - `crdt_documents`: um registro por documento (versão corrente, último acesso).
//! - `crdt_updates`: log *append-only* de updates incrementais (formato Yjs v1)
//!   recebidos desde o último snapshot. Garantia de durabilidade: cada update
//!   é gravado antes de ser retransmitido aos outros clientes.
//! - `crdt_snapshots`: snapshots versionados (estado completo codificado como
//!   update Yjs v1). Cada compactação gera uma nova versão; versões antigas
//!   ficam disponíveis para diff/restore na UI de resolução de conflitos.
//!
//! Garbage collection ([`CrdtStore::gc`]): remove snapshots não-correntes mais
//! antigos que a retenção (padrão 30 dias) e documentos vazios abandonados.
//! A versão corrente de cada documento nunca é removida — o GC não perde dados.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use yrs::updates::decoder::Decode;
use yrs::{Doc, GetString, ReadTxn, StateVector, Transact, Update};

/// Retenção padrão de versões obsoletas: 30 dias em milissegundos.
pub const DEFAULT_RETENTION_MS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Nome do tipo raiz `Y.Text` usado pelo cockpit para documentos de texto.
pub const TEXT_ROOT: &str = "content";

/// Erros do armazenamento CRDT.
#[derive(Debug, thiserror::Error)]
pub enum CrdtStoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("update CRDT inválido: {0}")]
    InvalidUpdate(String),
    #[error("lock envenenado")]
    Poisoned,
}

/// Metadados de uma versão (snapshot) persistida.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VersionInfo {
    pub version: i64,
    pub created_ms: i64,
    pub size_bytes: i64,
}

/// Resultado de uma rodada de garbage collection.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct GcReport {
    pub snapshots_removed: usize,
    pub documents_removed: usize,
}

/// Armazenamento SQLite para documentos CRDT.
pub struct CrdtStore {
    conn: Mutex<Connection>,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Mescla uma sequência de updates Yjs v1 em um único update (estado completo).
pub fn merge_updates(updates: &[Vec<u8>]) -> Result<Vec<u8>, CrdtStoreError> {
    if updates.is_empty() {
        return Ok(Doc::new()
            .transact()
            .encode_state_as_update_v1(&StateVector::default()));
    }
    let doc = Doc::new();
    {
        let mut txn = doc.transact_mut();
        for u in updates {
            let upd =
                Update::decode_v1(u).map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
            txn.apply_update(upd)
                .map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
        }
    }
    let state = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    Ok(state)
}

/// Decodifica um estado (update v1) e devolve o texto do root [`TEXT_ROOT`].
pub fn text_of_state(state: &[u8]) -> Result<String, CrdtStoreError> {
    let doc = Doc::new();
    let text = doc.get_or_insert_text(TEXT_ROOT);
    {
        let upd =
            Update::decode_v1(state).map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
        doc.transact_mut()
            .apply_update(upd)
            .map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
    }
    let s = text.get_string(&doc.transact());
    Ok(s)
}

impl CrdtStore {
    /// Abre (ou cria) o banco em `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CrdtStoreError> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Banco em memória (testes).
    pub fn open_in_memory() -> Result<Self, CrdtStoreError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, CrdtStoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS crdt_documents (
                 doc_id TEXT PRIMARY KEY,
                 current_version INTEGER NOT NULL DEFAULT 0,
                 created_ms INTEGER NOT NULL,
                 last_access_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS crdt_updates (
                 seq INTEGER PRIMARY KEY AUTOINCREMENT,
                 doc_id TEXT NOT NULL,
                 data BLOB NOT NULL,
                 created_ms INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_crdt_updates_doc ON crdt_updates(doc_id, seq);
             CREATE TABLE IF NOT EXISTS crdt_snapshots (
                 doc_id TEXT NOT NULL,
                 version INTEGER NOT NULL,
                 state BLOB NOT NULL,
                 created_ms INTEGER NOT NULL,
                 PRIMARY KEY (doc_id, version)
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CrdtStoreError> {
        self.conn.lock().map_err(|_| CrdtStoreError::Poisoned)
    }

    fn touch(conn: &Connection, doc_id: &str, now: i64) -> Result<(), CrdtStoreError> {
        conn.execute(
            "INSERT INTO crdt_documents (doc_id, current_version, created_ms, last_access_ms)
             VALUES (?1, 0, ?2, ?2)
             ON CONFLICT(doc_id) DO UPDATE SET last_access_ms = excluded.last_access_ms",
            params![doc_id, now],
        )?;
        Ok(())
    }

    /// Acrescenta um update incremental ao log do documento. Valida o binário
    /// antes de gravar para nunca persistir lixo.
    pub fn append_update(&self, doc_id: &str, update: &[u8]) -> Result<(), CrdtStoreError> {
        Update::decode_v1(update).map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
        let now = now_ms();
        let conn = self.lock()?;
        Self::touch(&conn, doc_id, now)?;
        conn.execute(
            "INSERT INTO crdt_updates (doc_id, data, created_ms) VALUES (?1, ?2, ?3)",
            params![doc_id, update, now],
        )?;
        Ok(())
    }

    /// Carrega o estado completo (snapshot corrente + updates pendentes) como
    /// um único update v1. `None` se o documento não existe.
    pub fn load(&self, doc_id: &str) -> Result<Option<Vec<u8>>, CrdtStoreError> {
        let conn = self.lock()?;
        let current: Option<i64> = conn
            .query_row(
                "SELECT current_version FROM crdt_documents WHERE doc_id = ?1",
                params![doc_id],
                |r| r.get(0),
            )
            .optional()?;
        let Some(version) = current else {
            return Ok(None);
        };
        let mut parts: Vec<Vec<u8>> = Vec::new();
        if version > 0 {
            let snap: Option<Vec<u8>> = conn
                .query_row(
                    "SELECT state FROM crdt_snapshots WHERE doc_id = ?1 AND version = ?2",
                    params![doc_id, version],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(s) = snap {
                parts.push(s);
            }
        }
        let mut stmt =
            conn.prepare("SELECT data FROM crdt_updates WHERE doc_id = ?1 ORDER BY seq ASC")?;
        let rows = stmt.query_map(params![doc_id], |r| r.get::<_, Vec<u8>>(0))?;
        for row in rows {
            parts.push(row?);
        }
        drop(stmt);
        Self::touch(&conn, doc_id, now_ms())?;
        drop(conn);
        merge_updates(&parts).map(Some)
    }

    /// Número de updates pendentes (não compactados) do documento.
    pub fn pending_updates(&self, doc_id: &str) -> Result<i64, CrdtStoreError> {
        let conn = self.lock()?;
        Ok(conn.query_row(
            "SELECT COUNT(*) FROM crdt_updates WHERE doc_id = ?1",
            params![doc_id],
            |r| r.get(0),
        )?)
    }

    /// Compacta: grava `state` (estado completo) como nova versão e remove os
    /// updates já incorporados. Atômico (uma transação SQLite).
    /// Retorna o número da nova versão.
    pub fn snapshot(&self, doc_id: &str, state: &[u8]) -> Result<i64, CrdtStoreError> {
        Update::decode_v1(state).map_err(|e| CrdtStoreError::InvalidUpdate(e.to_string()))?;
        let now = now_ms();
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        Self::touch(&tx, doc_id, now)?;
        let max_seq: Option<i64> = tx.query_row(
            "SELECT MAX(seq) FROM crdt_updates WHERE doc_id = ?1",
            params![doc_id],
            |r| r.get(0),
        )?;
        let version: i64 = tx.query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM crdt_snapshots WHERE doc_id = ?1",
            params![doc_id],
            |r| r.get(0),
        )?;
        tx.execute(
            "INSERT INTO crdt_snapshots (doc_id, version, state, created_ms) VALUES (?1, ?2, ?3, ?4)",
            params![doc_id, version, state, now],
        )?;
        tx.execute(
            "UPDATE crdt_documents SET current_version = ?2 WHERE doc_id = ?1",
            params![doc_id, version],
        )?;
        if let Some(max_seq) = max_seq {
            tx.execute(
                "DELETE FROM crdt_updates WHERE doc_id = ?1 AND seq <= ?2",
                params![doc_id, max_seq],
            )?;
        }
        tx.commit()?;
        Ok(version)
    }

    /// Compacta a partir do próprio log (sem réplica em memória).
    pub fn compact(&self, doc_id: &str) -> Result<Option<i64>, CrdtStoreError> {
        match self.load(doc_id)? {
            Some(state) => self.snapshot(doc_id, &state).map(Some),
            None => Ok(None),
        }
    }

    /// Lista versões (snapshots) do documento, mais recente primeiro.
    pub fn list_versions(&self, doc_id: &str) -> Result<Vec<VersionInfo>, CrdtStoreError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT version, created_ms, LENGTH(state) FROM crdt_snapshots
             WHERE doc_id = ?1 ORDER BY version DESC",
        )?;
        let rows = stmt.query_map(params![doc_id], |r| {
            Ok(VersionInfo {
                version: r.get(0)?,
                created_ms: r.get(1)?,
                size_bytes: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Estado de uma versão específica.
    pub fn load_version(
        &self,
        doc_id: &str,
        version: i64,
    ) -> Result<Option<Vec<u8>>, CrdtStoreError> {
        let conn = self.lock()?;
        Ok(conn
            .query_row(
                "SELECT state FROM crdt_snapshots WHERE doc_id = ?1 AND version = ?2",
                params![doc_id, version],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// IDs de todos os documentos persistidos.
    pub fn list_documents(&self) -> Result<Vec<String>, CrdtStoreError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT doc_id FROM crdt_documents ORDER BY doc_id")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Garbage collection com retenção explícita e relógio injetável (`now`).
    ///
    /// - remove snapshots **não-correntes** com `created_ms < now - retention_ms`;
    /// - remove documentos sem nenhum conteúdo (sem snapshot e sem updates)
    ///   cujo último acesso é anterior à retenção.
    pub fn gc_at(&self, now: i64, retention_ms: i64) -> Result<GcReport, CrdtStoreError> {
        let cutoff = now - retention_ms;
        let mut conn = self.lock()?;
        let tx = conn.transaction()?;
        let snapshots_removed = tx.execute(
            "DELETE FROM crdt_snapshots
             WHERE created_ms < ?1
               AND version <> (SELECT current_version FROM crdt_documents d
                               WHERE d.doc_id = crdt_snapshots.doc_id)",
            params![cutoff],
        )?;
        let documents_removed = tx.execute(
            "DELETE FROM crdt_documents
             WHERE last_access_ms < ?1
               AND NOT EXISTS (SELECT 1 FROM crdt_snapshots s WHERE s.doc_id = crdt_documents.doc_id)
               AND NOT EXISTS (SELECT 1 FROM crdt_updates u WHERE u.doc_id = crdt_documents.doc_id)",
            params![cutoff],
        )?;
        tx.commit()?;
        Ok(GcReport {
            snapshots_removed,
            documents_removed,
        })
    }

    /// GC com a retenção padrão de 30 dias.
    pub fn gc(&self) -> Result<GcReport, CrdtStoreError> {
        self.gc_at(now_ms(), DEFAULT_RETENTION_MS)
    }

    #[cfg(test)]
    fn backdate_snapshots(&self, doc_id: &str, created_ms: i64) {
        let conn = self.lock().unwrap();
        conn.execute(
            "UPDATE crdt_snapshots SET created_ms = ?2 WHERE doc_id = ?1",
            params![doc_id, created_ms],
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::{Text, TextRef};

    fn edit(doc: &Doc, text: &TextRef, idx: u32, s: &str) -> Vec<u8> {
        let sv = doc.transact().state_vector();
        text.insert(&mut doc.transact_mut(), idx, s);
        doc.transact().encode_diff_v1(&sv)
    }

    #[test]
    fn crdt_store_append_and_load_roundtrip() {
        let store = CrdtStore::open_in_memory().unwrap();
        let doc = Doc::new();
        let t = doc.get_or_insert_text(TEXT_ROOT);
        store
            .append_update("d1", &edit(&doc, &t, 0, "olá"))
            .unwrap();
        let end = t.len(&doc.transact());
        store
            .append_update("d1", &edit(&doc, &t, end, " mundo"))
            .unwrap();
        let state = store.load("d1").unwrap().unwrap();
        assert_eq!(text_of_state(&state).unwrap(), "olá mundo");
        assert!(store.load("inexistente").unwrap().is_none());
    }

    #[test]
    fn crdt_store_rejects_invalid_update() {
        let store = CrdtStore::open_in_memory().unwrap();
        assert!(matches!(
            store.append_update("d1", &[0xff, 0xff, 0xff]),
            Err(CrdtStoreError::InvalidUpdate(_))
        ));
    }

    #[test]
    fn crdt_store_snapshot_versions_and_compaction() {
        let store = CrdtStore::open_in_memory().unwrap();
        let doc = Doc::new();
        let t = doc.get_or_insert_text(TEXT_ROOT);
        for i in 0..20 {
            store.append_update("d", &edit(&doc, &t, i, "x")).unwrap();
        }
        assert_eq!(store.pending_updates("d").unwrap(), 20);
        assert_eq!(store.compact("d").unwrap(), Some(1));
        assert_eq!(store.pending_updates("d").unwrap(), 0);
        store.append_update("d", &edit(&doc, &t, 20, "y")).unwrap();
        assert_eq!(store.compact("d").unwrap(), Some(2));
        let versions = store.list_versions("d").unwrap();
        assert_eq!(
            versions.iter().map(|v| v.version).collect::<Vec<_>>(),
            vec![2, 1]
        );
        let v1 = store.load_version("d", 1).unwrap().unwrap();
        assert_eq!(text_of_state(&v1).unwrap(), "x".repeat(20));
        let cur = store.load("d").unwrap().unwrap();
        assert_eq!(text_of_state(&cur).unwrap(), format!("{}y", "x".repeat(20)));
    }

    #[test]
    fn crdt_store_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("crdt.db");
        let doc = Doc::new();
        let t = doc.get_or_insert_text(TEXT_ROOT);
        {
            let store = CrdtStore::open(&path).unwrap();
            store
                .append_update("skill-1", &edit(&doc, &t, 0, "abc"))
                .unwrap();
            store.compact("skill-1").unwrap();
            store
                .append_update("skill-1", &edit(&doc, &t, 3, "def"))
                .unwrap();
        }
        let store = CrdtStore::open(&path).unwrap();
        let state = store.load("skill-1").unwrap().unwrap();
        assert_eq!(text_of_state(&state).unwrap(), "abcdef");
        assert_eq!(store.list_documents().unwrap(), vec!["skill-1".to_string()]);
    }

    #[test]
    fn crdt_store_gc_removes_only_obsolete_versions_after_30_days() {
        let store = CrdtStore::open_in_memory().unwrap();
        let doc = Doc::new();
        let t = doc.get_or_insert_text(TEXT_ROOT);
        for i in 0..3 {
            store.append_update("d", &edit(&doc, &t, i, "z")).unwrap();
            store.compact("d").unwrap();
        }
        assert_eq!(store.list_versions("d").unwrap().len(), 3);
        let now = now_ms();
        // Nada a remover enquanto as versões têm menos de 30 dias.
        assert_eq!(
            store
                .gc_at(now, DEFAULT_RETENTION_MS)
                .unwrap()
                .snapshots_removed,
            0
        );
        // Envelhece todas as versões para 31 dias atrás.
        store.backdate_snapshots("d", now - DEFAULT_RETENTION_MS - 24 * 3600 * 1000);
        let report = store.gc_at(now, DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(report.snapshots_removed, 2);
        let versions = store.list_versions("d").unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, 3);
        // Estado corrente intacto após GC (zero perda de dados).
        let state = store.load("d").unwrap().unwrap();
        assert_eq!(text_of_state(&state).unwrap(), "zzz");
    }

    #[test]
    fn crdt_store_gc_removes_empty_abandoned_documents() {
        let store = CrdtStore::open_in_memory().unwrap();
        {
            let conn = store.lock().unwrap();
            CrdtStore::touch(&conn, "vazio", 0).unwrap();
        }
        let report = store.gc_at(now_ms(), DEFAULT_RETENTION_MS).unwrap();
        assert_eq!(report.documents_removed, 1);
        assert!(store.list_documents().unwrap().is_empty());
    }

    #[test]
    fn crdt_merge_updates_is_order_independent() {
        let a = Doc::with_client_id(1);
        let b = Doc::with_client_id(2);
        let ta = a.get_or_insert_text(TEXT_ROOT);
        let tb = b.get_or_insert_text(TEXT_ROOT);
        let ua = edit(&a, &ta, 0, "AAA");
        let ub = edit(&b, &tb, 0, "BBB");
        let m1 = text_of_state(&merge_updates(&[ua.clone(), ub.clone()]).unwrap()).unwrap();
        let m2 = text_of_state(&merge_updates(&[ub, ua]).unwrap()).unwrap();
        assert_eq!(m1, m2);
        assert_eq!(m1.len(), 6);
    }
}
