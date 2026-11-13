//! # zylos-evidence-ledger
//!
//! Tamper-evident evidence ledger for zylos-gateway:
//! - Append-only log of signed evidence entries
//! - Merkle tree root computation for batch integrity verification
//! - Ed25519 cryptographic signing and verification
//! - Chain linking via previous-entry hash for sequential integrity
//! - SQLite WAL-backed persistence with export/import capability

use chrono::{DateTime, Utc};
use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::Row;
use std::str::FromStr;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

/// A single evidence entry in the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Unique identifier.
    pub id: String,
    /// Monotonic sequence number (0-indexed).
    pub sequence: u64,
    /// Type of evidence (e.g., "tool_output", "agent_decision", "audit_event").
    pub evidence_type: String,
    /// The actor that produced this evidence.
    pub actor: String,
    /// Human-readable description.
    pub description: String,
    /// The actual evidence payload (JSON).
    pub payload: serde_json::Value,
    /// SHA-256 hash of the canonical payload bytes.
    pub payload_hash: String,
    /// SHA-256 hash of the previous entry's signature (chain link).
    pub previous_hash: String,
    /// Combined hash: SHA-256(payload_hash || previous_hash || sequence).
    pub entry_hash: String,
    /// Ed25519 signature over entry_hash.
    pub signature: String,
    /// Hex-encoded public key of the signer.
    pub signer_public_key: String,
    /// Timestamp of creation.
    pub timestamp: DateTime<Utc>,
    /// Optional metadata tags.
    pub tags: Vec<String>,
}

