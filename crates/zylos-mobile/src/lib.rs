//! zylos-mobile — Mobile MCP Server for Zylos-Next v6
//!
//! Provides AG-UI streaming bridge and mobile-optimized tool access
//! via Model Context Protocol (MCP). Part of Fase 3 (LIVE_COMM_SYSTEM).

pub mod error;
pub mod mcp_server;

pub use error::MobileError;
pub use mcp_server::{
    AttachmentStore, AuthConfig, JsonRpcRequest, JsonRpcResponse, McpErrorBody, McpPromptArgument,
    McpPromptDef, McpResourceDef, McpToolDef, MobileMcpServer, RateLimiter, ServerCapabilities,
};
