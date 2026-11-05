//! Secure Input Pipeline ("Ghost Typist") — Gap 3 remediation.
//!
//! Implements a credential injection pipeline where secrets are NEVER visible
//! to the LLM context window, tool transcripts, or trace logs. Credentials
//! flow from a masked UI component directly into the browser/sandbox stdin,
//! bypassing all agent reasoning layers.
//!
//! Architecture:
//!   Leptos masked input → SecureInputPipeline → WASM/Docker stdin
//!                              ↑
//!                     CredentialVault (encrypted at rest)
//!
//! The LLM only sees `[CREDENTIAL_INJECTED: ***]` placeholders in tool output.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Unique identifier for a stored credential.
pub type CredentialId = String;

/// A credential that has been securely stored and can be injected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureCredential {
    pub id: CredentialId,
    /// Human-readable label (e.g., "GitHub Token", "AWS Secret Key").
    pub label: String,
    /// The credential type determines how it's injected.
    pub kind: CredentialKind,
    /// Encrypted credential bytes (AES-256-GCM in production).
    /// In this reference implementation, we store a hash for verification
    /// and rely on the runtime to provide actual encrypted storage.
    pub encrypted_payload: Vec<u8>,
    /// SHA-256 fingerprint for verification without exposing the secret.
    pub fingerprint: String,
    /// Owner identity (agent, team, or user).
    pub owner_id: String,
    /// When this credential was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Optional expiration timestamp.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of times this credential has been injected.
    pub use_count: u64,
    /// Last time this credential was used.
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Tags for discovery and scoping.
    pub tags: Vec<String>,
}

/// How a credential should be injected into the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialKind {
    /// Typed character-by-character into a focused input field.
    KeyboardTyping,
    /// Pasted as a complete value (Ctrl+V equivalent).
    ClipboardPaste,
    /// Set as an environment variable in the sandbox.
    EnvironmentVariable,
    /// Written directly to a file inside the sandbox.
    FileContent,
    /// Sent as an HTTP header value.
    HttpHeader,
    /// Injected via browser CDP `Runtime.evaluate` with isolated world.
    BrowserCdpEval,
}

/// Masking strategy for displaying credential references in LLM context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMask {
    /// The placeholder shown in transcripts (e.g., `[CRED:github_token:***]`).
    pub placeholder: String,
    /// Whether the credential name/label is visible (true) or fully redacted (false).
    pub show_label: bool,
    /// Maximum number of characters to reveal in error messages (0 = never).
    pub max_error_hint_chars: usize,
}

impl InputMask {
    /// Create a standard mask for a credential.
    pub fn for_credential(cred: &SecureCredential) -> Self {
        Self {
            placeholder: format!("[CRED:{}:***]", sanitize_label(&cred.label)),
            show_label: true,
            max_error_hint_chars: 0,
        }
    }

    /// Create a fully redacted mask (no label visible).
    pub fn fully_redacted() -> Self {
        Self {
            placeholder: "[CREDENTIAL_INJECTED: ***]".to_string(),
            show_label: false,
            max_error_hint_chars: 0,
        }
    }
}

/// Result of a secure injection operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionResult {
    /// Whether the injection succeeded.
    pub success: bool,
    /// The mask to display in LLM context (never contains the secret).
    pub mask: InputMask,
    /// Audit trail ID for compliance logging.
    pub audit_id: String,
    /// Timestamp of the injection.
    pub injected_at: chrono::DateTime<chrono::Utc>,
    /// Error message if injection failed (safe to show to LLM).
    pub error: Option<String>,
}

/// Errors from the Ghost Typist pipeline.
#[derive(Debug, Clone, thiserror::Error)]
pub enum GhostTypistError {
    #[error("Credential not found: {0}")]
    NotFound(String),
    #[error("Credential expired: {0}")]
    Expired(String),
    #[error("Access denied: caller '{caller}' cannot access credential '{credential}'")]
    AccessDenied { caller: String, credential: String },
    #[error("Injection target unavailable: {0}")]
    TargetUnavailable(String),
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    #[error("Rate limit exceeded for credential {0}: max {1} uses per {2}s")]
    RateLimited(String, u64, u64),
    #[error("Lock poisoned")]
    LockPoisoned,
}

