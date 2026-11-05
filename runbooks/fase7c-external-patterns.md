# Runbook: Fase 7C — Padrões Externos (HITL, Triage, Sentinel, Approval)

## Visão Geral
Este runbook cobre a operação e manutenção dos módulos implementados na Fase 7C do Zylos-Next:
- **zylos-task-durability/outcome** — Outcome Uncertainty Tri-State (Succeeded/Failed/Unknown)
- **zylos-task-durability/approval_hash** — Equivalence-Based Approval Hashing (blake3, PolicyTierEvaluator, ExecutionAuthority)
- **zylos-soar/triage_message** — Two-Stage Message Triage Engine (classify_message + route_message)
- **zylos-sentinel** — Sentinel Health Scanner (AIOps, pure-function detectors)
- **zylos-live-comm/approval** — HITL Approval Cards com storage server-side
- **zylos-live-comm/task_durability** — Task Durability via SQL Leases (legacy, being migrated to zylos-task-durability)

**Total de testes:** 74 novos testes em Fase 7C implementable tasks (18 approval_hash + 20 triage_message + 15 outcome + 21 sentinel), todos passando.

---

## Nível Junior: Operações Básicas

### Verificar saúde dos módulos
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-live-comm 2>&1 | grep "^test result:"
```
Esperado: `test result: ok. 157 passed; 0 failed`

### Verificar um módulo específico
```bash
cargo test -p zylos-sentinel              # Health scanner (standalone)
cargo test -p zylos-soar triage_message   # Two-stage triage engine
cargo test -p zylos-task-durability approval_hash  # Equivalence-based approval hashing
cargo test -p zylos-task-durability outcome        # Uncertainty tri-state
cargo test -p zylos-live-comm approval             # HITL cards (legacy)
cargo test -p zylos-live-comm task_durability      # SQL leases (legacy)
```

### Compilar workspace inteiro
```bash
cargo check --workspace
```
Exit code 0 = tudo compila. Se falhar, verificar mensagem de erro específica.

### Erros comuns e correções
| Erro | Causa | Correção |
|------|-------|----------|
| `cannot find type ProofStatus` | Import faltando em guardrail.rs | Adicionar `use crate::proof::ProofStatus;` no módulo de testes |
| `js_sys::Math::random` em teste nativo | backoff_ms usava JS runtime | Já corrigido: usa hash determinístico LCG |
| `Hash not implemented for Severity` | Enum sem derive Hash | Adicionar `#[derive(Hash)]` ao enum |
| PII stripping não funciona | Parser manual com bugs de boundary | Reescrito com char-based scanning correto |

---

## Nível Pleno: Configuração e Integração

### Approval Cards (HITL)
- **Storage:** In-memory HashMap com CAS claim e TTL expiry
- **ArgsHash:** SHA-256 dos argumentos, nunca enviado ao cliente
- **Secret masking:** Campos sensíveis mascarados antes de exibir ao aprovador
- **Idempotent recovery:** Requests duplicados retornam aprovação existente

### Task Durability (SQL Leases)
- **State machine:** Queued → Running → WaitingApproval/WaitingInput → Succeeded/Failed/Cancelled
- **CAS claiming:** `UPDATE ... WHERE state IN ('queued','waiting_approval','waiting_input') AND lease_owner IS NULL`
- **Maintenance loop:** Recupera tarefas com lease expirado
- **Circuit breaker:** Trip após N falhas consecutivas, reset manual ou automático
- **Backoff exponencial:** Caps em max_retry_ms configurável

### Two-Stage Triage
- **Stage 1 (Rules):** Apenas safe negatives (CanWait, AutoHandle). NEVER produz NeedsYou.
- **Stage 2 (Model):** Fallback quando nenhuma rule match. Retorna Err se falhar — nunca default silencioso.
- **decided_by audit:** `"rule:<id>"` ou `"model:<name>"` em toda decisão
- **Framing patterns:** Wrappers para scheduled routines e handoffs previnem misinterpretação
- **Initiator tracking:** Person, Routine, Responsibility, Memory, Handoff, System

### Equivalence-Based Approval Hashing (`zylos-task-durability::approval_hash`)
- **Hash algorithm:** blake3 (not SHA-256) — faster, collision-resistant, 256-bit output
- **ApprovalDigest:** Opaque wrapper around blake3 hex string, implements Eq/Hash for dedup
- **PolicyTier enum:** Safety > TeamRule > PersonalRule > AutoReview > Default (strict ordering)
- **PolicyTierEvaluator:** Pure-function evaluator that takes `(actor_id, tool_ref, effect, scope)` and returns `TierDecision` with the winning tier + reason
- **ExecutionAuthority:** Task-local context carrying `(approval_id, digest, owner)` — set via `with_authority(auth, || async { ... })`, read synchronously via `current_authority()`
- **Validation probe:** `with_validation_probe(|| async { ... })` sets `VALIDATION_PROBE_ACTIVE` task-local; check via `is_approval_recheck()` — side-effect-free re-check path
- **Stable hashing:** Volatile fields stripped before hashing: snapshot_id, ref, tool_call_id, timestamp, nonce, request_id, trace_id
- **Safety immutability:** Safety tier always wins, checked BEFORE resolved guard
- **Lower-tier denials overridable:** `resolved_at` only set at TeamRule+, allowing higher-tier override
- **Escalation ungated:** Always available as safe exit regardless of grants

