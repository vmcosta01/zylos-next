//! Docker-based sandbox isolation for Zylos agents.
//!
//! Provides per-agent container isolation with filesystem sandboxing,
//! approval workflows for high-risk operations, and container lifecycle management.

pub mod approval;
pub mod container;
pub mod error;

pub use approval::{ApprovalDecision, ApprovalWorkflow, RiskLevel};
pub use container::{ContainerStatus, DockerSandbox, SandboxConfig};
pub use error::DockerError;
