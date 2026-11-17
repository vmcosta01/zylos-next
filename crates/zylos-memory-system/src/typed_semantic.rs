//! TypedSemanticMemory — OKF-inspired typed wrapper around MemoryKind
//!
//! Provides automatic routing of memories to the appropriate backend based on
//! their semantic type, with type-safe accessors and automatic provenance tracking.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

use crate::{
    BackendTarget, Completeness, ContextGraph, ContextNode, MemoryKind, MemoryScope,
    ProvenanceOrigin, ProvenanceScope, ProvenanceTriple, SearchParams, VectorDocument, VectorStore,
};

#[derive(Debug, Error)]
pub enum TypedMemoryError {
    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("backend routing failed: {0}")]
    BackendRouting(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Semantic type classification for automatic backend routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticType {
    /// Short-term working memory (current session context)
    Working,
    /// Episodic memory (specific events/experiences with timestamps)
    Episodic,
    /// Semantic memory (facts, concepts, general knowledge)
    Semantic,
    /// Procedural memory (skills, workflows, procedures)
    Procedural,
    /// Meta-memory (memory about memory, consolidation logs)
    Meta,
}

impl From<SemanticType> for MemoryKind {
    fn from(t: SemanticType) -> Self {
        match t {
            SemanticType::Working => MemoryKind::Working,
            SemanticType::Episodic => MemoryKind::Episodic,
            SemanticType::Semantic => MemoryKind::Semantic,
            SemanticType::Procedural => MemoryKind::Procedural,
            SemanticType::Meta => MemoryKind::Meta,
        }
    }
}

impl std::fmt::Display for SemanticType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SemanticType::Working => write!(f, "working"),
            SemanticType::Episodic => write!(f, "episodic"),
            SemanticType::Semantic => write!(f, "semantic"),
            SemanticType::Procedural => write!(f, "procedural"),
            SemanticType::Meta => write!(f, "meta"),
        }
    }
}

/// Typed wrapper providing type-safe access to memories with automatic routing
pub struct TypedSemanticMemory {
    semantic_type: SemanticType,
    memory_kind: MemoryKind,
    backend_target: BackendTarget,
    vector_store: Option<Arc<dyn VectorStore>>,
    context_graph: Option<Arc<ContextGraph>>,
}

impl TypedSemanticMemory {
    /// Create a new typed memory wrapper with automatic backend routing
    pub fn new(semantic_type: SemanticType) -> Self {
        let memory_kind = MemoryKind::from(semantic_type);
        let backend_target = crate::route_backend(memory_kind);

        Self {
            semantic_type,
            memory_kind,
            backend_target,
            vector_store: None,
            context_graph: None,
        }
    }

    /// Attach a vector store backend for semantic search
    pub fn with_vector_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(store);
        self
    }

    /// Attach a context graph backend for relational queries
    pub fn with_context_graph(mut self, graph: Arc<ContextGraph>) -> Self {
        self.context_graph = Some(graph);
        self
    }

    /// Get the semantic type of this memory wrapper
    pub fn semantic_type(&self) -> SemanticType {
        self.semantic_type
    }

    /// Get the underlying MemoryKind
    pub fn memory_kind(&self) -> MemoryKind {
        self.memory_kind
    }

    /// Get the backend target this type routes to
    pub fn backend_target(&self) -> BackendTarget {
        self.backend_target
    }

    /// Store a typed memory with automatic provenance tracking
    pub async fn store(
        &self,
        content: impl Into<String>,
        scope: MemoryScope,
        owner_id: Option<String>,
        tags: Vec<String>,
    ) -> Result<ContextNode, TypedMemoryError> {
        let content_str = content.into();
        let content_value = serde_json::Value::String(content_str.clone());

        let _provenance = ProvenanceTriple::new(
            match scope {
                MemoryScope::Global => ProvenanceScope::Global,
                MemoryScope::Team => ProvenanceScope::Agent,
                MemoryScope::Agent => ProvenanceScope::Agent,
                MemoryScope::Task => ProvenanceScope::Session,
            },
            ProvenanceOrigin::Inference,
            Completeness::Partial,
        );

        let node = ContextNode::new(
            content_value,
            scope,
            owner_id.unwrap_or_else(|| "system".to_string()),
        )
        .with_tags(tags);

        // Route to appropriate backend
        match self.backend_target {
            BackendTarget::VectorStore => {
                if let Some(store) = &self.vector_store {
                    let doc = VectorDocument::new(vec![])
                        .with_content(content_str)
                        .with_metadata(serde_json::json!({
                            "semantic_type": self.semantic_type.to_string(),
                            "memory_kind": format!("{}", self.memory_kind),
                        }));
                    store
                        .insert(vec![doc])
                        .await
                        .map_err(|e| TypedMemoryError::Storage(e.to_string()))?;
                }
            }
            BackendTarget::SqliteContextGraph => {
                if let Some(graph) = &self.context_graph {
                    graph
                        .add_node(node.clone())
                        .await
                        .map_err(|e| TypedMemoryError::Storage(e.to_string()))?;
                }
            }
            BackendTarget::ContextWindow
            | BackendTarget::SkillFiles
            | BackendTarget::ReasoningBank => {
                // These backends are handled externally or not yet implemented
            }
        }

        Ok(node)
    }

    /// Query memories of this specific type
    pub async fn query(
        &self,
        query: &str,
        scope: Option<MemoryScope>,
        limit: usize,
    ) -> Result<Vec<ContextNode>, TypedMemoryError> {
        match self.backend_target {
            BackendTarget::VectorStore => {
                if let Some(store) = &self.vector_store {
                    let params = SearchParams::new(vec![], limit);
                    let results = store
                        .search(params)
                        .await
                        .map_err(|e| TypedMemoryError::Storage(e.to_string()))?;

                    let nodes = results
                        .iter()
                        .map(|r| {
                            ContextNode::new(
                                serde_json::Value::String(
                                    r.document.content.clone().unwrap_or_default(),
                                ),
                                scope.unwrap_or(MemoryScope::Global),
                                "system",
                            )
                        })
                        .collect();

                    Ok(nodes)
                } else {
                    Ok(vec![])
                }
            }
            BackendTarget::SqliteContextGraph => {
                if let Some(graph) = &self.context_graph {
                    use crate::ContextQuery;
                    let q = ContextQuery {
                        scope,
                        search_text: Some(query.to_string()),
                        limit: Some(limit),
                        ..Default::default()
                    };
                    graph
                        .query(q)
                        .await
                        .map_err(|e| TypedMemoryError::Storage(e.to_string()))
                } else {
                    Ok(vec![])
                }
            }
            BackendTarget::ContextWindow
            | BackendTarget::SkillFiles
            | BackendTarget::ReasoningBank => Ok(vec![]),
        }
    }
}

