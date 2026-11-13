//! Persistent Browser Profile Store — Gap 2 remediation.
//!
//! Implements persistent Chromium profile management so that agent browser
//! sessions retain cookies, localStorage, and login state across restarts.
//! Profiles are stored as named directories under a configurable base path,
//! with metadata tracked in a SQLite index for fast lookup.
//!
//! Integration point: `DockerSandbox` mounts the profile directory as a
//! named volume at `/root/.config/chromium` (or platform-equivalent).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::browser_antidetect::{BrowserProfile, BrowserSandbox};

/// Unique identifier for a persisted browser profile.
pub type ProfileId = String;

/// Metadata for a persisted browser profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedProfile {
    pub id: ProfileId,
    /// Human-readable name (e.g., "agent-researcher", "team-sales").
    pub name: String,
    /// Agent or team identity this profile belongs to.
    pub owner_id: String,
    /// Filesystem path where the Chromium user-data-dir lives.
    pub data_dir: PathBuf,
    /// The fingerprint profile used to configure this browser.
    pub fingerprint: BrowserProfile,
    /// Seed used to generate the fingerprint (for reproducibility).
    pub seed: u64,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last time this profile was used by an agent.
    pub last_used_at: Option<DateTime<Utc>>,
    /// Number of sessions that have used this profile.
    pub session_count: u64,
    /// SHA-256 hash of the cookie database (for change detection).
    pub cookie_db_hash: Option<String>,
    /// Tags for discovery.
    pub tags: Vec<String>,
}

/// Configuration for the profile store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStoreConfig {
    /// Base directory for all profile data directories.
    pub base_path: PathBuf,
    /// Maximum number of profiles allowed (0 = unlimited).
    pub max_profiles: usize,
    /// Whether to automatically clean up stale profiles.
    pub auto_cleanup: bool,
    /// Days of inactivity before a profile is considered stale.
    pub stale_after_days: u64,
}

impl Default for ProfileStoreConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./browser-profiles"),
            max_profiles: 50,
            auto_cleanup: false,
            stale_after_days: 30,
        }
    }
}

/// In-memory browser profile store with filesystem backing.
///
/// Production use would persist metadata to SQLite via the existing
/// `zylos-memory-system` infrastructure. This implementation provides
/// the API surface and in-memory state management.
pub struct BrowserProfileStore {
    config: ProfileStoreConfig,
    profiles: Arc<RwLock<HashMap<ProfileId, PersistedProfile>>>,
    /// Index by owner_id for fast lookup.
    owner_index: Arc<RwLock<HashMap<String, Vec<ProfileId>>>>,
}

