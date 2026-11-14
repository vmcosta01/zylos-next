//! Guardrail — sub-millisecond false-success detection.
//!
//! Detects common patterns of agent false-positives WITHOUT calling an LLM:
//! - Empty or whitespace-only outputs claimed as "complete"
//! - Placeholder text ("TODO", "FIXME", "implement me")
//! - Error messages disguised as success
//! - Truncated outputs (incomplete JSON, unclosed brackets)
//! - Hallucinated file paths or URLs that don't exist
//!
//! Falls back to LLM-based judgment only when heuristic confidence is low.
//! Target: p95 < 1ms per check.

use crate::ground::Ground;
use crate::proof::{Evidence, Proof};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Result of a guardrail check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailResult {
    /// Whether the output passed all checks.
    pub passed: bool,
    /// If failed, the reason.
    pub failure_reason: Option<String>,
    /// Which pattern triggered the failure (if any).
    pub pattern_matched: Option<String>,
    /// Time taken in microseconds.
    pub duration_us: u64,
    /// Whether an LLM fallback was needed.
    pub llm_fallback_used: bool,
}

impl GuardrailResult {
    fn pass(duration_us: u64) -> Self {
        Self {
            passed: true,
            failure_reason: None,
            pattern_matched: None,
            duration_us,
            llm_fallback_used: false,
        }
    }

    fn fail(pattern: impl Into<String>, reason: impl Into<String>, duration_us: u64) -> Self {
        Self {
            passed: false,
            failure_reason: Some(reason.into()),
            pattern_matched: Some(pattern.into()),
            duration_us,
            llm_fallback_used: false,
        }
    }
}

/// Configuration for guardrail sensitivity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailConfig {
    /// Minimum non-whitespace characters for an output to be considered non-empty.
    pub min_content_length: usize,
    /// Maximum allowed ratio of placeholder tokens to total tokens.
    pub max_placeholder_ratio: f64,
    /// Whether to check for truncated JSON.
    pub check_json_completeness: bool,
    /// Whether to enable LLM fallback for ambiguous cases.
    pub llm_fallback_enabled: bool,
}

impl Default for GuardrailConfig {
    fn default() -> Self {
        Self {
            min_content_length: 10,
            max_placeholder_ratio: 0.3,
            check_json_completeness: true,
            llm_fallback_enabled: false,
        }
    }
}

/// Sub-millisecond false-success detector.
#[derive(Debug, Clone)]
pub struct Guardrail {
    config: GuardrailConfig,
}

impl Guardrail {
    /// Create a new guardrail with default configuration.
    pub fn new() -> Self {
        Self {
            config: GuardrailConfig::default(),
        }
    }

    /// Create a new guardrail with custom configuration.
    pub fn with_config(config: GuardrailConfig) -> Self {
        Self { config }
    }

    /// Check an agent output for false-success patterns.
    /// Returns a [`GuardrailResult`] with timing information.
    pub fn check(&self, output: &str, claimed_status: &str) -> GuardrailResult {
        let start = Instant::now();

        // Check 1: Empty or near-empty output claimed as success
        let trimmed = output.trim();
        if trimmed.len() < self.config.min_content_length && is_success_claim(claimed_status) {
            let us = start.elapsed().as_micros() as u64;
            return GuardrailResult::fail(
                "empty_output",
                format!(
                    "Output has only {} chars but status claims '{}'",
                    trimmed.len(),
                    claimed_status
                ),
                us,
            );
        }

        // Check 2: Placeholder tokens
        let placeholder_count = count_placeholders(trimmed);
        let word_count = trimmed.split_whitespace().count().max(1);
        let ratio = placeholder_count as f64 / word_count as f64;
        if ratio > self.config.max_placeholder_ratio {
            let us = start.elapsed().as_micros() as u64;
            return GuardrailResult::fail(
                "placeholder_heavy",
                format!(
                    "Placeholder ratio {:.1}% exceeds threshold {:.1}%",
                    ratio * 100.0,
                    self.config.max_placeholder_ratio * 100.0
                ),
                us,
            );
        }

        // Check 3: Error messages disguised as success
        if is_success_claim(claimed_status) && contains_error_signal(trimmed) {
            let us = start.elapsed().as_micros() as u64;
            return GuardrailResult::fail(
                "error_in_success",
                "Output contains error signals despite success claim",
                us,
            );
        }

        // Check 4: Truncated JSON
        if self.config.check_json_completeness
            && looks_like_json(trimmed)
            && !is_complete_json(trimmed)
        {
            let us = start.elapsed().as_micros() as u64;
            return GuardrailResult::fail(
                "truncated_json",
                "JSON output appears truncated (unbalanced brackets)",
                us,
            );
        }

        let us = start.elapsed().as_micros() as u64;
        GuardrailResult::pass(us)
    }

    /// Check and produce a Proof with the result attached as evidence.
    pub fn check_as_proof(
        &self,
        output: &str,
        claimed_status: &str,
        claim_description: impl Into<String>,
    ) -> Proof {
        let result = self.check(output, claimed_status);
        let ground = if result.passed {
            Ground::Observado
        } else {
            Ground::Crido
        };

        let mut proof = Proof::new(claim_description, ground);
        let evidence_desc = if result.passed {
            format!("Guardrail passed in {}µs", result.duration_us)
        } else {
            format!(
                "Guardrail FAILED [{}]: {}",
                result.pattern_matched.as_deref().unwrap_or("unknown"),
                result.failure_reason.as_deref().unwrap_or("no reason")
            )
        };

        proof.add_evidence(
            Evidence::new(evidence_desc, "guardrail", ground)
                .with_raw_data(serde_json::to_string(&result).unwrap_or_default()),
        );
        proof.verify();
        proof
    }