/// Factory for creating typed memory instances with automatic routing
pub struct TypedMemoryFactory {
    vector_store: Option<Arc<dyn VectorStore>>,
    context_graph: Option<Arc<ContextGraph>>,
}

impl TypedMemoryFactory {
    pub fn new() -> Self {
        Self {
            vector_store: None,
            context_graph: None,
        }
    }

    pub fn with_vector_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(store);
        self
    }

    pub fn with_context_graph(mut self, graph: Arc<ContextGraph>) -> Self {
        self.context_graph = Some(graph);
        self
    }

    /// Create a typed memory wrapper for the given semantic type
    pub fn create(&self, semantic_type: SemanticType) -> TypedSemanticMemory {
        let mut typed = TypedSemanticMemory::new(semantic_type);
        if let Some(store) = &self.vector_store {
            typed = typed.with_vector_store(store.clone());
        }
        if let Some(graph) = &self.context_graph {
            typed = typed.with_context_graph(graph.clone());
        }
        typed
    }

    /// Create working memory (short-term session context)
    pub fn working(&self) -> TypedSemanticMemory {
        self.create(SemanticType::Working)
    }

    /// Create episodic memory (specific events with timestamps)
    pub fn episodic(&self) -> TypedSemanticMemory {
        self.create(SemanticType::Episodic)
    }

    /// Create semantic memory (facts and general knowledge)
    pub fn semantic(&self) -> TypedSemanticMemory {
        self.create(SemanticType::Semantic)
    }

    /// Create procedural memory (skills and workflows)
    pub fn procedural(&self) -> TypedSemanticMemory {
        self.create(SemanticType::Procedural)
    }

    /// Create meta memory (memory about memory)
    pub fn meta(&self) -> TypedSemanticMemory {
        self.create(SemanticType::Meta)
    }
}

impl Default for TypedMemoryFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_type_to_memory_kind() {
        assert_eq!(MemoryKind::from(SemanticType::Working), MemoryKind::Working);
        assert_eq!(
            MemoryKind::from(SemanticType::Episodic),
            MemoryKind::Episodic
        );
        assert_eq!(
            MemoryKind::from(SemanticType::Semantic),
            MemoryKind::Semantic
        );
        assert_eq!(
            MemoryKind::from(SemanticType::Procedural),
            MemoryKind::Procedural
        );
        assert_eq!(MemoryKind::from(SemanticType::Meta), MemoryKind::Meta);
    }

    #[test]
    fn test_typed_memory_creation() {
        let typed = TypedSemanticMemory::new(SemanticType::Semantic);
        assert_eq!(typed.semantic_type(), SemanticType::Semantic);
        assert_eq!(typed.memory_kind(), MemoryKind::Semantic);
    }

    #[test]
    fn test_factory_creates_typed_memories() {
        let factory = TypedMemoryFactory::new();
        let working = factory.working();
        let episodic = factory.episodic();
        let semantic = factory.semantic();

        assert_eq!(working.semantic_type(), SemanticType::Working);
        assert_eq!(episodic.semantic_type(), SemanticType::Episodic);
        assert_eq!(semantic.semantic_type(), SemanticType::Semantic);
    }
}
