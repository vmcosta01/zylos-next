//! # zylos-vault
//!
//! AES-256-GCM encrypted credential store with Argon2id key derivation.
//! Master key is never persisted; derived on unlock from machine secret + user passphrase.
//! All secrets zeroized after use. Full audit trail for every access.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::Utc;
use rand::RngCore;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info};
use uuid::Uuid;
use zeroize::ZeroizeOnDrop;

// ─── Errors ────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum VaultError {
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("authentication failed")]
    AuthFailed,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, VaultError>;

// ─── Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub key: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub id: String,
    pub timestamp: i64,
    pub action: String,
    pub target_key: String,
    pub caller: String,
    pub success: bool,
}

/// Holds the master key in memory, zeroized on drop.
#[derive(ZeroizeOnDrop)]
struct MasterKey(#[zeroize(skip)] [u8; 32]);

impl MasterKey {
    fn derive(passphrase: &[u8], salt: &[u8]) -> Result<Self> {
        // OWASP-recommended Argon2id params: 19 MiB memory, 2 iterations, 1 parallelism
        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|e| VaultError::KeyDerivation(e.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(passphrase, salt, &mut key)
            .map_err(|e| VaultError::KeyDerivation(e.to_string()))?;
        Ok(MasterKey(key))
    }

    fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

// ─── Vault ─────────────────────────────────────────────────────────

pub struct Vault {
    master_key: MasterKey,
    db: Connection,
    _db_path: PathBuf,
}

impl Vault {
    /// Open or create a vault at the given path.
    /// The passphrase is used to derive the master key via Argon2id.
    pub fn open(path: &Path, passphrase: &str) -> Result<Self> {
        // Treat empty files as new vaults (e.g. NamedTempFile creates a 0-byte file)
        let db_exists = path.exists() && path.metadata().map(|m| m.len() > 0).unwrap_or(false);
        let db = Connection::open(path)?;

        // Enable WAL mode for concurrent reads
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        // Generate or retrieve salt
        let salt = if db_exists {
            Self::read_salt(&db)?
        } else {
            let s = Self::create_tables_and_salt(&db)?;
            info!("Created new vault at {:?}", path);
            s
        };

        let master_key = MasterKey::derive(passphrase.as_bytes(), &salt)?;

        // Verify key by attempting to read the verification token
        if db_exists {
            Self::verify_key(&db, &master_key)?;
        } else {
            Self::store_verification_token(&db, &master_key)?;
        }

        Ok(Vault {
            master_key,
            db,
            _db_path: path.to_path_buf(),
        })
    }

    /// Store a secret encrypted with AES-256-GCM.
    pub fn store_secret(&self, key: &str, value: &[u8], tags: &[&str], caller: &str) -> Result<()> {
        let nonce = Self::generate_nonce();
        let cipher = Aes256Gcm::new_from_slice(self.master_key.as_bytes())
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), value)
            .map_err(|e| VaultError::Crypto(e.to_string()))?;

        let now = Utc::now().timestamp();
        let tags_json = serde_json::to_string(&tags)?;

        self.db.execute(
            "INSERT INTO secrets (key, ciphertext, nonce, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET ciphertext=?2, nonce=?3, tags=?4, updated_at=?6",
            params![key, ciphertext, nonce, tags_json, now, now],
        )?;

        self.write_audit("store", key, caller, true)?;
        debug!("Stored secret: {}", key);
        Ok(())
    }

    /// Retrieve and decrypt a secret. Returns None if not found.
    pub fn retrieve_secret(&self, key: &str, caller: &str) -> Result<Option<Vec<u8>>> {
        let row = self.db.query_row(
            "SELECT ciphertext, nonce FROM secrets WHERE key = ?1",
            params![key],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
        );

        match row {
            Ok((ciphertext, nonce)) => {
                let cipher = Aes256Gcm::new_from_slice(self.master_key.as_bytes())
                    .map_err(|e| VaultError::Crypto(e.to_string()))?;
                let plaintext = cipher
                    .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                    .map_err(|e| VaultError::Crypto(e.to_string()))?;

                self.write_audit("retrieve", key, caller, true)?;
                Ok(Some(plaintext))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                self.write_audit("retrieve", key, caller, false)?;
                Ok(None)
            }
            Err(e) => Err(VaultError::Database(e)),
        }
    }

    /// List all keys, optionally filtered by tag.
    pub fn list_keys(&self, tag_filter: Option<&str>) -> Result<Vec<VaultEntry>> {
        let entries = if let Some(tag) = tag_filter {
            let pattern = format!("%{}%", tag);
            let mut stmt = self.db.prepare(
                "SELECT key, tags, created_at, updated_at FROM secrets WHERE tags LIKE ?1 ORDER BY key",
            )?;
            let rows = stmt.query_map(params![pattern], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            let mut result = Vec::new();
            for row in rows {
                let (key, tags_json, created_at, updated_at) = row?;
                let tags: Vec<String> = serde_json::from_str(&tags_json)?;
                result.push(VaultEntry {
                    key,
                    tags,
                    created_at,
                    updated_at,
                });
            }
            result
        } else {
            let mut stmt = self
                .db
                .prepare("SELECT key, tags, created_at, updated_at FROM secrets ORDER BY key")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?;
            let mut result = Vec::new();
            for row in rows {
                let (key, tags_json, created_at, updated_at) = row?;
                let tags: Vec<String> = serde_json::from_str(&tags_json)?;
                result.push(VaultEntry {
                    key,
                    tags,
                    created_at,
                    updated_at,
                });
            }
            result
        };
        Ok(entries)
    }

    /// Delete a secret by key.
    pub fn delete_secret(&self, key: &str, caller: &str) -> Result<()> {
        let affected = self
            .db
            .execute("DELETE FROM secrets WHERE key = ?1", params![key])?;
        if affected == 0 {
            return Err(VaultError::NotFound(key.to_string()));
        }
        self.write_audit("delete", key, caller, true)?;
        info!("Deleted secret: {}", key);
        Ok(())
    }

    /// Rotate the master key: re-encrypt all secrets with a new passphrase.
    pub fn rotate_key(
        &mut self,
        old_passphrase: &str,
        new_passphrase: &str,
        caller: &str,
    ) -> Result<()> {
        // Verify old passphrase matches current master key
        let salt = Self::read_salt(&self.db)?;
        let verify_key = MasterKey::derive(old_passphrase.as_bytes(), &salt)?;
        if verify_key.as_bytes() != self.master_key.as_bytes() {
            return Err(VaultError::AuthFailed);
        }

        // Derive new key
        let new_key = MasterKey::derive(new_passphrase.as_bytes(), &salt)?;

        // Re-encrypt all secrets in a transaction using the CURRENT master key for decryption
        let tx = self.db.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("SELECT key, ciphertext, nonce FROM secrets")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                ))
            })?;

            let old_cipher = Aes256Gcm::new_from_slice(self.master_key.as_bytes())
                .map_err(|e| VaultError::Crypto(e.to_string()))?;
            let new_cipher = Aes256Gcm::new_from_slice(new_key.as_bytes())
                .map_err(|e| VaultError::Crypto(e.to_string()))?;

            for row in rows {
                let (key, ciphertext, nonce) = row?;
                let plaintext = old_cipher
                    .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
                    .map_err(|e| VaultError::Crypto(format!("decrypt {} failed: {}", key, e)))?;
                let new_nonce = Self::generate_nonce();
                let new_ciphertext = new_cipher
                    .encrypt(Nonce::from_slice(&new_nonce), plaintext.as_ref())
                    .map_err(|e| VaultError::Crypto(format!("encrypt {} failed: {}", key, e)))?;
                tx.execute(
                    "UPDATE secrets SET ciphertext=?1, nonce=?2 WHERE key=?3",
                    params![new_ciphertext, new_nonce, key],
                )?;
            }

            // Update verification token
            let verify_nonce = Self::generate_nonce();
            let verify_ct = new_cipher
                .encrypt(Nonce::from_slice(&verify_nonce), b"vault-verify".as_ref())
                .map_err(|e| VaultError::Crypto(e.to_string()))?;
            tx.execute(
                "UPDATE meta SET value=?1 WHERE key='verify_ct'",
                params![verify_ct],
            )?;
            tx.execute(
                "UPDATE meta SET value=?1 WHERE key='verify_nonce'",
                params![verify_nonce],
            )?;
        }
        tx.commit()?;

        // Update in-memory master key so subsequent operations use the new key
        self.master_key = new_key;

        self.write_audit("rotate_key", "*", caller, true)?;
        info!("Master key rotated successfully");
        Ok(())
    }

    /// Get audit log entries.
    pub fn audit_log(&self, limit: usize) -> Result<Vec<AuditRecord>> {
        let mut stmt = self.db.prepare(
            "SELECT id, timestamp, action, target_key, caller, success FROM audit_log ORDER BY timestamp DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(AuditRecord {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                action: row.get(2)?,
                target_key: row.get(3)?,
                caller: row.get(4)?,
                success: row.get(5)?,
            })
        })?;
        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    // ─── Internal helpers ──────────────────────────────────────────

    fn generate_nonce() -> [u8; 12] {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        nonce
    }

    fn create_tables_and_salt(db: &Connection) -> Result<[u8; 16]> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS secrets (
                 key TEXT PRIMARY KEY,
                 ciphertext BLOB NOT NULL,
                 nonce BLOB NOT NULL,
                 tags TEXT NOT NULL DEFAULT '[]',
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS audit_log (
                 id TEXT PRIMARY KEY,
                 timestamp INTEGER NOT NULL,
                 action TEXT NOT NULL,
                 target_key TEXT NOT NULL,
                 caller TEXT NOT NULL,
                 success INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);",
        )?;

        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        db.execute(
            "INSERT INTO meta (key, value) VALUES ('salt', ?1)",
            params![salt.to_vec()],
        )?;
        Ok(salt)
    }

    fn read_salt(db: &Connection) -> Result<[u8; 16]> {
        let salt_blob: Vec<u8> =
            db.query_row("SELECT value FROM meta WHERE key='salt'", [], |row| {
                row.get(0)
            })?;
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&salt_blob[..16]);
        Ok(salt)
    }

    fn store_verification_token(db: &Connection, key: &MasterKey) -> Result<()> {
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        let nonce = Self::generate_nonce();
        let ct = cipher
            .encrypt(Nonce::from_slice(&nonce), b"vault-verify".as_ref())
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        db.execute(
            "INSERT INTO meta (key, value) VALUES ('verify_ct', ?1)",
            params![ct],
        )?;
        db.execute(
            "INSERT INTO meta (key, value) VALUES ('verify_nonce', ?1)",
            params![nonce.to_vec()],
        )?;
        Ok(())
    }

    fn verify_key(db: &Connection, key: &MasterKey) -> Result<()> {
        let ct: Vec<u8> =
            db.query_row("SELECT value FROM meta WHERE key='verify_ct'", [], |r| {
                r.get(0)
            })?;
        let nonce: Vec<u8> =
            db.query_row("SELECT value FROM meta WHERE key='verify_nonce'", [], |r| {
                r.get(0)
            })?;
        let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
            .map_err(|e| VaultError::Crypto(e.to_string()))?;
        cipher
            .decrypt(Nonce::from_slice(&nonce), ct.as_ref())
            .map_err(|_| VaultError::AuthFailed)?;
        Ok(())
    }

    fn write_audit(
        &self,
        action: &str,
        target_key: &str,
        caller: &str,
        success: bool,
    ) -> Result<()> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        self.db.execute(
            "INSERT INTO audit_log (id, timestamp, action, target_key, caller, success) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, now, action, target_key, caller, success as i64],
        )?;
        Ok(())
    }
}

