//! zylos-bot-system — Lifecycle, DAG execution, P2P messaging
//!
//! Parte do Zylos-Next v6.0. Este crate implementa o BOT_SYSTEM:
//! - LifecycleManager: spawn/kill/suspend agents via Tokio JoinSet + CancellationToken
//! - DAGExecutor: execução topológica de workflows com rollback (futuro)
//! - P2PMessenger: mensagens type-safe entre agentes (futuro)

pub mod autoloop_protocol;
pub mod cerebelo;
pub mod dag;
pub mod error;
pub mod goal_program;
pub mod kanban_bridge;
pub mod lifecycle;
pub mod p2p;
#[cfg(feature = "p2p-network")]
pub mod p2p_network;
pub mod workflow_dsl;

pub use dag::{DagEdge, DagExecutor, DagId, DagNode, NodeStatus};
pub use error::BotError;
pub use lifecycle::{AgentHandle, AgentId, AgentState, LifecycleManager};
pub use p2p::{Message, MessageId, P2PMessenger, Topic};
pub use workflow_dsl::{parse_workflow, validate_workflow, WorkflowSpec};