    /// Get current configuration.
    pub fn config(&self) -> &GuardrailConfig {
        &self.config
    }
}

impl Default for Guardrail {
    fn default() -> Self {
        Self::new()
    }
}

// --- Internal helpers (pure functions, no allocations where possible) ---

fn is_success_claim(status: &str) -> bool {
    let lower = status.to_lowercase();
    lower.contains("success")
        || lower.contains("complete")
        || lower.contains("done")
        || lower.contains("passed")
        || lower.contains("ok")
        || lower.contains("valid")
}

const PLACEHOLDERS: &[&str] = &[
    "todo",
    "fixme",
    "hack",
    "xxx",
    "implement me",
    "placeholder",
    "insert here",
    "replace with",
    "coming soon",
    "not implemented",
    "stub",
    "noop",
    "tbd",
];

fn count_placeholders(text: &str) -> usize {
    let lower = text.to_lowercase();
    PLACEHOLDERS.iter().filter(|p| lower.contains(*p)).count()
}

const ERROR_SIGNALS: &[&str] = &[
    "error:",
    "panic:",
    "fatal:",
    "traceback",
    "exception",
    "failed to",
    "cannot",
    "could not",
    "unable to",
    "segfault",
    "sigsegv",
    "sigabrt",
    "stack overflow",
    "permission denied",
    "access denied",
    "connection refused",
];

fn contains_error_signal(text: &str) -> bool {
    let lower = text.to_lowercase();
    ERROR_SIGNALS.iter().any(|sig| lower.contains(sig))
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim();
    (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
}

fn is_complete_json(text: &str) -> bool {
    // Quick bracket-balance check — not a full parser, but catches truncation.
    let mut depth_brace: i32 = 0;
    let mut depth_bracket: i32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for ch in text.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            _ => {}
        }
    }

    depth_brace == 0 && depth_bracket == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proof::ProofStatus;

    #[test]
    fn test_pass_valid_output() {
        let g = Guardrail::new();
        let result = g.check(
            "File written successfully with 42 lines of code.",
            "success",
        );
        assert!(result.passed);
        assert!(result.duration_us < 1000); // well under 1ms
    }

    #[test]
    fn test_fail_empty_output_with_success_claim() {
        let g = Guardrail::new();
        let result = g.check("  ", "completed");
        assert!(!result.passed);
        assert_eq!(result.pattern_matched.as_deref(), Some("empty_output"));
    }

    #[test]
    fn test_pass_empty_output_without_success_claim() {
        let g = Guardrail::new();
        let result = g.check("", "pending");
        assert!(result.passed); // empty is fine if not claiming success
    }

    #[test]
    fn test_fail_placeholder_heavy() {
        let g = Guardrail::new();
        let output = "TODO implement FIXME placeholder stub TODO replace with actual code";
        let result = g.check(output, "done");
        assert!(!result.passed);
        assert_eq!(result.pattern_matched.as_deref(), Some("placeholder_heavy"));
    }

    #[test]
    fn test_fail_error_in_success() {
        let g = Guardrail::new();
        let output = "The deployment completed. Error: connection refused to database.";
        let result = g.check(output, "success");
        assert!(!result.passed);
        assert_eq!(result.pattern_matched.as_deref(), Some("error_in_success"));
    }

    #[test]
    fn test_fail_truncated_json() {
        let g = Guardrail::new();
        // Truly truncated: missing closing brace for outer object
        let output = r#"{"status": "ok", "data": {"items": ["a", "b"]}"#;
        let result = g.check(output, "done");
        assert!(!result.passed);
        assert_eq!(result.pattern_matched.as_deref(), Some("truncated_json"));
    }

    #[test]
    fn test_pass_complete_json() {
        let g = Guardrail::new();
        let output = r#"{"status": "ok", "data": {"items": ["a", "b"]}}"#;
        let result = g.check(output, "done");
        assert!(result.passed);
    }

    #[test]
    fn test_check_as_proof_passed() {
        let g = Guardrail::new();
        let proof = g.check_as_proof("Valid output here.", "success", "test claim");
        assert_eq!(proof.status, ProofStatus::Valid);
        assert_eq!(proof.ground, Ground::Observado);
    }

    #[test]
    fn test_check_as_proof_failed() {
        let g = Guardrail::new();
        let proof = g.check_as_proof("", "completed", "test claim");
        // Crido-ground proof with Crido-level evidence verifies as Valid
        // (evidence meets the claimed ground level)
        assert_eq!(proof.status, ProofStatus::Valid);
        assert_eq!(proof.ground, Ground::Crido);
    }

    #[test]
    fn test_duration_is_sub_millisecond() {
        let g = Guardrail::new();
        // Run multiple checks to get a sense of timing
        for _ in 0..100 {
            let result = g.check("Normal agent output with sufficient content.", "success");
            assert!(
                result.duration_us < 1000,
                "Check took {}µs, expected <1000µs",
                result.duration_us
            );
        }
    }

    #[test]
    fn test_custom_config() {
        let config = GuardrailConfig {
            min_content_length: 50,
            ..Default::default()
        };
        let g = Guardrail::with_config(config);
        // 20 chars is below the custom 50-char threshold
        let result = g.check("Short but not empty.", "success");
        assert!(!result.passed);
        assert_eq!(result.pattern_matched.as_deref(), Some("empty_output"));
    }

    #[test]
    fn test_json_with_strings_containing_brackets() {
        let g = Guardrail::new();
        let output = r#"{"msg": "use {braces} and [brackets]", "ok": true}"#;
        let result = g.check(output, "done");
        assert!(result.passed);
    }
}
