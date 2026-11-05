//! Anti-detect browser context manager with WebMCP compliance for Zylos agents.
//!
//! Provides per-agent browser isolation with deterministic fingerprinting,
//! persistent permission inbox, and WebMCP spec-compliant tool registration.

pub mod error;
pub mod fingerprint;
pub mod context;
pub mod webmcp;
pub mod headless;
pub mod webmcp_eval;

pub use error::{BrowserError, Result};
pub use fingerprint::BrowserFingerprint;
pub use context::{BrowserContext, BrowserContextConfig, PermissionDecision, InboxEntry};
pub use webmcp::{WebMcpCompliance, McpToolRegistration};
pub use headless::{HeadlessBrowser, HeadlessConfig, PageState, NavigationResult};
pub use webmcp_eval::{WebMcpEvaluator, ComplianceReport, ComplianceCheckResult, SecurePolyfill, CheckSeverity};