impl LedgerEntry {
    /// Computes the canonical payload hash from the evidence payload.
    pub fn compute_payload_hash(payload: &serde_json::Value) -> String {
        let canonical = serde_json::to_string(payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Computes the combined entry hash from payload_hash, previous_hash, and sequence.
    pub fn compute_entry_hash(payload_hash: &str, previous_hash: &str, sequence: u64) -> String {
        let mut hasher = Sha256::new();
        hasher.update(payload_hash.as_bytes());
        hasher.update(b"\x00");
        hasher.update(previous_hash.as_bytes());
        hasher.update(b"\x00");
        hasher.update(sequence.to_le_bytes());
        hex::encode(hasher.finalize())
    }
}

/// A Merkle proof for verifying an entry's inclusion in a batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// ID of the entry this proof covers.
    pub entry_id: String,
    /// Hash of the entry being proved.
    pub entry_hash: String,
    /// Position of the entry in the ordered hash list.
    pub leaf_index: usize,
    /// Sibling hashes from leaf to root.
    pub path: Vec<MerklePathNode>,
    /// Expected Merkle root this proof validates against.
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A single node in a Merkle proof path.
pub struct MerklePathNode {
    /// Sibling hash at this level of the tree.
    pub hash: String,
    /// Whether this sibling is on the left or right.
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
/// Direction of a sibling node in a Merkle proof path.
pub enum Direction {
    Left,
    Right,
}

/// Verification result for a ledger entry or batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether all checks passed.
    pub valid: bool,
    /// ID of the verified entry.
    pub entry_id: String,
    /// Individual check results.
    pub checks: Vec<VerificationCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Result of a single verification check.
pub struct VerificationCheck {
    /// Name of the check (e.g. "payload_hash", "signature").
    pub name: String,
    /// Whether this check passed.
    pub passed: bool,
    /// Human-readable detail when the check fails.
    pub detail: Option<String>,
}

/// Request to append a new entry to the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendRequest {
    /// Category of evidence (e.g. "tool_output", "agent_decision").
    pub evidence_type: String,
    /// Identity of the actor that produced this evidence.
    pub actor: String,
    /// Human-readable description of the evidence.
    pub description: String,
    /// The actual evidence data as JSON.
    pub payload: serde_json::Value,
    /// Optional tags for filtering and grouping.
    pub tags: Vec<String>,
}

/// Statistics about the ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerStats {
    /// Total number of entries in the ledger.
    pub total_entries: u64,
    /// Sequence number of the most recent entry, if any.
    pub latest_sequence: Option<u64>,
    /// Entry hash of the most recent entry, if any.
    pub latest_entry_hash: Option<String>,
    /// Current Merkle root over all entries, if any.
    pub merkle_root: Option<String>,
    /// Breakdown of entries by evidence_type.
    pub evidence_type_counts: Vec<(String, u64)>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
/// Errors produced by evidence ledger operations.
pub enum LedgerError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Signing error: {0}")]
    Signing(String),
    #[error("Verification failed: {0}")]
    VerificationFailed(String),
    #[error("Entry not found: {0}")]
    NotFound(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Internal: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Key Pair Management
// ---------------------------------------------------------------------------

/// Manages Ed25519 signing keys for the ledger.
pub struct LedgerKeys {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl LedgerKeys {
    /// Generates a new random keypair.
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        Self {
            signing_key,
            verifying_key,
        }
    }

    /// Restores from hex-encoded secret key bytes.
    pub fn from_secret_hex(hex_str: &str) -> Result<Self, LedgerError> {
        let bytes = hex::decode(hex_str)
            .map_err(|e| LedgerError::InvalidKey(format!("Invalid hex: {}", e)))?;
        if bytes.len() != 32 {
            return Err(LedgerError::InvalidKey(format!(
                "Expected 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        let signing_key = SigningKey::from_bytes(&key_bytes);
        let verifying_key = signing_key.verifying_key();
        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Returns the hex-encoded secret key.
    pub fn secret_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }

    /// Returns the hex-encoded public key.
    pub fn public_hex(&self) -> String {
        hex::encode(self.verifying_key.to_bytes())
    }

    /// Signs a message and returns the hex-encoded signature.
    pub fn sign(&self, message: &[u8]) -> String {
        let sig = self.signing_key.sign(message);
        hex::encode(sig.to_bytes())
    }

    /// Verifies a hex-encoded signature against a message.
    pub fn verify(&self, message: &[u8], signature_hex: &str) -> Result<bool, LedgerError> {
        let sig_bytes = hex::decode(signature_hex)
            .map_err(|e| LedgerError::InvalidKey(format!("Invalid signature hex: {}", e)))?;
        if sig_bytes.len() != 64 {
            return Err(LedgerError::InvalidKey(format!(
                "Expected 64-byte signature, got {}",
                sig_bytes.len()
            )));
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);
        Ok(self.verifying_key.verify(message, &signature).is_ok())
    }

    /// Verifies using an arbitrary public key (for cross-ledger verification).
    pub fn verify_with_public_key(
        public_key_hex: &str,
        message: &[u8],
        signature_hex: &str,
    ) -> Result<bool, LedgerError> {
        let pk_bytes = hex::decode(public_key_hex)
            .map_err(|e| LedgerError::InvalidKey(format!("Invalid public key hex: {}", e)))?;
        if pk_bytes.len() != 32 {
            return Err(LedgerError::InvalidKey(format!(
                "Expected 32-byte public key, got {}",
                pk_bytes.len()
            )));
        }
        let mut pk_arr = [0u8; 32];
        pk_arr.copy_from_slice(&pk_bytes);
        let verifying_key = VerifyingKey::from_bytes(&pk_arr)
            .map_err(|e| LedgerError::InvalidKey(format!("Invalid public key: {}", e)))?;

        let sig_bytes = hex::decode(signature_hex)
            .map_err(|e| LedgerError::InvalidKey(format!("Invalid signature hex: {}", e)))?;
        if sig_bytes.len() != 64 {
            return Err(LedgerError::InvalidKey(format!(
                "Expected 64-byte signature, got {}",
                sig_bytes.len()
            )));
        }
        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

        Ok(verifying_key.verify(message, &signature).is_ok())
    }
}

// ---------------------------------------------------------------------------
// Evidence Ledger
// ---------------------------------------------------------------------------

/// Append-only tamper-evident ledger backed by SQLite WAL.
#[derive(Clone)]
pub struct EvidenceLedger {
    pool: SqlitePool,
}

impl EvidenceLedger {
    /// Opens or creates the evidence ledger at the given path.
    pub async fn open(path: &str) -> Result<Self, LedgerError> {
        let options = SqliteConnectOptions::from_str(path)?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePool::connect_with(options).await?;
        let ledger = Self { pool };
        ledger.run_migrations().await?;
        Ok(ledger)
    }

    async fn run_migrations(&self) -> Result<(), LedgerError> {
            sqlx::query("CREATE TABLE IF NOT EXISTS ledger_entries (id TEXT PRIMARY KEY, sequence INTEGER NOT NULL UNIQUE, evidence_type TEXT NOT NULL, actor TEXT NOT NULL, description TEXT NOT NULL DEFAULT '', payload TEXT NOT NULL, payload_hash TEXT NOT NULL, previous_hash TEXT NOT NULL, entry_hash TEXT NOT NULL, signature TEXT NOT NULL, signer_public_key TEXT NOT NULL, timestamp TEXT NOT NULL, tags TEXT NOT NULL DEFAULT '[]')")
                .execute(&self.pool).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_sequence ON ledger_entries(sequence)")
                .execute(&self.pool).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_type ON ledger_entries(evidence_type)")
                .execute(&self.pool).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_actor ON ledger_entries(actor)")
                .execute(&self.pool).await?;
            sqlx::query("CREATE INDEX IF NOT EXISTS idx_ledger_timestamp ON ledger_entries(timestamp)")
                .execute(&self.pool).await?;
            Ok(())
        }

    /// Appends a new signed entry to the ledger.
    pub async fn append(
        &self,
        request: &AppendRequest,
        keys: &LedgerKeys,
    ) -> Result<LedgerEntry, LedgerError> {
        // Get next sequence and previous hash
        let (next_seq, prev_hash) = self.next_sequence_and_prev_hash().await?;

        let payload_hash = LedgerEntry::compute_payload_hash(&request.payload);
        let entry_hash = LedgerEntry::compute_entry_hash(&payload_hash, &prev_hash, next_seq);
        let signature = keys.sign(entry_hash.as_bytes());

        let entry = LedgerEntry {
            id: Uuid::new_v4().to_string(),
            sequence: next_seq,
            evidence_type: request.evidence_type.clone(),
            actor: request.actor.clone(),
            description: request.description.clone(),
            payload: request.payload.clone(),
            payload_hash,
            previous_hash: prev_hash,
            entry_hash,
            signature,
            signer_public_key: keys.public_hex(),
            timestamp: Utc::now(),
            tags: request.tags.clone(),
        };

        let tags_json = serde_json::to_string(&entry.tags)?;
        let payload_json = serde_json::to_string(&entry.payload)?;

        sqlx::query(
            r#"INSERT INTO ledger_entries
                (id, sequence, evidence_type, actor, description, payload, payload_hash,
                 previous_hash, entry_hash, signature, signer_public_key, timestamp, tags)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&entry.id)
        .bind(entry.sequence as i64)
        .bind(&entry.evidence_type)
        .bind(&entry.actor)
        .bind(&entry.description)
        .bind(&payload_json)
        .bind(&entry.payload_hash)
        .bind(&entry.previous_hash)
        .bind(&entry.entry_hash)
        .bind(&entry.signature)
        .bind(&entry.signer_public_key)
        .bind(entry.timestamp.to_rfc3339())
        .bind(&tags_json)
        .execute(&self.pool)
        .await?;

        Ok(entry)
    }

    /// Retrieves an entry by ID.
    pub async fn get_entry(&self, id: &str) -> Result<Option<LedgerEntry>, LedgerError> {
        let row = sqlx::query("SELECT * FROM ledger_entries WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_entry(&r))),
            None => Ok(None),
        }
    }

    /// Retrieves an entry by sequence number.
    pub async fn get_by_sequence(&self, seq: u64) -> Result<Option<LedgerEntry>, LedgerError> {
        let row = sqlx::query("SELECT * FROM ledger_entries WHERE sequence = ?")
            .bind(seq as i64)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(r) => Ok(Some(row_to_entry(&r))),
            None => Ok(None),
        }
    }

    /// Lists entries in sequence order with optional pagination.
    pub async fn list_entries(
        &self,
        offset: u64,
        limit: u64,
    ) -> Result<Vec<LedgerEntry>, LedgerError> {
        let rows = sqlx::query(
            "SELECT * FROM ledger_entries ORDER BY sequence ASC LIMIT ? OFFSET ?",
        )
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(row_to_entry).collect())
    }

    /// Verifies a single entry's signature and chain link.
    pub async fn verify_entry(&self, id: &str) -> Result<VerificationResult, LedgerError> {
        let entry = self
            .get_entry(id)
            .await?
            .ok_or_else(|| LedgerError::NotFound(id.to_string()))?;

        let mut checks = Vec::new();

        // Check 1: payload hash matches
        let expected_payload_hash = LedgerEntry::compute_payload_hash(&entry.payload);
        let payload_hash_ok = expected_payload_hash == entry.payload_hash;
        checks.push(VerificationCheck {
            name: "payload_hash".into(),
            passed: payload_hash_ok,
            detail: if payload_hash_ok {
                None
            } else {
                Some(format!(
                    "expected {}, got {}",
                    expected_payload_hash, entry.payload_hash
                ))
            },
        });

        // Check 2: entry hash matches
        let expected_entry_hash = LedgerEntry::compute_entry_hash(
            &entry.payload_hash,
            &entry.previous_hash,
            entry.sequence,
        );
        let entry_hash_ok = expected_entry_hash == entry.entry_hash;
        checks.push(VerificationCheck {
            name: "entry_hash".into(),
            passed: entry_hash_ok,
            detail: if entry_hash_ok {
                None
            } else {
                Some(format!(
                    "expected {}, got {}",
                    expected_entry_hash, entry.entry_hash
                ))
            },
        });

        // Check 3: signature verifies
        let sig_ok = LedgerKeys::verify_with_public_key(
            &entry.signer_public_key,
            entry.entry_hash.as_bytes(),
            &entry.signature,
        )
        .unwrap_or(false);
        checks.push(VerificationCheck {
            name: "signature".into(),
            passed: sig_ok,
            detail: if sig_ok {
                None
            } else {
                Some("Ed25519 signature verification failed".into())
            },
        });

        // Check 4: chain link (previous_hash matches predecessor's entry_hash)
        let chain_ok = if entry.sequence == 0 {
            entry.previous_hash == "0".repeat(64)
        } else {
            match self.get_by_sequence(entry.sequence - 1).await? {
                Some(prev) => prev.entry_hash == entry.previous_hash,
                None => false,
            }
        };
        checks.push(VerificationCheck {
            name: "chain_link".into(),
            passed: chain_ok,
            detail: if chain_ok {
                None
            } else {
                Some("Previous hash does not match predecessor entry hash".into())
            },
        });

        let all_passed = checks.iter().all(|c| c.passed);
        Ok(VerificationResult {
            valid: all_passed,
            entry_id: id.to_string(),
            checks,
        })
    }

    /// Verifies the entire chain from genesis to the latest entry.
    pub async fn verify_full_chain(&self) -> Result<Vec<VerificationResult>, LedgerError> {
        let entries = self.list_entries(0, u64::MAX).await?;
        let mut results = Vec::new();

        for entry in &entries {
            let result = self.verify_entry(&entry.id).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Computes the Merkle root over all entry hashes.
    pub async fn merkle_root(&self) -> Result<Option<String>, LedgerError> {
        let hashes: Vec<String> =
            sqlx::query_scalar("SELECT entry_hash FROM ledger_entries ORDER BY sequence ASC")
                .fetch_all(&self.pool)
                .await?;

        if hashes.is_empty() {
            return Ok(None);
        }

        Ok(Some(compute_merkle_root(&hashes)))
    }

    /// Generates a Merkle proof for a specific entry.
    pub async fn merkle_proof(&self, id: &str) -> Result<MerkleProof, LedgerError> {
        let entry = self
            .get_entry(id)
            .await?
            .ok_or_else(|| LedgerError::NotFound(id.to_string()))?;

        let hashes: Vec<String> =
            sqlx::query_scalar("SELECT entry_hash FROM ledger_entries ORDER BY sequence ASC")
                .fetch_all(&self.pool)
                .await?;

        let leaf_index = hashes
            .iter()
            .position(|h| h == &entry.entry_hash)
            .ok_or_else(|| LedgerError::NotFound("Entry hash not found in ledger".into()))?;

        let path = build_merkle_proof_path(&hashes, leaf_index);
        let root = compute_merkle_root(&hashes);

        Ok(MerkleProof {
            entry_id: id.to_string(),
            entry_hash: entry.entry_hash.clone(),
            leaf_index,
            path,
            root,
        })
    }

    /// Returns aggregate statistics.
    pub async fn stats(&self) -> Result<LedgerStats, LedgerError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ledger_entries")
            .fetch_one(&self.pool)
            .await?;

        let latest: Option<(i64, String)> = sqlx::query(
            "SELECT sequence, entry_hash FROM ledger_entries ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| (r.get(0), r.get(1)));

        let type_counts: Vec<(String, i64)> = sqlx::query(
            "SELECT evidence_type, COUNT(*) FROM ledger_entries GROUP BY evidence_type ORDER BY COUNT(*) DESC",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();

        let merkle_root = self.merkle_root().await?;

        Ok(LedgerStats {
            total_entries: total as u64,
            latest_sequence: latest.as_ref().map(|(s, _)| *s as u64),
            latest_entry_hash: latest.map(|(_, h)| h),
            merkle_root,
            evidence_type_counts: type_counts
                .into_iter()
                .map(|(t, c)| (t, c as u64))
                .collect(),
        })
    }

    /// Exports all entries as JSON lines.
    pub async fn export_jsonl(&self) -> Result<String, LedgerError> {
        let entries = self.list_entries(0, u64::MAX).await?;
        let mut lines = Vec::new();
        for entry in &entries {
            lines.push(serde_json::to_string(entry)?);
        }
        Ok(lines.join("\n"))
    }

    // -- Internal helpers --

    async fn next_sequence_and_prev_hash(&self) -> Result<(u64, String), LedgerError> {
        let latest: Option<(i64, String)> = sqlx::query(
            "SELECT sequence, entry_hash FROM ledger_entries ORDER BY sequence DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|r| (r.get(0), r.get(1)));

        match latest {
            Some((seq, hash)) => Ok(((seq as u64) + 1, hash)),
            None => Ok((0, "0".repeat(64))),
        }
    }
}

// ---------------------------------------------------------------------------
// Merkle Tree Helpers
// ---------------------------------------------------------------------------

fn hash_pair(left: &str, right: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    hex::encode(hasher.finalize())
}

fn compute_merkle_root(hashes: &[String]) -> String {
    if hashes.is_empty() {
        return "0".repeat(64);
    }
    if hashes.len() == 1 {
        return hashes[0].clone();
    }

    let mut current_level: Vec<String> = hashes.to_vec();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(hash_pair(&chunk[0], &chunk[1]));
            } else {
                // Odd node: promote by hashing with itself
                next_level.push(hash_pair(&chunk[0], &chunk[0]));
            }
        }
        current_level = next_level;
    }

    current_level.into_iter().next().unwrap()
}

fn build_merkle_proof_path(hashes: &[String], leaf_index: usize) -> Vec<MerklePathNode> {
    if hashes.len() <= 1 {
        return Vec::new();
    }

    let mut path = Vec::new();
    let mut current_level: Vec<String> = hashes.to_vec();
    let mut idx = leaf_index;

    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        let sibling_idx = if idx.is_multiple_of(2) { idx + 1 } else { idx - 1 };
         let direction = if idx.is_multiple_of(2) {
            Direction::Right
        } else {
            Direction::Left
        };

        // Get sibling hash
        let sibling_hash = if sibling_idx < current_level.len() {
            current_level[sibling_idx].clone()
        } else {
            current_level[idx].clone() // duplicate last node
        };

        path.push(MerklePathNode {
            hash: sibling_hash,
            direction,
        });

        // Build next level
        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(hash_pair(&chunk[0], &chunk[1]));
            } else {
                next_level.push(hash_pair(&chunk[0], &chunk[0]));
            }
        }

        idx /= 2;
        current_level = next_level;
    }

    path
}

/// Verifies a Merkle proof independently.
pub fn verify_merkle_proof(proof: &MerkleProof) -> bool {
    let mut current_hash = proof.entry_hash.clone();

    for node in &proof.path {
        current_hash = match node.direction {
            Direction::Left => hash_pair(&node.hash, &current_hash),
            Direction::Right => hash_pair(&current_hash, &node.hash),
        };
    }

    current_hash == proof.root
}

// ---------------------------------------------------------------------------
// Row Mapping
// ---------------------------------------------------------------------------

fn row_to_entry(row: &sqlx::sqlite::SqliteRow) -> LedgerEntry {
    let payload_str: String = row.get("payload");
    let tags_str: String = row.get("tags");

    LedgerEntry {
        id: row.get("id"),
        sequence: row.get::<i64, _>("sequence") as u64,
        evidence_type: row.get("evidence_type"),
        actor: row.get("actor"),
        description: row.get("description"),
        payload: serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null),
        payload_hash: row.get("payload_hash"),
        previous_hash: row.get("previous_hash"),
        entry_hash: row.get("entry_hash"),
        signature: row.get("signature"),
        signer_public_key: row.get("signer_public_key"),
        timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
            .unwrap_or_else(|_| Utc::now().into())
            .with_timezone(&Utc),
        tags: serde_json::from_str(&tags_str).unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_ledger() -> (EvidenceLedger, LedgerKeys) {
        let ledger = EvidenceLedger::open(":memory:").await.unwrap();
        let keys = LedgerKeys::generate();
        (ledger, keys)
    }

    #[test]
    fn test_key_generation_and_signing() {
        let keys = LedgerKeys::generate();
        let message = b"test message";
        let sig = keys.sign(message);
        assert!(keys.verify(message, &sig).unwrap());
        assert!(!keys.verify(b"wrong message", &sig).unwrap());
    }

    #[test]
    fn test_key_roundtrip() {
        let keys = LedgerKeys::generate();
        let secret = keys.secret_hex();
        let restored = LedgerKeys::from_secret_hex(&secret).unwrap();
        assert_eq!(keys.public_hex(), restored.public_hex());
    }

    #[test]
    fn test_cross_key_verification() {
        let keys = LedgerKeys::generate();
        let message = b"cross-key test";
        let sig = keys.sign(message);
        let public = keys.public_hex();
        assert!(LedgerKeys::verify_with_public_key(&public, message, &sig).unwrap());
    }

    #[test]
    fn test_payload_hash_deterministic() {
        let payload = serde_json::json!({"key": "value"});
        let h1 = LedgerEntry::compute_payload_hash(&payload);
        let h2 = LedgerEntry::compute_payload_hash(&payload);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_entry_hash_changes_with_sequence() {
        let ph = "abc123";
        let prev = "def456";
        let h1 = LedgerEntry::compute_entry_hash(ph, prev, 0);
        let h2 = LedgerEntry::compute_entry_hash(ph, prev, 1);
        assert_ne!(h1, h2);
    }

    #[tokio::test]
    async fn test_append_and_verify() {
        let (ledger, keys) = test_ledger().await;

        let req = AppendRequest {
            evidence_type: "tool_output".into(),
            actor: "agent-1".into(),
            description: "Test evidence".into(),
            payload: serde_json::json!({"result": "success"}),
            tags: vec!["test".into()],
        };

        let entry = ledger.append(&req, &keys).await.unwrap();
        assert_eq!(entry.sequence, 0);
        assert_eq!(entry.previous_hash, "0".repeat(64));

        let result = ledger.verify_entry(&entry.id).await.unwrap();
        assert!(result.valid);
        assert!(result.checks.iter().all(|c| c.passed));
    }

    #[tokio::test]
    async fn test_chain_integrity() {
        let (ledger, keys) = test_ledger().await;

        let mut prev_hash = String::new();
        for i in 0..5 {
            let req = AppendRequest {
                evidence_type: "audit".into(),
                actor: "system".into(),
                description: format!("Entry {}", i),
                payload: serde_json::json!({"index": i}),
                tags: vec![],
            };
            let entry = ledger.append(&req, &keys).await.unwrap();
            assert_eq!(entry.sequence, i);

            if i == 0 {
                assert_eq!(entry.previous_hash, "0".repeat(64));
            } else {
                assert_eq!(entry.previous_hash, prev_hash);
            }
            prev_hash = entry.entry_hash.clone();
        }

        // Verify full chain
        let results = ledger.verify_full_chain().await.unwrap();
        assert_eq!(results.len(), 5);
        assert!(results.iter().all(|r| r.valid));
    }

    #[tokio::test]
    async fn test_merkle_root_and_proof() {
        let (ledger, keys) = test_ledger().await;

        for i in 0..4 {
            let req = AppendRequest {
                evidence_type: "test".into(),
                actor: "agent".into(),
                description: format!("Entry {}", i),
                payload: serde_json::json!({"i": i}),
                tags: vec![],
            };
            ledger.append(&req, &keys).await.unwrap();
        }

        let root = ledger.merkle_root().await.unwrap();
        assert!(root.is_some());
        let expected_root = root.unwrap();

        // Generate and verify proof for each entry
        let entries = ledger.list_entries(0, 10).await.unwrap();
        for entry in &entries {
            let proof = ledger.merkle_proof(&entry.id).await.unwrap();
            assert!(verify_merkle_proof(&proof));
            assert_eq!(proof.root, expected_root);
        }
    }

    #[tokio::test]
    async fn test_stats() {
        let (ledger, keys) = test_ledger().await;

        let stats = ledger.stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
        assert!(stats.latest_sequence.is_none());

        for i in 0..3 {
            let req = AppendRequest {
                evidence_type: if i < 2 { "type_a" } else { "type_b" }.into(),
                actor: "agent".into(),
                description: format!("Entry {}", i),
                payload: serde_json::json!({}),
                tags: vec![],
            };
            ledger.append(&req, &keys).await.unwrap();
        }

        let stats = ledger.stats().await.unwrap();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.latest_sequence, Some(2));
        assert!(stats.merkle_root.is_some());
    }

    #[tokio::test]
    async fn test_export_jsonl() {
        let (ledger, keys) = test_ledger().await;

        let req = AppendRequest {
            evidence_type: "export_test".into(),
            actor: "agent".into(),
            description: "Export me".into(),
            payload: serde_json::json!({"data": true}),
            tags: vec![],
        };
        ledger.append(&req, &keys).await.unwrap();

        let jsonl = ledger.export_jsonl().await.unwrap();
        assert!(jsonl.contains("export_test"));
        assert!(jsonl.contains("Export me"));
    }

    #[test]
    fn test_merkle_root_single() {
        let hashes = vec!["abc".to_string()];
        assert_eq!(compute_merkle_root(&hashes), "abc");
    }

    #[test]
    fn test_merkle_root_empty() {
        assert_eq!(compute_merkle_root(&[]), "0".repeat(64));
    }

    #[test]
    fn test_merkle_root_two() {
        let hashes = vec!["a".to_string(), "b".to_string()];
        let root = compute_merkle_root(&hashes);
        assert_eq!(root, hash_pair("a", "b"));
    }

    #[test]
    fn test_verify_merkle_proof_standalone() {
        let hashes = vec![
            "hash0".to_string(),
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
        ];
        let root = compute_merkle_root(&hashes);
        let path = build_merkle_proof_path(&hashes, 2);
        let proof = MerkleProof {
            entry_id: "test".into(),
            entry_hash: "hash2".into(),
            leaf_index: 2,
            path,
            root,
        };
        assert!(verify_merkle_proof(&proof));
    }
}