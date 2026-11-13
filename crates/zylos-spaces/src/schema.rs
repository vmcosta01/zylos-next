//! Schema de dados para Spaces/Docs Tab
//!
//! Tipos serializáveis para persistência SQLite e comunicação via API REST.
//! Suporta hierarquia de páginas (pai-filho) e outline de headings markdown.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Um Space (container de documentos)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Uma Page (documento markdown dentro de um Space)
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Page {
    pub id: String,
    pub space_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub content_markdown: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Nó da árvore de outline (headings extraídos do markdown)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineNode {
    pub level: u8,      // 1-6 (h1-h6)
    pub text: String,   // Texto do heading
    pub anchor: String, // Slug para âncora HTML
}

/// Nó da árvore de páginas (para navegação hierárquica)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTreeNode {
    pub id: String,
    pub title: String,
    pub position: i64,
    pub children: Vec<PageTreeNode>,
}

/// Request para criar novo Space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSpaceRequest {
    pub name: String,
    pub description: Option<String>,
    pub owner_id: String,
}

/// Request para criar nova Page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePageRequest {
    pub title: String,
    pub content_markdown: Option<String>,
    pub parent_id: Option<String>,
    pub position: Option<i64>,
}

/// Request para atualizar Page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePageRequest {
    pub title: Option<String>,
    pub content_markdown: Option<String>,
    pub parent_id: Option<String>,
    pub position: Option<i64>,
}

/// Response de listagem de páginas com hierarquia
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagesListResponse {
    pub pages: Vec<Page>,
    pub tree: Vec<PageTreeNode>,
    pub total: usize,
}

/// Extrai outline (headings) de conteúdo markdown
pub fn extract_outline(markdown: &str) -> Vec<OutlineNode> {
    let mut nodes = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count() as u8;
            if (1..=6).contains(&level) {
                let text = trimmed[level as usize..].trim().to_string();
                if !text.is_empty() {
                    let anchor = text
                     .to_lowercase()
                     .chars()
                     .map(|c| {
                     if c.is_alphanumeric() || c == '-' {
                     c
                     } else {
                     '-'
                     }
                     })
                     .collect::<String>()
                     .split('-')
                     .filter(|s| !s.is_empty())
                     .collect::<Vec<_>>()
                     .join("-");
                    nodes.push(OutlineNode { level, text, anchor });
                }
            }
        }
    }
    nodes
}

/// Constrói árvore de páginas a partir de lista plana
pub fn build_page_tree(pages: &[Page]) -> Vec<PageTreeNode> {
    let mut children_map: std::collections::HashMap<Option<String>, Vec<&Page>> =
        std::collections::HashMap::new();

    for page in pages {
        children_map
            .entry(page.parent_id.clone())
            .or_default()
            .push(page);
    }

    fn build_children(
        parent_id: Option<String>,
        map: &std::collections::HashMap<Option<String>, Vec<&Page>>,
    ) -> Vec<PageTreeNode> {
        let mut nodes = Vec::new();
        if let Some(children) = map.get(&parent_id) {
            for page in children {
                let node = PageTreeNode {
                    id: page.id.clone(),
                    title: page.title.clone(),
                    position: page.position,
                    children: build_children(Some(page.id.clone()), map),
                };
                nodes.push(node);
            }
        }
        nodes.sort_by_key(|n| n.position);
        nodes
    }

    build_children(None, &children_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_outline_basic() {
        let md = "# Título Principal\n## Seção 1\n### Subseção 1.1\nTexto normal\n## Seção 2\n";
        let outline = extract_outline(md);
        assert_eq!(outline.len(), 4);
        assert_eq!(outline[0].level, 1);
        assert_eq!(outline[0].text, "Título Principal");
        assert_eq!(outline[1].level, 2);
        assert_eq!(outline[1].text, "Seção 1");
        assert_eq!(outline[2].level, 3);
        assert_eq!(outline[2].text, "Subseção 1.1");
        assert_eq!(outline[3].level, 2);
        assert_eq!(outline[3].text, "Seção 2");
    }

    #[test]
    fn test_extract_outline_anchors() {
        let md = "# Hello World\n## Configuração Inicial\n### Passo-a-Passo\n";
        let outline = extract_outline(md);
        assert_eq!(outline[0].anchor, "hello-world");
        assert_eq!(outline[1].anchor, "configuração-inicial");
        assert_eq!(outline[2].anchor, "passo-a-passo");
    }

    #[test]
    fn test_extract_outline_empty() {
        let outline = extract_outline("Texto sem headings\nApenas parágrafos\n");
        assert!(outline.is_empty());
    }

    #[test]
    fn test_extract_outline_invalid_level() {
        let md = "####### Nível 7 inválido\n# Válido\n";
        let outline = extract_outline(md);
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].text, "Válido");
    }

    #[test]
    fn test_build_page_tree_flat() {
        let pages = vec![
            Page {
                id: "p1".into(),
                space_id: "s1".into(),
                parent_id: None,
                title: "Root 1".into(),
                content_markdown: "".into(),
                position: 0,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Page {
                id: "p2".into(),
                space_id: "s1".into(),
                parent_id: None,
                title: "Root 2".into(),
                content_markdown: "".into(),
                position: 1,
                created_at: "".into(),
                updated_at: "".into(),
            },
        ];
        let tree = build_page_tree(&pages);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree[0].title, "Root 1");
        assert_eq!(tree[1].title, "Root 2");
        assert!(tree[0].children.is_empty());
    }

    #[test]
    fn test_build_page_tree_nested() {
        let pages = vec![
            Page {
                id: "p1".into(),
                space_id: "s1".into(),
                parent_id: None,
                title: "Root".into(),
                content_markdown: "".into(),
                position: 0,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Page {
                id: "p2".into(),
                space_id: "s1".into(),
                parent_id: Some("p1".into()),
                title: "Child 1".into(),
                content_markdown: "".into(),
                position: 0,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Page {
                id: "p3".into(),
                space_id: "s1".into(),
                parent_id: Some("p1".into()),
                title: "Child 2".into(),
                content_markdown: "".into(),
                position: 1,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Page {
                id: "p4".into(),
                space_id: "s1".into(),
                parent_id: Some("p2".into()),
                title: "Grandchild".into(),
                content_markdown: "".into(),
                position: 0,
                created_at: "".into(),
                updated_at: "".into(),
            },
        ];
        let tree = build_page_tree(&pages);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].title, "Root");
        assert_eq!(tree[0].children.len(), 2);
        assert_eq!(tree[0].children[0].title, "Child 1");
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert_eq!(tree[0].children[0].children[0].title, "Grandchild");
        assert_eq!(tree[0].children[1].title, "Child 2");
    }

    #[test]
    fn test_build_page_tree_empty() {
        let tree = build_page_tree(&[]);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_create_space_request_serialization() {
        let req = CreateSpaceRequest {
            name: "My Space".into(),
            description: Some("Test space".into()),
            owner_id: "user-1".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("My Space"));
        assert!(json.contains("user-1"));
    }

    #[test]
    fn test_update_page_request_partial() {
        let req = UpdatePageRequest {
            title: Some("New Title".into()),
            content_markdown: None,
            parent_id: None,
            position: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("New Title"));
        assert!(json.contains("null"));
    }
}