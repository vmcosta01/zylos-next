# Migration Guide: Hermes v1 → Zylos-Next v6.0

## Overview

Zylos-Next is the Rust-native successor to Hermes v1's TypeScript orchestration layer. This guide covers migrating existing Hermes deployments to Zylos-Next while preserving agent state, skills, and workflows.

Migration is designed to be incremental: you can run both systems in parallel during the transition period, validating each component before fully switching over.

## What Changes

| Aspect | Hermes v1 | Zylos-Next v6.0 |
|--------|-----------|-----------------|
| Language | TypeScript/Node.js | Rust |
| Memory | In-memory only | SQLite + consolidation + decay |
| Verification | Basic guards | 5-level Ground proofs + adversarial panels |
| Sandbox | None | WASM + Docker + browser anti-detect |
| HITL | Simple approve/deny | Equivalence-hashed cards + 4-tier decisions |
| Triage | Manual | Two-stage (rules + model) with audit trail |
| Monitoring | Logs only | Sentinel AIOps + SOAR integration |
| Skills | Inline definitions | Registry (338 migrated) with eval harness |
| Frontend | React | Leptos 0.7 WASM (SSR native) |
| RAM per agent | ~100MB | ~30MB idle |
| Config format | JSON/YAML | TOML (with env var overrides) |
| API protocol | REST + WebSocket | REST + SSE (AG-UI compatible) |

## Pre-Migration Checklist

Before starting migration, ensure:

- [ ] Hermes v1 is running a supported version (>= 3.2.0)
- [ ] All active agents have been backed up (`hermes export --format json`)
- [ ] Skills directory has been exported (`hermes skills export --dir ./hermes-skills/`)
- [ ] Memory snapshots exist for all critical agents (`hermes memory snapshot`)
- [ ] Custom playbooks and runbooks are documented
- [ ] Integration endpoints (webhooks, API consumers) are inventoried
- [ ] Test environment is available for validation
- [ ] Rollback plan is defined (keep Hermes v1 running in parallel)

## Migration Steps

### Step 1: Export Hermes v1 State

Export all agent configurations, skills, and memory from your running Hermes v1 instance:

```bash
# Export agent configurations
hermes export --format json --output hermes-export.json

# Export skills registry
hermes skills export --dir ./hermes-skills/

# Export memory snapshots for all agents
hermes memory snapshot --output hermes-memory.db

# Export custom playbooks
cp -r ./playbooks/ ./hermes-playbooks-backup/

# Export custom runbooks
cp -r ./runbooks/ ./hermes-runbooks-backup/
```

Verify exports:

```bash
# Check agent count
jq '.agents | length' hermes-export.json
# Expected: your agent count

# Check skills count
ls ./hermes-skills/ | wc -l
# Expected: your skill count

# Check memory database
sqlite3 hermes-memory.db "SELECT COUNT(*) FROM memories;"
# Expected: your memory entry count
```

### Step 2: Install Zylos-Next

Choose your installation method:

#### From Source

```bash
git clone https://github.com/nousresearch/zylos-next.git
cd zylos-next
cargo build --release

# Verify build
./target/release/zylos-gateway --version
# zylos-gateway 0.1.0
```

#### Docker

```bash
docker pull ghcr.io/nousresearch/zylos-gateway:latest

# Verify image
docker run --rm ghcr.io/nousresearch/zylos-gateway:latest --version
```

#### Kubernetes

```bash
helm repo add zylos https://charts.nousresearch.com
helm repo update
helm install zylos zylos/gateway --namespace zylos --create-namespace
```

See [Deployment Guide](DEPLOYMENT.md) for full deployment instructions.

### Step 3: Migrate Skills

Skills are auto-migrated on first startup if placed in the configured skills directory:

```bash
# Copy Hermes skills to Zylos skills directory
cp -r ./hermes-skills/ ./skills/

# Ensure config points to skills directory
# In config.toml:
# [skills]
# index_path = "./skills"
```

Start the gateway once to trigger automatic migration:

```bash
./target/release/zylos-gateway --config config.toml
# Watch logs for skill migration messages:
# [INFO] Migrating skill: web-search v2.1 -> v3.0
# [INFO] Migrating skill: code-review v1.5 -> v2.0
# [WARN] Skill 'legacy-parser' requires manual review
```

