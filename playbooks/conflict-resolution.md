# Playbook: Estratégia de Resolução de Conflitos Multi-Agente

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Conflitos recorrentes entre agentes ou decisão crítica com outputs divergentes
**Público-alvo:** Tech Lead, Agent Operator, Compliance Officer
**SLA:** Conflito SEV-1 resolvido em <30min; conflito rotineiro em <4h

---

## 1. Visão Geral

Este playbook define **QUANDO** usar cada estratégia de resolução e **QUEM** tem autoridade para decidir. Para execução técnica, consulte o runbook associado.

**Runbooks associados:**
- [`runbooks/resolver-conflicts.md`](../runbooks/resolver-conflicts.md) — Operação do Resolver

---

## 2. Nível Senior — Estratégia de Resolução

### Matriz de Seleção de Estratégia
| Cenário | Estratégia Recomendada | Autoridade de Override |
|---------|----------------------|----------------------|
| Output não-crítico, ≥5 agentes | Majority Vote | Tech Lead |
| Agentes com confiança diferenciada | Seniority Weight | Arquiteto |
| Output crítico (financeiro/legal) | Adversarial Panel | Compliance Officer |
| Ambiguidade irresolvível | Human-in-the-Loop | Tech Lead |
| Dados PII ou regulados | HITL obrigatório | Compliance Officer |

### Matriz RACI
| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|-------------|-------------|-----------|----------|
| Configuração de estratégia default | Tech Lead | Arquiteto | Agent Operators | — |
| Override de estratégia em runtime | Tech Lead | Compliance (se regulado) | — | Agent Operators |
| Análise de padrões de conflito | Data Scientist | Tech Lead | Agent Operators | Product |
| Decisão HITL final | Domain Expert | Tech Lead | Compliance | — |

### Gates
| Gate | Condição | Ação |
|------|----------|------|
| G1: Auto-resolução | Confiança >90% + não regulado? | Resolver automaticamente |
| G2: HITL | Regulado ou confiança <70%? | Bloquear até aprovação humana |
| G3: Pós-resolução | Decisão documentada? | Permitir aprendizado do modelo |

---

## 3. Nível Pleno — Operação

### Fluxo de Decisão
```
Conflito detectado → É regulado/PII?
  ├─ SIM → HITL obrigatório → Escalar domain expert
  └─ NÃO → Quantos agentes?
       ├─ ≥5 → Majority Vote
       ├─ 3-4 → Seniority Weight
       └─ 2 → Adversarial Panel
            → Confiança >90%? → Aceitar
            → Senão → HITL
```

### Tabela de Escalação
| Situação | Quem | Canal | Timeout |
|----------|------|-------|---------|
| Conflito em dados financeiros | Compliance Officer | Slack #compliance | 15min |
| >10 conflitos/hora no mesmo domínio | Tech Lead | Slack #engineering | 30min |
| HITL sem resposta 30min (SEV-1) | PagerDuty on-call | Phone | Imediato |
| Padrão de conflito novo | Data Scientist | GitHub issue | 24h |

---

## 4. Nível Júnior — Triagem

### Verificar Conflitos Ativos
```bash
zylos resolver conflicts list --status open --format table
```

### Quando NÃO Tentar Resolver Sozinho
- Dados financeiros, médicos ou PII
- Conflito envolve >3 agentes
- Output será usado em decisão legal/regulatória
- Você não entende o domínio do conflito

### Regras
- Sempre documentar o rationale da decisão
- Se dúvida → escalar, nunca chutar
- Conflitos resolvidos alimentam aprendizado do sistema

---

## 5. Referências Cruzadas

- **Runbook:** [`runbooks/resolver-conflicts.md`](../runbooks/resolver-conflicts.md)
- **SOAR Workflow:** [`docs/soar/runbooks/hitl-approval.runbook.yaml`](../docs/soar/runbooks/hitl-approval.runbook.yaml)
- **Playbook relacionado:** [`adversarial-audit.md`](adversarial-audit.md)
- **Próxima revisão:** 2027-02-05
