//! Document Spaces — Persistent artifact storage with versioning and access control.
//!
//! Implements Gap 4 from REA competitive analysis: a write_space / document_vault
//! system for agent-generated artifacts (specs, reports, code snippets, wiki pages).
//! Built on top of the existing ContextGraph + SQLite WAL backend.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

use crate::types::MemoryScope;
use crate::error::{MemoryError, Result};

/// Unique identifier for a document space.
pub type SpaceId = String;

/// Unique identifier for a page within a space.
pub type PageId = String;

/// A document space — a named, scoped container for pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSpace {
    pub id: SpaceId,
    pub name: String,
    pub description: String,
    pub scope: MemoryScope,
    pub owner_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Maximum number of pages allowed (None = unlimited).
    pub max_pages: Option<usize>,
    /// Tags for discovery and filtering.
    pub tags: Vec<String>,
}

/// A single page/versioned document within a space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacePage {
    pub id: PageId,
    pub space_id: SpaceId,
    pub title: String,
    pub content: String,
    pub content_hash: String,
    pub mime_type: String,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
    /// Previous version hashes for audit trail.
    pub version_history: Vec<VersionEntry>,
    /// Optional metadata (frontmatter, tags, links).
    pub metadata: serde_json::Value,
}

/// A historical version entry for audit and rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub version: u64,
    pub content_hash: String,
    pub updated_at: DateTime<Utc>,
    pub updated_by: String,
    pub change_summary: Option<String>,
}

/// Request to create or update a page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritePageRequest {
    pub space_id: SpaceId,
    pub title: String,
    pub content: String,
    pub mime_type: Option<String>,
    pub created_by: String,
    pub change_summary: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing pages.
#[derive(Debug, Clone, Default)]
pub struct PageQuery {
    pub space_id: Option<SpaceId>,
    pub tag: Option<String>,
    pub author: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

/// Statistics about a document space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceStats {
    pub total_pages: usize,
    pub total_versions: usize,
    pub total_bytes: usize,
    pub last_updated: Option<DateTime<Utc>>,
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// In-memory document space store backed by RwLock.
/// Production would persist to SQLite via the existing ContextGraph infrastructure.
pub struct DocumentStore {
    spaces: Arc<RwLock<HashMap<SpaceId, DocumentSpace>>>,
    pages: Arc<RwLock<HashMap<PageId, SpacePage>>>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            spaces: Arc::new(RwLock::new(HashMap::new())),
            pages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new document space.
    pub fn create_space(
        &self,
        name: String,
        description: String,
        scope: MemoryScope,
        owner_id: String,
        max_pages: Option<usize>,
        tags: Vec<String>,
    ) -> Result<DocumentSpace> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let space = DocumentSpace {
            id: id.clone(),
            name,
            description,
            scope,
            owner_id,
            created_at: now,
            updated_at: now,
            max_pages,
            tags,
        };

        let mut spaces = self.spaces.write().map_err(|_| MemoryError::LockPoisoned)?;
        spaces.insert(id, space.clone());
        Ok(space)
    }

    /// Get a space by ID.
    pub fn get_space(&self, space_id: &str) -> Result<Option<DocumentSpace>> {
        let spaces = self.spaces.read().map_err(|_| MemoryError::LockPoisoned)?;
        Ok(spaces.get(space_id).cloned())
    }

    /// List all spaces visible to a given scope/owner.
    pub fn list_spaces(&self, scope: Option<MemoryScope>, owner_id: Option<&str>) -> Result<Vec<DocumentSpace>> {
        let spaces = self.spaces.read().map_err(|_| MemoryError::LockPoisoned)?;
        Ok(spaces
            .values()
            .filter(|s| {
                let scope_match = scope.is_none_or(|sc| s.scope == sc);
                let owner_match = owner_id.is_none_or(|oid| s.owner_id == oid || s.scope == MemoryScope::Global);
                scope_match && owner_match
            })
            .cloned()
            .collect())
    }