/// Configuration for the secure input pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureInputConfig {
    /// Maximum credential uses per rate-limit window.
    pub max_uses_per_window: u64,
    /// Rate limit window duration.
    pub rate_limit_window_secs: u64,
    /// Whether to enforce ownership checks.
    pub enforce_ownership: bool,
    /// Maximum credential lifetime (days, 0 = unlimited).
    pub max_lifetime_days: u64,
    /// Whether to log injection events for audit.
    pub audit_logging: bool,
}

impl Default for SecureInputConfig {
    fn default() -> Self {
        Self {
            max_uses_per_window: 100,
            rate_limit_window_secs: 3600,
            enforce_ownership: true,
            max_lifetime_days: 90,
            audit_logging: true,
        }
    }
}

/// In-memory credential vault with rate limiting and audit trail.
///
/// Production deployments MUST replace the in-memory store with an encrypted
/// backend (e.g., OS keychain, HashiCorp Vault, AWS Secrets Manager). This
/// implementation provides the API surface and security guarantees at the
/// Rust type level.
pub struct CredentialVault {
    config: SecureInputConfig,
    credentials: Arc<RwLock<HashMap<CredentialId, SecureCredential>>>,
    /// Rate limiting: credential_id → list of recent use timestamps.
    rate_limits: Arc<RwLock<HashMap<CredentialId, Vec<Instant>>>>,
    /// Audit log entries.
    audit_log: Arc<RwLock<Vec<AuditEntry>>>,
}

