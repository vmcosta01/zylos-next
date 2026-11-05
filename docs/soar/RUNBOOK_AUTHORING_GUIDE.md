# Runbook Authoring Guide for Zylos SOAR

**Version:** 1.0
**Audience:** Developers writing SOAR-compatible runbooks
**Prerequisite:** Familiarity with Rust async traits and the `zylos-soar` crate

---

## Overview

A **runbook** in Zylos SOAR is an atomic, executable unit of incident response logic. Runbooks are invoked by playbooks (YAML state machines) through the `SoarRunnable` trait defined in `crates/zylos-soar/src/runnable.rs`. Every runbook must implement pre-condition validation, execution, and optional rollback.

Runbooks are registered in the `RunbookRegistry` (`registry.rs`) by name, and referenced from playbook YAML via the `states[].runbook` field.

---

## The SoarRunnable Trait

```rust
// crates/zylos-soar/src/runnable.rs

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait SoarRunnable: Send + Sync {
    /// Human-readable name — must match the registry key exactly.
    fn name(&self) -> &str;

    /// Validate pre-conditions before execution.
    /// Return Err to abort; the executor will record the failure in the audit trail.
    fn validate_preconditions<'a>(
        &'a self,
        context: &'a IncidentContext,
    ) -> BoxFuture<'a, Result<()>>;

    /// Execute the runbook and return a structured result.
    fn execute<'a>(
        &'a self,
        context: IncidentContext,
    ) -> BoxFuture<'a, Result<RunbookResult>>;

    /// Optional rollback procedure if execution fails mid-way.
    /// Default implementation returns None (no rollback).
    fn rollback(&self) -> Option<Box<dyn SoarRunnable>> {
        None
    }
}
```

### Why BoxFuture instead of async fn?

The trait uses `BoxFuture` return types to remain **dyn-compatible** (object-safe). This allows the `RunbookRegistry` to store `Box<dyn SoarRunnable>` and dispatch dynamically at runtime. If you use `async fn`, the compiler cannot create a vtable for the trait object.

---

## IncidentContext

Every runbook receives an `IncidentContext` carrying the current incident state:

```rust
pub struct IncidentContext {
    pub incident_id: Uuid,
    pub event_id: Uuid,
    pub severity: String,          // "Critical", "High", "Medium", "Low", "Info"
    pub category: String,          // "Breach", "Anomaly", "Vulnerability", etc.
    pub affected_assets: Vec<String>,
    pub actor: String,             // Who/what triggered this execution
    pub started_at: DateTime<Utc>,
    pub metadata: serde_json::Value, // Accumulated key-value data from prior states
}
```

Use `metadata` to pass data between playbook states. For example, a triage runbook can write `{"confirmed_breach": true}` into metadata, and a downstream state's transition condition can evaluate it.

---

## RunbookResult

Return structured outcomes from `execute()`:

```rust
pub struct RunbookResult {
    pub success: bool,
    pub message: String,
    pub artifacts: Vec<String>,     // File paths, URLs, or IDs produced
    pub duration_ms: u64,
    pub metadata: serde_json::Value, // Output data for downstream states
}
```

Constructors:
- `RunbookResult::ok(message, duration_ms)` — success with no artifacts
- `RunbookResult::fail(message, duration_ms)` — failure

---

## Implementing a Runbook: Step by Step

### 1. Define the struct

```rust
use zylos_soar::runnable::{BoxFuture, IncidentContext, RunbookResult, SoarRunnable};
use zylos_soar::error::Result;
use std::pin::Pin;
use std::future::Future;

pub struct IsolateCompromisedHost;
```

### 2. Implement SoarRunnable

```rust
impl SoarRunnable for IsolateCompromisedHost {
    fn name(&self) -> &str {
        "isolate-compromised-host"  // Must match playbook YAML reference
    }

    fn validate_preconditions<'a>(
        &'a self,
        context: &'a IncidentContext,
    ) -> BoxFuture<'a, Result<()>> {
        Box::pin(async move {
            // Verify we have at least one affected asset
            if context.affected_assets.is_empty() {
                return Err(zylos_soar::error::SoarError::RunbookExecution(
                    "No affected assets specified — cannot isolate".into(),
                ));
            }
            Ok(())
        })
    }

    fn execute<'a>(
        &'a self,
        context: IncidentContext,
    ) -> BoxFuture<'a, Result<RunbookResult>> {
        Box::pin(async move {
            let start = chrono::Utc::now();

            // ... isolation logic here ...

            let duration_ms = (chrono::Utc::now() - start).num_milliseconds().max(0) as u64;
            Ok(RunbookResult::ok(
                format!("Isolated {} assets", context.affected_assets.len()),
                duration_ms,
            ))
        })
    }

    fn rollback(&self) -> Option<Box<dyn SoarRunnable>> {
        Some(Box::new(RestoreNetworkAccess))
    }
}
```

