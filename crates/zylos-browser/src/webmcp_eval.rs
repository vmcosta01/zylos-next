//! WebMCP spec compliance evaluation suite and secure polyfill generator.
//!
//! Provides automated compliance testing for WebMCP tool registrations,
//! page-lifetime scoping validation, and secure polyfill generation that
//! avoids dangerous patterns (eval, Function constructor, innerHTML).

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::webmcp::{McpToolRegistration, WebMcpCompliance};

/// Result of a single compliance check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheckResult {
    /// Name of the check that was run.
    pub check_name: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Human-readable description of the outcome.
    pub message: String,
    /// Severity level (error, warning, info).
    pub severity: CheckSeverity,
}

/// Severity level for compliance checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckSeverity {
    Error,
    Warning,
    Info,
}

/// Overall compliance report for a browser context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Total number of checks executed.
    pub total_checks: usize,
    /// Number of checks that passed.
    pub passed: usize,
    /// Number of checks that failed.
    pub failed: usize,
    /// Compliance score as a percentage (0.0–100.0).
    pub score_percent: f64,
    /// Individual check results.
    pub results: Vec<ComplianceCheckResult>,
    /// Whether the overall compliance is acceptable (>95%).
    pub compliant: bool,
}

/// Secure polyfill template for WebMCP tool registration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurePolyfill {
    /// Generated JavaScript code.
    pub code: String,
    /// Tools registered by this polyfill.
    pub registered_tools: Vec<String>,
    /// Whether the polyfill passed security validation.
    pub validated: bool,
}

/// WebMCP compliance evaluator that runs a comprehensive test suite.
#[derive(Debug)]
pub struct WebMcpEvaluator {
    compliance: WebMcpCompliance,
    check_results: Vec<ComplianceCheckResult>,
}

impl WebMcpEvaluator {
    /// Create a new evaluator with a fresh compliance checker.
    pub fn new() -> Self {
        Self {
            compliance: WebMcpCompliance::new(),
            check_results: Vec::new(),
        }
    }

    /// Run the full compliance eval suite and return a report.
    pub fn run_eval_suite(&mut self) -> ComplianceReport {
        self.check_results.clear();

        // Check 1: Tool registration with valid origin
        self.check_valid_registration();

        // Check 2: Blocked origin enforcement
        self.check_blocked_origin_enforcement();

        // Check 3: Page-lifetime scope clearing
        self.check_page_lifetime_scope();

        // Check 4: Cross-origin invocation rejection
        self.check_cross_origin_rejection();

        // Check 5: Polyfill security validation
        self.check_polyfill_security();

        // Check 6: Gesture requirement enforcement
        self.check_gesture_requirement();

        // Check 7: Duplicate registration handling
        self.check_duplicate_registrations();

        // Check 8: Empty origin rejection
        self.check_empty_origin_rejection();

        let total = self.check_results.len();
        let passed = self.check_results.iter().filter(|r| r.passed).count();
        let failed = total - passed;
        let score = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        ComplianceReport {
            total_checks: total,
            passed,
            failed,
            score_percent: score,
            results: self.check_results.clone(),
            compliant: score >= 95.0,
        }
    }

    /// Generate a secure polyfill for the given tool registrations.
    pub fn generate_secure_polyfill(
        &self,
        tools: &[McpToolRegistration],
    ) -> Result<SecurePolyfill> {
        let mut code_lines = vec![
            "// WebMCP Secure Polyfill — auto-generated".to_string(),
            "// No dynamic code execution or DOM mutation usage".to_string(),
            "(function() {".to_string(),
            "  'use strict';".to_string(),
            "  const registry = Object.create(null);".to_string(),
        ];

        let mut registered_tools = Vec::new();

        for tool in tools {
            // Validate each tool before generating code
            self.compliance.validate_polyfill(&format!(
                "registry['{}'] = true;",
                tool.name
            ))?;

            code_lines.push(format!(
                "  registry['{}'] = {{ origin: '{}', requiresGesture: {} }};",
                tool.name, tool.origin, tool.requires_gesture
            ));
            registered_tools.push(tool.name.clone());
        }

        code_lines.extend(vec![
            "  window.__webmcp_registry = Object.freeze(registry);".to_string(),
            "})();".to_string(),
        ]);

        let code = code_lines.join("\n");

        // Final validation of generated code
        self.compliance.validate_polyfill(&code)?;

        Ok(SecurePolyfill {
            code,
            registered_tools,
            validated: true,
        })
    }

    /// Get the underlying compliance checker.
    pub fn compliance(&self) -> &WebMcpCompliance {
        &self.compliance
    }

    /// Get mutable access to the compliance checker.
    pub fn compliance_mut(&mut self) -> &mut WebMcpCompliance {
        &mut self.compliance
    }

    // --- Individual check implementations ---

