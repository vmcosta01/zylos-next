//! Zylos Memory System — Persistent context graph with SQLite WAL backend
//!
//! This crate implements the MEMORY_SYSTEM for Zylos-Next, providing:
//! - ContextGraph: Persistent graph storage with SQLite WAL + petgraph
//! - 4-scope memory isolation (Global, Team, Agent, Task)
//! - Embedding-based similarity search
//! - Neighborhood queries and shortest path calculations

pub mod error;
pub mod types;
pub mod memory_kind;
pub mod utils;
pub mod context_graph;
pub mod vector_store;
#[cfg(feature = "qdrant")]
pub mod qdrant_adapter;
#[cfg(feature = "lancedb")]
pub mod lancedb_adapter;
pub mod rerank;
pub mod contradiction;
pub mod memory_agent;
pub mod write_gate;
pub mod session_goal;
pub mod migration;
pub mod document_spaces;

pub use error::{MemoryError, Result};
pub use types::{ContextNode, ContextEdge, ContextQuery, MemoryScope, NodeId};
pub use memory_kind::{
    MemoryKind, BackendTarget, route_backend,
    ProvenanceTriple, ProvenanceScope, ProvenanceOrigin, Completeness,
    CompressionPipeline, ExtractedFact, EntityLink, DecayScore, GcAction, GcDisposition,
    EvidenceCapsule, EvidenceDisposition,
};
pub use context_graph::{ContextGraph, GraphStats};
pub use vector_store::{VectorStore, VectorDocument, VectorId, SearchParams, SearchResult, InMemoryVectorStore};
pub use rerank::{BriefAwareReranker, RerankConfig, ResearchBrief, RankedMemory};
pub use contradiction::{Contradiction, ContradictionDetector, ContradictionType, Resolution};
pub use memory_agent::{MemoryAgent, MemoryAgentConfig, AgentEvent, ConsolidationReport};
pub use write_gate::{WriteGate, EvidenceCapsule as WriteGateEvidenceCapsule, Disposition, GateResult};
pub use session_goal::{SessionGoal, Criterion, GoalStatus};
pub use migration::{MigrationAdapter, MigrationReport, IntegrityReport};
