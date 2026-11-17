//! SpacesStore — persistência SQLite para Spaces/Docs Tab
//!
//! CRUD de Spaces e Pages com suporte a hierarquia pai-filho,
//! outline de headings e ordenação por posição.

use crate::schema::*;
use sqlx::SqlitePool;
use uuid::Uuid;

/// Store para operações CRUD de Spaces e Pages
#[derive(Clone)]
pub struct SpacesStore {
    pool: SqlitePool,
}

impl SpacesStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // === Spaces ===

    /// Cria novo Space
    pub async fn create_space(&self, req: CreateSpaceRequest) -> Result<Space, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO spaces (id, name, description, owner_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.owner_id)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        tracing::info!(space_id = %id, name = %req.name, "Space created");

        Ok(Space {
            id,
            name: req.name,
            description: req.description,
            owner_id: req.owner_id,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Lista todos os Spaces
    pub async fn list_spaces(&self) -> Result<Vec<Space>, sqlx::Error> {
        sqlx::query_as::<_, Space>("SELECT * FROM spaces ORDER BY name")
            .fetch_all(&self.pool)
            .await
    }

    /// Busca Space por ID
    pub async fn get_space(&self, space_id: &str) -> Result<Option<Space>, sqlx::Error> {
        sqlx::query_as::<_, Space>("SELECT * FROM spaces WHERE id = ?")
            .bind(space_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Deleta Space e todas as suas páginas
    pub async fn delete_space(&self, space_id: &str) -> Result<u64, sqlx::Error> {
        // Deletar páginas primeiro (cascade não automático em SQLite sem FK)
        sqlx::query("DELETE FROM pages WHERE space_id = ?")
            .bind(space_id)
            .execute(&self.pool)
            .await?;

        let result = sqlx::query("DELETE FROM spaces WHERE id = ?")
            .bind(space_id)
            .execute(&self.pool)
            .await?;

        tracing::info!(space_id = %space_id, rows = result.rows_affected(), "Space deleted");
        Ok(result.rows_affected())
    }

    // === Pages ===

    /// Cria nova Page em um Space
    pub async fn create_page(
        &self,
        space_id: &str,
        req: CreatePageRequest,
    ) -> Result<Page, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let content = req.content_markdown.unwrap_or_default();
        let position = req.position.unwrap_or(0);

        sqlx::query(
            "INSERT INTO pages (id, space_id, parent_id, title, content_markdown, position, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(space_id)
        .bind(&req.parent_id)
        .bind(&req.title)
        .bind(&content)
        .bind(position)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        tracing::info!(page_id = %id, space_id = %space_id, title = %req.title, "Page created");

        Ok(Page {
            id,
            space_id: space_id.to_string(),
            parent_id: req.parent_id,
            title: req.title,
            content_markdown: content,
            position,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    /// Lista páginas de um Space (plana + árvore)
    pub async fn list_pages(&self, space_id: &str) -> Result<PagesListResponse, sqlx::Error> {
        let pages = sqlx::query_as::<_, Page>(
            "SELECT * FROM pages WHERE space_id = ? ORDER BY position, created_at",
        )
        .bind(space_id)
        .fetch_all(&self.pool)
        .await?;

        let tree = build_page_tree(&pages);
        let total = pages.len();

        Ok(PagesListResponse { pages, tree, total })
    }

    /// Busca Page por ID
    pub async fn get_page(&self, page_id: &str) -> Result<Option<Page>, sqlx::Error> {
        sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
            .bind(page_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Atualiza Page (título, conteúdo, parent, posição)
    pub async fn update_page(
        &self,
        page_id: &str,
        req: UpdatePageRequest,
    ) -> Result<Page, sqlx::Error> {
        let existing = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
            .bind(page_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;

        let title = req.title.unwrap_or(existing.title);
        let content = req.content_markdown.unwrap_or(existing.content_markdown);
        let parent_id = req.parent_id.or(existing.parent_id);
        let position = req.position.unwrap_or(existing.position);
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE pages SET title = ?, content_markdown = ?, parent_id = ?, position = ?, updated_at = ? WHERE id = ?"
        )
        .bind(&title)
        .bind(&content)
        .bind(&parent_id)
        .bind(position)
        .bind(&now)
        .bind(page_id)
        .execute(&self.pool)
        .await?;

        tracing::info!(page_id = %page_id, title = %title, "Page updated");

        Ok(Page {
            id: page_id.to_string(),
            space_id: existing.space_id,
            parent_id,
            title,
            content_markdown: content,
            position,
            created_at: existing.created_at,
            updated_at: now,
        })
    }

    /// Deleta Page e todas as subpáginas recursivamente
    pub async fn delete_page(&self, page_id: &str) -> Result<u64, sqlx::Error> {
        // Coletar IDs de subpáginas recursivamente (filhos primeiro para respeitar FK)
        let mut to_delete = vec![page_id.to_string()];
        let mut idx = 0;

        while idx < to_delete.len() {
            let current_id = &to_delete[idx];
            let children: Vec<(String,)> =
                sqlx::query_as("SELECT id FROM pages WHERE parent_id = ?")
                    .bind(current_id)
                    .fetch_all(&self.pool)
                    .await?;

            for (child_id,) in children {
                to_delete.push(child_id);
            }
            idx += 1;
        }

        // Deletar em ordem reversa (filhos antes dos pais) para respeitar FK constraints
        to_delete.reverse();
        let mut total_deleted = 0u64;
        for id in &to_delete {
            let result = sqlx::query("DELETE FROM pages WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;
            total_deleted += result.rows_affected();
        }

        tracing::info!(page_id = %page_id, deleted = total_deleted, "Page and subpages deleted");
        Ok(total_deleted)
    }

    /// Retorna outline (headings) de uma página
    pub async fn get_outline(&self, page_id: &str) -> Result<Vec<OutlineNode>, sqlx::Error> {
        let page = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = ?")
            .bind(page_id)
            .fetch_optional(&self.pool)
            .await?;

        match page {
            Some(p) => Ok(extract_outline(&p.content_markdown)),
            None => Ok(vec![]),
        }
    }

    /// Aplica migrations SQL (statements individuais para sqlx SQLite)
    pub async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS spaces (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                owner_id TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pages (
                id TEXT PRIMARY KEY,
                space_id TEXT NOT NULL REFERENCES spaces(id),
                parent_id TEXT REFERENCES pages(id),
                title TEXT NOT NULL,
                content_markdown TEXT NOT NULL DEFAULT '',
                position INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_pages_space ON pages(space_id, position)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_pages_parent ON pages(parent_id)")
            .execute(&self.pool)
            .await?;

        tracing::info!("Spaces migrations applied successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_test_store() -> SpacesStore {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        let store = SpacesStore::new(pool);
        store.run_migrations().await.unwrap();
        store
    }

    fn sample_space() -> CreateSpaceRequest {
        CreateSpaceRequest {
            name: "Test Space".into(),
            description: Some("A test space".into()),
            owner_id: "user-1".into(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_space() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        assert_eq!(space.name, "Test Space");
        assert_eq!(space.owner_id, "user-1");

        let retrieved = store.get_space(&space.id).await.unwrap().unwrap();
        assert_eq!(retrieved.id, space.id);
        assert_eq!(retrieved.name, "Test Space");
    }

    #[tokio::test]
    async fn test_list_spaces() {
        let store = setup_test_store().await;
        store.create_space(sample_space()).await.unwrap();
        store
            .create_space(CreateSpaceRequest {
                name: "Second Space".into(),
                description: None,
                owner_id: "user-2".into(),
            })
            .await
            .unwrap();

        let spaces = store.list_spaces().await.unwrap();
        assert_eq!(spaces.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_space_cascades_pages() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Page 1".into(),
                    content_markdown: Some("# Hello".into()),
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        store.delete_space(&space.id).await.unwrap();

        assert!(store.get_space(&space.id).await.unwrap().is_none());
        let pages = store.list_pages(&space.id).await.unwrap();
        assert_eq!(pages.total, 0);
    }

    #[tokio::test]
    async fn test_create_and_get_page() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let page = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "My Page".into(),
                    content_markdown: Some("# Title\n## Section\nContent here.".into()),
                    parent_id: None,
                    position: Some(0),
                },
            )
            .await
            .unwrap();

        assert_eq!(page.title, "My Page");
        assert_eq!(page.space_id, space.id);
        assert!(page.parent_id.is_none());

        let retrieved = store.get_page(&page.id).await.unwrap().unwrap();
        assert_eq!(
            retrieved.content_markdown,
            "# Title\n## Section\nContent here."
        );
    }

    #[tokio::test]
    async fn test_list_pages_with_tree() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let root = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Root".into(),
                    content_markdown: None,
                    parent_id: None,
                    position: Some(0),
                },
            )
            .await
            .unwrap();

        store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Child".into(),
                    content_markdown: None,
                    parent_id: Some(root.id.clone()),
                    position: Some(0),
                },
            )
            .await
            .unwrap();

        let response = store.list_pages(&space.id).await.unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.tree.len(), 1); // Only root at top level
        assert_eq!(response.tree[0].children.len(), 1); // One child
        assert_eq!(response.tree[0].children[0].title, "Child");
    }

    #[tokio::test]
    async fn test_update_page() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let page = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Original".into(),
                    content_markdown: Some("Old content".into()),
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        let updated = store
            .update_page(
                &page.id,
                UpdatePageRequest {
                    title: Some("Updated Title".into()),
                    content_markdown: Some("New content".into()),
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.content_markdown, "New content");
        assert_ne!(updated.updated_at, updated.created_at);
    }

    #[tokio::test]
    async fn test_update_page_partial() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let page = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Original".into(),
                    content_markdown: Some("Content".into()),
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        // Atualizar apenas título
        let updated = store
            .update_page(
                &page.id,
                UpdatePageRequest {
                    title: Some("New Title".into()),
                    content_markdown: None,
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.title, "New Title");
        assert_eq!(updated.content_markdown, "Content"); // Preserved
    }

    #[tokio::test]
    async fn test_delete_page_recursive() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let root = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Root".into(),
                    content_markdown: None,
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        let child = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Child".into(),
                    content_markdown: None,
                    parent_id: Some(root.id.clone()),
                    position: None,
                },
            )
            .await
            .unwrap();

        store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Grandchild".into(),
                    content_markdown: None,
                    parent_id: Some(child.id.clone()),
                    position: None,
                },
            )
            .await
            .unwrap();

        // Deletar root deve deletar child e grandchild também
        let deleted = store.delete_page(&root.id).await.unwrap();
        assert_eq!(deleted, 3);

        let pages = store.list_pages(&space.id).await.unwrap();
        assert_eq!(pages.total, 0);
    }

    #[tokio::test]
    async fn test_get_outline() {
        let store = setup_test_store().await;
        let space = store.create_space(sample_space()).await.unwrap();

        let page = store
            .create_page(
                &space.id,
                CreatePageRequest {
                    title: "Doc".into(),
                    content_markdown: Some(
                        "# Intro\n## Setup\n### Step 1\nText\n## Usage\n".into(),
                    ),
                    parent_id: None,
                    position: None,
                },
            )
            .await
            .unwrap();

        let outline = store.get_outline(&page.id).await.unwrap();
        assert_eq!(outline.len(), 4);
        assert_eq!(outline[0].level, 1);
        assert_eq!(outline[0].text, "Intro");
        assert_eq!(outline[1].level, 2);
        assert_eq!(outline[1].text, "Setup");
        assert_eq!(outline[2].level, 3);
        assert_eq!(outline[2].text, "Step 1");
        assert_eq!(outline[3].level, 2);
        assert_eq!(outline[3].text, "Usage");
    }

    #[tokio::test]
    async fn test_get_outline_nonexistent_page() {
        let store = setup_test_store().await;
        let outline = store.get_outline("nonexistent").await.unwrap();
        assert!(outline.is_empty());
    }

    #[tokio::test]
    async fn test_update_nonexistent_page_returns_error() {
        let store = setup_test_store().await;
        let result = store
            .update_page(
                "nonexistent",
                UpdatePageRequest {
                    title: Some("X".into()),
                    content_markdown: None,
                    parent_id: None,
                    position: None,
                },
            )
            .await;
        assert!(result.is_err());
    }
}