### 3. Register in the RunbookRegistry

```rust
let mut registry = RunbookRegistry::new();
registry.register(Box::new(IsolateCompromisedHost));
```

### 4. Reference from a playbook YAML

```yaml
states:
  - name: isolate
    auto: false
    approval_required: true
    runbook: "isolate-compromised-host"   # ← matches SoarRunnable::name()
    timeout_minutes: 15
    escalation: "incident-commander"
    next:
      - to: investigate
```

---

## Pre-condition Validation

`validate_preconditions()` runs **before** `execute()` and **before** any retries. Use it to fail fast on invalid inputs:

- Missing required assets or parameters
- Incompatible severity/category combinations
- External dependency health checks (e.g., API reachable)
- Permission or authorization verification

If preconditions fail, the executor records `AuditResult::Failure` and does **not** attempt execution or retries.

---

## Rollback

Override `rollback()` to return a compensating runbook when partial execution leaves the system in an inconsistent state:

```rust
fn rollback(&self) -> Option<Box<dyn SoarRunnable>> {
    Some(Box::new(RestoreNetworkAccess))
}
```

Rollback guidelines:
- Keep rollbacks **idempotent** — safe to call multiple times
- Rollbacks should be **best-effort** — log failures but don't panic
- Not every runbook needs rollback; the default `None` is correct for read-only or purely additive operations
- The rollback runbook itself must implement `SoarRunnable`

---

## Executor Behavior

The `PlaybookExecutor` (`executor.rs`) drives runbook execution with these defaults:

| Parameter | Default | Description |
|---|---|---|
| `default_timeout_secs` | 300 | Max wall-clock time per state |
| `max_retries` | 3 | Retry attempts on transient failure |
| `retry_delay_ms` | 1000 | Delay between retries |

Execution flow per state:
1. Look up runbook by name in `RunbookRegistry`
2. Call `validate_preconditions()` — abort on error
3. Call `execute()` with retry loop
4. Record `AuditEntry` with timing, result, and state hash
5. Advance `PlaybookCursor` to next state

If a runbook is not found in the registry, the executor returns `SoarError::RunbookExecution("runbook 'X' not found in registry")`.

---

## Audit Trail Integration

Every state execution automatically produces a signed `AuditEntry` containing:

- Incident ID and actor
- Action string: `execute_state:<state_name>`
- Parameters JSON: playbook name, state name, runbook name, duration, result
- Chained state hash for tamper detection
- HMAC-SHA256 signature

You do not need to write audit entries manually — the executor handles this. If your runbook produces additional auditable side effects, include them in the `RunbookResult.metadata` field.

---

## Testing Runbooks

Use the pattern from `runnable.rs` tests:

```rust
#[tokio::test]
async fn test_isolate_runbook() {
    let runbook = IsolateCompromisedHost;
    let ctx = IncidentContext::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        "critical",
        "breach",
        vec!["host-01".to_string()],
        "test-agent",
    );

    runbook.validate_preconditions(&ctx).await.unwrap();
    let result = runbook.execute(ctx).await.unwrap();
    assert!(result.success);
}
```

Test both happy path and precondition failures. Verify that `name()` returns the exact string used in playbook YAML.

---

## Checklist Before Shipping

- [ ] `name()` returns a stable, kebab-case string matching the playbook YAML reference
- [ ] `validate_preconditions()` fails fast on invalid inputs without side effects
- [ ] `execute()` returns `RunbookResult` with accurate `duration_ms`
- [ ] `rollback()` is implemented if the runbook has side effects that need compensation
- [ ] Runbook is registered in `RunbookRegistry` during application startup
- [ ] Unit tests cover success, precondition failure, and execution failure paths
- [ ] No secrets or credentials hardcoded — use vault or environment injection
- [ ] Playbook YAML referencing this runbook passes `Playbook::from_yaml()` validation