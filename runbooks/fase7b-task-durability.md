# Fase 7B: Task Durability — Runbook Zylos-Next

## Visão Geral

O task durability system (`zylos-task-durability`) implementa SQL leases com heartbeat, LostLeaseError para CAS failures, circuit breaker para consecutive failures, maintenance loop para reclaim stale leases, approval hashing multi-tier, e outcome tracking com dedup de notificações.

**Módulos principais** (de `crates/zylos-task-durability/src/lib.rs`):
- Root: `TaskStatus` enum (Queued=0, Running=1, WaitingApproval=2, WaitingInput=3, Scheduled=4, Paused=5, Succeeded=6, Failed=7, Cancelled=8), `LostLeaseError`, `TaskDurabilityError`, `Lease`, `Attempts`, `TaskRecord`
- `approval_hash` — ApprovalContext, ApprovalDigest, ExecutionAuthority, PolicyTier, PolicyTierEvaluator, RefusalReason, StepCall, TierDecision
- `outcome` — Outcome, OperationError, OutcomeTracker, NotificationDedupKey, OutcomeAuditEntry

**Design decisions críticas** (do doc comment em `lib.rs:7-18`):
- **Database-as-clock**: All lease timing uses SQL `now() + interval`, NEVER `Utc::now() + Duration` in Rust. Prevents clock-skew bugs.
- **SELECT FOR UPDATE SKIP LOCKED**: Each replica takes rows nobody else holds, never waits behind another — zero contention.
- **LostLeaseError**: Thrown when CAS fails or abort signal fires — reverts task to `queued` atomically.
- **pg_notify inside transaction**: Fires on commit never before; listener on own connection.

**Critério de Saída:** Leases expire corretamente sob clock skew; circuit breaker abre após N failures consecutivas; maintenance loop reclama stale leases sem race conditions.

## Pré-requisitos

- Fases 0-7A completas
- Dependências: `rusqlite`, `tokio`, `chrono`, `uuid`, `serde`, `thiserror`, `tracing`
- SQLite database com schema de tasks/leases criado
- Event loop Tokio rodando para heartbeat/maintenance

### Setup

```bash
cargo build -p zylos-task-durability
cargo test -p zylos-task-durability
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Database-as-Clock Pattern**: Nunca use `Utc::now() + Duration` para calcular lease expiry. Sempre use SQL `now() + interval`. Isso elimina bugs de clock skew onde um node 90s atrasado escreve uma lease de 60s que o Postgres considera expirada na chegada. Todas as funções de lease seguem este princípio.

2. **SELECT FOR UPDATE SKIP LOCKED**: Cada replica pega linhas que ninguém mais segura, nunca espera atrás de outra. Zero contention em deployments multi-replica. A query padrão é: `SELECT * FROM tasks WHERE status = 'queued' ORDER BY created_at LIMIT 1 FOR UPDATE SKIP LOCKED`.

3. **LostLeaseError como Sinal de Reversão**: Quando CAS falha ou abort signal dispara, `LostLeaseError` é thrown. O handler DEVE reverter a task para `Queued` atomicamente. Todos os writes via `guard()/checkpoint()/event()` throw este erro on stale lease.

4. **TaskStatus State Machine**: Transições validadas em Rust via `can_transition_to()`. Transições inválidas retornam `InvalidTransition`. Retry é permitido apenas de `Failed → Queued`.

5. **Circuit Breaker**: `CircuitBreakerOpen` error após N consecutive failures. Requer reset manual para prevenir cascading failures. Contador atômico (`AtomicU32`) para lock-free reads.

6. **Approval Hashing Multi-Tier**: `PolicyTierEvaluator` avalia cada step call contra policy tiers. `ApprovalDigest` é hash determinístico do contexto de aprovação. `ExecutionAuthority` define quem pode aprovar. `RefusalReason` documenta por que uma aprovação foi negada.

7. **Outcome Tracking com Dedup**: `OutcomeTracker` registra outcomes com `NotificationDedupKey` para prevenir notificações duplicadas. `OutcomeAuditEntry` para compliance.

### Extension Points

- **Novos task statuses**: Adicione ao enum (cuidado: `#[repr(u8)]` muda serialization)
- **Custom policy tiers**: Estenda `PolicyTier` para domínios específicos
- **Novos outcome types**: Adicione ao `Outcome` enum
- **Custom maintenance strategies**: Implemente estratégias alternativas de reclaim
- **External notification backends**: Integre `OutcomeTracker` com Slack/email/PagerDuty

### Performance Tuning

- Lease acquisition <5ms (single SELECT FOR UPDATE SKIP LOCKED)
- Heartbeat update <2ms (UPDATE com WHERE lease_id match)
- Maintenance loop scan <50ms para 10k tasks
- Circuit breaker check <1μs (atomic read)
- Use connection pooling (r2d2 ou deadpool) para evitar overhead de conexão
- Batch maintenance operations para reduzir round-trips

## Para Dev Pleno

### Acquiring a Lease

```rust
use zylos_task_durability::{TaskRecord, Lease, TaskStatus};
use uuid::Uuid;

// Database-as-clock: SQL computes expiry, not Rust
let lease: Lease = conn.query_row(
    "INSERT INTO leases (task_id, owner_id, lease_until)
     SELECT id, ?1, datetime('now', '+60 seconds')
     FROM tasks
     WHERE status = 'queued'
     ORDER BY created_at
     LIMIT 1
     RETURNING *",
    params![owner_id],
    |row| Ok(Lease {
        lease_id: row.get("lease_id")?,
        task_id: row.get("task_id")?,
        lease_until: row.get("lease_until")?,
        owner_id: row.get("owner_id")?,
    }),
)?;

// Update task status atomically
conn.execute(
    "UPDATE tasks SET status = 'running', lease_id = ?1, updated_at = datetime('now')
     WHERE id = ?2 AND status = 'queued'",
    params![lease.lease_id, lease.task_id],
)?;
```

