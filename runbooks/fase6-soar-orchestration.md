# Fase 6: SOAR Orchestration — Runbook Zylos-Next

## Visão Geral

O SOAR (Security Orchestration, Automation and Response) engine (`zylos-soar`) implementa event intake, auto-triage, playbook execution, audit trail, runbook registry, orchestrator multi-agente, webhook integration, e export de incidentes.

**Módulos principais** (de `crates/zylos-soar/src/lib.rs`):
- `event` — ZylosSecurityEvent com Severity, EventCategory
- `triage` — TriageEngine com TriageDecision, TriageAction
- `triage_message` — Mensagens de triage estruturadas
- `audit` — AuditEntry, AuditTrail para compliance
- `playbook` — Playbook com PlaybookState, PlaybookCursor, Transition, PlaybookRoles, PlaybookTrigger
- `runnable` — SoarRunnable trait, RunbookResult, IncidentContext, BoxFuture
- `registry` — RunbookRegistry para discovery de runbooks
- `executor` — PlaybookExecutor com ExecutorConfig, StateExecutionResult
- `router` — Event routing para playbooks apropriados
- `webhook` — Webhook integration para notificações externas
- `orchestrator` — Multi-agent orchestration
- `validation` — Playbook/runbook validation
- `export` — Incident export formats
- `error` — SoarError

**Critério de Saída:** Playbooks executam com audit trail completo; triage automático classifica eventos corretamente; runbooks YAML são carregáveis e executáveis.

## Pré-requisitos

- Fases 0-5 completas
- Dependências: `serde`, `serde_yaml`, `tokio`, `chrono`, `uuid`, `tracing`
- `docs/soar/runbooks/` com runbooks YAML definidos
- `docs/soar/runbook-schema.yaml` para validação
- EventBus disponível (de `zylos-live-comm`) para event intake

### Setup

```bash
cargo build -p zylos-soar
cargo test -p zylos-soar

# Validar runbooks contra schema
cargo run -p zylos-soar --bin validate-runbooks
```

## Para Dev Senior

### Decisões Arquiteturais

1. **Event-Driven Architecture**: `ZylosSecurityEvent` entra via EventBus, `Router` direciona para playbook apropriado baseado em `EventCategory` e `Severity`. Triage é automático mas overrideable.

2. **Playbook State Machine**: `Playbook` tem estados (`PlaybookState`) com transições válidas (`Transition`). `PlaybookCursor` trackea posição atual na execução. `PlaybookRoles` define quem pode executar cada step. `PlaybookTrigger` define condições de ativação.

3. **SoarRunnable Trait**: Interface comum para qualquer ação executável. `IncidentContext` carrega dados do incidente através dos steps. `RunbookResult` padroniza outcomes. `BoxFuture` permite async dynamic dispatch.

4. **Audit Trail Obrigatório**: Todo step de playbook gera `AuditEntry` com timestamp, actor, action, outcome. `AuditTrail` é append-only e tamper-evident. Compliance requirement.

5. **RunbookRegistry**: Descobre e carrega runbooks YAML de `docs/soar/runbooks/`. Validação contra schema em load time. Hot-reload suportado.

6. **PlaybookExecutor**: Executa playbooks step-by-step com `ExecutorConfig` (timeouts, retry policy, approval gates). `StateExecutionResult` indica sucesso/falha/pending approval.

7. **Multi-Agent Orchestration**: `Orchestrator` coordena múltiplos agentes para incidentes complexos. Delega tasks via P2P messaging (de `zylos-bot-system`).

### Extension Points

- **Novos event categories**: Adicione ao `EventCategory` enum, configure routing rules
- **Custom triage logic**: Implemente `TriageEngine` customizado para domínios específicos
- **Novos playbook triggers**: Estenda `PlaybookTrigger` para condições customizadas
- **Custom export formats**: Implemente export para SIEM/ticketing systems
- **Novos runnable types**: Implemente `SoarRunnable` para novas ações (ex: Slack notification, PagerDuty)

### Performance Tuning

- Triage decision <10ms por evento
- Playbook step execution overhead <5ms (excluindo ação real)
- Audit trail write <1ms (append-only, buffered)
- Runbook registry lookup <5ms (in-memory index)
- Batch events para reduzir overhead de routing

## Para Dev Pleno

### Creating a Security Event

```rust
use zylos_soar::{ZylosSecurityEvent, Severity, EventCategory};

let event = ZylosSecurityEvent {
    id: Uuid::new_v4().to_string(),
    category: EventCategory::UnauthorizedAccess,
    severity: Severity::High,
    source: "auth-service".into(),
    description: "Multiple failed login attempts from IP 192.168.1.100".into(),
    timestamp: chrono::Utc::now(),
    metadata: serde_json::json!({
        "ip": "192.168.1.100",
        "attempts": 5,
        "user": "admin"
    }),
};
```

