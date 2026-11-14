//! WebMCP spec compliance: page-lifetime registration and secure polyfill.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::error::{BrowserError, Result};

/// WebMCP tool registration with page-lifetime scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolRegistration {
    /// Tool name (e.g., "browser.navigate").
    pub name: String,
    /// Page origin this tool is registered for.
    pub origin: String,
    /// Whether the tool requires user gesture.
    pub requires_gesture: bool,
    /// Allowed methods/patterns.
    pub allowed_patterns: Vec<String>,
}

/// WebMCP compliance checker for browser contexts.
#[derive(Debug)]
pub struct WebMcpCompliance {
    registrations: Vec<McpToolRegistration>,
    blocked_origins: HashSet<String>,
}

impl WebMcpCompliance {
    pub fn new() -> Self {
        Self {
            registrations: Vec::new(),
            blocked_origins: HashSet::new(),
        }
    }

    /// Register a tool for a specific page origin (page-lifetime scope).
    pub fn register_tool(&mut self, reg: McpToolRegistration) -> Result<()> {
        if self.blocked_origins.contains(&reg.origin) {
            return Err(BrowserError::WebMcpViolation(format!(
                "Origin '{}' is blocked",
                reg.origin
            )));
        }
        self.registrations.push(reg);
        Ok(())
    }

    /// Check if a tool invocation is compliant with registered permissions.
    pub fn check_invocation(&self, tool_name: &str, origin: &str) -> Result<bool> {
        if self.blocked_origins.contains(origin) {
            return Err(BrowserError::WebMcpViolation(format!(
                "Invocation blocked for origin '{}'",
                origin
            )));
        }

        let matching: Vec<&McpToolRegistration> = self
            .registrations
            .iter()
            .filter(|r| r.name == tool_name && r.origin == origin)
            .collect();

        if matching.is_empty() {
            return Ok(false); // Not registered = not allowed
        }

        Ok(true)
    }

    /// Block an origin entirely.
    pub fn block_origin(&mut self, origin: &str) {
        self.blocked_origins.insert(origin.to_string());
        // Remove existing registrations for this origin
        self.registrations.retain(|r| r.origin != origin);
    }

    /// Clear all registrations for a specific origin (page unload).
    pub fn clear_origin(&mut self, origin: &str) {
        self.registrations.retain(|r| r.origin != origin);
    }

    /// Get count of active registrations.
    pub fn registration_count(&self) -> usize {
        self.registrations.len()
    }

    /// Validate polyfill security: ensure no eval() or Function() constructor usage.
    pub fn validate_polyfill(&self, script_content: &str) -> Result<()> {
        let dangerous_patterns = [
            "eval(",
            "Function(",
            "new Function(",
            "document.write(",
            "innerHTML =",
        ];

        for pattern in &dangerous_patterns {
            if script_content.contains(pattern) {
                return Err(BrowserError::WebMcpViolation(format!(
                    "Polyfill contains forbidden pattern: '{}'",
                    pattern
                )));
            }
        }

        Ok(())
    }
}

impl Default for WebMcpCompliance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_check() {
        let mut mcp = WebMcpCompliance::new();
        let reg = McpToolRegistration {
            name: "browser.navigate".to_string(),
            origin: "https://example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec!["*".to_string()],
        };
        mcp.register_tool(reg).unwrap();

        assert!(mcp
            .check_invocation("browser.navigate", "https://example.com")
            .unwrap());
        assert!(!mcp
            .check_invocation("browser.navigate", "https://other.com")
            .unwrap());
    }

    #[test]
    fn test_blocked_origin() {
        let mut mcp = WebMcpCompliance::new();
        mcp.block_origin("https://evil.com");

        let reg = McpToolRegistration {
            name: "browser.read".to_string(),
            origin: "https://evil.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };
        assert!(mcp.register_tool(reg).is_err());
        assert!(mcp
            .check_invocation("browser.read", "https://evil.com")
            .is_err());
    }

    #[test]
    fn test_clear_origin() {
        let mut mcp = WebMcpCompliance::new();
        let reg = McpToolRegistration {
            name: "tool.a".to_string(),
            origin: "https://page.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };
        mcp.register_tool(reg).unwrap();
        assert_eq!(mcp.registration_count(), 1);

        mcp.clear_origin("https://page.com");
        assert_eq!(mcp.registration_count(), 0);
    }

    #[test]
    fn test_polyfill_validation() {
        let mcp = WebMcpCompliance::new();
        assert!(mcp.validate_polyfill("const x = 1;").is_ok());
        assert!(mcp.validate_polyfill("eval('code')").is_err());
        assert!(mcp.validate_polyfill("new Function('return 1')").is_err());
        assert!(mcp.validate_polyfill("el.innerHTML = '<div>'").is_err());
    }
}
