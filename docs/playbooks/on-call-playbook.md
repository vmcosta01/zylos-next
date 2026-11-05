# Zylos-Next On-Call Playbook

## Rotation Schedule
- **Primary:** PagerDuty rotation (weekly)
- **Secondary:** Backup on-call (same week)
- **Escalation:** Tech Lead → Product Owner → CTO

## Shift Handoff Checklist
- [ ] Review open incidents in PagerDuty
- [ ] Check Sentinel findings dashboard
- [ ] Verify all health probes green
- [ ] Read last 24h of `/var/log/zylos-gateway.log` for warnings
- [ ] Confirm backup completed successfully
- [ ] Note any config changes made during previous shift

## Decision Trees

### Gateway Unresponsive
```
Is /gateway/live responding?
├── YES → Check /gateway/ready
│   ├── ready=true → Application issue, check logs
│   └── ready=false → Subsystem init failure, restart
└── NO → Process dead or port conflict
    ├── Process exists → Kill + restart
    └── No process → Start fresh, check startup logs
```

### Approval Storm (>100 approvals/hour)
```
Are approvals from single equivalence_hash?
├── YES → Likely retry loop, check caller
│   └── Fix caller or add rate limit exception
└── NO → Legitimate load spike
    ├── Within capacity → Monitor, no action
    └── Exceeding capacity → Scale horizontally or increase ZYLOS_EVENT_BUS_CAPACITY
```

### Sentinel Incident Flood
```
Same signature repeating?
├── YES → Rate limiter should suppress (5/hr)
│   ├── Suppressed → Normal, verify suppression working
│   └── Not suppressed → Bug in rate limiter, escalate P1
└── NO → Multiple distinct issues
    └── Triage by severity, handle P0/P1 first
```

### Memory Consolidation Failure
```
Error type?
├── SQLITE_BUSY → Another process holding lock
│   └── Wait or kill competing process
├── Corruption → Restore from daily backup
│   └── sqlite3 data/zylos-memory.db "PRAGMA integrity_check;"
└── Timeout → Large DB, increase consolidation window
    └── Adjust ZYLOS_CONSOLIDATION_TIMEOUT_SECS
```

## Communication Templates

### Incident Start (Slack #incidents)
```
🚨 [P{severity}] {brief description}
Started: {timestamp}
Impact: {what's broken, who's affected}
Status: Investigating
On-call: @{name}
Thread: {link}
```

### Status Update (every 30min for P0/P1)
```
📊 Update [{elapsed}]
Status: {investigating|mitigating|resolved}
Progress: {what was tried, what changed}
Next: {next planned action}
ETA: {estimated resolution or next update}
```

### Incident Resolved
```
✅ [RESOLVED] {brief description}
Duration: {start → end}
Root cause: {one sentence}
Fix: {what was done}
Follow-up: {post-mortem scheduled? ticket created?}
```

## Post-Incident Requirements
- **P0/P1:** Full post-mortem within 48h (`docs/postmortems/YYYY-MM-DD-{slug}.md`)
- **P2:** Brief writeup in incident ticket
- **P3:** No post-mortem required

### Post-Mortem Template
```markdown
# Post-Mortem: {title}
**Date:** YYYY-MM-DD
**Severity:** P{0-3}
**Duration:** {start} → {end}
**Author:** {name}

## Summary
{2-3 sentences}

## Timeline
| Time | Event |
|------|-------|
| HH:MM | First alert |
| ... | ... |

## Root Cause
{detailed technical explanation}

## What Went Well
- {bullet points}

## What Went Poorly
- {bullet points}

## Action Items
| Action | Owner | Due | Ticket |
|--------|-------|-----|--------|
| ... | ... | ... | ... |
```

## Emergency Contacts
| Service | Contact | Method |
|---------|---------|--------|
| PagerDuty | On-call rotation | Auto-page |
| Slack | #zylos-incidents | Real-time |
| Email | ops@nousresearch.com | Non-urgent |
| Security | security@nousresearch.com | Encrypted email only |