### Running Triage

```rust
use zylos_soar::{TriageEngine, TriageDecision, TriageAction};

let engine = TriageEngine::default();
let decision = engine.triage(&event)?;

match decision.action {
    TriageAction::AutoResolve => println!("Auto-resolved: {}", decision.rationale),
    TriageAction::Escalate => println!("Escalated to: {:?}", decision.assignee),
    TriageAction::RunPlaybook(id) => println!("Running playbook: {}", id),
    TriageAction::Ignore => println!("Ignored: {}", decision.rationale),
}
```

### Executing a Playbook

```rust
use zylos_soar::{PlaybookExecutor, ExecutorConfig, RunbookRegistry};

let registry = RunbookRegistry::load("docs/soar/runbooks/")?;
let playbook = registry.get("incident-response-auth")?;

let config = ExecutorConfig {
    step_timeout: Duration::from_secs(300),
    max_retries: 3,
    require_approval_for_destructive: true,
};

let executor = PlaybookExecutor::new(config);
let result = executor.execute(playbook, incident_context).await?;

match result {
    StateExecutionResult::Completed => println!("Playbook completed successfully"),
    StateExecutionResult::Failed(err) => eprintln!("Playbook failed: {}", err),
    StateExecutionResult::PendingApproval => println!("Waiting for approval"),
}
```

### Audit Trail

```rust
use zylos_soar::{AuditTrail, AuditEntry};

let trail = AuditTrail::new("audit.log");

trail.append(AuditEntry {
    timestamp: chrono::Utc::now(),
    actor: "soar-engine".into(),
    action: "playbook_step_executed".into(),
    outcome: "success".into(),
    details: serde_json::json!({"step": "block_ip", "target": "192.168.1.100"}),
})?;

// Query audit trail
let entries = trail.query(QueryFilter {
    actor: Some("soar-engine".into()),
    since: Some(chrono::Utc::now() - chrono::Duration::hours(24)),
    ..Default::default()
})?;
```

## Para Dev Junior

### Passo a Passo: Criando um Runbook

1. **Defina o runbook YAML** em `docs/soar/runbooks/my-runbook.yaml`:
   ```yaml
   name: my-runbook
   trigger:
     category: unauthorized_access
     severity_min: medium
   steps:
     - id: gather_context
       action: collect_logs
       params:
         source: auth-service
     - id: block_ip
       action: firewall_block
       requires_approval: true
       params:
         ip: "{{event.metadata.ip}}"
   ```
2. **Valide**: `cargo run -p zylos-soar --bin validate-runbooks`
3. **Teste**: Crie evento de teste e dispare triage
4. **Verifique audit trail**: Confirme que todos os steps foram logados

### Pitfalls Comuns

- **Nunca execute playbook sem audit trail**: Compliance requirement absoluto
- **Não ignore approval gates**: Steps destrutivos requerem aprovação explícita
- **Não hardcode valores em runbooks**: Use template variables (`{{event.metadata.ip}}`)
- **Não pule validação de schema**: Runbooks inválidos falham em runtime
- **Não assuma ordem de execução**: Steps paralelos podem completar fora de ordem

### Debugging Tips

- Audit trail é append-only — nunca edite diretamente
- `PlaybookCursor` mostra qual step está executando
- `TriageDecision.rationale` explica por que uma decisão foi tomada
- Use `RUST_LOG=zylos_soar=debug` para ver routing e execution traces
- Webhook failures logam com retry count e last error

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `SoarError::PlaybookNotFound` | Runbook ID não existe no registry | Verifique ID, liste runbooks disponíveis |
| `SoarError::ValidationFailed` | Runbook YAML não conforme ao schema | Valide contra `runbook-schema.yaml` |
| `SoarError::StepTimeout` | Step excedeu timeout configurado | Aumente timeout ou otimize ação |
| `SoarError::ApprovalRequired` | Step destrutivo sem aprovação | Obtenha aprovação via approval workflow |
| `SoarError::AuditWriteFailed` | Disco cheio ou permissão negada | Verifique espaço, permissões do arquivo |
| `SoarError::WebhookFailed` | Endpoint externo inacessível | Verifique URL, network, retry policy |
| Triage incorreto | Rules de triage desatualizadas | Atualize triage config, adicione feedback loop |

## Referências

- **Source**: `crates/zylos-soar/src/` (14 módulos)
- **Key files**: `lib.rs:1-27`, `event.rs`, `triage.rs`, `playbook.rs`, `executor.rs`, `runnable.rs`, `audit.rs`, `registry.rs`, `orchestrator.rs`
- **Runbooks**: `docs/soar/runbooks/` (7 runbooks YAML)
- **Schema**: `docs/soar/runbook-schema.yaml`
- **Architecture**: `docs/soar/SOAR_ARCHITECTURE.md`
- **Master Plan**: Linhas relacionadas a SOAR orchestration