The registry validates format compatibility and reports any skills requiring manual adjustment. Check `migration-report.json` for details:

```bash
cat migration-report.json | jq '.skills_requiring_review'
```

#### Manual Skill Adjustments

Some Hermes v1 skills may need updates for Zylos-Next compatibility:

1. **Async handlers**: Convert callback-based handlers to async/await
2. **Error types**: Update error handling to use `thiserror` patterns
3. **Memory access**: Replace direct memory access with `MemoryStore` API
4. **Sandbox calls**: Add sandbox tier annotations for untrusted operations

Example skill migration:

```rust
// Before (Hermes v1 TypeScript)
export default {
  name: "web-search",
  handler: (query, callback) => {
    fetch(`https://api.search.com?q=${query}`)
      .then(res => res.json())
      .then(data => callback(null, data))
      .catch(err => callback(err));
  }
}

// After (Zylos-Next Rust)
use zylos_core::{Skill, SkillResult};

pub struct WebSearch;

#[async_trait]
impl Skill for WebSearch {
    fn name(&self) -> &str { "web-search" }

    async fn execute(&self, input: Value) -> SkillResult<Value> {
        let query = input["query"].as_str()
            .ok_or_else(|| SkillError::invalid_input("missing query"))?;

        let response = reqwest::get(format!(
            "https://api.search.com?q={}", query
        )).await?;

        let data: Value = response.json().await?;
        Ok(data)
    }
}
```

### Step 4: Import Agent Configurations

Convert Hermes JSON agent configs to Zylos TOML format:

```bash
# Run migration tool
./target/release/zylos-migrate \
  --input hermes-export.json \
  --output zylos-agents/ \
  --format toml
```

Review generated configs:

```bash
ls zylos-agents/
# agent-01.toml, agent-02.toml, ...

# Spot-check a config
cat zylos-agents/agent-01.toml
```

Key differences in agent configuration:

| Hermes v1 Field | Zylos-Next Equivalent | Notes |
|-----------------|----------------------|-------|
| `agent.id` | `[agent] id` | Same format |
| `agent.model` | `[llm] provider` | Now supports multiple providers |
| `agent.tools` | `[tools] enabled` | Array of tool names |
| `agent.memory` | `[memory] strategy` | Added consolidation options |
| `agent.sandbox` | `[sandbox] tier` | New: wasm, docker, browser |
| `agent.approval` | `[hitl] policy` | New: equivalence hashing |

### Step 5: Migrate Memory

Import Hermes memory snapshots into Zylos SQLite store:

```bash
# Import memory database
./target/release/zylos-migrate \
  --memory hermes-memory.db \
  --output data/memory.db
```

Memory consolidation runs automatically on first startup:

```bash
# Start gateway to trigger consolidation
./target/release/zylos-gateway --config config.toml

# Monitor consolidation progress in logs:
# [INFO] Consolidation started: 12,847 entries
# [INFO] Merged 3,412 duplicate entries
# [INFO] Detected 198 contradictions (flagged for review)
# [INFO] Applied temporal decay to 1,003 stale entries
# [INFO] Consolidation complete: 8,234 entries (36% reduction)
```

Verify memory integrity:

```bash
sqlite3 data/memory.db "PRAGMA integrity_check;"
# Expected: ok

sqlite3 data/memory.db "SELECT COUNT(*) FROM memories;"
# Expected: consolidated count
```

### Step 6: Migrate Playbooks and Runbooks

Copy operational documents and update references:

```bash
# Copy playbooks
cp -r ./hermes-playbooks-backup/ ./playbooks/

# Copy runbooks
cp -r ./hermes-runbooks-backup/ ./runbooks/

# Update internal links (Hermes v1 paths -> Zylos paths)
find ./playbooks/ ./runbooks/ -name '*.md' -exec sed -i \
  's|hermes/api/v1|/api/v1|g; s|hermes/skills|/opt/zylos/skills|g' {} +
```

Validate playbook syntax against Zylos SOAR engine:

```bash
./target/release/zylos-gateway --validate-playbooks ./playbooks/
# Expected: All playbooks valid
```

### Step 7: Validate Migration

Run the comprehensive migration validator:

```bash
./target/release/zylos-gateway --validate-migration
```

Expected output:

```
=== Migration Validation ===
Agents:     47/47 migrated successfully
Skills:     335/338 validated (3 require manual review)
Memory:     12,847 entries imported, consolidated to 8,234
Playbooks:  23/23 valid
Runbooks:   15/15 valid
Config:     Valid TOML, all required fields present

