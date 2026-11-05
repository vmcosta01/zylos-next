# SOAR Workflow: Zylos-Next Incident Response & Automation

## Visão Geral
Este documento define os workflows SOAR (Security Orchestration, Automation and Response) para o Zylos-Next, integrando Sentinel health scanning, approval gating, e incident response automatizado.

---

## Workflow 1: Health Anomaly Detection → Triage → Approval → Remediation

### Trigger
Sentinel detector identifica finding com severity ≥ Warning.

### Steps
```yaml
workflow: health_anomaly_response
trigger: sentinel.finding.severity >= Warning
steps:
  - id: dedup
    action: sentinel.issue_store.upsert
    input: finding
    output: issue
    
  - id: rate_check
    action: sentinel.rate_limiter.try_report
    input: now()
    condition: issue.count == 1  # Only on first occurrence
    on_fail: skip_to: notify_suppressed
    
  - id: triage
    action: triage.engine.triage
    input:
      item: issue.to_triage_input()
      initiator: System("sentinel")
    output: verdict
    
  - id: auto_handle
    condition: verdict == CanWait || verdict == AutoHandle
    action: execute_remediation(verdict)
    
  - id: approval_gate
    condition: verdict == NeedsYou
    action: equivalence_approval.submit
    input:
      actor: "sentinel-bot"
      tool_ref: "remediation.execute"
      effect: "write"
      scope: issue.severity
    output: approval_hash
    
  - id: wait_approval
    condition: approval.state == Pending
    action: wait_for_resolution(timeout: 3600s)
    
  - id: execute_approved
    condition: approval.state == Approved
    action: remediation.execute(approval.continuation)
    
  - id: log_outcome
    action: outcome.classify(result)
    
  - id: notify_suppressed
    condition: rate_check.suppressed
    action: log.info("Incident suppressed by rate limiter")
```

### Safety Rules
- **Never auto-execute** severity=Critical without human approval
- **Rate limit** incident reports to 5/hour per signature
- **PII strip** all summaries before logging or notifying
- **Escalation ungated** — always available as fallback

---

## Workflow 2: Task Failure Recovery with Equivalence Approval

### Trigger
Task durability maintenance loop detects stale lease or failed task.

### Steps
```yaml
workflow: task_failure_recovery
trigger: task_durability.maintenance.detect_stale()
steps:
  - id: classify_outcome
    action: outcome.classify_write_result(task.last_result)
    output: outcome
    
  - id: retry_if_unknown
    condition: outcome == OutcomeUnknown && task.retry_count < max_retries
    action: task_durability.requeue(task.id)
    
  - id: escalate_if_failed
    condition: outcome == Failed || task.retry_count >= max_retries
    action: equivalence_approval.escalate
    input:
      from_actor: "maintenance-loop"
      to_target: "on-call-engineer"
      urgency: High
      related_approval_hash: task.original_approval_hash
      
  - id: notify_human
    condition: outcome == Failed
    action: notification.send(team_channel, task.failure_summary)
```

---

## Workflow 3: Triage Model Drift Detection

### Trigger
Scheduled eval run (daily) detects accuracy drop > 10%.

### Steps
```yaml
workflow: triage_model_drift
trigger: schedule(daily) AND triage.eval.accuracy < threshold
steps:
  - id: run_eval
    action: triage.eval.run(corpus: labeled_corpus)
    output: result
    
  - id: check_drift
    condition: result.accuracy < baseline * 0.9
    action: alert.create(severity: Warning)
    
  - id: snapshot_model
    action: model.snapshot(current_version)
    
  - id: rollback_if_critical
    condition: result.accuracy < baseline * 0.7
    action: model.rollback(last_known_good)
    
  - id: notify_team
    action: notification.send(ml_team, drift_report)
```

---

## Integration Points

### Sentinel → Triage
```rust
// Sentinel findings feed into triage as System-initiated items
let input = TriageInput {
    id: issue.signature.clone(),
    subject: format!("Health: {}", issue.summary),
    body: serde_json::to_string(&issue).unwrap(),
    sender: "sentinel@zylos".into(),
    initiator: Initiator::System("health-scanner".into()),
    ..Default::default()
};
let verdict = triage_engine.triage(&input)?;
```

### Triage → Approval
```rust
// NeedsYou verdicts create equivalence-hashed approval requests
if matches!(verdict.verdict, TriageVerdict::NeedsYou) {
    let hash = compute_equivalence_hash(
        "sentinel", "zylos-bot", "remediate", "write", 
        &issue.severity.to_string(), &target, &args
    );
    approval_service.submit_approval(/* ... */);
}
```

### Approval → Task Durability
```rust
// Approved actions claim durable tasks via SQL leases
if approval.state == ApprovalState::Approved {
    let task = durable_store.enqueue(action.to_task());
    durable_store.try_claim(&task.id)?;
}
```

---

## Rate Limiting Configuration

| Resource | Limit | Window | Action on Exceed |
|----------|-------|--------|------------------|
| Incident reports | 5 | 1 hour | Suppress + log |
| Approval requests | 100 | 1 hour | Queue with backoff |
| Triage model calls | 1000 | 1 hour | Fallback to rules only |
| Escalations | Unlimited | N/A | Never rate-limit |

---

## Referências
- Sentinel: `crates/zylos-live-comm/src/sentinel.rs`
- Triage: `crates/zylos-live-comm/src/triage.rs`
- Approval: `crates/zylos-live-comm/src/equivalence_approval.rs`
- Task Durability: `crates/zylos-live-comm/src/task_durability.rs`
- Outcome: `crates/zylos-live-comm/src/outcome.rs`