    /// Write (create or update) a page in a space.
    /// If a page with the same space_id + title exists, creates a new version.
    pub fn write_page(&self, req: WritePageRequest) -> Result<SpacePage> {
        // Verify space exists
        {
            let spaces = self.spaces.read().map_err(|_| MemoryError::LockPoisoned)?;
            if !spaces.contains_key(&req.space_id) {
                return Err(MemoryError::NotFound(format!("space {}", req.space_id)));
            }
        }

        let content_hash = compute_hash(&req.content);
        let mime_type = req.mime_type.unwrap_or_else(|| "text/markdown".to_string());
        let now = Utc::now();

        // Check for existing page with same space + title
        let existing_id = {
            let pages = self.pages.read().map_err(|_| MemoryError::LockPoisoned)?;
            pages
                .values()
                .find(|p| p.space_id == req.space_id && p.title == req.title)
                .map(|p| p.id.clone())
        };

        let mut pages = self.pages.write().map_err(|_| MemoryError::LockPoisoned)?;

        if let Some(page_id) = existing_id {
            // Update existing page — bump version
            let page = pages.get_mut(&page_id).ok_or_else(|| MemoryError::NotFound(page_id.clone()))?;

            // Skip if content unchanged
            if page.content_hash == content_hash {
                return Ok(page.clone());
            }

            // Push current state to history
            page.version_history.push(VersionEntry {
                version: page.version,
                content_hash: page.content_hash.clone(),
                updated_at: page.updated_at,
                updated_by: page.created_by.clone(),
                change_summary: None,
            });

            page.content = req.content;
            page.content_hash = content_hash;
            page.mime_type = mime_type;
            page.version += 1;
            page.updated_at = now;
            page.created_by = req.created_by;
            if let Some(meta) = req.metadata {
                page.metadata = meta;
            }

            Ok(page.clone())
        } else {
            // Create new page
            let id = Uuid::new_v4().to_string();
            let page = SpacePage {
                id: id.clone(),
                space_id: req.space_id,
                title: req.title,
                content: req.content,
                content_hash,
                mime_type,
                version: 1,
                created_at: now,
                updated_at: now,
                created_by: req.created_by,
                version_history: Vec::new(),
                metadata: req.metadata.unwrap_or(serde_json::Value::Null),
            };
            pages.insert(id, page.clone());
            Ok(page)
        }
    }

    /// Read a page by ID.
    pub fn get_page(&self, page_id: &str) -> Result<Option<SpacePage>> {
        let pages = self.pages.read().map_err(|_| MemoryError::LockPoisoned)?;
        Ok(pages.get(page_id).cloned())
    }

    /// List pages matching a query.
    pub fn list_pages(&self, query: PageQuery) -> Result<Vec<SpacePage>> {
        let pages = self.pages.read().map_err(|_| MemoryError::LockPoisoned)?;
        let mut results: Vec<SpacePage> = pages
            .values()
            .filter(|p| {
                let space_match = query.space_id.as_ref().is_none_or(|sid| p.space_id == *sid);
                 let author_match = query.author.as_ref().is_none_or(|a| p.created_by == *a);
                space_match && author_match
            })
            .cloned()
            .collect();

        // Sort by updated_at descending
        results.sort_by_key(|a| std::cmp::Reverse(a.updated_at));

        // Apply pagination
        let start = query.offset.min(results.len());
        let end = (start + query.limit.max(1)).min(results.len());
        Ok(results[start..end].to_vec())
    }

    /// Get statistics for a space.
    pub fn space_stats(&self, space_id: &str) -> Result<SpaceStats> {
        let pages = self.pages.read().map_err(|_| MemoryError::LockPoisoned)?;
        let space_pages: Vec<&SpacePage> = pages.values().filter(|p| p.space_id == space_id).collect();

        let total_pages = space_pages.len();
        let total_versions: usize = space_pages.iter().map(|p| p.version_history.len() + 1).sum();
        let total_bytes: usize = space_pages.iter().map(|p| p.content.len()).sum();
        let last_updated = space_pages.iter().map(|p| p.updated_at).max();

        Ok(SpaceStats {
            total_pages,
            total_versions,
            total_bytes,
            last_updated,
        })
    }

    /// Delete a page by ID.
    pub fn delete_page(&self, page_id: &str) -> Result<bool> {
        let mut pages = self.pages.write().map_err(|_| MemoryError::LockPoisoned)?;
        Ok(pages.remove(page_id).is_some())
    }

    /// Total number of spaces.
    pub fn space_count(&self) -> usize {
        self.spaces.read().map(|s| s.len()).unwrap_or(0)
    }

    /// Total number of pages across all spaces.
    pub fn page_count(&self) -> usize {
        self.pages.read().map(|p| p.len()).unwrap_or(0)
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> DocumentStore {
        DocumentStore::new()
    }

    #[test]
    fn test_create_space() {
        let store = test_store();
        let space = store
            .create_space(
                "Engineering".into(),
                "Team engineering docs".into(),
                MemoryScope::Team,
                "team-eng".into(),
                Some(100),
                vec!["eng".into(), "docs".into()],
            )
            .unwrap();

        assert_eq!(space.name, "Engineering");
        assert_eq!(space.scope, MemoryScope::Team);
        assert_eq!(space.max_pages, Some(100));

        let retrieved = store.get_space(&space.id).unwrap().unwrap();
        assert_eq!(retrieved.name, "Engineering");
    }

    #[test]
    fn test_write_and_read_page() {
        let store = test_store();
        let space = store
            .create_space(
                "Wiki".into(),
                "".into(),
                MemoryScope::Global,
                "system".into(),
                None,
                vec![],
            )
            .unwrap();

        let page = store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "Architecture".into(),
                content: "# Architecture\n\nOverview...".into(),
                mime_type: Some("text/markdown".into()),
                created_by: "agent-architect".into(),
                change_summary: None,
                metadata: Some(serde_json::json!({"tags": ["architecture"]})),
            })
            .unwrap();

        assert_eq!(page.title, "Architecture");
        assert_eq!(page.version, 1);
        assert!(!page.content_hash.is_empty());

