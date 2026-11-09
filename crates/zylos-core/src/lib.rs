//! zylos-core — foundational types shared across all Zylos-Next crates.
//!
//! This crate provides the common error types, identifiers, and trait
//! definitions that every other crate in the workspace depends on.
//! It has no dependencies on other `zylos-*` crates to avoid circular
//! imports.
//!
//! # Quick Start
//!
//! ```rust
//! use zylos_core::{AgentId, CoreError, Result};
//!
//! let id = AgentId::new("worker-1");
//! assert_eq!(id.as_str(), "worker-1");
//!
//! let err = CoreError::not_found("agent", "worker-42");
//! assert!(err.to_string().contains("not found"));
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

/// Unique identifier for an agent within the Zylos system.
///
/// Wraps a `String` with validation and convenience constructors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new `AgentId` from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Return the identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume self and return the inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ---------------------------------------------------------------------------
// CoreError
// ---------------------------------------------------------------------------

/// Top-level error type for cross-cutting failures in the Zylos workspace.
///
/// Each variant carries enough context to produce actionable log messages
/// without requiring callers to wrap errors further.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoreError {
    /// A requested entity was not found.
    #[error("{entity} not found: {detail}")]
    NotFound {
        /// Kind of entity (e.g. "agent", "task", "profile").
        entity: String,
        /// Human-readable detail or identifier.
        detail: String,
    },

    /// An operation was attempted in an invalid state.
    #[error("invalid state for {operation}: {reason}")]
    InvalidState {
        /// The operation that failed.
        operation: String,
        /// Why the current state is invalid.
        reason: String,
    },

    /// A configuration value is missing or malformed.
    #[error("config error: {message}")]
    Config {
        /// Description of the configuration problem.
        message: String,
    },

    /// Catch-all for errors originating from external systems.
    #[error("external error [{source_name}]: {message}")]
    External {
        /// Name of the external system (e.g. "docker", "github").
        source_name: String,
        /// Error description from the external system.
        message: String,
    },
}

impl CoreError {
    /// Convenience constructor for [`CoreError::NotFound`].
    pub fn not_found(entity: impl Into<String>, detail: impl fmt::Display) -> Self {
        Self::NotFound {
            entity: entity.into(),
            detail: detail.to_string(),
        }
    }

    /// Convenience constructor for [`CoreError::InvalidState`].
    pub fn invalid_state(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidState {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Convenience constructor for [`CoreError::Config`].
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Convenience constructor for [`CoreError::External`].
    pub fn external(source_name: impl Into<String>, message: impl Into<String>) -> Self {
        Self::External {
            source_name: source_name.into(),
            message: message.into(),
        }
    }
}

/// Workspace-wide result alias using [`CoreError`].
pub type Result<T> = std::result::Result<T, CoreError>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- AgentId -----------------------------------------------------------

    #[test]
    fn agent_id_new_and_as_str() {
        let id = AgentId::new("worker-1");
        assert_eq!(id.as_str(), "worker-1");
    }

    #[test]
    fn agent_id_display() {
        let id = AgentId::new("abc");
        assert_eq!(format!("{id}"), "abc");
    }

    #[test]
    fn agent_id_from_str() {
        let id: AgentId = "test".into();
        assert_eq!(id.as_str(), "test");
    }

    #[test]
    fn agent_id_from_string() {
        let id: AgentId = String::from("owned").into();
        assert_eq!(id.as_str(), "owned");
    }

    #[test]
    fn agent_id_into_string() {
        let id = AgentId::new("consume");
        assert_eq!(id.into_string(), "consume");
    }

    #[test]
    fn agent_id_equality_and_hash() {
        use std::collections::HashSet;
        let a = AgentId::new("x");
        let b = AgentId::new("x");
        let c = AgentId::new("y");
        assert_eq!(a, b);
        assert_ne!(a, c);

        let mut set = HashSet::new();
        set.insert(a.clone());
        set.insert(b);
        assert_eq!(set.len(), 1);
        set.insert(c);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn agent_id_serde_roundtrip() {
        let id = AgentId::new("serde-test");
        let json = serde_json::to_string(&id).unwrap();
        let back: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    // -- CoreError ---------------------------------------------------------

    #[test]
    fn error_not_found_display() {
        let err = CoreError::not_found("agent", "worker-42");
        assert_eq!(err.to_string(), "agent not found: worker-42");
    }

    #[test]
    fn error_invalid_state_display() {
        let err = CoreError::invalid_state("spawn", "agent already running");
        assert_eq!(
            err.to_string(),
            "invalid state for spawn: agent already running"
        );
    }

    #[test]
    fn error_config_display() {
        let err = CoreError::config("missing ZYLOS_HOME");
        assert_eq!(err.to_string(), "config error: missing ZYLOS_HOME");
    }

    #[test]
    fn error_external_display() {
        let err = CoreError::external("docker", "connection refused");
        assert_eq!(
            err.to_string(),
            "external error [docker]: connection refused"
        );
    }

    #[test]
    fn error_clone_equality() {
        let a = CoreError::not_found("task", "t1");
        let b = a.clone();
        assert_eq!(a.to_string(), b.to_string());
    }

    #[test]
    fn result_alias_ok() {
        fn ok_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(ok_fn().unwrap(), 42);
    }

    #[test]
    fn result_alias_err() {
        fn err_fn() -> Result<()> {
            Err(CoreError::config("boom"))
        }
        assert!(err_fn().is_err());
    }
}
