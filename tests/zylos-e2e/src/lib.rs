//! Zylos-Next End-to-End Test Suite
//!
//! 20 E2E scenarios covering Memory, Orchestrator, Verification, Sandbox,
//! Skills, Cockpit, HITL, Task Durability, SOAR, and Sentinel subsystems.
//!
//! Run: `cargo test -p zylos-e2e -- --test-threads=1`

// ---------------------------------------------------------------------------
// Shared test infrastructure
// ---------------------------------------------------------------------------
mod helpers;

// ---------------------------------------------------------------------------
// E2E Scenarios (Tasks E.2.1 – E.2.20)
// ---------------------------------------------------------------------------

/// E.2.1 — Memory consolidation: contradictory facts resolve to latest timestamp
#[cfg(test)]
mod memory_consolidation;

/// E.2.2 — Memory decay: stale entries pruned after TTL
#[cfg(test)]
mod memory_decay;

/// E.2.3 — Orchestrator DAG with 100 nodes executes in topological order
#[cfg(test)]
mod orchestrator_dag_100;

/// E.2.4 — Orchestrator rollback on mid-DAG failure
#[cfg(test)]
mod orchestrator_rollback;

/// E.2.5 — Orchestrator recovery from checkpoint after crash
#[cfg(test)]
mod orchestrator_recovery;

/// E.2.6 — Verification proof generation and validation round-trip
#[cfg(test)]
mod verification_proof;

/// E.2.7 — Verification adversarial panel rejects weak output
#[cfg(test)]
mod verification_panel;

/// E.2.8 — Sandbox WASM execution with resource limits
#[cfg(test)]
mod sandbox_wasm;

/// E.2.9 — Sandbox Docker execution with network isolation
#[cfg(test)]
mod sandbox_docker;

/// E.2.10 — Skills registry: register, list, eval, export
#[cfg(test)]
mod skills_registry;

/// E.2.11 — Cockpit UI serves static assets and WASM bundle
#[cfg(test)]
mod cockpit_serve;

/// E.2.12 — Cockpit SSE streaming delivers events in order
#[cfg(test)]
mod cockpit_streaming;

/// E.2.13 — Cockpit lazy-roster renders 500 agents under 100ms
#[cfg(test)]
mod cockpit_lazy_roster;

/// E.2.14 — HITL approval lifecycle: request → approve → execute
#[cfg(test)]
mod hitl_approval;

/// E.2.15 — HITL equivalence reuse: same-action skips second approval
#[cfg(test)]
mod hitl_equivalence;

/// E.2.16 — Task durability: lease steal prevented by CAS
#[cfg(test)]
mod task_durability_cas;

/// E.2.17 — Task durability: maintenance loop recovers orphaned leases
#[cfg(test)]
mod task_durability_recovery;

/// E.2.18 — SOAR playbook dispatch on matching event trigger
#[cfg(test)]
mod soar_playbook_dispatch;

/// E.2.19 — Sentinel health scanner detects degraded subsystem
#[cfg(test)]
mod sentinel_health;

/// E.2.20 — Full pipeline: event → SOAR → HITL → orchestrate → cockpit stream
#[cfg(test)]
mod full_pipeline;

// ---------------------------------------------------------------------------
// E.4 — Benchmark Final
// ---------------------------------------------------------------------------

/// E.4 — Performance benchmarks: throughput, latency p99, DAG resolution, roster render
#[cfg(test)]
mod benchmark_final;