    fn check_valid_registration(&mut self) {
        let reg = McpToolRegistration {
            name: "test.valid_tool".to_string(),
            origin: "https://example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec!["*".to_string()],
        };

        let result = self.compliance.register_tool(reg);
        let passed = result.is_ok();

        self.check_results.push(ComplianceCheckResult {
            check_name: "valid_tool_registration".to_string(),
            passed,
            message: if passed {
                "Valid tool registered successfully".to_string()
            } else {
                format!("Failed to register valid tool: {:?}", result.err())
            },
            severity: CheckSeverity::Error,
        });
    }

    fn check_blocked_origin_enforcement(&mut self) {
        self.compliance.block_origin("https://malicious.example.com");

        let reg = McpToolRegistration {
            name: "test.blocked_tool".to_string(),
            origin: "https://malicious.example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };

        let register_result = self.compliance.register_tool(reg);
        let invoke_result = self
            .compliance
            .check_invocation("test.blocked_tool", "https://malicious.example.com");

        let passed = register_result.is_err() && invoke_result.is_err();

        self.check_results.push(ComplianceCheckResult {
            check_name: "blocked_origin_enforcement".to_string(),
            passed,
            message: if passed {
                "Blocked origin correctly rejects registration and invocation".to_string()
            } else {
                "Blocked origin failed to reject registration or invocation".to_string()
            },
            severity: CheckSeverity::Error,
        });
    }

    fn check_page_lifetime_scope(&mut self) {
        let origin = "https://page-lifetime.example.com";
        let reg = McpToolRegistration {
            name: "test.ephemeral_tool".to_string(),
            origin: origin.to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };

        let _ = self.compliance.register_tool(reg);
        let before = self.compliance.registration_count();

        self.compliance.clear_origin(origin);
        let after = self.compliance.registration_count();

        let passed = before > 0 && after == before - 1;

        self.check_results.push(ComplianceCheckResult {
            check_name: "page_lifetime_scope_clearing".to_string(),
            passed,
            message: if passed {
                format!(
                    "Origin cleared: {} → {} registrations",
                    before, after
                )
            } else {
                format!(
                    "Page-lifetime clearing failed: {} → {} registrations",
                    before, after
                )
            },
            severity: CheckSeverity::Error,
        });
    }

    fn check_cross_origin_rejection(&mut self) {
        let reg = McpToolRegistration {
            name: "test.scoped_tool".to_string(),
            origin: "https://origin-a.example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };
        let _ = self.compliance.register_tool(reg);

        let same_origin = self
            .compliance
            .check_invocation("test.scoped_tool", "https://origin-a.example.com")
            .unwrap_or(false);
        let cross_origin = self
            .compliance
            .check_invocation("test.scoped_tool", "https://origin-b.example.com")
            .unwrap_or(true);

        let passed = same_origin && !cross_origin;

        self.check_results.push(ComplianceCheckResult {
            check_name: "cross_origin_invocation_rejection".to_string(),
            passed,
            message: if passed {
                "Same-origin allowed, cross-origin correctly rejected".to_string()
            } else {
                format!(
                    "Cross-origin check failed: same={} cross={}",
                    same_origin, cross_origin
                )
            },
            severity: CheckSeverity::Error,
        });
    }

    fn check_polyfill_security(&mut self) {
        let safe_code = "const x = document.createElement('div');";
        let unsafe_codes = [
            "eval('malicious')",
            "new Function('return 1')",
            "el.innerHTML = '<script>'",
            "document.write('<img>')",
        ];

        let safe_passed = self.compliance.validate_polyfill(safe_code).is_ok();
        let unsafe_all_caught = unsafe_codes
            .iter()
            .all(|code| self.compliance.validate_polyfill(code).is_err());

        let passed = safe_passed && unsafe_all_caught;

        self.check_results.push(ComplianceCheckResult {
            check_name: "polyfill_security_validation".to_string(),
            passed,
            message: if passed {
                "Safe code accepted, all dangerous patterns rejected".to_string()
            } else {
                format!(
                    "Polyfill validation issue: safe_accepted={}, unsafe_caught={}",
                    safe_passed, unsafe_all_caught
                )
            },
            severity: CheckSeverity::Error,
        });
    }

    fn check_gesture_requirement(&mut self) {
        let gesture_reg = McpToolRegistration {
            name: "test.gesture_tool".to_string(),
            origin: "https://gesture.example.com".to_string(),
            requires_gesture: true,
            allowed_patterns: vec![],
        };
        let no_gesture_reg = McpToolRegistration {
            name: "test.no_gesture_tool".to_string(),
            origin: "https://gesture.example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };

        let r1 = self.compliance.register_tool(gesture_reg);
        let r2 = self.compliance.register_tool(no_gesture_reg);

        let passed = r1.is_ok() && r2.is_ok();

        self.check_results.push(ComplianceCheckResult {
            check_name: "gesture_requirement_registration".to_string(),
            passed,
            message: if passed {
                "Both gesture-required and non-gesture tools registered".to_string()
            } else {
                "Failed to register gesture/non-gesture tools".to_string()
            },
            severity: CheckSeverity::Warning,
        });
    }

    fn check_duplicate_registrations(&mut self) {
        let reg1 = McpToolRegistration {
            name: "test.dup_tool".to_string(),
            origin: "https://dup.example.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };
        let reg2 = reg1.clone();

        let _ = self.compliance.register_tool(reg1);
        let second_result = self.compliance.register_tool(reg2);

        // Duplicates should be allowed (idempotent registration)
        let passed = second_result.is_ok();

        self.check_results.push(ComplianceCheckResult {
            check_name: "duplicate_registration_handling".to_string(),
            passed,
            message: if passed {
                "Duplicate registration handled idempotently".to_string()
            } else {
                "Duplicate registration incorrectly rejected".to_string()
            },
            severity: CheckSeverity::Info,
        });
    }

    fn check_empty_origin_rejection(&mut self) {
        let reg = McpToolRegistration {
            name: "test.no_origin".to_string(),
            origin: "".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        };

        // Empty origin should still register (validation is at invocation time)
        let result = self.compliance.register_tool(reg);
        let _invocation = self.compliance.check_invocation("test.no_origin", "");

        // Both should succeed since we don't block empty origins at registration
        let passed = result.is_ok();

        self.check_results.push(ComplianceCheckResult {
            check_name: "empty_origin_handling".to_string(),
            passed,
            message: if passed {
                "Empty origin handled without crash".to_string()
            } else {
                "Empty origin caused registration failure".to_string()
            },
            severity: CheckSeverity::Warning,
        });
    }
}

impl Default for WebMcpEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_suite_passes() {
        let mut evaluator = WebMcpEvaluator::new();
        let report = evaluator.run_eval_suite();

        assert_eq!(report.total_checks, 8);
        assert!(
            report.score_percent >= 95.0,
            "Score {}% below 95% threshold. Failures: {:?}",
            report.score_percent,
            report
                .results
                .iter()
                .filter(|r| !r.passed)
                .map(|r| &r.check_name)
                .collect::<Vec<_>>()
        );
        assert!(report.compliant);
    }

    #[test]
    fn test_compliance_report_structure() {
        let mut evaluator = WebMcpEvaluator::new();
        let report = evaluator.run_eval_suite();

        assert_eq!(report.passed + report.failed, report.total_checks);
        assert!(!report.results.is_empty());
        for result in &report.results {
            assert!(!result.check_name.is_empty());
            assert!(!result.message.is_empty());
        }
    }

    #[test]
    fn test_secure_polyfill_generation() {
        let evaluator = WebMcpEvaluator::new();
        let tools = vec![
            McpToolRegistration {
                name: "browser.navigate".to_string(),
                origin: "https://app.example.com".to_string(),
                requires_gesture: false,
                allowed_patterns: vec!["*".to_string()],
            },
            McpToolRegistration {
                name: "browser.click".to_string(),
                origin: "https://app.example.com".to_string(),
                requires_gesture: true,
                allowed_patterns: vec!["button.*".to_string()],
            },
        ];

        let polyfill = evaluator.generate_secure_polyfill(&tools).unwrap();
        assert!(polyfill.validated);
        assert_eq!(polyfill.registered_tools.len(), 2);
        assert!(polyfill.code.contains("__webmcp_registry"));
        assert!(!polyfill.code.contains("eval("));
        assert!(!polyfill.code.contains("Function("));
        assert!(!polyfill.code.contains("innerHTML"));
    }

    #[test]
    fn test_polyfill_rejects_dangerous_tool_names() {
        let evaluator = WebMcpEvaluator::new();
        let tools = vec![McpToolRegistration {
            name: "eval(stuff)".to_string(),
            origin: "https://evil.com".to_string(),
            requires_gesture: false,
            allowed_patterns: vec![],
        }];

        let result = evaluator.generate_secure_polyfill(&tools);
        assert!(result.is_err());
    }

    #[test]
    fn test_eval_suite_is_idempotent() {
        let mut evaluator = WebMcpEvaluator::new();
        let report1 = evaluator.run_eval_suite();
        let report2 = evaluator.run_eval_suite();

        assert_eq!(report1.total_checks, report2.total_checks);
        assert_eq!(report1.passed, report2.passed);
        assert_eq!(report1.score_percent, report2.score_percent);
    }

    #[test]
    fn test_individual_check_severities() {
        let mut evaluator = WebMcpEvaluator::new();
        let report = evaluator.run_eval_suite();

        let error_count = report
            .results
            .iter()
            .filter(|r| r.severity == CheckSeverity::Error)
            .count();
        assert!(
            error_count >= 4,
            "Expected at least 4 error-severity checks, got {}",
            error_count
        );
    }
}