        let retrieved = store.get_page(&page.id).unwrap().unwrap();
        assert_eq!(retrieved.content, "# Architecture\n\nOverview...");
    }

    #[test]
    fn test_page_versioning() {
        let store = test_store();
        let space = store
            .create_space("Docs".into(), "".into(), MemoryScope::Agent, "agent-1".into(), None, vec![])
            .unwrap();

        // v1
        let p1 = store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "Spec".into(),
                content: "v1 content".into(),
                mime_type: None,
                created_by: "agent-1".into(),
                change_summary: Some("Initial draft".into()),
                metadata: None,
            })
            .unwrap();
        assert_eq!(p1.version, 1);
        assert!(p1.version_history.is_empty());

        // v2 — same title, different content
        let p2 = store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "Spec".into(),
                content: "v2 content updated".into(),
                mime_type: None,
                created_by: "agent-1".into(),
                change_summary: Some("Added details".into()),
                metadata: None,
            })
            .unwrap();
        assert_eq!(p2.version, 2);
        assert_eq!(p2.version_history.len(), 1);
        assert_eq!(p2.version_history[0].version, 1);

        // Same content → no new version
        let p3 = store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "Spec".into(),
                content: "v2 content updated".into(),
                mime_type: None,
                created_by: "agent-1".into(),
                change_summary: None,
                metadata: None,
            })
            .unwrap();
        assert_eq!(p3.version, 2); // unchanged
    }

    #[test]
    fn test_list_pages_with_pagination() {
        let store = test_store();
        let space = store
            .create_space("Blog".into(), "".into(), MemoryScope::Global, "system".into(), None, vec![])
            .unwrap();

        for i in 0..5 {
            store
                .write_page(WritePageRequest {
                    space_id: space.id.clone(),
                    title: format!("Post {}", i),
                    content: format!("Content {}", i),
                    mime_type: None,
                    created_by: "writer".into(),
                    change_summary: None,
                    metadata: None,
                })
                .unwrap();
        }

        let all = store.list_pages(PageQuery { space_id: Some(space.id.clone()), limit: 10, ..Default::default() }).unwrap();
        assert_eq!(all.len(), 5);

        let page = store.list_pages(PageQuery { space_id: Some(space.id.clone()), limit: 2, offset: 0, ..Default::default() }).unwrap();
        assert_eq!(page.len(), 2);

        let page2 = store.list_pages(PageQuery { space_id: Some(space.id.clone()), limit: 2, offset: 3, ..Default::default() }).unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[test]
    fn test_space_stats() {
        let store = test_store();
        let space = store
            .create_space("Stats".into(), "".into(), MemoryScope::Agent, "a1".into(), None, vec![])
            .unwrap();

        store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "A".into(),
                content: "hello world".into(),
                mime_type: None,
                created_by: "a1".into(),
                change_summary: None,
                metadata: None,
            })
            .unwrap();

        store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "A".into(),
                content: "hello world updated".into(),
                mime_type: None,
                created_by: "a1".into(),
                change_summary: None,
                metadata: None,
            })
            .unwrap();

        let stats = store.space_stats(&space.id).unwrap();
        assert_eq!(stats.total_pages, 1);
        assert_eq!(stats.total_versions, 2);
        assert!(stats.total_bytes > 0);
        assert!(stats.last_updated.is_some());
    }

    #[test]
    fn test_write_to_nonexistent_space_fails() {
        let store = test_store();
        let result = store.write_page(WritePageRequest {
            space_id: "nonexistent".into(),
            title: "Test".into(),
            content: "content".into(),
            mime_type: None,
            created_by: "agent".into(),
            change_summary: None,
            metadata: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_page() {
        let store = test_store();
        let space = store
            .create_space("Del".into(), "".into(), MemoryScope::Task, "t1".into(), None, vec![])
            .unwrap();

        let page = store
            .write_page(WritePageRequest {
                space_id: space.id.clone(),
                title: "ToDelete".into(),
                content: "bye".into(),
                mime_type: None,
                created_by: "t1".into(),
                change_summary: None,
                metadata: None,
            })
            .unwrap();

        assert_eq!(store.page_count(), 1);
        assert!(store.delete_page(&page.id).unwrap());
        assert_eq!(store.page_count(), 0);
        assert!(store.get_page(&page.id).unwrap().is_none());
    }

    #[test]
    fn test_list_spaces_by_scope() {
        let store = test_store();
        store.create_space("Global".into(), "".into(), MemoryScope::Global, "sys".into(), None, vec![]).unwrap();
        store.create_space("Team".into(), "".into(), MemoryScope::Team, "t1".into(), None, vec![]).unwrap();
        store.create_space("Agent".into(), "".into(), MemoryScope::Agent, "a1".into(), None, vec![]).unwrap();

        let global = store.list_spaces(Some(MemoryScope::Global), None).unwrap();
        assert_eq!(global.len(), 1);

        let all = store.list_spaces(None, None).unwrap();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = compute_hash("hello world");
        let h2 = compute_hash("hello world");
        assert_eq!(h1, h2);

        let h3 = compute_hash("different");
        assert_ne!(h1, h3);
    }
}