//! zylos-mobile — Mobile MCP Server for Zylos-Next v6
//!
//! Provides AG-UI streaming bridge and mobile-optimized tool access
//! via Model Context Protocol (MCP). Part of Fase 3 (LIVE_COMM_SYSTEM).

pub mod mcp_server;
pub mod error;

pub use mcp_server::{
    MobileMcpServer, AuthConfig, AttachmentStore, RateLimiter,
    JsonRpcRequest, JsonRpcResponse, McpErrorBody,
    McpToolDef, McpResourceDef, McpPromptDef, McpPromptArgument,
    ServerCapabilities,
};
pub use error::MobileError;
