#!/usr/bin/env python3
"""Add missing /// inline docs to three lib.rs files."""
import re

def add_doc(lines, idx, doc_text):
    """Insert /// doc before the line at idx, matching indentation."""
    indent = len(lines[idx]) - len(lines[idx].lstrip())
    doc_line = ' ' * indent + '/// ' + doc_text + '\n'
    lines.insert(idx, doc_line)

def has_doc_above(lines, i):
    """Check if line at index i already has a /// doc above it."""
    j = i - 1
    while j >= 0:
        prev = lines[j].strip()
        if prev == '' or prev.startswith('#['):
            j -= 1
            continue
        if prev.startswith('///'):
            return True
        break
    return False

# ============================================================
# zylos-swarm-orchestrator
# ============================================================
path = 'crates/zylos-swarm-orchestrator/src/lib.rs'
with open(path) as f:
    lines = f.readlines()

insertions = []

for i, line in enumerate(lines):
    stripped = line.strip()
    
    # Task struct fields
    if stripped == 'pub id: TaskId,':
        insertions.append((i, 'Unique identifier for this task.'))
    elif stripped == 'pub name: String,' and 90 < i < 110:
        insertions.append((i, 'Human-readable name for this task.'))
    elif stripped == 'pub description: String,' and 90 < i < 110:
        insertions.append((i, 'Detailed description of the work to be done.'))
    elif stripped == 'pub state: TaskState,' and 90 < i < 110:
        insertions.append((i, 'Current lifecycle state of this task.'))
    elif stripped == 'pub priority: Priority,' and 90 < i < 110:
        insertions.append((i, 'Scheduling priority level.'))
    elif stripped == 'pub created_at: DateTime<Utc>,' and 110 < i < 135:
        insertions.append((i, 'Timestamp when this task was created.'))
    elif stripped == 'pub updated_at: DateTime<Utc>,' and 110 < i < 135:
        insertions.append((i, 'Timestamp of the last modification.'))
    elif stripped == 'pub started_at: Option<DateTime<Utc>>,':
        insertions.append((i, 'Timestamp when execution started, if applicable.'))
    elif stripped == 'pub completed_at: Option<DateTime<Utc>>,':
        insertions.append((i, 'Timestamp when the task reached a terminal state.'))
    
    # SwarmAgent fields
    elif stripped == 'pub id: AgentId,':
        insertions.append((i, 'Unique identifier for this agent.'))
    elif stripped == 'pub name: String,' and 210 < i < 230:
        insertions.append((i, 'Human-readable name for this agent.'))
    elif stripped == 'pub capabilities: Vec<String>,':
        insertions.append((i, 'List of capability tags this agent provides.'))
    elif stripped == 'pub max_concurrent_tasks: usize,':
        insertions.append((i, 'Maximum number of tasks this agent can run simultaneously.'))
    elif stripped == 'pub current_load: usize,':
        insertions.append((i, 'Number of tasks currently assigned to this agent.'))
    elif stripped == 'pub active: bool,':
        insertions.append((i, 'Whether this agent is currently available for scheduling.'))
    elif stripped == 'pub last_heartbeat: DateTime<Utc>,':
        insertions.append((i, 'Timestamp of the last heartbeat received from this agent.'))
    elif stripped == 'pub metadata: HashMap<String, String>,':
        insertions.append((i, 'Arbitrary key-value metadata attached to this agent.'))
    
    # SwarmError enum
    elif stripped == 'pub enum SwarmError {' and not has_doc_above(lines, i):
        insertions.append((i, 'Errors produced by swarm orchestrator operations.'))

# Apply in reverse order
for idx, doc in sorted(insertions, reverse=True):
    add_doc(lines, idx, doc)

with open(path, 'w') as f:
    f.writelines(lines)
print(f'Updated {path}: {len(insertions)} docs added')

# ============================================================
# zylos-evidence-ledger
# ============================================================
path = 'crates/zylos-evidence-ledger/src/lib.rs'
with open(path) as f:
    lines = f.readlines()