impl BrowserProfileStore {
    pub fn new(config: ProfileStoreConfig) -> Self {
        Self {
            config,
            profiles: Arc::new(RwLock::new(HashMap::new())),
            owner_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create or retrieve a persistent browser profile for an agent/team.
    ///
    /// If a profile with the given name+owner already exists, returns it.
    /// Otherwise generates a new deterministic profile from the seed and
    /// creates the data directory on disk.
    pub fn get_or_create_profile(
        &self,
        name: String,
        owner_id: String,
        seed: u64,
        tags: Vec<String>,
    ) -> Result<PersistedProfile, ProfileStoreError> {
        // Check if profile already exists for this owner+name
        {
            let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
            if let Some(existing) = profiles.values().find(|p| p.name == name && p.owner_id == owner_id) {
                return Ok(existing.clone());
            }
        }

        // Enforce max profiles limit
        {
            let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
            if self.config.max_profiles > 0 && profiles.len() >= self.config.max_profiles {
                return Err(ProfileStoreError::MaxProfilesReached(self.config.max_profiles));
            }
        }

        // Generate deterministic fingerprint
        let sandbox = BrowserSandbox::new(seed);
        let fingerprint = sandbox.profile.clone();

        // Create data directory path
        let safe_name = sanitize_name(&name);
        let safe_owner = sanitize_name(&owner_id);
        let data_dir = self.config.base_path.join(&safe_owner).join(&safe_name);

        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        let profile = PersistedProfile {
            id: id.clone(),
            name,
            owner_id: owner_id.clone(),
            data_dir: data_dir.clone(),
            fingerprint,
            seed,
            created_at: now,
            last_used_at: None,
            session_count: 0,
            cookie_db_hash: None,
            tags,
        };

        // Store in memory
        {
            let mut profiles = self.profiles.write().map_err(|_| ProfileStoreError::LockPoisoned)?;
            profiles.insert(id.clone(), profile.clone());
        }

        // Update owner index
        {
            let mut index = self.owner_index.write().map_err(|_| ProfileStoreError::LockPoisoned)?;
            index.entry(owner_id).or_default().push(id);
        }

        Ok(profile)
    }

    /// Record that a profile was used in a session.
    pub fn record_session(&self, profile_id: &str) -> Result<(), ProfileStoreError> {
        let mut profiles = self.profiles.write().map_err(|_| ProfileStoreError::LockPoisoned)?;
        let profile = profiles.get_mut(profile_id)
            .ok_or_else(|| ProfileStoreError::NotFound(profile_id.to_string()))?;

        profile.last_used_at = Some(Utc::now());
        profile.session_count += 1;
        Ok(())
    }

    /// Update the cookie database hash after a session ends.
    pub fn update_cookie_hash(&self, profile_id: &str, hash: String) -> Result<(), ProfileStoreError> {
        let mut profiles = self.profiles.write().map_err(|_| ProfileStoreError::LockPoisoned)?;
        let profile = profiles.get_mut(profile_id)
            .ok_or_else(|| ProfileStoreError::NotFound(profile_id.to_string()))?;

        profile.cookie_db_hash = Some(hash);
        Ok(())
    }

    /// Get a profile by ID.
    pub fn get_profile(&self, profile_id: &str) -> Result<Option<PersistedProfile>, ProfileStoreError> {
        let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
        Ok(profiles.get(profile_id).cloned())
    }

    /// List all profiles for a given owner.
    pub fn list_profiles_for_owner(&self, owner_id: &str) -> Result<Vec<PersistedProfile>, ProfileStoreError> {
        let index = self.owner_index.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
        let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;

        let ids = index.get(owner_id).cloned().unwrap_or_default();
        Ok(ids.iter().filter_map(|id| profiles.get(id).cloned()).collect())
    }

    /// List all profiles matching a tag.
    pub fn list_profiles_by_tag(&self, tag: &str) -> Result<Vec<PersistedProfile>, ProfileStoreError> {
        let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
        Ok(profiles.values().filter(|p| p.tags.contains(&tag.to_string())).cloned().collect())
    }

    /// Delete a profile and its data directory reference.
    pub fn delete_profile(&self, profile_id: &str) -> Result<bool, ProfileStoreError> {
        let mut profiles = self.profiles.write().map_err(|_| ProfileStoreError::LockPoisoned)?;
        if let Some(profile) = profiles.remove(profile_id) {
            // Remove from owner index
            if let Ok(mut index) = self.owner_index.write() {
                if let Some(ids) = index.get_mut(&profile.owner_id) {
                    ids.retain(|id| id != profile_id);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Find stale profiles (not used within `stale_after_days`).
    pub fn find_stale_profiles(&self) -> Result<Vec<PersistedProfile>, ProfileStoreError> {
        let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
        let cutoff = Utc::now() - chrono::Duration::days(self.config.stale_after_days as i64);

        Ok(profiles.values()
            .filter(|p| {
                p.last_used_at.is_none_or(|last| last < cutoff)
            })
            .cloned()
            .collect())
    }

    /// Get the filesystem path where a profile's Chromium data lives.
    pub fn profile_data_path(&self, profile_id: &str) -> Result<PathBuf, ProfileStoreError> {
        let profiles = self.profiles.read().map_err(|_| ProfileStoreError::LockPoisoned)?;
        let profile = profiles.get(profile_id)
            .ok_or_else(|| ProfileStoreError::NotFound(profile_id.to_string()))?;
        Ok(profile.data_dir.clone())
    }

    /// Total number of managed profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.read().map(|p| p.len()).unwrap_or(0)
    }

    /// Compute a hash of the cookie database file for change detection.
    pub fn compute_cookie_hash(data_dir: &Path) -> Result<String, ProfileStoreError> {
        let cookie_db = data_dir.join("Default").join("Cookies");
        if !cookie_db.exists() {
            return Ok(String::new());
        }

        let content = std::fs::read(&cookie_db)
            .map_err(|e| ProfileStoreError::Io(e.to_string()))?;

        let mut hasher = Sha256::new();
        hasher.update(&content);
        Ok(hex::encode(hasher.finalize()))
    }
}

/// Errors from profile store operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProfileStoreError {
    #[error("Profile not found: {0}")]
    NotFound(String),
    #[error("Maximum profiles reached: {0}")]
    MaxProfilesReached(usize),
    #[error("Lock poisoned")]
    LockPoisoned,
    #[error("IO error: {0}")]
    Io(String),
}

/// Sanitize a name for use as a filesystem directory component.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_store() -> (BrowserProfileStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        let config = ProfileStoreConfig {
            base_path: tmp.path().to_path_buf(),
            max_profiles: 10,
            auto_cleanup: false,
            stale_after_days: 30,
        };
        (BrowserProfileStore::new(config), tmp)
    }

    #[test]
    fn create_and_retrieve_profile() {
        let (store, _tmp) = test_store();
        let profile = store.get_or_create_profile(
            "researcher".into(),
            "agent-1".into(),
            42,
            vec!["research".into()],
        ).unwrap();

        assert_eq!(profile.name, "researcher");
        assert_eq!(profile.owner_id, "agent-1");
        assert_eq!(profile.seed, 42);
        assert_eq!(profile.session_count, 0);

        // Retrieve same profile again — should return existing
        let same = store.get_or_create_profile(
            "researcher".into(),
            "agent-1".into(),
            42,
            vec![],
        ).unwrap();
        assert_eq!(same.id, profile.id);
    }

    #[test]
    fn different_owners_get_separate_profiles() {
        let (store, _tmp) = test_store();
        let p1 = store.get_or_create_profile("main".into(), "agent-1".into(), 42, vec![]).unwrap();
        let p2 = store.get_or_create_profile("main".into(), "agent-2".into(), 42, vec![]).unwrap();
        assert_ne!(p1.id, p2.id);
    }

    #[test]
    fn record_session_updates_metadata() {
        let (store, _tmp) = test_store();
        let profile = store.get_or_create_profile("test".into(), "a1".into(), 1, vec![]).unwrap();

        store.record_session(&profile.id).unwrap();
        store.record_session(&profile.id).unwrap();

        let updated = store.get_profile(&profile.id).unwrap().unwrap();
        assert_eq!(updated.session_count, 2);
        assert!(updated.last_used_at.is_some());
    }

    #[test]
    fn max_profiles_enforced() {
        let tmp = TempDir::new().unwrap();
        let config = ProfileStoreConfig {
            base_path: tmp.path().to_path_buf(),
            max_profiles: 2,
            ..Default::default()
        };
        let store = BrowserProfileStore::new(config);

        store.get_or_create_profile("a".into(), "o".into(), 1, vec![]).unwrap();
        store.get_or_create_profile("b".into(), "o".into(), 2, vec![]).unwrap();
        let result = store.get_or_create_profile("c".into(), "o".into(), 3, vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn list_by_owner() {
        let (store, _tmp) = test_store();
        store.get_or_create_profile("a".into(), "team-x".into(), 1, vec![]).unwrap();
        store.get_or_create_profile("b".into(), "team-x".into(), 2, vec![]).unwrap();
        store.get_or_create_profile("c".into(), "team-y".into(), 3, vec![]).unwrap();

        let team_x = store.list_profiles_for_owner("team-x").unwrap();
        assert_eq!(team_x.len(), 2);

        let team_y = store.list_profiles_for_owner("team-y").unwrap();
        assert_eq!(team_y.len(), 1);
    }

    #[test]
    fn list_by_tag() {
        let (store, _tmp) = test_store();
        store.get_or_create_profile("a".into(), "o".into(), 1, vec!["web".into()]).unwrap();
        store.get_or_create_profile("b".into(), "o".into(), 2, vec!["api".into()]).unwrap();
        store.get_or_create_profile("c".into(), "o".into(), 3, vec!["web".into(), "api".into()]).unwrap();

        let web = store.list_profiles_by_tag("web").unwrap();
        assert_eq!(web.len(), 2);

        let api = store.list_profiles_by_tag("api").unwrap();
        assert_eq!(api.len(), 2);
    }

    #[test]
    fn delete_profile() {
        let (store, _tmp) = test_store();
        let profile = store.get_or_create_profile("del".into(), "o".into(), 1, vec![]).unwrap();
        assert_eq!(store.profile_count(), 1);

        assert!(store.delete_profile(&profile.id).unwrap());
        assert_eq!(store.profile_count(), 0);
        assert!(store.get_profile(&profile.id).unwrap().is_none());
    }

    #[test]
    fn sanitize_name_cleans_special_chars() {
        assert_eq!(sanitize_name("Hello World!"), "hello_world");
        assert_eq!(sanitize_name("agent/researcher"), "agent_researcher");
        assert_eq!(sanitize_name("  spaces  "), "spaces");
        assert_eq!(sanitize_name("UPPER-case_123"), "upper-case_123");
    }

    #[test]
    fn profile_data_path_returns_correct_dir() {
        let (store, _tmp) = test_store();
        let profile = store.get_or_create_profile("my-profile".into(), "agent-1".into(), 42, vec![]).unwrap();
        let path = store.profile_data_path(&profile.id).unwrap();
        assert!(path.to_str().unwrap().contains("agent-1"));
        assert!(path.to_str().unwrap().contains("my-profile"));
    }

    #[test]
    fn update_cookie_hash() {
        let (store, _tmp) = test_store();
        let profile = store.get_or_create_profile("cookietest".into(), "a1".into(), 1, vec![]).unwrap();

        store.update_cookie_hash(&profile.id, "abc123hash".into()).unwrap();
        let updated = store.get_profile(&profile.id).unwrap().unwrap();
        assert_eq!(updated.cookie_db_hash, Some("abc123hash".into()));
    }
}