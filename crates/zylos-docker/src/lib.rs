//! Docker-based sandbox isolation for Zylos agents.
//!
//! Provides per-agent container isolation with filesystem sandboxing,
//! approval workflows for high-risk operations, and container lifecycle management.

pub mod container;
pub mod approval;
pub mod error;

pub use container::{DockerSandbox, SandboxConfig, ContainerStatus};
pub use approval::{ApprovalWorkflow, ApprovalDecision, RiskLevel};
pub use error::DockerError;