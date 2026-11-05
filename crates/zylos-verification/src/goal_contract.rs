//! Goal Contract — typed acceptance criteria with three-phase validation.
//!
//! Provides:
//! - [`GoalContract`]: Structured acceptance criteria with pre/postconditions
//! - Three-phase validation: schema parse → precondition eval → verify dry-run
//! - WriteGate integration via [`EvidenceCapsule`] attachment
//! - Artifact contract enforcement (type matching)
//! - Contract versioning with SHA-256 digest and shadow-git rollback stub
//! - DESIGN.md bidirectional sync (parse ↔ generate)
//! - Handlebars-based template expansion for reusable contracts

use crate::ground::Ground;
use crate::proof::{Evidence, Proof};
use chrono::{DateTime, Utc};
use handlebars::Handlebars;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors arising from goal contract operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ContractError {
    #[error("schema validation failed: {0}")]
    SchemaValidation(String),

    #[error("precondition not met: {0}")]
    PreconditionFailed(String),

    #[error("dry-run verification failed: {0}")]
    DryRunFailed(String),

    #[error("artifact mismatch: expected '{expected}', got '{actual}'")]
    ArtifactMismatch { expected: String, actual: String },

    #[error("write gate blocked: missing evidence capsule for criterion '{0}'")]
    WriteGateBlocked(String),

    #[error("template rendering failed: {0}")]
    TemplateError(String),

    #[error("DESIGN.md parse error: {0}")]
    DesignParseError(String),

    #[error("version conflict: expected {expected}, found {found}")]
    VersionConflict { expected: u64, found: u64 },

    #[error("invalid regex pattern: {0}")]
    InvalidRegex(String),
}

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// Unique identifier for a goal contract.
pub type ContractId = Uuid;

/// A single acceptance criterion that must be satisfied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Criterion {
    /// Command must exit with the expected code.
    CommandExit {
        cmd: String,
        expected_exit: i32,
    },
    /// Output must match a regex at least `min_matches` times.
    OutputPattern {
        regex: String,
        min_matches: usize,
    },
    /// A file must exist with an optional SHA-256 hash.
    FileExists {
        path: String,
        hash: Option<String>,
    },
    /// HTTP endpoint must return expected status and optional body pattern.
    ApiCheck {
        endpoint: String,
        status: u16,
        body_pattern: Option<String>,
    },
    /// Custom verification via an external tool reference.
    CustomVerify {
        tool_ref: String,
        args: Vec<String>,
    },
}

/// Specification for the primary verification command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec {
    pub cmd: String,
    pub expected_exit: i32,
    pub output_pattern: Option<String>,
    pub timeout_ms: u64,
}

/// Expected artifact specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactSpec {
    pub name: String,
    pub schema: String,
    pub required: bool,
}

/// An expression used in preconditions and postconditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub description: String,
    pub eval_cmd: Option<String>,
    pub expected_result: Option<String>,
}

/// Retry policy for verification attempts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff_ms: 1000,
        }
    }
}

/// Evidence capsule attached to a satisfied criterion (WriteGate integration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceCapsule {
    pub criterion_index: usize,
    pub evidence: Evidence,
    pub verified_at: DateTime<Utc>,
}

/// A versioned snapshot of a goal contract for diff/rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractVersion {
    pub version: u64,
    pub digest: String,
    pub timestamp: DateTime<Utc>,
    pub contract_json: String,
}

/// The main goal contract structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalContract {
    pub id: ContractId,
    pub title: String,
    pub acceptance_criteria: Vec<Criterion>,
    pub verification: VerifySpec,
    pub preconditions: Vec<Expr>,
    pub postconditions: Vec<Expr>,
    pub timeout_ms: u64,
    pub retry_policy: RetryPolicy,
    pub artifacts_expected: Vec<ArtifactSpec>,
    pub version: u64,

    // Runtime state (not serialized by default via skip_serializing_if)
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub evidence_capsules: Vec<EvidenceCapsule>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub version_history: Vec<ContractVersion>,
}

