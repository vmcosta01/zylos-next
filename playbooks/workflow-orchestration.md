# Playbook: Governança de Orquestração de Workflows

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Workflow crítico falhado, deadlock sistêmico, ou decisão de arquitetura de DAG
**Público-alvo:** Tech Lead, Platform Engineer, Agent Operator
**SLA:** Workflow crítico restaurado em <1h; decisão de arquitetura em <24h

---

## 1. Visão Geral

Este playbook define como gerenciar falhas sistêmicas na orquestração de workflows e tomar decisões sobre arquitetura de DAGs. Para debug técnico, consulte o runbook associado.

**Runbooks associados:**
- [`runbooks/cerebelo-orchestration.md`](../runbooks/cerebelo-orchestration.md) — Debug e operação do Cerebelo
- [`runbooks/fase7b-task-durability.md`](../runbooks/fase7b-task-durability.md) — Durabilidade de tarefas

---

## 2. Nível Senior — Decisões de Arquitetura

### Matriz RACI
| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|-------------|-------------|-----------|----------|
| Aprovação de novo DAG | Tech Lead | Arquiteto | Platform Engineer | Agent Operators |
| Decisão de retry vs abort | Platform Engineer | Tech Lead | SRE Lead | — |
| Mudança de estratégia de checkpoint | Arquiteto | CTO | Tech Lead | Platform Engineers |
| Escalação de worker pool | SRE Lead | Platform Engineer | Tech Lead | Finance (custo) |
| Post-mortem de workflow failure | Tech Lead | Platform Engineer | On-call Engineer | Todos |

### Critérios de Decisão
| Situação | Decisão Default | Override Por |
|----------|----------------|-------------|
| Stage falhou 3x | Abort workflow + notificar owner | Tech Lead pode forçar retry manual |
| Deadlock detectado | Cancel todos os stages dependentes | Nenhum — deadlock é sempre erro de design |
| Worker saturation | Queue + backpressure | SRE Lead pode aprovar scale-up emergencial |
| Checkpoint corruption | Replay desde último checkpoint válido | Se nenhum checkpoint válido → re-executar do zero |

### Gates
| Gate | Condição | Ação |
|------|----------|------|
| G1: Novo DAG | Review aprovado por ≥2 seniors? | Deploy em staging → produção |
| G2: Falha sistêmica | >3 workflows stuck em 10min? | Declarar incidente → acionar SRE |
| G3: Scale-up | Custo adicional >$100/dia? | Aprovação Finance + Tech Lead |

---

## 3. Nível Pleno — Operação

### Fluxo de Resposta a Falha Sistêmica
```
>3 workflows stuck → Declarar incidente
  → Identificar causa comum (infra? bug? resource?)
  → Se infra: acionar SRE
  → Se bug: cancelar workflows afetados + abrir ticket
  → Se resource: scale-up emergencial (com aprovação)
  → Monitorar recuperação
  → Post-mortem em 24h
```

### Tabela de Escalação
| Situação | Quem | Canal | Timeout |
|----------|------|-------|---------|
| Deadlock sistêmico | Platform Engineer | Slack #platform | 15min |
| Worker exhaustion | SRE Lead | PagerDuty | 5min |
| Bug recorrente em DAG | Tech Lead | GitHub issue | 1h |
| Custo inesperado >$500 | Finance + CTO | Email | 4h |

---

## 4. Nível Júnior — Monitoramento

### Verificar Saúde dos Workflows
```bash
zylos cerebelo workflows list --status running --format table
zylos cerebelo metrics --last 1h --format table
```

### Quando Escalar
- Qualquer workflow SEV-1 parado >15min
- >3 workflows stuck simultaneamente
- Erro desconhecido nos logs

### Regras
- Nunca forçar retry sem entender a causa raiz
- Sempre documentar decisões de abort/retry
- Se dúvida → escalar para Platform Engineer

---

## 5. Referências Cruzadas

- **Runbook:** [`runbooks/cerebelo-orchestration.md`](../runbooks/cerebelo-orchestration.md)
- **SOAR Workflow:** [`docs/soar/runbooks/dag-execution.runbook.yaml`](../docs/soar/runbooks/dag-execution.runbook.yaml)
- **Playbook relacionado:** [`incident-response.md`](incident-response.md)
- **Próxima revisão:** 2027-02-05
