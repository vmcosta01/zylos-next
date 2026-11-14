//! Zylos Memory System — Persistent context graph with SQLite WAL backend
//!
//! This crate implements the MEMORY_SYSTEM for Zylos-Next, providing:
//! - ContextGraph: Persistent graph storage with SQLite WAL + petgraph
//! - 4-scope memory isolation (Global, Team, Agent, Task)
//! - Embedding-based similarity search
//! - Neighborhood queries and shortest path calculations

pub mod auto_improvement;
pub mod context_graph;
pub mod context_room;
pub mod contradiction;
pub mod document_spaces;
pub mod error;
#[cfg(feature = "lancedb")]
pub mod lancedb_adapter;
pub mod lifecycle;
pub mod memory_agent;
pub mod memory_kind;
pub mod mental_models;
pub mod migration;
#[cfg(feature = "qdrant")]
pub mod qdrant_adapter;
pub mod rerank;
pub mod session_goal;
pub mod typed_semantic;
pub mod types;
pub mod utils;
pub mod vector_store;
pub mod write_gate;

pub use context_graph::{ContextGraph, GraphStats};
pub use contradiction::{Contradiction, ContradictionDetector, ContradictionType, Resolution};
pub use error::{MemoryError, Result};
pub use memory_agent::{AgentEvent, ConsolidationReport, MemoryAgent, MemoryAgentConfig};
pub use memory_kind::{
    route_backend, BackendTarget, Completeness, CompressionPipeline, DecayScore, EntityLink,
    EvidenceCapsule, EvidenceDisposition, ExtractedFact, GcAction, GcDisposition, MemoryKind,
    ProvenanceOrigin, ProvenanceScope, ProvenanceTriple,
};
pub use migration::{IntegrityReport, MigrationAdapter, MigrationReport};
pub use rerank::{BriefAwareReranker, RankedMemory, RerankConfig, ResearchBrief};
pub use session_goal::{Criterion, GoalStatus, SessionGoal};
pub use types::{ContextEdge, ContextNode, ContextQuery, MemoryScope, NodeId};
pub use vector_store::{
    InMemoryVectorStore, SearchParams, SearchResult, VectorDocument, VectorId, VectorStore,
};
pub use write_gate::{
    Disposition, EvidenceCapsule as WriteGateEvidenceCapsule, GateResult, WriteGate,
};