/// An audit log entry for credential injection events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    audit_id: String,
    credential_id: CredentialId,
    caller_id: String,
    target_type: String,
    success: bool,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl CredentialVault {
    pub fn new(config: SecureInputConfig) -> Self {
        Self {
            config,
            credentials: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Store a new credential. Returns the credential ID and fingerprint.
    ///
    /// The raw secret is hashed for the fingerprint and then the caller
    /// should encrypt it before passing `encrypted_payload`. In this
    /// reference implementation, we accept pre-encrypted bytes.
    pub fn store_credential(
        &self,
        label: String,
        kind: CredentialKind,
        encrypted_payload: Vec<u8>,
        owner_id: String,
        tags: Vec<String>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(CredentialId, String), GhostTypistError> {
        let id = Uuid::new_v4().to_string();

        // Compute fingerprint from encrypted payload (not plaintext)
        let mut hasher = Sha256::new();
        hasher.update(&encrypted_payload);
        let fingerprint = hex::encode(&hasher.finalize()[..16]);

        let cred = SecureCredential {
            id: id.clone(),
            label,
            kind,
            encrypted_payload,
            fingerprint: fingerprint.clone(),
            owner_id,
            created_at: chrono::Utc::now(),
            expires_at,
            use_count: 0,
            last_used_at: None,
            tags,
        };

        let mut creds = self.credentials.write().map_err(|_| GhostTypistError::LockPoisoned)?;
        creds.insert(id.clone(), cred);

        Ok((id, fingerprint))
    }

    /// Retrieve a credential's metadata (NOT the secret) for display.
    pub fn get_metadata(&self, credential_id: &str) -> Result<InputMask, GhostTypistError> {
        let creds = self.credentials.read().map_err(|_| GhostTypistError::LockPoisoned)?;
        let cred = creds.get(credential_id)
            .ok_or_else(|| GhostTypistError::NotFound(credential_id.to_string()))?;
        Ok(InputMask::for_credential(cred))
    }

    /// Inject a credential into a target. This is the core Ghost Typist operation.
    ///
    /// The `inject_fn` callback receives the decrypted credential bytes and
    /// performs the actual injection (typing, pasting, env var set, etc.).
    /// The pipeline handles rate limiting, expiry checks, ownership validation,
    /// and audit logging around the injection.
    ///
    /// Returns an `InjectionResult` safe to include in LLM context.
    pub fn inject<F>(
        &self,
        credential_id: &str,
        caller_id: &str,
        target_type: &str,
        inject_fn: F,
    ) -> Result<InjectionResult, GhostTypistError>
    where
        F: FnOnce(&[u8]) -> Result<(), String>,
    {
        let audit_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        // 1. Lookup credential
        let cred = {
            let creds = self.credentials.read().map_err(|_| GhostTypistError::LockPoisoned)?;
            creds.get(credential_id).cloned()
                .ok_or_else(|| GhostTypistError::NotFound(credential_id.to_string()))?
        };

        // 2. Check expiration
        if let Some(expires) = cred.expires_at {
            if now > expires {
                self.log_audit(&audit_id, credential_id, caller_id, target_type, false);
                return Err(GhostTypistError::Expired(credential_id.to_string()));
            }
        }

        // 3. Check ownership
        if self.config.enforce_ownership && cred.owner_id != caller_id {
            self.log_audit(&audit_id, credential_id, caller_id, target_type, false);
            return Err(GhostTypistError::AccessDenied {
                caller: caller_id.to_string(),
                credential: credential_id.to_string(),
            });
        }

        // 4. Rate limiting
        self.check_rate_limit(credential_id)?;

        // 5. Perform injection via callback
        let mask = InputMask::for_credential(&cred);
        let injection_result = inject_fn(&cred.encrypted_payload);

        let success = injection_result.is_ok();
        let error = injection_result.err();

        // 6. Update usage counters
        if success {
            let mut creds = self.credentials.write().map_err(|_| GhostTypistError::LockPoisoned)?;
            if let Some(c) = creds.get_mut(credential_id) {
                c.use_count += 1;
                c.last_used_at = Some(now);
            }

            // Record rate limit timestamp
            if let Ok(mut limits) = self.rate_limits.write() {
                limits.entry(credential_id.to_string())
                    .or_default()
                    .push(Instant::now());
            }
        }

        // 7. Audit log
        self.log_audit(&audit_id, credential_id, caller_id, target_type, success);

        Ok(InjectionResult {
            success,
            mask,
            audit_id,
            injected_at: now,
            error,
        })
    }

    /// List all credentials owned by a given identity (metadata only).
    pub fn list_credentials(&self, owner_id: &str) -> Result<Vec<(CredentialId, String, CredentialKind)>, GhostTypistError> {
        let creds = self.credentials.read().map_err(|_| GhostTypistError::LockPoisoned)?;
        Ok(creds.values()
            .filter(|c| c.owner_id == owner_id)
            .map(|c| (c.id.clone(), c.label.clone(), c.kind))
            .collect())
    }

    /// Delete a credential.
    pub fn delete_credential(&self, credential_id: &str, caller_id: &str) -> Result<bool, GhostTypistError> {
        let mut creds = self.credentials.write().map_err(|_| GhostTypistError::LockPoisoned)?;
        if let Some(cred) = creds.get(credential_id) {
            if self.config.enforce_ownership && cred.owner_id != caller_id {
                return Err(GhostTypistError::AccessDenied {
                    caller: caller_id.to_string(),
                    credential: credential_id.to_string(),
                });
            }
        }
        Ok(creds.remove(credential_id).is_some())
    }

    /// Get audit log entries (most recent first).
    pub fn get_audit_log(&self, limit: usize) -> Result<Vec<AuditEntry>, GhostTypistError> {
        let log = self.audit_log.read().map_err(|_| GhostTypistError::LockPoisoned)?;
        let entries: Vec<AuditEntry> = log.iter().rev().take(limit).cloned().collect();
        Ok(entries)
    }

    fn check_rate_limit(&self, credential_id: &str) -> Result<(), GhostTypistError> {
        let limits = self.rate_limits.read().map_err(|_| GhostTypistError::LockPoisoned)?;
        if let Some(uses) = limits.get(credential_id) {
            let window = Duration::from_secs(self.config.rate_limit_window_secs);
            let cutoff = Instant::now() - window;
            let recent_count = uses.iter().filter(|t| **t > cutoff).count() as u64;
            if recent_count >= self.config.max_uses_per_window {
                return Err(GhostTypistError::RateLimited(
                    credential_id.to_string(),
                    self.config.max_uses_per_window,
                    self.config.rate_limit_window_secs,
                ));
            }
        }
        Ok(())
    }

    fn log_audit(&self, audit_id: &str, credential_id: &str, caller_id: &str, target_type: &str, success: bool) {
        if !self.config.audit_logging {
            return;
        }
        let entry = AuditEntry {
            audit_id: audit_id.to_string(),
            credential_id: credential_id.to_string(),
            caller_id: caller_id.to_string(),
            target_type: target_type.to_string(),
            success,
            timestamp: chrono::Utc::now(),
        };
        if let Ok(mut log) = self.audit_log.write() {
            log.push(entry);
        }
    }
}

/// High-level secure input pipeline combining vault + injection targets.
pub struct SecureInputPipeline {
    vault: CredentialVault,
}

impl SecureInputPipeline {
    pub fn new(config: SecureInputConfig) -> Self {
        Self {
            vault: CredentialVault::new(config),
        }
    }

    /// Access the underlying vault for direct operations.
    pub fn vault(&self) -> &CredentialVault {
        &self.vault
    }

    /// Inject a credential as keyboard typing simulation.
    /// Returns the safe mask for LLM context.
    pub fn inject_as_typing(
        &self,
        credential_id: &str,
        caller_id: &str,
        target_description: &str,
    ) -> Result<InjectionResult, GhostTypistError> {
        self.vault.inject(credential_id, caller_id, target_description, |_bytes| {
            // In production: send bytes to browser CDP Input.dispatchKeyEvent
            // or WASM sandbox stdin character by character with random delays.
            // Reference implementation: validate payload is non-empty.
            if _bytes.is_empty() {
                Err("Empty credential payload".to_string())
            } else {
                Ok(())
            }
        })
    }

    /// Inject a credential as an environment variable.
    pub fn inject_as_env_var(
        &self,
        credential_id: &str,
        caller_id: &str,
        var_name: &str,
    ) -> Result<InjectionResult, GhostTypistError> {
        self.vault.inject(credential_id, caller_id, &format!("env:{var_name}"), |_bytes| {
            // In production: set env var in Docker container or WASM sandbox
            // via the sandbox API, never through shell commands visible in traces.
            if _bytes.is_empty() {
                Err("Empty credential payload".to_string())
            } else {
                Ok(())
            }
        })
    }

    /// Inject a credential via browser CDP isolated world evaluation.
    pub fn inject_via_cdp(
        &self,
        credential_id: &str,
        caller_id: &str,
        js_expression_template: &str,
    ) -> Result<InjectionResult, GhostTypistError> {
        self.vault.inject(credential_id, caller_id, "cdp:isolated_world", |_bytes| {
            // In production: use CDP Runtime.evaluate in an isolated world
            // where the credential value is bound as a parameter, never
            // interpolated into the JS string.
            if _bytes.is_empty() {
                Err("Empty credential payload".to_string())
            } else {
                // Validate template doesn't contain obvious interpolation patterns
                if js_expression_template.contains("${secret}") || js_expression_template.contains("{secret}") {
                    Err("Template must not interpolate secret directly; use CDP argument binding".to_string())
                } else {
                    Ok(())
                }
            }
        })
    }
}

fn sanitize_label(label: &str) -> String {
    let sanitized: String = label
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect::<String>()
        .to_lowercase();
    // Collapse consecutive underscores and trim leading/trailing
    let collapsed: String = sanitized
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    collapsed.chars().take(32).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pipeline() -> SecureInputPipeline {
        SecureInputPipeline::new(SecureInputConfig {
            max_uses_per_window: 10,
            rate_limit_window_secs: 60,
            enforce_ownership: true,
            max_lifetime_days: 30,
            audit_logging: true,
        })
    }

    #[test]
    fn store_and_inject_credential() {
        let pipeline = test_pipeline();
        let (id, fingerprint) = pipeline.vault().store_credential(
            "GitHub Token".into(),
            CredentialKind::EnvironmentVariable,
            b"ghp_fake_token_12345".to_vec(),
            "agent-1".into(),
            vec!["github".into()],
            None,
        ).unwrap();

        assert!(!fingerprint.is_empty());

        let result = pipeline.inject_as_env_var(&id, "agent-1", "GITHUB_TOKEN").unwrap();
        assert!(result.success);
        assert!(result.mask.placeholder.contains("github_token"));
        assert!(!result.mask.placeholder.contains("ghp_fake"));
    }

    #[test]
    fn injection_blocks_wrong_owner() {
        let pipeline = test_pipeline();
        let (id, _) = pipeline.vault().store_credential(
            "Secret".into(),
            CredentialKind::KeyboardTyping,
            b"password123".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        let result = pipeline.vault().inject(&id, "agent-2", "browser", |_| Ok(()));
        assert!(result.is_err());
        match result.unwrap_err() {
            GhostTypistError::AccessDenied { caller, .. } => {
                assert_eq!(caller, "agent-2");
            }
            other => panic!("Expected AccessDenied, got {:?}", other),
        }
    }

    #[test]
    fn expired_credential_rejected() {
        let pipeline = test_pipeline();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let (id, _) = pipeline.vault().store_credential(
            "Old Token".into(),
            CredentialKind::HttpHeader,
            b"expired_token".to_vec(),
            "agent-1".into(),
            vec![],
            Some(past),
        ).unwrap();

        let result = pipeline.vault().inject(&id, "agent-1", "http", |_| Ok(()));
        assert!(matches!(result, Err(GhostTypistError::Expired(_))));
    }

    #[test]
    fn rate_limit_enforced() {
        let pipeline = SecureInputPipeline::new(SecureInputConfig {
            max_uses_per_window: 3,
            rate_limit_window_secs: 3600,
            enforce_ownership: false,
            ..Default::default()
        });

        let (id, _) = pipeline.vault().store_credential(
            "Rate Limited".into(),
            CredentialKind::ClipboardPaste,
            b"secret".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        // Use up the limit
        for _ in 0..3 {
            pipeline.vault().inject(&id, "agent-1", "browser", |_| Ok(())).unwrap();
        }

        // Next use should be rate limited
        let result = pipeline.vault().inject(&id, "agent-1", "browser", |_| Ok(()));
        assert!(matches!(result, Err(GhostTypistError::RateLimited(_, _, _))));
    }

    #[test]
    fn mask_never_contains_secret() {
        let pipeline = test_pipeline();
        let secret = "super_secret_password_do_not_leak";
        let (id, _) = pipeline.vault().store_credential(
            "Password".into(),
            CredentialKind::KeyboardTyping,
            secret.as_bytes().to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        let result = pipeline.inject_as_typing(&id, "agent-1", "login_form").unwrap();
        assert!(!result.mask.placeholder.contains(secret));
        assert!(!result.mask.placeholder.contains("super_secret"));
        assert!(result.mask.placeholder.contains("***"));
    }

    #[test]
    fn audit_log_records_injections() {
        let pipeline = test_pipeline();
        let (id, _) = pipeline.vault().store_credential(
            "Audited".into(),
            CredentialKind::FileContent,
            b"content".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        pipeline.vault().inject(&id, "agent-1", "file:/tmp/secret", |_| Ok(())).unwrap();

        let log = pipeline.vault().get_audit_log(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].credential_id, id);
        assert_eq!(log[0].caller_id, "agent-1");
        assert!(log[0].success);
    }

    #[test]
    fn delete_requires_ownership() {
        let pipeline = test_pipeline();
        let (id, _) = pipeline.vault().store_credential(
            "Protected".into(),
            CredentialKind::BrowserCdpEval,
            b"data".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        assert!(pipeline.vault().delete_credential(&id, "agent-2").is_err());
        assert!(pipeline.vault().delete_credential(&id, "agent-1").unwrap());
    }

    #[test]
    fn cdp_injection_rejects_interpolation_template() {
        let pipeline = test_pipeline();
        let (id, _) = pipeline.vault().store_credential(
            "CDP Cred".into(),
            CredentialKind::BrowserCdpEval,
            b"value".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        let result = pipeline.inject_via_cdp(&id, "agent-1", "document.getElementById('pw').value = '${secret}'");
        assert!(result.is_ok()); // inject succeeds but inner validation catches it
        let injection = result.unwrap();
        assert!(!injection.success);
        assert!(injection.error.unwrap().contains("interpolate"));
    }

    #[test]
    fn list_credentials_returns_metadata_only() {
        let pipeline = test_pipeline();
        pipeline.vault().store_credential(
            "Token A".into(),
            CredentialKind::HttpHeader,
            b"aaa".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();
        pipeline.vault().store_credential(
            "Token B".into(),
            CredentialKind::EnvironmentVariable,
            b"bbb".to_vec(),
            "agent-1".into(),
            vec![],
            None,
        ).unwrap();

        let list = pipeline.vault().list_credentials("agent-1").unwrap();
        assert_eq!(list.len(), 2);
        // Verify no secrets leaked in metadata
        for (_, label, _) in &list {
            assert!(!label.contains("aaa"));
            assert!(!label.contains("bbb"));
        }
    }

    #[test]
    fn sanitize_label_strips_unsafe_chars() {
        assert_eq!(sanitize_label("My GitHub Token!"), "my_github_token");
        assert_eq!(sanitize_label("aws-secret/key"), "aws-secret_key");
        assert_eq!(sanitize_label(""), "");
    }
}