Warnings:
  - Skill 'legacy-parser' uses deprecated API (see migration-report.json)
  - Skill 'custom-etl' missing sandbox annotation
  - Skill 'internal-auth' requires updated credentials format

Recommendations:
  - Review migration-report.json for detailed skill issues
  - Run integration tests before production cutover
  - Keep Hermes v1 running in read-only mode during validation period
```

### Step 8: Integration Testing

Run end-to-end tests against the migrated system:

```bash
# Run E2E test suite
cargo test --workspace --features e2e

# Or run specific integration tests
cargo test -p zylos-gateway --test e2e_integration

# Manual smoke tests
curl http://localhost:3000/gateway/health
curl http://localhost:3000/api/v1/agents | jq '.[].id'
curl http://localhost:3000/api/v1/skills | jq '.[].name'
```

### Step 9: Deploy and Monitor

Follow the [Deployment Guide](DEPLOYMENT.md) for production setup. Monitor via:

```bash
# Health endpoint
curl http://localhost:3000/gateway/health

# Event stream (SSE)
curl -N http://localhost:3000/events

# Metrics (if Prometheus enabled)
curl http://localhost:3000/metrics
```

### Step 10: Decommission Hermes v1

After validation period (recommended: 2 weeks):

1. Stop Hermes v1 write operations (read-only mode)
2. Final memory sync: `hermes memory snapshot --final`
3. Import final snapshot into Zylos
4. Redirect all traffic to Zylos-Next
5. Archive Hermes v1 deployment
6. Remove Hermes v1 dependencies

## Parallel Operation

During the transition period, run both systems side by side:

```
                    ┌─────────────┐
                    │ Load Balancer│
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              │                         │
     ┌────────▼────────┐      ┌────────▼────────┐
     │   Hermes v1     │      │  Zylos-Next     │
     │  (read-only)    │      │  (active)       │
     └─────────────────┘      └─────────────────┘
```

Configure load balancer to route:
- Write operations → Zylos-Next only
- Read operations → Either (for comparison testing)
- Health checks → Both

## Troubleshooting

### Skill Migration Failures

```bash
# Get detailed migration report
cat migration-report.json | jq '.failed_skills[] | {name, error}'

# Re-run migration for specific skill
./target/release/zylos-migrate --skill web-search --force
```

### Memory Import Errors

```bash
# Check SQLite compatibility
sqlite3 hermes-memory.db ".schema"

# Repair corrupted database
sqlite3 hermes-memory.db ".recover" | sqlite3 data/memory-repaired.db

# Skip problematic entries
./target/release/zylos-migrate --memory hermes-memory.db --skip-invalid
```

### Configuration Mismatches

```bash
# Validate config against schema
./target/release/zylos-gateway --validate-config config.toml

# Generate default config for reference
./target/release/zylos-gateway --generate-config > config-default.toml

# Diff your config against defaults
diff config.toml config-default.toml
```

### Performance Regression

If Zylos-Next performs worse than Hermes v1 for specific workloads:

1. Check consolidation settings (too aggressive = CPU overhead)
2. Review sandbox tier assignments (WASM adds latency)
3. Profile with `RUST_LOG=trace` to identify bottlenecks
4. Compare memory usage: `curl http://localhost:3000/api/v1/agents | jq '.[].memory_mb'`

## Post-Migration Optimization

After successful migration, tune Zylos-Next for your workload:

```toml
# config.toml optimizations
[memory]
consolidation_interval_secs = 7200    # Reduce frequency if CPU-bound
decay_half_life_days = 60             # Extend if losing useful context

[sandbox]
wasm_enabled = false                  # Disable if not running untrusted code
docker_enabled = true                 # Keep for system access isolation

[soar]
event_bus_capacity = 512              # Increase if dropping events
incident_rate_limit = 10              # Raise if throttling legitimate alerts
```

## Support

- Migration issues: Open issue on GitHub with `migration` label
- Skill compatibility: Check `migration-report.json` for specific errors
- Performance concerns: Include benchmark comparison in issue
- Documentation gaps: Submit PR to improve this guide
