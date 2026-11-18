//! zylos-auth — Multi-tenant authentication, JWT tokens, RBAC and CRDT collaboration primitives.
//!
//! This crate provides:
//! - JWT token generation and validation (stateless Bearer tokens)
//! - Role-based access control (RBAC) with tenant isolation
//! - Axum middleware for protecting routes
//! - CRDT sync primitives for collaborative editing (Yjs-compatible WebSocket protocol)
//! - Presence tracking for real-time collaboration indicators

pub mod claims;
pub mod crdt;
pub mod crdt_persistence;
pub mod middleware;
pub mod permissions;
pub mod presence;
pub mod revocation_store;
pub mod roles;
pub mod tokens;

pub use claims::{Claims, TenantId};
pub use crdt::{CrdtMessage, CrdtSyncServer, DocumentId};
pub use crdt_persistence::{CrdtStore, CrdtStoreError, GcReport, VersionInfo};
pub use middleware::auth_middleware;
pub use permissions::Permission;
pub use presence::{PresenceEvent, PresenceTracker};
pub use roles::Role;
pub use tokens::{TokenConfig, TokenService};