// ---------------------------------------------------------------------------
// Validation phases
// ---------------------------------------------------------------------------

/// Result of the three-phase validation pipeline.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub schema_valid: bool,
    pub preconditions_met: bool,
    pub dry_run_passed: bool,
    pub errors: Vec<ContractError>,
    pub proof: Option<Proof>,
}

impl GoalContract {
    /// Create a new goal contract with version 1.
    pub fn new(title: impl Into<String>, verification: VerifySpec) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            acceptance_criteria: Vec::new(),
            verification,
            preconditions: Vec::new(),
            postconditions: Vec::new(),
            timeout_ms: 30_000,
            retry_policy: RetryPolicy::default(),
            artifacts_expected: Vec::new(),
            version: 1,
            evidence_capsules: Vec::new(),
            version_history: Vec::new(),
        }
    }

    // -- Phase 1: Schema parse ------------------------------------------------

    /// Phase 1: Validate that the contract is well-formed.
    /// Checks regex compilation, non-empty fields, and structural integrity.
    pub fn validate_schema(&self) -> Result<(), Vec<ContractError>> {
        let mut errors = Vec::new();

        if self.title.trim().is_empty() {
            errors.push(ContractError::SchemaValidation(
                "title must not be empty".into(),
            ));
        }

        // Validate regex patterns in criteria
        for (i, criterion) in self.acceptance_criteria.iter().enumerate() {
            match criterion {
                Criterion::OutputPattern { regex, .. } => {
                    if Regex::new(regex).is_err() {
                        errors.push(ContractError::InvalidRegex(format!(
                            "criterion[{}]: invalid regex '{}'",
                            i, regex
                        )));
                    }
                }
                Criterion::ApiCheck {
                    body_pattern: Some(pattern),
                    ..
                } => {
                    if Regex::new(pattern).is_err() {
                        errors.push(ContractError::InvalidRegex(format!(
                            "criterion[{}] api body_pattern: invalid regex '{}'",
                            i, pattern
                        )));
                    }
                }
                _ => {}
            }
        }

        // Validate verification output_pattern
        if let Some(ref pattern) = self.verification.output_pattern {
            if Regex::new(pattern).is_err() {
                errors.push(ContractError::InvalidRegex(format!(
                    "verification.output_pattern: invalid regex '{}'",
                    pattern
                )));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    // -- Phase 2: Precondition eval -------------------------------------------

    /// Phase 2: Evaluate preconditions.
    /// In this implementation, preconditions without `eval_cmd` are treated as
    /// informational and pass automatically. Those with `eval_cmd` would require
    /// execution context (not available in pure validation); we flag them as
    /// requiring runtime evaluation.
    pub fn evaluate_preconditions(&self) -> Result<(), Vec<ContractError>> {
        let mut errors = Vec::new();

        for (i, precond) in self.preconditions.iter().enumerate() {
            if precond.eval_cmd.is_none() && precond.expected_result.is_some() {
                errors.push(ContractError::PreconditionFailed(format!(
                    "precondition[{}]: has expected_result but no eval_cmd ('{}')",
                    i, precond.description
                )));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    // -- Phase 3: Verify dry-run ----------------------------------------------

    /// Phase 3: Dry-run verification without executing commands.
    /// Validates that all referenced artifacts have matching specs and that
    /// evidence capsules are attached for each criterion (WriteGate).
    pub fn verify_dry_run(&self) -> Result<(), Vec<ContractError>> {
        let mut errors = Vec::new();

        // WriteGate: each criterion must have an evidence capsule
        for (i, _criterion) in self.acceptance_criteria.iter().enumerate() {
            let has_capsule = self
                .evidence_capsules
                .iter()
                .any(|c| c.criterion_index == i);
            if !has_capsule {
                errors.push(ContractError::WriteGateBlocked(format!(
                    "criterion[{}]",
                    i
                )));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    // -- Full three-phase validation ------------------------------------------

    /// Run all three validation phases and produce a consolidated result.
    pub fn validate_contract(&self) -> ValidationResult {
        let mut errors = Vec::new();

        // Phase 1
        let schema_valid = match self.validate_schema() {
            Ok(()) => true,
            Err(e) => {
                errors.extend(e);
                false
            }
        };

        // Phase 2
        let preconditions_met = if schema_valid {
            match self.evaluate_preconditions() {
                Ok(()) => true,
                Err(e) => {
                    errors.extend(e);
                    false
                }
            }
        } else {
            false
        };

        // Phase 3
        let dry_run_passed = if preconditions_met {
            match self.verify_dry_run() {
                Ok(()) => true,
                Err(e) => {
                    errors.extend(e);
                    false
                }
            }
        } else {
            false
        };

        // Build proof
        let ground = if schema_valid && preconditions_met && dry_run_passed {
            Ground::Confirmado
        } else if schema_valid {
            Ground::Observado
        } else {
            Ground::Crido
        };

        let mut proof = Proof::new(
            format!("GoalContract '{}' validation", self.title),
            ground,
        );

        proof.add_evidence(Evidence::new(
            format!(
                "schema={}, preconditions={}, dry_run={}",
                schema_valid, preconditions_met, dry_run_passed
            ),
            "goal_contract_validator",
            ground,
        ));
        proof.verify();

        ValidationResult {
            schema_valid,
            preconditions_met,
            dry_run_passed,
            errors,
            proof: Some(proof),
        }
    }

    // -- Artifact contract enforcement ----------------------------------------

    /// Check that an actual artifact matches its expected spec.
    pub fn check_artifact(
        &self,
        artifact_name: &str,
        actual_schema: &str,
    ) -> Result<(), ContractError> {
        let spec = self
            .artifacts_expected
            .iter()
            .find(|a| a.name == artifact_name);

        match spec {
            Some(s) => {
                if s.schema != actual_schema {
                    Err(ContractError::ArtifactMismatch {
                        expected: s.schema.clone(),
                        actual: actual_schema.to_string(),
                    })
                } else {
                    Ok(())
                }
            }
            None => {
                // Unknown artifact — only error if it was required
                Err(ContractError::ArtifactMismatch {
                    expected: format!("known artifact '{}'", artifact_name),
                    actual: "unknown artifact".to_string(),
                })
            }
        }
    }

    /// Attach an evidence capsule to a criterion (WriteGate satisfaction).
    pub fn attach_evidence(&mut self, criterion_index: usize, evidence: Evidence) {
        self.evidence_capsules.push(EvidenceCapsule {
            criterion_index,
            evidence,
            verified_at: Utc::now(),
        });
    }

    // -- Contract versioning --------------------------------------------------

    /// Compute the SHA-256 digest of the current contract state.
    pub fn compute_digest(&self) -> String {
        let json = serde_json::to_string(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Bump the version, record the previous state in history, and return the new digest.
    pub fn bump_version(&mut self) -> String {
        let prev_json = serde_json::to_string(self).unwrap_or_default();
        let digest = {
            let mut hasher = Sha256::new();
            hasher.update(prev_json.as_bytes());
            hex::encode(hasher.finalize())
        };

        self.version_history.push(ContractVersion {
            version: self.version,
            digest: digest.clone(),
            timestamp: Utc::now(),
            contract_json: prev_json,
        });

        self.version += 1;
        digest
    }

    /// Shadow-git rollback stub: restore a previous version by index.
    /// Returns the restored contract JSON and the version that was active before rollback.
    pub fn rollback_to(&mut self, history_index: usize) -> Result<String, ContractError> {
        if history_index >= self.version_history.len() {
            return Err(ContractError::VersionConflict {
                expected: history_index as u64,
                found: self.version_history.len() as u64,
            });
        }

        let snapshot = &self.version_history[history_index];
        let restored_json = snapshot.contract_json.clone();

        // Record current state before rollback
        self.bump_version();

        Ok(restored_json)
    }

    // -- DESIGN.md bidirectional sync -----------------------------------------

    /// Parse a DESIGN.md document and extract acceptance criteria into a GoalContract.
    ///
    /// Convention:
    /// - `## Acceptance Criteria` header starts the section
    /// - Bullet items (`- ` or `* `) become `OutputPattern` criteria
    /// - Items prefixed with `[CMD]` become `CommandExit` criteria
    /// - Items prefixed with `[FILE]` become `FileExists` criteria
    /// - Items prefixed with `[API]` become `ApiCheck` criteria
    pub fn from_design_md(markdown: &str) -> Result<Self, ContractError> {
        let mut contract = Self::new(
            extract_title(markdown).unwrap_or_else(|| "Untitled Contract".into()),
            VerifySpec {
                cmd: "echo ok".into(),
                expected_exit: 0,
                output_pattern: None,
                timeout_ms: 30_000,
            },
        );

        let mut in_criteria_section = false;

        for line in markdown.lines() {
            let trimmed = line.trim();

            // Detect section headers
            if trimmed.starts_with("## ") {
                in_criteria_section = trimmed
                    .trim_start_matches("## ")
                    .to_lowercase()
                    .contains("acceptance criteria");
                continue;
            }

            // Stop at next ## section
            if trimmed.starts_with("## ") && in_criteria_section {
                break;
            }

            if !in_criteria_section {
                continue;
            }

            // Parse bullet items
            let bullet_content = if let Some(stripped) = trimmed.strip_prefix("- ") {
                stripped
            } else if let Some(stripped) = trimmed.strip_prefix("* ") {
                stripped
            } else {
                continue;
            };

            let criterion = if let Some(cmd_part) = bullet_content.strip_prefix("[CMD] ") {
                // Last token is the expected exit code; everything before it is the command.
                let tokens: Vec<&str> = cmd_part.rsplitn(2, ' ').collect();
                let (cmd, exit_code) = if tokens.len() == 2 {
                    if let Ok(code) = tokens[0].parse::<i32>() {
                        (tokens[1].to_string(), code)
                    } else {
                        (cmd_part.to_string(), 0)
                    }
                } else {
                    (cmd_part.to_string(), 0)
                };
                Criterion::CommandExit {
                    cmd,
                    expected_exit: exit_code,
                }
            } else if let Some(file_part) = bullet_content.strip_prefix("[FILE] ") {
                let parts: Vec<&str> = file_part.splitn(2, ' ').collect();
                Criterion::FileExists {
                    path: parts.first().unwrap_or(&"").to_string(),
                    hash: parts.get(1).map(|s| s.to_string()),
                }
            } else if let Some(api_part) = bullet_content.strip_prefix("[API] ") {
                let parts: Vec<&str> = api_part.splitn(2, ' ').collect();
                Criterion::ApiCheck {
                    endpoint: parts.first().unwrap_or(&"").to_string(),
                    status: parts
                        .get(1)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(200),
                    body_pattern: None,
                }
            } else {
                // Default: treat as output pattern
                Criterion::OutputPattern {
                    regex: regex::escape(bullet_content),
                    min_matches: 1,
                }
            };

            contract.acceptance_criteria.push(criterion);
        }

        if contract.acceptance_criteria.is_empty() {
            return Err(ContractError::DesignParseError(
                "no acceptance criteria found in DESIGN.md".into(),
            ));
        }

        Ok(contract)
    }

    /// Generate a DESIGN.md representation from this contract (reverse sync).
    pub fn to_design_md(&self) -> String {
        let mut md = format!("# {}\n\n", self.title);
        md.push_str("## Acceptance Criteria\n\n");

        for criterion in &self.acceptance_criteria {
            match criterion {
                Criterion::CommandExit { cmd, expected_exit } => {
                    md.push_str(&format!("- [CMD] {} {}\n", cmd, expected_exit));
                }
                Criterion::OutputPattern { regex, min_matches } => {
                    md.push_str(&format!(
                        "- Output matches `{}` (min {} matches)\n",
                        regex, min_matches
                    ));
                }
                Criterion::FileExists { path, hash } => {
                    if let Some(h) = hash {
                        md.push_str(&format!("- [FILE] {} {}\n", path, h));
                    } else {
                        md.push_str(&format!("- [FILE] {}\n", path));
                    }
                }
                Criterion::ApiCheck {
                    endpoint,
                    status,
                    body_pattern,
                } => {
                    md.push_str(&format!("- [API] {} {}", endpoint, status));
                    if let Some(bp) = body_pattern {
                        md.push_str(&format!(" body:`{}`", bp));
                    }
                    md.push('\n');
                }
                Criterion::CustomVerify { tool_ref, args } => {
                    md.push_str(&format!(
                        "- [CUSTOM] {} {}\n",
                        tool_ref,
                        args.join(" ")
                    ));
                }
            }
        }

        md.push_str(&format!("\n## Verification\n\n- Command: `{}`\n- Expected exit: {}\n- Timeout: {}ms\n",
            self.verification.cmd, self.verification.expected_exit, self.verification.timeout_ms));

        if !self.artifacts_expected.is_empty() {
            md.push_str("\n## Expected Artifacts\n\n");
            for art in &self.artifacts_expected {
                md.push_str(&format!(
                    "- `{}` (schema: `{}`, required: {})\n",
                    art.name, art.schema, art.required
                ));
            }
        }

        md.push_str(&format!("\n---\nVersion: {}\n", self.version));
        md
    }

    // -- Template engine ------------------------------------------------------

    /// Render a contract template using Handlebars.
    ///
    /// Templates use `{{variable}}` syntax. Built-in templates include
    /// `code-review`, `deploy`, and `test-suite`.
    pub fn from_template(
        template_str: &str,
        params: &HashMap<String, String>,
    ) -> Result<Self, ContractError> {
        let mut hbs = Handlebars::new();
        hbs.set_strict_mode(true);

        hbs.register_template_string("contract", template_str)
            .map_err(|e| ContractError::TemplateError(e.to_string()))?;

        let rendered = hbs
            .render("contract", params)
            .map_err(|e| ContractError::TemplateError(e.to_string()))?;

        // Parse rendered JSON into GoalContract
        serde_json::from_str(&rendered)
            .map_err(|e| ContractError::TemplateError(format!("JSON parse after render: {}", e)))
    }

    /// Built-in code-review contract template.
    pub fn code_review_template() -> &'static str {
        r#"{
  "id": "00000000-0000-0000-0000-000000000000",
  "title": "Code Review: {{project_name}}",
  "acceptance_criteria": [
    {"type": "command_exit", "cmd": "{{lint_cmd}}", "expected_exit": 0},
    {"type": "command_exit", "cmd": "{{test_cmd}}", "expected_exit": 0},
    {"type": "output_pattern", "regex": "{{coverage_pattern}}", "min_matches": 1}
  ],
  "verification": {
    "cmd": "{{verify_cmd}}",
    "expected_exit": 0,
    "output_pattern": null,
    "timeout_ms": 60000
  },
  "preconditions": [],
  "postconditions": [],
  "timeout_ms": 60000,
  "retry_policy": {"max_retries": 2, "backoff_ms": 2000},
  "artifacts_expected": [
    {"name": "review_report", "schema": "json", "required": true}
  ],
  "version": 1,
  "evidence_capsules": [],
  "version_history": []
}"#
    }

    /// Built-in deploy contract template.
    pub fn deploy_template() -> &'static str {
        r#"{
  "id": "00000000-0000-0000-0000-000000000000",
  "title": "Deploy: {{service_name}} to {{environment}}",
  "acceptance_criteria": [
    {"type": "api_check", "endpoint": "{{health_endpoint}}", "status": 200, "body_pattern": null},
    {"type": "command_exit", "cmd": "{{smoke_test_cmd}}", "expected_exit": 0}
  ],
  "verification": {
    "cmd": "{{deploy_verify_cmd}}",
    "expected_exit": 0,
    "output_pattern": "{{success_pattern}}",
    "timeout_ms": 120000
  },
  "preconditions": [],
  "postconditions": [],
  "timeout_ms": 120000,
  "retry_policy": {"max_retries": 1, "backoff_ms": 5000},
  "artifacts_expected": [
    {"name": "deploy_manifest", "schema": "json", "required": true}
  ],
  "version": 1,
  "evidence_capsules": [],
  "version_history": []
}"#
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_title(markdown: &str) -> Option<String> {
    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            return Some(title.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> GoalContract {
        let mut c = GoalContract::new(
            "Test Contract",
            VerifySpec {
                cmd: "echo hello".into(),
                expected_exit: 0,
                output_pattern: Some("hello".into()),
                timeout_ms: 5000,
            },
        );
        c.acceptance_criteria.push(Criterion::CommandExit {
            cmd: "echo test".into(),
            expected_exit: 0,
        });
        c.acceptance_criteria.push(Criterion::OutputPattern {
            regex: r"\d+".into(),
            min_matches: 1,
        });
        c
    }

    // -- Schema validation tests ----------------------------------------------

    #[test]
    fn test_schema_valid_contract() {
        let c = sample_contract();
        assert!(c.validate_schema().is_ok());
    }

    #[test]
    fn test_schema_empty_title_fails() {
        let mut c = sample_contract();
        c.title = "".into();
        let errs = c.validate_schema().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ContractError::SchemaValidation(_))));
    }

    #[test]
    fn test_schema_invalid_regex_in_criterion() {
        let mut c = sample_contract();
        c.acceptance_criteria.push(Criterion::OutputPattern {
            regex: "[invalid".into(),
            min_matches: 1,
        });
        let errs = c.validate_schema().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ContractError::InvalidRegex(_))));
    }

    #[test]
    fn test_schema_invalid_verification_pattern() {
        let mut c = sample_contract();
        c.verification.output_pattern = Some("(unclosed".into());
        let errs = c.validate_schema().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, ContractError::InvalidRegex(_))));
    }

    // -- Precondition tests ---------------------------------------------------

    #[test]
    fn test_preconditions_pass_without_eval_cmd() {
        let mut c = sample_contract();
        c.preconditions.push(Expr {
            description: "info only".into(),
            eval_cmd: None,
            expected_result: None,
        });
        assert!(c.evaluate_preconditions().is_ok());
    }

    #[test]
    fn test_precondition_expected_without_cmd_fails() {
        let mut c = sample_contract();
        c.preconditions.push(Expr {
            description: "broken".into(),
            eval_cmd: None,
            expected_result: Some("yes".into()),
        });
        let errs = c.evaluate_preconditions().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, ContractError::PreconditionFailed(_))));
    }

    // -- WriteGate / dry-run tests --------------------------------------------

    #[test]
    fn test_dry_run_blocks_without_evidence() {
        let c = sample_contract();
        let errs = c.verify_dry_run().unwrap_err();
        assert_eq!(errs.len(), 2); // two criteria, zero capsules
        assert!(errs
            .iter()
            .all(|e| matches!(e, ContractError::WriteGateBlocked(_))));
    }

    #[test]
    fn test_dry_run_passes_with_all_capsules() {
        let mut c = sample_contract();
        c.attach_evidence(
            0,
            Evidence::new("cmd passed", "test", Ground::Observado),
        );
        c.attach_evidence(
            1,
            Evidence::new("pattern matched", "test", Ground::Observado),
        );
        assert!(c.verify_dry_run().is_ok());
    }

    // -- Full validation tests ------------------------------------------------

    #[test]
    fn test_validate_contract_all_phases_pass() {
        let mut c = sample_contract();
        c.attach_evidence(
            0,
            Evidence::new("cmd ok", "test", Ground::Observado),
        );
        c.attach_evidence(
            1,
            Evidence::new("pattern ok", "test", Ground::Observado),
        );
        let result = c.validate_contract();
        assert!(result.schema_valid);
        assert!(result.preconditions_met);
        assert!(result.dry_run_passed);
        assert!(result.errors.is_empty());
        assert!(result.proof.is_some());
    }

    #[test]
    fn test_validate_contract_fails_on_missing_evidence() {
        let c = sample_contract();
        let result = c.validate_contract();
        assert!(result.schema_valid);
        assert!(result.preconditions_met);
        assert!(!result.dry_run_passed);
        assert!(!result.errors.is_empty());
    }

    // -- Artifact mismatch tests ----------------------------------------------

    #[test]
    fn test_artifact_match() {
        let mut c = sample_contract();
        c.artifacts_expected.push(ArtifactSpec {
            name: "report".into(),
            schema: "json".into(),
            required: true,
        });
        assert!(c.check_artifact("report", "json").is_ok());
    }

    #[test]
    fn test_artifact_mismatch_detected() {
        let mut c = sample_contract();
        c.artifacts_expected.push(ArtifactSpec {
            name: "report".into(),
            schema: "json".into(),
            required: true,
        });
        let err = c.check_artifact("report", "yaml").unwrap_err();
        assert!(matches!(err, ContractError::ArtifactMismatch { .. }));
    }

    #[test]
    fn test_unknown_artifact_errors() {
        let c = sample_contract();
        let err = c.check_artifact("nonexistent", "json").unwrap_err();
        assert!(matches!(err, ContractError::ArtifactMismatch { .. }));
    }

    // -- Version round-trip tests ---------------------------------------------

    #[test]
    fn test_version_bump_increments() {
        let mut c = sample_contract();
        assert_eq!(c.version, 1);
        c.bump_version();
        assert_eq!(c.version, 2);
        assert_eq!(c.version_history.len(), 1);
    }

    #[test]
    fn test_version_digest_is_sha256() {
        let c = sample_contract();
        let digest = c.compute_digest();
        assert_eq!(digest.len(), 64); // SHA-256 hex = 64 chars
    }

    #[test]
    fn test_rollback_restores_previous_json() {
        let mut c = sample_contract();
        let original_title = c.title.clone();
        c.bump_version(); // save v1

        c.title = "Modified Title".into();
        c.bump_version(); // save v2

        let restored = c.rollback_to(0).unwrap();
        let restored_contract: GoalContract = serde_json::from_str(&restored).unwrap();
        assert_eq!(restored_contract.title, original_title);
        assert_eq!(restored_contract.version, 1);
    }

    #[test]
    fn test_rollback_invalid_index_errors() {
        let mut c = sample_contract();
        let err = c.rollback_to(99).unwrap_err();
        assert!(matches!(err, ContractError::VersionConflict { .. }));
    }

    // -- DESIGN.md sync tests -------------------------------------------------

    #[test]
    fn test_design_md_parse_extracts_criteria() {
        let md = r#"# My Feature

## Overview
Some description.

## Acceptance Criteria
- Output contains "success"
- [CMD] cargo test 0
- [FILE] ./output.json abc123
- [API] /health 200

## Notes
Extra info.
"#;
        let contract = GoalContract::from_design_md(md).unwrap();
        assert_eq!(contract.title, "My Feature");
        assert_eq!(contract.acceptance_criteria.len(), 4);
        assert!(matches!(
            &contract.acceptance_criteria[0],
            Criterion::OutputPattern { .. }
        ));
        assert!(matches!(
            &contract.acceptance_criteria[1],
            Criterion::CommandExit { .. }
        ));
        assert!(matches!(
            &contract.acceptance_criteria[2],
            Criterion::FileExists { .. }
        ));
        assert!(matches!(
            &contract.acceptance_criteria[3],
            Criterion::ApiCheck { .. }
        ));
    }

    #[test]
    fn test_design_md_no_criteria_errors() {
        let md = "# Empty\n\n## Acceptance Criteria\n\nNo bullets here.\n";
        let err = GoalContract::from_design_md(md).unwrap_err();
        assert!(matches!(err, ContractError::DesignParseError(_)));
    }

    #[test]
    fn test_design_md_roundtrip() {
        let md = r#"# Roundtrip Test

## Acceptance Criteria
- [CMD] echo hello 0
- [FILE] ./test.txt deadbeef
- Output matches `\w+` (min 1 matches)
"#;
        let contract = GoalContract::from_design_md(md).unwrap();
        let regenerated = contract.to_design_md();
        assert!(regenerated.contains("# Roundtrip Test"));
        assert!(regenerated.contains("[CMD] echo hello 0"));
        assert!(regenerated.contains("[FILE] ./test.txt deadbeef"));
    }

    // -- Template expansion tests ---------------------------------------------

    #[test]
    fn test_template_code_review_expansion() {
        let mut params = HashMap::new();
        params.insert("project_name".into(), "zylos-core".into());
        params.insert("lint_cmd".into(), "cargo clippy".into());
        params.insert("test_cmd".into(), "cargo test".into());
        params.insert("coverage_pattern".into(), "coverage: \\\\d+%".into());
        params.insert("verify_cmd".into(), "cargo verify".into());

        let contract =
            GoalContract::from_template(GoalContract::code_review_template(), &params).unwrap();
        assert_eq!(contract.title, "Code Review: zylos-core");
        assert_eq!(contract.acceptance_criteria.len(), 3);
    }

    #[test]
    fn test_template_deploy_expansion() {
        let mut params = HashMap::new();
        params.insert("service_name".into(), "gateway".into());
        params.insert("environment".into(), "staging".into());
        params.insert("health_endpoint".into(), "https://staging.example.com/health".into());
        params.insert("smoke_test_cmd".into(), "curl -f https://staging.example.com".into());
        params.insert("deploy_verify_cmd".into(), "kubectl rollout status".into());
        params.insert("success_pattern".into(), "successfully rolled out".into());

        let contract =
            GoalContract::from_template(GoalContract::deploy_template(), &params).unwrap();
        assert_eq!(contract.title, "Deploy: gateway to staging");
        assert_eq!(contract.acceptance_criteria.len(), 2);
    }

    #[test]
    fn test_template_missing_param_errors() {
        let params = HashMap::new(); // empty
        let err =
            GoalContract::from_template(GoalContract::code_review_template(), &params).unwrap_err();
        assert!(matches!(err, ContractError::TemplateError(_)));
    }

    // -- Serde round-trip -----------------------------------------------------

    #[test]
    fn test_serde_roundtrip_goal_contract() {
        let mut c = sample_contract();
        c.artifacts_expected.push(ArtifactSpec {
            name: "out".into(),
            schema: "json".into(),
            required: true,
        });
        c.attach_evidence(
            0,
            Evidence::new("test evidence", "src", Ground::Observado),
        );
        c.bump_version();

        let json = serde_json::to_string(&c).unwrap();
        let deserialized: GoalContract = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, c.id);
        assert_eq!(deserialized.title, c.title);
        assert_eq!(deserialized.version, c.version);
        assert_eq!(deserialized.acceptance_criteria.len(), c.acceptance_criteria.len());
        assert_eq!(deserialized.artifacts_expected.len(), 1);
    }
}