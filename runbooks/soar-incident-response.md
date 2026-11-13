# Runbook SOAR — Resposta Automatizada a Incidentes de Segurança

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`incident-response-soar.md`](../playbooks/incident-response-soar.md), [`soar-workflows.md`](../playbooks/soar-workflows.md)
**Crate:** `zylos-soar`, `zylos-live-comm`
**Público:** SRE, Security Engineer, Agentes SOAR Orchestrator
**Tempo Estimado:** 5-15 min (dependendo da complexidade do incidente)

---

## 1. Visão Geral

Este runbook documenta a operação do módulo SOAR (Security Orchestration, Automation and Response) do Zylos-Next. O SOAR orquestra playbooks de segurança automaticamente quando eventos são roteados pelo `EventRouter`, executando estados sequenciais com gates de aprovação HITL quando necessário.

### Arquitetura SOAR
```
ZylosSecurityEvent → EventRouter → Playbook Dispatch → State Machine Execution
                                          ↓
                                   HITL Approval (se necessário)
                                          ↓
                                   Action Execution + Audit Trail
                                          ↓
                                   AG-UI Streaming (Cockpit)
```

## 2. NÍVEL SENIOR

### 2.1 Playbooks Disponíveis

| Playbook | Trigger | Estados | Auto-Approve |
|----------|---------|---------|-------------|
| `security-incident-response` | `severity >= high` | triage → containment → eradication → recovery → post_incident → closed | Não (containment requer HITL) |
| `false-positive-suppression` | `triage.false_positive == true` | suppress → notify → closed | Sim |
| `vendor-dependency-pivot` | `supply_chain_alert` | assess → isolate → remediate → verify → closed | Não |

### 2.2 Configuração do EventRouter

O roteamento é definido em `crates/zylos-soar/src/router.rs`. Regras são avaliadas em ordem; primeira match vence.

```rust
// Exemplo de regra no router
PlaybookRule {
    playbook_name: "security-incident-response".into(),
    condition: RuleCondition::FieldGte("severity".into(), "high".into()),
    priority: 100,
}
```

### 2.3 Gates de Aprovação HITL

Estados com `approval_required: true` pausam execução e criam `ApprovalRequest` via `zylos-hitl`. O cockpit exibe card de aprovação com contexto completo.

- **Carry On**: aprova e continua para próximo estado
- **Withdraw**: cancela execução atual, retorna ao estado anterior
- **Abandon**: termina playbook sem conclusão

### 2.4 Audit Trail

Todas as transições de estado são registradas no Evidence Ledger:
```bash
zylos-cli evidence query --type soar_transition --incident {event_id}
```

### 2.5 Métricas de Performance

| Métrica | Target | Atual |
|---------|--------|-------|
| Tempo de roteamento | <10ms | ~3ms |
| Execução de estado (sem HITL) | <100ms | ~45ms |
| Throughput de eventos | >1k/s | ~2.5k/s |

## 3. NÍVEL PLENO

### 3.1 Verificar Status de Incidentes Ativos

```bash
# Listar incidentes SOAR ativos
curl -s http://localhost:9876/api/v1/soar/incidents | jq .

# Verificar estado específico
curl -s http://localhost:9876/api/v1/soar/incidents/{event_id}/state | jq .

# Listar approvals pendentes
curl -s http://localhost:9876/api/v1/hitl/pending | jq .
```

### 3.2 Disparar Playbook Manualmente

```bash
# Via API
curl -X POST http://localhost:9876/api/v1/soar/dispatch \
  -H "Content-Type: application/json" \
  -d '{
    "event_id": "manual-test-001",
    "playbook": "security-incident-response",
    "severity": "high",
    "source": "manual",
    "title": "Teste manual de playbook",
    "description": "Verificação de fluxo SOAR"
  }'
```

### 3.3 Aprovar/Rejeitar via CLI

```bash
# Listar approvals pendentes
zylos-cli hitl list --status pending

# Aprovar
zylos-cli hitl approve --request-id {id} --reason "Validado por analista"

# Rejeitar
zylos-cli hitl reject --request-id {id} --reason "Falso positivo confirmado"
```

### 3.4 Debug de Falha em Estado

```bash
# Ver logs do estado específico
journalctl -u zylos-gateway --since "1 hour ago" | grep "soar.*{event_id}"

# Verificar outcome do estado
curl -s http://localhost:9876/api/v1/soar/incidents/{event_id}/history | jq '.[] | select(.state == "containment")'

# Se OutcomeUncertainty::Failed → verificar stderr no audit trail
zylos-cli evidence get --hash {artifact_hash} | jq .stderr
```

### 3.5 Checklist de Validação Pós-Incidente

- [ ] Todos os estados executaram com `OutcomeUncertainty::Succeeded`
- [ ] Audit trail completo no Evidence Ledger
- [ ] Notificações enviadas via AG-UI streaming
- [ ] Metrics exportadas para Prometheus/Grafana
- [ ] Post-mortem criado (SEV-1/2 obrigatório)

## 4. NÍVEL JÚNIOR

### 4.1 Glossário

| Termo | Definição |
|-------|-----------|
| **Playbook** | Sequência definida de estados para responder a um tipo de incidente |
| **Estado** | Unidade atômica de trabalho dentro de um playbook (ex: "triage", "containment") |
| **HITL** | Human-in-the-Loop — aprovação humana necessária antes de continuar |
| **EventRouter** | Componente que avalia eventos e dispara playbooks correspondentes |
| **OutcomeUncertainty** | Tri-state: Succeeded / Failed / Uncertain(String) |
| **Evidence Ledger** | Registro imutável de todas as ações e artefatos do SOAR |
| **AG-UI** | Protocolo de streaming para atualizações em tempo real no Cockpit |

### 4.2 Quando Escalar

| Situação | Ação |
|----------|------|
| Playbook travou em estado HITL por >30 min | Notificar SRE on-call |
| `OutcomeUncertainty::Failed` em 3+ estados consecutivos | Escalar para SEV-2 |
| EventRouter não está roteando eventos | Verificar health do gateway, escalar SEV-1 se down |
| Approval request sem resposta em >1h | Notificar owner do playbook via Slack |
| Erro de serialização/deserialização de evento | Criar issue GitHub com label `bug/soar` |

### 4.3 Comandos Rápidos

```bash
# Health check do SOAR
curl -s http://localhost:9876/api/v1/health | jq .soar

# Contar incidentes ativos
curl -s http://localhost:9876/api/v1/soar/incidents/count | jq .

# Ver último evento processado
curl -s http://localhost:9876/api/v1/soar/events/latest | jq .

# Restart do SOAR orchestrator (sem afetar gateway)
zylos-cli soar restart
```

## 5. Referências Cruzadas

- Playbook estratégico: [`incident-response-soar.md`](../playbooks/incident-response-soar.md)
- Workflows SOAR: [`soar-workflows.md`](../playbooks/soar-workflows.md)
- HITL approval cards: [`sop-incident-triage.md`](../playbooks/sop-incident-triage.md)
- Evidence Ledger: [`evidence-ledger.md`](./evidence-ledger.md)
- Cerebelo orchestration: [`cerebelo-orchestration.md`](./cerebelo-orchestration.md)
- Crate source: `crates/zylos-soar/src/`
- Testes: `crates/zylos-soar/src/lib.rs` (68 testes)