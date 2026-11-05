# Zylos-Next Operational Runbook

## Incident Response Procedures

### Severity Classification

| Severity | Definition | Response Time | Escalation |
|----------|-----------|---------------|------------|
| P0 - Critical | Service down, data loss risk | 15min | Immediate page + Slack #incidents |
| P1 - High | Degraded service, single subsystem failure | 30min | Page during business hours |
| P2 - Medium | Non-critical feature broken | 4h | Ticket + next business day |
| P3 - Low | Cosmetic, documentation | Next sprint | Backlog |

### P0: Gateway Down

```bash
# 1. Check process
ps aux | grep zylos-gateway

# 2. Check port binding
ss -tlnp | grep 3000

# 3. Check logs
journalctl -u zylos-gateway --since "1 hour ago" --no-pager

# 4. Restart if hung
systemctl restart zylos-gateway

# 5. Verify recovery
curl -sf http://localhost:3000/gateway/health && echo "OK" || echo "STILL DOWN"

# 6. If persistent, check dependencies
docker ps  # Docker daemon for sandbox
sqlite3 ./data/zylos-memory.db "SELECT COUNT(*) FROM memory;"  # DB accessible
```

### P1: Subsystem Failure (Health OK, Ready NOT)

```bash
# Identify which subsystem failed
curl -s http://localhost:3000/gateway/ready | jq .

# Common causes:
# - Skills index missing: check config.skills.index_path exists
# - Docker unavailable: systemctl status docker
# - Memory DB locked: fuser ./data/zylos-memory.db
# - EventBus saturated: check ZYLOS_EVENT_BUS_CAPACITY vs load
```

### P1: Approval System Stuck

```bash
# Check for unresolved approvals older than TTL
sqlite3 ./data/zylos-memory.db \
  "SELECT id, equivalence_hash, created_at FROM approvals 
   WHERE resolved_at IS NULL AND created_at < datetime('now', '-1 hour');"

# Force-resolve stale approvals (last resort)
sqlite3 ./data/zylos-memory.db \
  "UPDATE approvals SET resolved_at = datetime('now'), decision = 'expired' 
   WHERE resolved_at IS NULL AND created_at < datetime('now', '-2 hours');"
```

### P2: Sentinel Not Detecting Known Errors

```bash
# 1. Verify observations are being fed
grep "sentinel.scan" /var/log/zylos-gateway.log | tail -5

# 2. Check detector thresholds in config
grep -A5 "\[sentinel\]" config.production.toml

# 3. Verify rate limiter isn't suppressing
grep "rate_limit.suppressed" /var/log/zylos-gateway.log | tail -10

# 4. Test detector directly
cargo test -p zylos-sentinel -- --nocapture
```

### P2: Triage Model Fallback Loop

```bash
# Check model call failures
grep "triage.model.error" /var/log/zylos-gateway.log | tail -20

# Verify API key is valid
echo $ZYLOS_TRIAGE_API_KEY | head -c 10

# Force rules-only mode temporarily
export ZYLOS_TRIAGE_MODEL_DISABLED=true
systemctl restart zylos-gateway
```

## Scaling Procedures

### Horizontal Scale (Multiple Instances)

```bash
# Each instance needs unique config
cp config.production.toml config.instance-2.toml
sed -i 's/listen_addr = "0.0.0.0:3000"/listen_addr = "0.0.0.0:3001"/' config.instance-2.toml

# Shared state requires external coordination:
# - SQLite: use WAL mode + shared filesystem or migrate to Postgres
# - EventBus: switch to Redis/NATS pub/sub
# - Approvals: shared database with CAS claims
```

### Vertical Scale (Single Instance)

```bash
# Increase EventBus capacity for high-throughput
sed -i 's/event_bus_capacity = 512/event_bus_capacity = 2048/' config.production.toml

# Increase max connections
sed -i 's/max_connections = 2048/max_connections = 4096/' config.production.toml

# Hot-reload without restart
kill -HUP $(pgrep zylos-gateway)
```

## Maintenance Windows

### Scheduled Maintenance Checklist

- [ ] Notify stakeholders 24h in advance
- [ ] Enable maintenance mode banner in cockpit
- [ ] Drain active requests: `curl -X POST http://localhost:3000/gateway/drain`
- [ ] Wait for `/gateway/ready` to return `draining`
- [ ] Perform maintenance
- [ ] Restart gateway
- [ ] Verify all health probes pass
- [ ] Disable maintenance mode
- [ ] Notify stakeholders of completion

### Database Maintenance

```bash
# Vacuum SQLite (reclaim space after deletions)
sqlite3 ./data/zylos-memory.db "VACUUM;"

# Integrity check
sqlite3 ./data/zylos-memory.db "PRAGMA integrity_check;"

# Backup before maintenance
./scripts/backup-daily.sh
```

## Debugging Commands

```bash
# Full system state dump
curl -s http://localhost:3000/gateway/debug/state | jq .

# Active approvals count
sqlite3 ./data/zylos-memory.db "SELECT COUNT(*) FROM approvals WHERE resolved_at IS NULL;"

# Memory consolidation stats
sqlite3 ./data/zylos-memory.db "SELECT ground_level, COUNT(*) FROM memories GROUP BY ground_level;"

# EventBus backlog
curl -s http://localhost:3000/api/v1/events/backlog

# Recent sentinel findings
sqlite3 ./data/zylos-memory.db "SELECT signature, count, last_seen FROM sentinel_findings ORDER BY last_seen DESC LIMIT 10;"

# SOAR playbook execution history
sqlite3 ./data/zylos-memory.db "SELECT playbook_id, status, started_at, completed_at FROM soar_executions ORDER BY started_at DESC LIMIT 10;"
```

## Contact Information

| Role | Contact | Channel |
|------|---------|---------|
| On-call Engineer | See PagerDuty rotation | PagerDuty + Slack |
| Tech Lead | @tech-lead | Slack #zylos-dev |
| Product Owner | @product-owner | Slack #zylos-product |
| Security | security@nousresearch.com | Email (encrypted) |