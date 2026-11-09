# Runbook: Zylos Cerebelo — Orquestração de Workflows Multi-Agente

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Workflow travado, agente órfão, ou necessidade de debug de DAG
**Público-alvo:** Platform Engineer / Agent Operator
**Pré-requisitos:** Crate `zylos-cerebelo` compilado, acesso ao gateway

---

## 1. Visão Geral

O Cerebelo é o orquestrador central de workflows do Zylos:
- Execução de DAGs multi-agente com dependências
- Retry automático com backoff exponencial
- Checkpoint e recovery estruturado
- Timeout e cancellation safety
- Métricas de throughput e latência por stage

**Playbook associado:** [`playbooks/workflow-orchestration.md`](../playbooks/workflow-orchestration.md)

---

## 2. Nível Senior

### Decisões de Arquitetura
| Decisão | Alternativa | Racional |
|---------|------------|----------|
| DAG estático em YAML | Grafo dinâmico runtime | Auditabilidade e versionamento; dinamismo via templates |
| Checkpoint a cada stage | Checkpoint apenas no final | Recovery granular reduz retrabalho em 70% |
| Worker pool fixo | Auto-scaling por workflow | Previsibilidade de recursos; scaling via HPA no K8s |

### Modelos de Falha
| Tipo de Falha | Comportamento | Recovery |
|--------------|---------------|----------|
| Timeout de stage | Cancel + retry (max 3x) | Backoff 30s/60s/120s |
| Crash de agente | Mark stage failed | Skip ou abort dependendo de `on_failure` |
| Dependência falhada | Cascade skip | Notificar owner via SOAR |
| Gateway indisponível | Buffer local | Replay quando reconectar |

---

## 3. Nível Pleno

### Comandos
```bash
# Listar workflows ativos
zylos cerebelo workflows list --status running

# Inspeccionar DAG específico
zylos cerebelo dag inspect --workflow-id <ID>

# Ver checkpoints de um workflow
zylos cerebelo checkpoints --workflow-id <ID>

# Forçar retry de stage falhado
zylos cerebelo retry --workflow-id <ID> --stage <STAGE_ID>

# Cancelar workflow gracefully
zylos cerebelo cancel --workflow-id <ID> --reason "manual abort"

# Métricas de throughput
zylos cerebelo metrics --last 24h --format table
```

### Troubleshooting
| Sintoma | Causa | Solução |
|---------|-------|---------|
| Workflow preso em "pending" | Dependência circular ou resource lock | `zylos cerebelo dag validate --workflow-id <ID>` |
| Stage retry infinito | Bug no agente ou input inválido | Verificar logs do agente; cancelar e corrigir spec |
| Checkpoint corrompido | Disk full ou write interrompido | `zylos cerebelo checkpoint repair --workflow-id <ID>` |
| Throughput degradado | Worker saturation | Aumentar pool size ou escalar horizontalmente |

---

## 4. Nível Júnior

### Debug de Workflow Travado (Passo a Passo)

1. **Identificar o workflow problemático**
   ```bash
   zylos cerebelo workflows list --status stuck
   ```

2. **Ver qual stage está bloqueado**
   ```bash
   zylos cerebelo dag inspect --workflow-id <ID> --highlight blocked
   ```

3. **Checar logs do agente responsável**
   ```bash
   zylos agent logs <AGENT_ID> --tail 50
   ```

4. **Se for timeout:** retry manual
   ```bash
   zylos cerebelo retry --workflow-id <ID> --stage <STAGE>
   ```

5. **Se for erro de input:** cancelar e re-submeter com spec corrigida
   ```bash
   zylos cerebelo cancel --workflow-id <ID>
   zylos task submit corrected-spec.yaml
   ```

### Quando Escalar
- Workflow crítico parado >30min → Tech Lead
- Múltiplos workflows stuck simultaneamente → SRE (possível problema de infra)
- Checkpoint corruption recorrente → Platform Engineer (bug no cerebelo)

---

## 5. Referências Cruzadas

- **Playbook:** [`playbooks/workflow-orchestration.md`](../playbooks/workflow-orchestration.md)
- **SOAR Workflow:** [`docs/soar/runbooks/dag-execution.runbook.yaml`](../docs/soar/runbooks/dag-execution.runbook.yaml)
- **Crate:** `crates/zylos-cerebelo/src/lib.rs`
- **Runbook relacionado:** [`task-durability.md`](fase7b-task-durability.md)