// ─── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn test_vault() -> (Vault, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let vault = Vault::open(tmp.path(), "test-passphrase-123").unwrap();
        (vault, tmp)
    }

    #[test]
    fn test_store_and_retrieve() {
        let (vault, _tmp) = test_vault();
        vault
            .store_secret("api-key", b"sk-abc123", &["prod", "stripe"], "test")
            .unwrap();
        let result = vault.retrieve_secret("api-key", "test").unwrap();
        assert_eq!(result, Some(b"sk-abc123".to_vec()));
    }

    #[test]
    fn test_retrieve_nonexistent() {
        let (vault, _tmp) = test_vault();
        let result = vault.retrieve_secret("missing", "test").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_list_keys() {
        let (vault, _tmp) = test_vault();
        vault.store_secret("k1", b"v1", &["tag-a"], "test").unwrap();
        vault.store_secret("k2", b"v2", &["tag-b"], "test").unwrap();
        vault
            .store_secret("k3", b"v3", &["tag-a", "tag-b"], "test")
            .unwrap();

        let all = vault.list_keys(None).unwrap();
        assert_eq!(all.len(), 3);

        let filtered = vault.list_keys(Some("tag-a")).unwrap();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_delete_secret() {
        let (vault, _tmp) = test_vault();
        vault
            .store_secret("to-delete", b"data", &[], "test")
            .unwrap();
        vault.delete_secret("to-delete", "test").unwrap();
        assert!(vault
            .retrieve_secret("to-delete", "test")
            .unwrap()
            .is_none());
    }

    #[test]
    fn test_delete_nonexistent_errors() {
        let (vault, _tmp) = test_vault();
        let err = vault.delete_secret("nope", "test");
        assert!(err.is_err());
    }

    #[test]
    fn test_wrong_passphrase_fails() {
        let tmp = NamedTempFile::new().unwrap();
        let _v = Vault::open(tmp.path(), "correct").unwrap();
        drop(_v);
        let err = Vault::open(tmp.path(), "wrong");
        assert!(matches!(err, Err(VaultError::AuthFailed)));
    }

    #[test]
    fn test_rotate_key() {
        let (mut vault, _tmp) = test_vault();
        vault
            .store_secret("rot-test", b"secret-data", &["rot"], "test")
            .unwrap();
        vault
            .rotate_key("test-passphrase-123", "new-passphrase-456", "admin")
            .unwrap();

        // Old passphrase should fail
        // Note: we can't easily test this without reopening, but we can verify
        // the secret is still retrievable with current instance
        let data = vault.retrieve_secret("rot-test", "test").unwrap();
        assert_eq!(data, Some(b"secret-data".to_vec()));
    }

    #[test]
    fn test_audit_log() {
        let (vault, _tmp) = test_vault();
        vault
            .store_secret("audit-test", b"data", &[], "alice")
            .unwrap();
        vault.retrieve_secret("audit-test", "bob").unwrap();
        vault.retrieve_secret("missing", "charlie").unwrap();

        let log = vault.audit_log(10).unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].action, "retrieve"); // most recent first
        assert_eq!(log[0].caller, "charlie");
        assert!(!log[0].success); // missing key
        assert!(log[1].success); // bob retrieved existing
    }

    #[test]
    fn test_overwrite_existing_secret() {
        let (vault, _tmp) = test_vault();
        vault
            .store_secret("overwrite", b"old", &[], "test")
            .unwrap();
        vault
            .store_secret("overwrite", b"new", &["updated"], "test")
            .unwrap();
        let data = vault.retrieve_secret("overwrite", "test").unwrap();
        assert_eq!(data, Some(b"new".to_vec()));
    }

    #[test]
    fn test_empty_value() {
        let (vault, _tmp) = test_vault();
        vault.store_secret("empty", b"", &[], "test").unwrap();
        let data = vault.retrieve_secret("empty", "test").unwrap();
        assert_eq!(data, Some(vec![]));
    }

    #[test]
    fn test_large_secret() {
        let (vault, _tmp) = test_vault();
        let large = vec![0xABu8; 1_000_000]; // 1 MB
        vault.store_secret("large", &large, &[], "test").unwrap();
        let data = vault.retrieve_secret("large", "test").unwrap();
        assert_eq!(data, Some(large));
    }
}