### Sentinel Health Scanner
- **Pure-function detectors:** http_5xx, timeout, repeated_failures — zero side effects
- **Normalized signatures:** SHA-256 truncado (128-bit) de `tool|stripped_error`
- **PII stripping:** Emails, UUIDs, IPv4, hex tokens ≥16 chars → redacted placeholders
- **Issue store:** Upsert por signature, JSONL append-only output
- **Rate limiter:** Token bucket, default 5 reports/hour
- **Privacy-first:** Todo output passa por strip_pii() antes de persistência

### Integração entre módulos
```
Inbound item → Triage (Stage1 rules → Stage2 model)
  ├─ CanWait/AutoHandle → execute directly
  └─ NeedsYou/Escalate → Approval Cards (equivalence hash)
       ├─ Approved → Task Durability (SQL lease claim)
       │    └─ Execute → Outcome (tri-state classify)
       │         ├─ Succeeded → done
       │         ├─ Failed → retry policy / escalation
       │         └─ Unknown → human confirmation required
       └─ Declined → Refusal message (distinct per reason)

Sentinel (parallel): scans observations → pure detectors → issue store → rate-limited incident report
```

---

## Nível Senior: Debugging Avançado e Extensão

### Adicionar nova regra de triage
```rust
use zylos_live_comm::triage::{TriageRule, TriageVerdict};

let custom_rule = TriageRule {
    id: "vip-sender".into(),
    description: "VIP senders always need attention".into(),
    matcher: Box::new(|input| input.sender.ends_with("@ceo.company.com")),
    verdict: TriageVerdict::NeedsYou, // ⚠️ NÃO! Rules não podem produzir NeedsYou
    priority: 1,
};
// CORRETO: Use Stage 2 model para NeedsYou, ou crie uma rule que faz AutoHandle("escalate:vip")
```

### Adicionar novo detector ao Sentinel
```rust
use zylos_live_comm::sentinel::{Observation, DetectorFinding, Severity};

pub fn detect_auth_failures(observations: &[Observation]) -> Vec<DetectorFinding> {
    observations.iter()
        .filter(|o| o.error_message.as_ref().map_or(false, |m| m.contains("auth") && m.contains("fail")))
        .map(|o| DetectorFinding {
            detector: "auth_failure".into(),
            severity: Severity::Warning,
            summary: format!("Auth failure on {}", o.tool),
            signature_input: format!("{}|auth_failure", o.tool),
        })
        .collect()
}
// Registrar em SentinelScanner::scan()
```

### Debug: Safety tier override failing
Se um safety block não pode ser overriden mas deveria:
1. Verificar `final_tier == DecisionTier::Safety` no record
2. Safety é INTENCIONALMENTE imutável — se precisa mudar, remova a safety rule, não tente override
3. Error message deve conter "safety" — se contém "already resolved", o resolved guard está errado

### Debug: Lower-tier denial blocking higher-tier override
Causa: `resolved_at` foi setado prematuramente.
Fix: Apenas TeamRule+ denials setam `resolved_at`. Default/AutoReview denials deixam `resolved_at=None`.
Verificar: `is_resolved()` retorna `self.resolved_at.is_some()`, NÃO `matches!(state, Declined(_))`.

### Debug: PII not stripped in output
1. Verificar ordem em `strip_pii()`: UUIDs → Emails → IPv4 → Hex tokens
2. UUIDs devem ser processados ANTES de hex tokens (UUIDs contêm hex chars)
3. Testar com `strip_pii("user@test.com 192.168.1.1 abcdef0123456789")` — todos devem ser redacted

### Métricas de monitoramento
- **Triage accuracy:** `TriageEval::run()` contra corpus rotulado
- **Approval reuse rate:** `counts_by_state()["approved"] / total`
- **Sentinel dedup ratio:** unique signatures / total findings
- **Incident storm detection:** `rate_limiter.retry_after_seconds()` > 0 indica throttling ativo

---

## Referências
- Master plan: `C:\Users\Paulo\AppData\Local\hermes\cache\scratch\zylos-next-master-plan.md` linhas 1762-1830
- Approval Hashing + Outcome: `crates/zylos-task-durability/src/{approval_hash,outcome}.rs`
- Two-Stage Triage: `crates/zylos-soar/src/triage_message.rs`
- Sentinel Health Scanner: `crates/zylos-sentinel/src/lib.rs`
- Legacy HITL + SQL Leases: `crates/zylos-live-comm/src/{approval,task_durability}.rs`
- Audits: `evals/audits/fase7c-audit-{adversarial,isolated}.md`
- TASKS.md: `C:/Users/Paulo/zylos-next/TASKS.md` (Fase 7C tasks marked complete)