insertions = []

for i, line in enumerate(lines):
    stripped = line.strip()
    
    # MerkleProof fields
    if stripped == 'pub entry_id: String,' and 75 < i < 85:
        insertions.append((i, 'ID of the entry this proof covers.'))
    elif stripped == 'pub entry_hash: String,' and 75 < i < 85:
        insertions.append((i, 'Hash of the entry being proved.'))
    elif stripped == 'pub leaf_index: usize,':
        insertions.append((i, 'Position of the entry in the ordered hash list.'))
    elif stripped == 'pub path: Vec<MerklePathNode>,':
        insertions.append((i, 'Sibling hashes from leaf to root.'))
    elif stripped == 'pub root: String,' and 75 < i < 85:
        insertions.append((i, 'Expected Merkle root this proof validates against.'))
    
    # MerklePathNode
    elif stripped == 'pub struct MerklePathNode {' and not has_doc_above(lines, i):
        insertions.append((i, 'A single node in a Merkle proof path.'))
    elif stripped == 'pub hash: String,' and 85 < i < 92:
        insertions.append((i, 'Sibling hash at this level of the tree.'))
    elif stripped == 'pub direction: Direction,':
        insertions.append((i, 'Whether this sibling is on the left or right.'))
    
    # Direction enum
    elif stripped == 'pub enum Direction {' and not has_doc_above(lines, i):
        insertions.append((i, 'Direction of a sibling node in a Merkle proof path.'))
    
    # VerificationResult fields
    elif stripped == 'pub valid: bool,':
        insertions.append((i, 'Whether all checks passed.'))
    elif stripped == 'pub entry_id: String,' and 98 < i < 108:
        insertions.append((i, 'ID of the verified entry.'))
    elif stripped == 'pub checks: Vec<VerificationCheck>,':
        insertions.append((i, 'Individual check results.'))
    
    # VerificationCheck
    elif stripped == 'pub struct VerificationCheck {' and not has_doc_above(lines, i):
        insertions.append((i, 'Result of a single verification check.'))
    elif stripped == 'pub name: String,' and 105 < i < 115:
        insertions.append((i, 'Name of the check (e.g. "payload_hash", "signature").'))
    elif stripped == 'pub passed: bool,':
        insertions.append((i, 'Whether this check passed.'))
    elif stripped == 'pub detail: Option<String>,':
        insertions.append((i, 'Human-readable detail when the check fails.'))
    
    # AppendRequest fields
    elif stripped == 'pub evidence_type: String,' and 113 < i < 125:
        insertions.append((i, 'Category of evidence (e.g. "tool_output", "agent_decision").'))
    elif stripped == 'pub actor: String,' and 113 < i < 125:
        insertions.append((i, 'Identity of the actor that produced this evidence.'))
    elif stripped == 'pub description: String,' and 113 < i < 125:
        insertions.append((i, 'Human-readable description of the evidence.'))
    elif stripped == 'pub payload: serde_json::Value,' and 113 < i < 125:
        insertions.append((i, 'The actual evidence data as JSON.'))
    elif stripped == 'pub tags: Vec<String>,' and 113 < i < 125:
        insertions.append((i, 'Optional tags for filtering and grouping.'))
    
    # LedgerStats fields
    elif stripped == 'pub total_entries: u64,':
        insertions.append((i, 'Total number of entries in the ledger.'))
    elif stripped == 'pub latest_sequence: Option<u64>,':
        insertions.append((i, 'Sequence number of the most recent entry, if any.'))
    elif stripped == 'pub latest_entry_hash: Option<String>,':
        insertions.append((i, 'Entry hash of the most recent entry, if any.'))
    elif stripped == 'pub merkle_root: Option<String>,':
        insertions.append((i, 'Current Merkle root over all entries, if any.'))
    elif stripped == 'pub evidence_type_counts: Vec<(String, u64)>,':
        insertions.append((i, 'Breakdown of entries by evidence_type.'))
    
    # LedgerError enum
    elif stripped == 'pub enum LedgerError {' and not has_doc_above(lines, i):
        insertions.append((i, 'Errors produced by evidence ledger operations.'))

