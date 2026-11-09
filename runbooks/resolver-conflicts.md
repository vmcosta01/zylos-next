# Runbook: Zylos Resolver — Resolução de Conflitos Multi-Agente

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Conflito detectado entre outputs de agentes ou decisões concorrentes
**Público-alvo:** Agent Operator / Tech Lead
**Pré-requisitos:** Crate `zylos-resolver` compilado

---

## 1. Visão Geral

O Resolver gerencia conflitos quando múltiplos agentes produzem outputs divergentes:
- Detecção automática de divergência (semantic diff)
- Estratégias de resolução: majority vote, seniority, adversarial panel, human-in-the-loop
- Logging completo de decisão para auditoria
- Integração com HITL (Human-in-the-Loop) para casos ambíguos

**Playbook associado:** [`playbooks/conflict-resolution.md`](../playbooks/conflict-resolution.md)

---

## 2. Nível Senior

### Estratégias de Resolução
| Estratégia | Quando Usar | Latência | Confiabilidade |
|-----------|-------------|----------|----------------|
| Majority Vote | ≥3 agentes concordam | <1s | Alta se N≥5 |
| Seniority Weight | Agentes têm níveis de confiança diferentes | <1s | Média |
| Adversarial Panel | Outputs críticos divergem | 30-60s | Muito alta |
| Human-in-the-Loop | Ambiguidade irresolvível automaticamente | Minutos-horas | Definitiva |

### Decisões
| Decisão | Racional |
|---------|----------|
| Semantic diff vs exact match | Outputs equivalentes não devem gerar falso conflito |
| Default para HITL em SEV-1 | Segurança sobre velocidade em cenários críticos |
| Cache de resoluções passadas | Mesmo conflito não deve ser re-resolvido do zero |

---

## 3. Nível Pleno

### Comandos
```bash
# Listar conflitos ativos
zylos resolver conflicts list --status open

# Ver detalhes de conflito
zylos resolver conflict inspect --conflict-id <ID>

# Forçar resolução manual
zylos resolver resolve --conflict-id <ID> --strategy human --decision <DECISION>

# Ver histórico de resoluções
zylos resolver history --last 7d --format table

# Configurar estratégia default
zylos resolver config set default_strategy adversarial_panel
```

### Troubleshooting
| Sintoma | Causa | Solução |
|---------|-------|---------|
| Conflitos em excesso | Prompts mal especificados gerando outputs divergentes | Revisar specs dos agentes |
| Resolução lenta | Adversarial panel com timeout | Aumentar timeout ou mudar para majority vote |
| HITL nunca responde | Canal de notificação quebrado | Verificar integração Slack/email |
| Resolução incorreta recorrente | Estratégia inadequada para domínio | Mudar estratégia ou ajustar pesos |

---

## 4. Nível Júnior

### Resolver Conflito Manualmente

1. **Listar conflitos abertos**
   ```bash
   zylos resolver conflicts list --status open
   ```

2. **Inspecionar o conflito**
   ```bash
   zylos resolver conflict inspect --conflict-id <ID>
   ```
   Leia os outputs divergentes e o contexto.

3. **Decidir qual output é correto**
   - Se óbvio: use `--strategy manual`
   - Se ambíguo: escale para Tech Lead ou use adversarial panel

4. **Registrar decisão**
   ```bash
   zylos resolver resolve --conflict-id <ID> --strategy manual --decision "agent_A_output"
   ```

5. **Documentar rationale** no comentário da resolução para aprendizado futuro.

### Quando Escalar
- Conflito envolve dados financeiros ou PII → Compliance Officer
- >5 conflitos/hora no mesmo domínio → Tech Lead (problema sistêmico)
- HITL sem resposta em 30min para SEV-1 → PagerDuty

---

## 5. Referências Cruzadas

- **Playbook:** [`playbooks/conflict-resolution.md`](../playbooks/conflict-resolution.md)
- **SOAR Workflow:** [`docs/soar/runbooks/hitl-approval.runbook.yaml`](../docs/soar/runbooks/hitl-approval.runbook.yaml)
- **Crate:** `crates/zylos-resolver/src/lib.rs`
- **Runbook relacionado:** [`hitl-approval.md`](hitl-approval.md)
