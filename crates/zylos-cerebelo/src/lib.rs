//! # zylos-cerebelo
//!
//! Checkpoint/rollback com shadow-git, audit chain SHA-256 e test-ratchet.
//! Garante que nenhuma regressão passe despercebida.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CerebeloError {
    #[error("checkpoint não encontrado: {0}")]
    CheckpointNotFound(String),
    #[error("audit chain quebrada: esperado={expected}, encontrado={found}")]
    AuditChainBroken { expected: String, found: String },
    #[error("test-ratchet falhou: cobertura {current:.2}% < {minimum:.2}%")]
    TestRatchetFailed { current: f64, minimum: f64 },
    #[error("arquivo sensível bloqueado: {0}")]
    SensitiveFileBlocked(String),
    #[error("rollback escopado: arquivo não tracked no shadow repo: {0}")]
    UntrackedFile(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub sequence: u64,
    pub action: String,
    pub state_hash: String,
    pub prev_hash: String,
    pub integrity_hash: String,
    pub timestamp: String,
    pub metadata: HashMap<String, String>,
}

impl AuditEntry {
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.action.as_bytes());
        hasher.update(self.state_hash.as_bytes());
        hasher.update(self.prev_hash.as_bytes());
        hasher.update(self.integrity_hash.as_bytes());
        hasher.update(self.timestamp.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Computa integrity_hash por linha do conteúdo.
    pub fn compute_integrity_hash(content: &str) -> String {
        let mut hasher = Sha256::new();
        for line in content.lines() {
            hasher.update(line.as_bytes());
            hasher.update(b"\n");
        }
        format!("{:x}", hasher.finalize())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditChain {
    pub entries: Vec<AuditEntry>,
    pub head_hash: String,
}

impl AuditChain {
    pub fn new() -> Self {
        Self { entries: Vec::new(), head_hash: "genesis".into() }
    }

    pub fn append(&mut self, action: impl Into<String>, state_hash: impl Into<String>, content: &str) -> AuditEntry {
        let seq = self.entries.len() as u64 + 1;
        let entry = AuditEntry {
            sequence: seq,
            action: action.into(),
            state_hash: state_hash.into(),
            prev_hash: self.head_hash.clone(),
            integrity_hash: AuditEntry::compute_integrity_hash(content),
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        };
        self.head_hash = entry.compute_hash();
        self.entries.push(entry.clone());
        entry
    }

    pub fn verify_integrity(&self) -> Result<(), CerebeloError> {
        let mut expected_prev = "genesis".to_string();
        for entry in &self.entries {
            if entry.prev_hash != expected_prev {
                return Err(CerebeloError::AuditChainBroken {
                    expected: expected_prev,
                    found: entry.prev_hash.clone(),
                });
            }
            expected_prev = entry.compute_hash();
        }
        Ok(())
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
}

impl Default for AuditChain {
    fn default() -> Self { Self::new() }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub state_snapshot: String,
    pub audit_head: String,
    pub test_coverage: f64,
    pub timestamp: String,
    pub tracked_files: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRatchet {
    pub minimum_coverage: f64,
    pub history: Vec<(String, f64)>,
}

impl TestRatchet {
    pub fn new(minimum: f64) -> Self {
        Self { minimum_coverage: minimum, history: Vec::new() }
    }

    pub fn record(&mut self, label: impl Into<String>, coverage: f64) -> Result<(), CerebeloError> {
        if coverage < self.minimum_coverage {
            return Err(CerebeloError::TestRatchetFailed { current: coverage, minimum: self.minimum_coverage });
        }
        if let Some((_, last)) = self.history.last() {
            if coverage < *last {
                return Err(CerebeloError::TestRatchetFailed { current: coverage, minimum: *last });
            }
        }
        self.history.push((label.into(), coverage));
        Ok(())
    }

    pub fn current_minimum(&self) -> f64 {
        self.history.last().map(|(_, c)| *c).unwrap_or(self.minimum_coverage)
    }
}

/// Shadow-git: repo isolado fora do worktree principal.
pub struct ShadowGit {
    shadow_dir: PathBuf,
    tracked_files: HashSet<String>,
    sensitive_patterns: Vec<String>,
}

impl ShadowGit {
    pub fn new(shadow_dir: impl Into<PathBuf>) -> Self {
        Self {
            shadow_dir: shadow_dir.into(),
            tracked_files: HashSet::new(),
            sensitive_patterns: vec![
                ".env".into(), "*.key".into(), "*.pem".into(),
                "*.secret".into(), "credentials".into(), "*.token".into(),
            ],
        }
    }

    pub fn shadow_dir(&self) -> &Path { &self.shadow_dir }

    /// Verifica se arquivo é sensível.
    pub fn is_sensitive(&self, path: &str) -> bool {
        let lower = path.to_lowercase();
        self.sensitive_patterns.iter().any(|p| {
            if p.starts_with("*") {
                lower.ends_with(&p[1..])
            } else {
                lower.contains(p.as_str())
            }
        })
    }

    /// Adiciona arquivo ao tracking do shadow repo.
    pub fn track_file(&mut self, path: impl Into<String>) -> Result<(), CerebeloError> {
        let p = path.into();
        if self.is_sensitive(&p) {
            return Err(CerebeloError::SensitiveFileBlocked(p));
        }
        self.tracked_files.insert(p);
        Ok(())
    }

    /// Verifica se arquivo está tracked no shadow repo.
    pub fn is_tracked(&self, path: &str) -> bool {
        self.tracked_files.contains(path)
    }

    /// Rollback escopado: só arquivos tracked no shadow repo.
    pub fn rollback_scope(&self, files: &[String]) -> Result<Vec<String>, CerebeloError> {
        let mut valid = Vec::new();
        for f in files {
            if !self.is_tracked(f) {
                return Err(CerebeloError::UntrackedFile(f.clone()));
            }
            valid.push(f.clone());
        }
        Ok(valid)
    }

    pub fn tracked_files(&self) -> &HashSet<String> { &self.tracked_files }
}

/// CLI commands para cerebelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CerebeloCommand {
    Historia,
    Rewind { checkpoint_id: String },
    Fork { from_checkpoint: String, branch_name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_chain_append_and_verify() {
        let mut chain = AuditChain::new();
        chain.append("init", "hash1", "content1");
        chain.append("deploy", "hash2", "content2");
        chain.append("verify", "hash3", "content3");
        assert_eq!(chain.len(), 3);
        assert!(chain.verify_integrity().is_ok());
    }

    #[test]
    fn test_audit_chain_detects_tampering() {
        let mut chain = AuditChain::new();
        chain.append("init", "hash1", "c1");
        chain.append("deploy", "hash2", "c2");
        chain.entries[1].prev_hash = "tampered".into();
        assert!(chain.verify_integrity().is_err());
    }

    #[test]
    fn test_test_ratchet_allows_increase() {
        let mut r = TestRatchet::new(80.0);
        assert!(r.record("v1", 85.0).is_ok());
        assert!(r.record("v2", 90.0).is_ok());
    }

    #[test]
    fn test_test_ratchet_blocks_decrease() {
        let mut r = TestRatchet::new(80.0);
        r.record("v1", 90.0).unwrap();
        assert!(r.record("v2", 85.0).is_err());
    }

    #[test]
    fn test_test_ratchet_blocks_below_minimum() {
        let mut r = TestRatchet::new(80.0);
        assert!(r.record("v1", 75.0).is_err());
    }

    #[test]
    fn test_shadow_git_blocks_sensitive_files() {
        let sg = ShadowGit::new("/tmp/shadow");
        assert!(sg.is_sensitive(".env"));
        assert!(sg.is_sensitive("app.key"));
        assert!(sg.is_sensitive("secrets.pem"));
        assert!(!sg.is_sensitive("main.rs"));
    }

    #[test]
    fn test_shadow_git_track_and_rollback() {
        let mut sg = ShadowGit::new("/tmp/shadow");
        sg.track_file("src/main.rs").unwrap();
        sg.track_file("src/lib.rs").unwrap();
        assert!(sg.is_tracked("src/main.rs"));
        let result = sg.rollback_scope(&["src/main.rs".into(), "src/lib.rs".into()]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_shadow_git_rollback_rejects_untracked() {
        let mut sg = ShadowGit::new("/tmp/shadow");
        sg.track_file("src/main.rs").unwrap();
        let result = sg.rollback_scope(&["src/main.rs".into(), "unknown.rs".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_shadow_git_track_blocks_sensitive() {
        let mut sg = ShadowGit::new("/tmp/shadow");
        assert!(sg.track_file(".env").is_err());
        assert!(sg.track_file("secret.key").is_err());
    }

    #[test]
    fn test_checkpoint_serialization() {
        let cp = Checkpoint {
            id: "cp-001".into(),
            state_snapshot: "{}".into(),
            audit_head: "abc".into(),
            test_coverage: 92.5,
            timestamp: "2026-11-03T00:00:00Z".into(),
            tracked_files: HashSet::new(),
        };
        let json = serde_json::to_string(&cp).unwrap();
        let de: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(de.test_coverage, 92.5);
    }

    #[test]
    fn test_integrity_hash_deterministic() {
        let h1 = AuditEntry::compute_integrity_hash("line1\nline2\n");
        let h2 = AuditEntry::compute_integrity_hash("line1\nline2\n");
        assert_eq!(h1, h2);
        let h3 = AuditEntry::compute_integrity_hash("line1\nline3\n");
        assert_ne!(h1, h3);
    }
}
