//! Persistência SQLite da denylist de JWTs revogados — F2.
//!
//! Sobrevive a restarts do gateway. A tabela armazena `(jti, exp)` e é
//! limpa periodicamente via [`RevocationStore::purge_expired`].
//!
//! Caminho padrão: `data/revoked.db` (env `ZYLOS_REVOKED_DB`).

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Erros do store de revogação.
#[derive(Debug, thiserror::Error)]
pub enum RevocationStoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("lock envenenado")]
    Poisoned,
}

type Result<T> = std::result::Result<T, RevocationStoreError>;

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Store SQLite persistente para JWTs revogados.
pub struct RevocationStore {
    conn: Mutex<Connection>,
}

impl RevocationStore {
    /// Abre (ou cria) o banco em `path`. Cria diretórios pais se necessário.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// Store em memória (para testes).
    pub fn in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS revoked_jtis (
                 jti TEXT PRIMARY KEY,
                 exp INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_revoked_exp ON revoked_jtis(exp);",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Insere ou atualiza um JTI revogado com sua expiração.
    pub fn revoke(&self, jti: &str, exp: i64) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| RevocationStoreError::Poisoned)?;
        conn.execute(
            "INSERT INTO revoked_jtis (jti, exp) VALUES (?1, ?2)
             ON CONFLICT(jti) DO UPDATE SET exp = excluded.exp",
            params![jti, exp],
        )?;
        Ok(())
    }

    /// Verifica se um JTI está na denylist persistente.
    pub fn is_revoked(&self, jti: &str) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| RevocationStoreError::Poisoned)?;
        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM revoked_jtis WHERE jti = ?1 AND exp > ?2",
                params![jti, now_ts()],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    /// Remove entradas expiradas. Retorna quantidade removida.
    pub fn purge_expired(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| RevocationStoreError::Poisoned)?;
        let removed = conn.execute(
            "DELETE FROM revoked_jtis WHERE exp <= ?1",
            params![now_ts()],
        )?;
        Ok(removed)
    }

    /// Conta entradas ativas (não expiradas). Útil para métricas/testes.
    pub fn count_active(&self) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| RevocationStoreError::Poisoned)?;
        let n: usize = conn.query_row(
            "SELECT COUNT(*) FROM revoked_jtis WHERE exp > ?1",
            params![now_ts()],
            |row| row.get(0),
        )?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revoke_and_check_persists() {
        let store = RevocationStore::in_memory().unwrap();
        let future = now_ts() + 3600;
        store.revoke("jti-abc", future).unwrap();
        assert!(store.is_revoked("jti-abc").unwrap());
        assert!(!store.is_revoked("jti-xyz").unwrap());
    }

    #[test]
    fn expired_entry_not_found() {
        let store = RevocationStore::in_memory().unwrap();
        let past = now_ts() - 100;
        store.revoke("jti-old", past).unwrap();
        assert!(!store.is_revoked("jti-old").unwrap());
    }

    #[test]
    fn purge_removes_expired_only() {
        let store = RevocationStore::in_memory().unwrap();
        store.revoke("jti-expired", now_ts() - 10).unwrap();
        store.revoke("jti-valid", now_ts() + 3600).unwrap();
        let removed = store.purge_expired().unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.count_active().unwrap(), 1);
    }

    #[test]
    fn reopen_preserves_data() {
        let dir = std::env::temp_dir().join("zylos-revocation-test");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("revoked.db");

        {
            let store = RevocationStore::open(&path).unwrap();
            store.revoke("jti-durable", now_ts() + 7200).unwrap();
        }
        // Reabre — dado deve persistir.
        {
            let store = RevocationStore::open(&path).unwrap();
            assert!(store.is_revoked("jti-durable").unwrap());
            assert_eq!(store.count_active().unwrap(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