### Heartbeat Renewal

```rust
use zylos_task_durability::LostLeaseError;

async fn heartbeat(conn: &Connection, lease: &Lease, token: CancellationToken) -> Result<()> {
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(30)) => {
                // Database-as-clock renewal
                let rows = conn.execute(
                    "UPDATE leases SET lease_until = datetime('now', '+60 seconds')
                     WHERE lease_id = ?1 AND task_id = ?2",
                    params![lease.lease_id, lease.task_id],
                )?;
                
                if rows == 0 {
                    return Err(LostLeaseError {
                        task_id: lease.task_id.clone(),
                        reason: "heartbeat CAS failed".into(),
                    }.into());
                }
            }
        }
    }
    Ok(())
}
```

### Handling LostLeaseError

```rust
use zylos_task_durability::{LostLeaseError, TaskDurabilityError, TaskStatus};

async fn execute_task(task_id: &str) -> Result<()> {
    match do_work(task_id).await {
        Ok(_) => complete_task(task_id).await,
        Err(TaskDurabilityError::LostLease(e)) => {
            // MUST revert to queued atomically
            conn.execute(
                "UPDATE tasks SET status = 'queued', lease_id = NULL, updated_at = datetime('now')
                 WHERE id = ?1",
                params![e.task_id],
            )?;
            tracing::warn!("Lost lease on {}: {}", e.task_id, e.reason);
            Ok(()) // Not a failure — another worker will pick it up
        }
        Err(e) => Err(e),
    }
}
```

### State Machine Validation

```rust
use zylos_task_durability::TaskStatus;

let current = TaskStatus::Running;
let target = TaskStatus::Succeeded;

if current.can_transition_to(target) {
    // Proceed with transition
} else {
    return Err(TaskDurabilityError::InvalidTransition {
        task_id: "task_001".into(),
        from: current,
        to: target,
    });
}
```

### Approval Hashing

```rust
use zylos_task_durability::approval_hash::{PolicyTierEvaluator, ApprovalContext, StepCall};

let evaluator = PolicyTierEvaluator::new(policy_config);
let context = ApprovalContext {
    task_id: "task_001".into(),
    step: StepCall { name: "delete_records".into(), params: serde_json::json!({}) },
    actor: "agent_001".into(),
};

let decision = evaluator.evaluate(&context);
match decision {
    TierDecision::Approved(digest) => println!("Approved: {:?}", digest),
    TierDecision::Refused(reason) => println!("Refused: {:?}", reason),
    TierDecision::Escalated(tier) => println!("Escalated to: {:?}", tier),
}
```

## Para Dev Junior

### Passo a Passo: Implementando um Worker

1. **Configure DB connection**: Use connection pool, enable WAL mode
2. **Implemente lease acquisition loop**: SELECT FOR UPDATE SKIP LOCKED
3. **Spawn heartbeat task**: Renova lease a cada 30s via database-as-clock
4. **Execute work**: Cheque CancellationToken periodicamente
5. **Handle LostLeaseError**: Reverta para queued, não trate como falha
6. **Complete ou fail**: Atualize status com transição válida

### Pitfalls Comuns

- **NUNCA use `Utc::now()` para lease timing**: Sempre SQL `now()`. Clock skew causa leases fantasmas.
- **Não ignore LostLeaseError**: É sinal de que outro worker assumiu. Reverta atomicamente.
- **Não faça heartbeat fora da transação**: Se heartbeat falhar, a lease já pode ter sido tomada.
- **Não esqueça SKIP LOCKED**: Sem isso, workers ficam bloqueados esperando locks.
- **Não permita transições inválidas**: Use `can_transition_to()` sempre.
- **Não rode maintenance sem índice**: Adicione índice em `(status, lease_until)` para scan eficiente.

### Debugging Tips

- Log lease acquisitions e renewals com task_id e owner_id
- `LostLeaseError.reason` explica por que a lease foi perdida
- Circuit breaker state visível via métricas (consecutive_failures count)
- Maintenance loop logs reclaimed leases em nível `info`
- Use `RUST_LOG=zylos_task_durability=debug` para verbose

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `LostLeaseError` frequente | Heartbeat interval > lease duration | Reduza heartbeat interval ou aumente lease duration |
| `CircuitBreakerOpen` | Muitas falhas consecutivas | Investigue root cause, reset manual após fix |
| `InvalidTransition` | Tentativa de transição ilegal | Consulte `can_transition_to()`, ajuste lógica |
| `ActiveCapReached` | Muitas tasks running simultaneamente | Aumente cap ou escale horizontalmente |
| Stale leases acumulando | Maintenance loop não rodando | Verifique scheduling, rode maintenance manualmente |
| Clock skew warnings | Nodes com clocks dessincronizados | Configure NTP, use database-as-clock exclusivamente |
| Deadlock em lease acquisition | Falta SKIP LOCKED | Adicione SKIP LOCKED à query |
| Notification storm | Dedup key mal configurada | Revise NotificationDedupKey strategy |

## Referências

- **Source**: `crates/zylos-task-durability/src/` (3 módulos: lib.rs, approval_hash.rs, outcome.rs)
- **Key files**: `lib.rs:1-200` (TaskStatus, LostLeaseError, Lease, TaskRecord, design decisions), `approval_hash.rs`, `outcome.rs`
- **Master Plan**: Fase 7B task durability section
- **Inspirações**: OpenMuse `worker.ts:64-76`, OpenDuet `queue.ts:77-81`