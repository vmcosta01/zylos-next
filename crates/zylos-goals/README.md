# zylos-goals

Goal tracking system for **zylos-gateway** with hierarchical goals, OKR key results, milestone checkpoints, cron-based progress watchers, and a full audit trail — all persisted in SQLite WAL mode.

## Features

- **Hierarchical goals** — parent/child relationships with status lifecycle (`draft → active → on_track / at_risk / blocked → completed / cancelled`)
- **OKR key results** — measurable outcomes with automatic status transitions when targets are met
- **Milestones** — timeline checkpoints with due-date tracking
- **Progress watchers** — cron-scheduled metric collectors (skill execution, integration actions, SQL queries, HTTP endpoints, or manual updates)
- **Audit log** — every mutation is recorded with actor, old/new values, and timestamp
- **Dashboard summary** — aggregate metrics across all entities
- **Async-safe** — internal `RwLock` around the SQLite connection for concurrent access

## Quick Start

```rust
use chrono::Utc;
use zylos_goals::{Goal, GoalStatus, GoalStore, KeyResult, KrStatus, MetricType, Priority};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = GoalStore::open(std::path::Path::new("goals.db"))?;
    let now = Utc::now();

    let goal = Goal {
        id: "launch-mvp".into(),
        title: "Launch MVP by Q2".into(),
        description: Some("Ship the first public release".into()),
        owner: Some("alice".into()),
        status: GoalStatus::Active,
        priority: Priority::High,
        created_at: now,
        updated_at: now,
        due_date: None,
        parent_goal_id: None,
        tags: vec!["product".into(), "q2".into()],
    };
    store.create_goal(&goal).await?;

    let kr = KeyResult {
        id: "kr-users".into(),
        goal_id: "launch-mvp".into(),
        title: "Reach 1,000 signups".into(),
        metric_type: MetricType::Count,
        target_value: 1000.0,
        current_value: 0.0,
        unit: Some("users".into()),
        status: KrStatus::NotStarted,
        created_at: now,
        updated_at: now,
    };
    store.create_key_result(&kr).await?;

    store.update_kr_progress("kr-users", 1000.0, Some("bot")).await?;

    let summary = store.dashboard_summary().await?;
    println!("Total goals: {}, Avg KR progress: {:.1}%",
        summary.total_goals, summary.avg_kr_progress_pct);
    Ok(())
}
```

### In-Memory Store (Testing)

```rust
let store = GoalStore::in_memory()?;
```

## Types

| Type | Purpose |
|------|---------|
| `Goal` | High-level objective with status, priority, tags, and optional parent |
| `GoalStatus` | Lifecycle: `Draft`, `Active`, `OnTrack`, `AtRisk`, `Blocked`, `Completed`, `Cancelled` |
| `Priority` | Importance level: `Low`, `Medium`, `High`, `Critical` |
| `KeyResult` | Measurable outcome (OKR KR) with metric_type, target/current value, auto-status |
| `MetricType` | Kind of metric: `Percentage`, `Count`, `Currency`, `Duration`, `Boolean`, `Custom` |
| `KrStatus` | KR completion: `NotStarted`, `InProgress`, `Achieved`, `Missed` |
| `Milestone` | Timeline checkpoint with due date and completion status |
| `MilestoneStatus` | Milestone state: `Pending`, `Completed`, `Overdue`, `Skipped` |
| `ProgressWatcher` | Cron-scheduled metric collector for a key result |
| `WatcherSourceType` | Watcher source: `SkillExecution`, `IntegrationAction`, `SqlQuery`, `HttpEndpoint`, `Manual` |
| `GoalAuditEntry` | Immutable audit log entry for entity mutations |
| `DashboardSummary` | Aggregate snapshot: totals by status, average KR progress |
| `GoalStore` | SQLite WAL store with `RwLock` for async-safe access |
| `GoalsError` | Unified error type (see Error Variants below) |

All enums implement `Display` (lowercase snake_case) and `from_str()` for parsing.

### GoalStore Methods

**Goals:** `create_goal`, `get_goal`, `list_goals(parent_id)`, `update_goal_status`, `delete_goal` (cascade).

**Key Results:** `create_key_result`, `list_key_results(goal_id)`, `update_kr_progress` (auto-updates status).

**Milestones:** `create_milestone`, `list_milestones(goal_id)`, `complete_milestone`.

**Watchers:** `create_watcher` (validates cron), `list_enabled_watchers`, `record_watcher_run`.

**Audit & Dashboard:** `get_audit_log(entity_id, limit)`, `dashboard_summary`.

## Error Variants

`GoalsError` (via `thiserror`):

| Variant | Meaning |
|---------|---------|
| `Database(rusqlite::Error)` | SQLite connection, query, or migration failure |
| `NotFound(String)` | Entity (goal, KR, milestone, watcher) not found by ID |
| `InvalidStatus(String)` | Invalid string for `GoalStatus` parsing |
| `InvalidPriority(String)` | Invalid string for `Priority` parsing |
| `InvalidMetricType(String)` | Invalid string for `MetricType` parsing |
| `InvalidKrStatus(String)` | Invalid string for `KrStatus` parsing |
| `InvalidMilestoneStatus(String)` | Invalid string for `MilestoneStatus` parsing |
| `InvalidWatcherSource(String)` | Invalid string for `WatcherSourceType` parsing |
| `InvalidCron(String)` | Invalid cron expression in watcher |
| `Serialization(serde_json::Error)` | Invalid JSON in source_config or tags |
| `Io(std::io::Error)` | Filesystem or I/O error when opening database |

## Gaps

- **Watchers not auto-executed** — watchers are stored with cron expressions but require an external scheduler to trigger `record_watcher_run`.
- **No title uniqueness** — goals/KRs/milestones can have duplicate titles within the same parent.
- **KR progress not audited** — `update_kr_progress` updates the value but does not log an audit entry for the change.
- **No bulk operations** — no batch update or bulk create for KRs, milestones, or watchers.
- **Dashboard is flat** — `DashboardSummary` does not break down metrics by owner, tag, or priority.
- **Hard deletes only** — `delete_goal` is permanent with cascade; no soft-delete or archival.
- **Cron validation is basic** — depends on the `cron` crate; non-standard expressions may be silently rejected.
- **No KR weights** — all KRs within a goal contribute equally to average progress; no relative weighting.
- **No overdue detection** — milestones past their due date are not automatically marked `Overdue`.

## Dependencies

`serde`, `serde_json`, `tokio`, `thiserror`, `tracing`, `uuid`, `chrono`, `rusqlite`, `cron`

## License

Same as the workspace root.
