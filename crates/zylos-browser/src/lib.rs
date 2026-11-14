//! Anti-detect browser context manager with WebMCP compliance for Zylos agents.
//!
//! Provides per-agent browser isolation with deterministic fingerprinting,
//! persistent permission inbox, and WebMCP spec-compliant tool registration.

pub mod context;
pub mod error;
pub mod fingerprint;
pub mod headless;
pub mod webmcp;
pub mod webmcp_eval;

pub use context::{BrowserContext, BrowserContextConfig, InboxEntry, PermissionDecision};
pub use error::{BrowserError, Result};
pub use fingerprint::BrowserFingerprint;
pub use headless::{HeadlessBrowser, HeadlessConfig, NavigationResult, PageState};
pub use webmcp::{McpToolRegistration, WebMcpCompliance};
pub use webmcp_eval::{
    CheckSeverity, ComplianceCheckResult, ComplianceReport, SecurePolyfill, WebMcpEvaluator,
};
