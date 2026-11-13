# SOAR Workflow: Guardrail Enforcement Automation (Fase 8)

**Versão:** 1.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `soar-orchestration`
**Módulos:** `zylos-personalities::guardrails`, `zylos-agent-shield::{capability_token,output_sanitizer}`

---

## 1. Objetivo

Automatizar a resposta a violações de guardrail detectadas pelo `GuardrailEnforcer`, integrando com capability tokens, output sanitization e notificações HITL para contenção automática sem intervenção humana imediata.

## 2. Gatilhos do Workflow

| Evento | Fonte | Severidade |
|--------|-------|------------|
| `GuardrailAction::Block` | GuardrailEnforcer decision log | High |
| `GuardrailAction::RequireApproval` sem resposta em 5min | PlanFirstGate timeout | Medium |
| Capability token usado após revogação | CapabilityTokenStore | Critical |
| Output sanitizer redactou >3 secrets em 1min | OutputSanitizer pipeline | High |
| Rate limit excedido 3x em 10min | EmployeeRateLimiter | Medium |

## 3. Workflow Steps

### Step 1: Detect & Classify

```yaml
trigger: guardrail_decision OR capability_violation OR sanitize_alert
action:
  - classify_severity:
      block_action: high
      approval_timeout: medium
      revoked_token: critical
      mass_redaction: high
      rate_limit_trip: medium
  - enrich_context:
      employee_id: from_event
      role: from_employee_registry
      routine: from_event
      resource: from_event
      timestamp: from_event
```

### Step 2: Contain (Automático)

```yaml
if severity == critical:
  - revoke_all_tokens(employee_id)
  - disable_employee(employee_id)
  - alert_oncall(severity=critical)
  
elif severity == high:
  - suspend_capability_tokens(employee_id, scope=matched_resource)
  - force_output_sanitizer(strategy=redact, rules=all)
  - notify_admin(channel=#security-alerts)
  
elif severity == medium:
  - throttle_rate_limit(employee_id, factor=0.5)
  - queue_for_review(priority=high)
  - log_to_evidence_ledger(event_type=guardrail_warning)
```

### Step 3: Investigate (Semi-Automático)

```yaml
actions:
  - pull_recent_decisions(employee_id, limit=20)
  - correlate_with_mental_models(employee_id)
  - check_skill_evolution_graph(affected_routine)
  - generate_incident_summary(template=guardrail_violation)
```

### Step 4: Remediate

```yaml
options:
  false_positive:
    - whitelist_routine(pattern, duration=24h)
    - adjust_guardrail_priority(rule_name, delta=-10)
    - acknowledge_false_positive(incident_id)
    
  true_positive:
    - update_guardrail_rule(action=block, add_roles=[...])
    - retrain_skill_contract(skill_id, new_assertions=[...])
    - schedule_employee_retraining(employee_id, module=security)
    
  escalation_needed:
    - create_jira_ticket(project=SEC, type=incident)
    - assign_to(on_call_security_engineer)
    - sla_timer(start, breach_after=4h)
```

### Step 5: Verify & Close

```yaml
verification:
  - replay_decision_with_updated_rules(expect=allow|block)
  - confirm_capability_tokens_restored(if false_positive)
  - validate_output_sanitizer_no_longer_triggering
  
closure:
  - update_evidence_ledger(status=resolved)
  - notify_stakeholders(resolution_summary)
  - archive_incident(retention=90d)
```

## 4. Integrações

| Sistema | Integração | Direção |
|---------|-----------|---------|
| GuardrailEnforcer | Decision log stream | Input |
| CapabilityTokenStore | Revoke/suspend API | Output |
| OutputSanitizer | Force strategy override | Output |
| EmployeeRateLimiter | Throttle API | Output |
| EvidenceLedger | Event logging | Bidirectional |
| MentalModelStore | Correlation queries | Input |
| SkillEvolutionGraph | Contract updates | Output |
| Slack/Notion | Alert notifications | Output |
| Jira/Linear | Incident tickets | Output |

## 5. Escalação Automática

| Condição | Ação |
|----------|------|
| 3+ violations same employee em 1h | Disable employee + page on-call |
| 5+ false positives same rule em 24h | Auto-lower rule priority by 20 |
| Critical violation fora de horário comercial | Page on-call immediately |
| Violation envolve financial-* routine | Always require human sign-off |

## 6. Métricas do Workflow

| Métrica | Target | Medição |
|---------|--------|---------|
| Mean Time to Contain (MTTC) | <30s | Timestamp diff trigger→contain |
| False Positive Rate | <5% | Monthly review |
| Manual Escalation Rate | <10% | Incidents requiring human / total |
| Rule Adjustment Frequency | <2/week | Guardrail PATCH count |
| Employee Retraining Triggered | Track only | Count per month |

## 7. Rollback do Workflow

Se o SOAR workflow causa ação indevida:

1. **Pause workflow:** `POST /api/v1/soar/workflows/guardrail-enforcement/pause`
2. **Revert containments:** Batch restore tokens + re-enable employees
3. **Audit log:** Review all actions taken during incident window
4. **Adjust thresholds:** Increase severity classification criteria
5. **Resume:** `POST /api/v1/soar/workflows/guardrail-enforcement/resume`

## 8. Cross-References

- **POP:** `pop-guardrail-rule-update.md`
- **Runbook:** `runbooks/zylos-personalities.md`, `runbooks/zylos-desktop-fase8.md`
- **Handoff:** `docs/handoffs/HANDOFF_FASE8.md` §2.5, §2.7
- **Código:** `crates/zylos-personalities/src/guardrails.rs`, `crates/zylos-agent-shield/src/{capability_token,output_sanitizer}.rs`
- **Playbook relacionado:** `skill-evolution-lifecycle.md`

## 9. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-11 | Implementation Agent | Criação inicial |