for idx, doc in sorted(insertions, reverse=True):
    add_doc(lines, idx, doc)

with open(path, 'w') as f:
    f.writelines(lines)
print(f'Updated {path}: {len(insertions)} docs added')

# ============================================================
# zylos-goals
# ============================================================
path = 'crates/zylos-goals/src/lib.rs'
with open(path) as f:
    lines = f.readlines()

insertions = []

for i, line in enumerate(lines):
    stripped = line.strip()
    
    # Goal fields
    if stripped == 'pub id: String,' and 22 < i < 38:
        insertions.append((i, 'Unique identifier for this goal.'))
    elif stripped == 'pub title: String,' and 22 < i < 38:
        insertions.append((i, 'Short title for this goal.'))
    elif stripped == 'pub description: Option<String>,' and 22 < i < 38:
        insertions.append((i, 'Optional longer description.'))
    elif stripped == 'pub owner: Option<String>,' and 22 < i < 38:
        insertions.append((i, 'Person or team responsible for this goal.'))
    elif stripped == 'pub status: GoalStatus,':
        insertions.append((i, 'Current lifecycle status.'))
    elif stripped == 'pub priority: Priority,' and 22 < i < 38:
        insertions.append((i, 'Importance level for scheduling and display.'))
    elif stripped == 'pub created_at: DateTime<Utc>,' and 22 < i < 38:
        insertions.append((i, 'Timestamp when this goal was created.'))
    elif stripped == 'pub updated_at: DateTime<Utc>,' and 22 < i < 38:
        insertions.append((i, 'Timestamp of the last modification.'))
    elif stripped == 'pub due_date: Option<DateTime<Utc>>,':
        insertions.append((i, 'Optional target completion date.'))
    elif stripped == 'pub parent_goal_id: Option<String>,':
        insertions.append((i, 'Parent goal ID for hierarchical nesting.'))
    elif stripped == 'pub tags: Vec<String>,' and 22 < i < 38:
        insertions.append((i, 'Free-form tags for filtering and grouping.'))
    
    # KeyResult fields
    elif stripped == 'pub id: String,' and 115 < i < 130:
        insertions.append((i, 'Unique identifier for this key result.'))
    elif stripped == 'pub goal_id: String,':
        insertions.append((i, 'ID of the goal this key result belongs to.'))
    elif stripped == 'pub title: String,' and 115 < i < 130:
        insertions.append((i, 'Short title describing the measurable outcome.'))
    elif stripped == 'pub metric_type: MetricType,':
        insertions.append((i, 'Kind of metric being tracked.'))
    elif stripped == 'pub target_value: f64,':
        insertions.append((i, 'Target value that constitutes achievement.'))
    elif stripped == 'pub current_value: f64,':
        insertions.append((i, 'Current measured value.'))
    elif stripped == 'pub unit: Option<String>,':
        insertions.append((i, 'Display unit (e.g. "users", "%", "USD").'))
    elif stripped == 'pub status: KrStatus,':
        insertions.append((i, 'Completion status, auto-updated based on current vs target value.'))
    elif stripped == 'pub created_at: DateTime<Utc>,' and 120 < i < 130:
        insertions.append((i, 'Timestamp when this key result was created.'))
    elif stripped == 'pub updated_at: DateTime<Utc>,' and 120 < i < 130:
        insertions.append((i, 'Timestamp of the last progress update.'))
    
    # Milestone fields
    elif stripped == 'pub id: String,' and 204 < i < 218:
        insertions.append((i, 'Unique identifier for this milestone.'))
    elif stripped == 'pub goal_id: String,' and 204 < i < 218:
        insertions.append((i, 'ID of the goal this milestone belongs to.'))
    elif stripped == 'pub title: String,' and 204 < i < 218:
        insertions.append((i, 'Short title for the milestone.'))
    elif stripped == 'pub description: Option<String>,' and 204 < i < 218:
        insertions.append((i, 'Optional description of what this checkpoint represents.'))
    elif stripped == 'pub due_date: DateTime<Utc>,' and 204 < i < 218:
        insertions.append((i, 'Target date for this milestone.'))
    elif stripped == 'pub completed_at: Option<DateTime<Utc>>,' and 204 < i < 218:
        insertions.append((i, 'Actual completion timestamp, if completed.'))
    elif stripped == 'pub status: MilestoneStatus,':
        insertions.append((i, 'Current completion status.'))
    elif stripped == 'pub created_at: DateTime<Utc>,' and 204 < i < 218:
        insertions.append((i, 'Timestamp when this milestone was created.'))
    
    # ProgressWatcher fields
    elif stripped == 'pub id: String,' and 250 < i < 268:
        insertions.append((i, 'Unique identifier for this watcher.'))
    elif stripped == 'pub key_result_id: String,':
        insertions.append((i, 'ID of the key result this watcher monitors.'))
    elif stripped == 'pub cron_expr: String,':
        insertions.append((i, 'Cron expression defining the evaluation schedule.'))
    elif stripped == 'pub source_type: WatcherSourceType,':
        insertions.append((i, 'Kind of data source used to collect the metric.'))
    elif stripped == 'pub source_config: serde_json::Value,':
        insertions.append((i, 'Source-specific configuration (query, URL, skill name, etc.).'))
    elif stripped == 'pub enabled: bool,' and 250 < i < 268:
        insertions.append((i, 'Whether this watcher is currently active.'))
    elif stripped == 'pub last_run_at: Option<DateTime<Utc>>,' and 250 < i < 268:
        insertions.append((i, 'Timestamp of the last evaluation run.'))
    elif stripped == 'pub last_result: Option<f64>,':
        insertions.append((i, 'Numeric value returned by the last evaluation.'))
    elif stripped == 'pub last_error: Option<String>,' and 250 < i < 268:
        insertions.append((i, 'Error message from the last evaluation, if it failed.'))
    elif stripped == 'pub created_at: DateTime<Utc>,' and 250 < i < 268:
        insertions.append((i, 'Timestamp when this watcher was created.'))
    
    # WatcherSourceType enum
    elif stripped == 'pub enum WatcherSourceType {' and not has_doc_above(lines, i):
        insertions.append((i, 'Source type for a progress watcher.'))
    
    # GoalAuditEntry fields
    elif stripped == 'pub id: String,' and 306 < i < 320:
        insertions.append((i, 'Unique identifier for this audit entry.'))
    elif stripped == 'pub entity_type: String,':
        insertions.append((i, 'Type of entity mutated ("goal", "key_result", "milestone", "watcher").'))
    elif stripped == 'pub entity_id: String,':
        insertions.append((i, 'ID of the mutated entity.'))
    elif stripped == 'pub action: String,':
        insertions.append((i, 'Action performed (e.g. "created", "status_changed").'))
    elif stripped == 'pub old_value: Option<String>,':
        insertions.append((i, 'Previous value before the mutation, if applicable.'))
    elif stripped == 'pub new_value: Option<String>,':
        insertions.append((i, 'New value after the mutation, if applicable.'))
    elif stripped == 'pub actor: Option<String>,' and 306 < i < 320:
        insertions.append((i, 'Identity of the actor who performed the mutation.'))
    elif stripped == 'pub timestamp: DateTime<Utc>,' and 306 < i < 320:
        insertions.append((i, 'Timestamp when the mutation occurred.'))

for idx, doc in sorted(insertions, reverse=True):
    add_doc(lines, idx, doc)

with open(path, 'w') as f:
    f.writelines(lines)
print(f'Updated {path}: {len(insertions)} docs added')

print('Done! All inline docs added.')