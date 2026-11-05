//! Anti-detect browser context with persistent inbox and filesystem isolation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{BrowserError, Result};
use crate::fingerprint::BrowserFingerprint;

/// Permission decision for browser actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

/// Persistent inbox entry for permission decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxEntry {
    pub id: String,
    pub domain: String,
    pub permission_type: String,
    pub decision: PermissionDecision,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Configuration for a browser context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserContextConfig {
    /// Agent identity seed for fingerprint derivation.
    pub agent_seed: String,
    /// Whether to persist cookies/localStorage across sessions.
    pub persistent_storage: bool,
    /// Data directory for this context (None = ephemeral).
    pub data_dir: Option<String>,
    /// Maximum concurrent tabs.
    pub max_tabs: usize,
}

impl Default for BrowserContextConfig {
    fn default() -> Self {
        Self {
            agent_seed: Uuid::new_v4().to_string(),
            persistent_storage: true,
            data_dir: None,
            max_tabs: 5,
        }
    }
}

/// An isolated browser context with anti-detect fingerprinting.
#[derive(Debug)]
pub struct BrowserContext {
    pub id: String,
    pub config: BrowserContextConfig,
    pub fingerprint: BrowserFingerprint,
    inbox: HashMap<String, InboxEntry>,
    active_tabs: usize,
}

impl BrowserContext {
    /// Create a new browser context with a deterministic fingerprint.
    pub fn new(config: BrowserContextConfig) -> Result<Self> {
        let fingerprint = BrowserFingerprint::from_seed(&config.agent_seed);
        let id = format!("ctx-{}", &fingerprint.instance_id[..8]);

        tracing::info!(
            id = %id,
            instance = %fingerprint.instance_id,
            "Browser context created"
        );

        Ok(Self {
            id,
            config,
            fingerprint,
            inbox: HashMap::new(),
            active_tabs: 0,
        })
    }

    /// Check or record a permission decision for a domain.
    pub fn check_permission(&mut self, domain: &str, perm_type: &str) -> PermissionDecision {
        let key = format!("{}:{}", domain, perm_type);
        if let Some(entry) = self.inbox.get(&key) {
            return entry.decision.clone();
        }
        // Default: ask on first encounter
        PermissionDecision::Ask
    }

    /// Record a permission decision in the persistent inbox.
    pub fn record_decision(
        &mut self,
        domain: &str,
        perm_type: &str,
        decision: PermissionDecision,
    ) {
        let key = format!("{}:{}", domain, perm_type);
        let entry = InboxEntry {
            id: Uuid::new_v4().to_string(),
            domain: domain.to_string(),
            permission_type: perm_type.to_string(),
            decision,
            created_at: chrono::Utc::now(),
        };
        self.inbox.insert(key, entry);
    }

    /// Open a new tab. Enforces max_tabs limit.
    pub fn open_tab(&mut self) -> Result<()> {
        if self.active_tabs >= self.config.max_tabs {
            return Err(BrowserError::LaunchFailed(format!(
                "Max tabs ({}) reached",
                self.config.max_tabs
            )));
        }
        self.active_tabs += 1;
        Ok(())
    }

    /// Close a tab.
    pub fn close_tab(&mut self) {
        if self.active_tabs > 0 {
            self.active_tabs -= 1;
        }
    }

    /// Get current active tab count.
    pub fn active_tab_count(&self) -> usize {
        self.active_tabs
    }

    /// Get all inbox entries.
    pub fn inbox_entries(&self) -> Vec<&InboxEntry> {
        self.inbox.values().collect()
    }

    /// Verify fingerprint consistency against a stored hash.
    pub fn verify_fingerprint(&self, expected_hash: &str) -> bool {
        self.fingerprint.consistency_hash() == expected_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let config = BrowserContextConfig {
            agent_seed: "test-agent".to_string(),
            ..Default::default()
        };
        let ctx = BrowserContext::new(config).unwrap();
        assert!(ctx.id.starts_with("ctx-"));
        assert_eq!(ctx.fingerprint.instance_id.len(), 16);
    }

    #[test]
    fn test_permission_inbox() {
        let config = BrowserContextConfig {
            agent_seed: "perm-test".to_string(),
            ..Default::default()
        };
        let mut ctx = BrowserContext::new(config).unwrap();

        // First check returns Ask
        assert_eq!(
            ctx.check_permission("example.com", "geolocation"),
            PermissionDecision::Ask
        );

        // Record Allow
        ctx.record_decision("example.com", "geolocation", PermissionDecision::Allow);
        assert_eq!(
            ctx.check_permission("example.com", "geolocation"),
            PermissionDecision::Allow
        );

        // Different domain still asks
        assert_eq!(
            ctx.check_permission("other.com", "geolocation"),
            PermissionDecision::Ask
        );
    }

    #[test]
    fn test_max_tabs_enforcement() {
        let config = BrowserContextConfig {
            agent_seed: "tabs-test".to_string(),
            max_tabs: 2,
            ..Default::default()
        };
        let mut ctx = BrowserContext::new(config).unwrap();

        assert!(ctx.open_tab().is_ok());
        assert!(ctx.open_tab().is_ok());
        assert!(ctx.open_tab().is_err());
        assert_eq!(ctx.active_tab_count(), 2);

        ctx.close_tab();
        assert_eq!(ctx.active_tab_count(), 1);
        assert!(ctx.open_tab().is_ok());
    }

    #[test]
    fn test_fingerprint_verification() {
        let config = BrowserContextConfig {
            agent_seed: "verify-test".to_string(),
            ..Default::default()
        };
        let ctx = BrowserContext::new(config).unwrap();
        let hash = ctx.fingerprint.consistency_hash();
        assert!(ctx.verify_fingerprint(&hash));
        assert!(!ctx.verify_fingerprint("wrong-hash"));
    }
}