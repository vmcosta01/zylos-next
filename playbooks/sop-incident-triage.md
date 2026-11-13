# SOP — Triagem de Incidentes (Standard Operating Procedure)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`incident-response.md`](./incident-response.md)
**Runbooks Relacionados:** `zylos-gateway.md`, `sentinel-aiops.md`, `cerebelo-orchestration.md`
**Público:** SRE On-Call, DevOps Pleno+, Agentes Sentinel/Cerebelo

---

## 1. Objetivo

Padronizar a triagem inicial de qualquer alerta ou incidente reportado no Zylos-Next, garantindo classificação consistente de severidade, roteamento correto e tempo máximo de resposta dentro do SLA.

## 2. Escopo

Aplica-se a:
- Alertas automáticos do Sentinel/AIOps
- Reportes manuais via cockpit ou Slack
- Falhas detectadas por health checks do gateway
- Anomalias em DAGs do Cerebelo

Não aplica-se a: requests de feature, dúvidas de uso, onboarding.

## 3. Matriz de Severidade

| Severidade | Critério | SLA Resposta | SLA Resolução | Exemplo |
|-----------|----------|-------------|---------------|---------|
| **SEV-1** | Sistema indisponível para todos os usuários; perda de dados; breach de segurança | 5 min | 1h | Gateway down, DB corrompido |
| **SEV-2** | Funcionalidade crítica degradada; impacto em >50% dos agentes | 15 min | 4h | Memory system lento, sandbox crash loop |
| **SEV-3** | Funcionalidade não-crítica afetada; workaround disponível | 1h | 24h | Skill específica falhando, UI glitch |
| **SEV-4** | Cosmetic, log noise, melhoria menor | 4h | Próximo sprint | Warning spam, typo na UI |

## 4. Procedimento Passo-a-Passo

### Step 1: Receber e Registrar (≤2 min)
```bash
# Se alerta automático → já registrado no Sentinel
# Se reporte manual → criar ticket:
zylos-cli incident create --severity TBD --summary "descrição curta"
```
- Anotar timestamp exato da detecção
- Identificar reporter (humano/agente/sistema)
- Linkar logs relevantes se disponíveis

### Step 2: Classificar Severidade (≤3 min)
Consultar matriz acima. Em caso de dúvida:
- **Escalonar para SEV superior** (melhor over-classify que under-classify)
- Verificar dashboards: `http://localhost:9876/api/v1/health`
- Checar status dos agentes: `zylos-cli agents list --status error`

### Step 3: Roteamento
| Severidade | Ação Imediata | Notificar |
|-----------|--------------|-----------|
| SEV-1 | Acionar war room, notificar Tech Lead + SRE Lead | Slack #incidents, PagerDuty |
| SEV-2 | Atribuir ao owner do componente afetado | Slack #incidents |
| SEV-3 | Criar task no Kanban, atribuir no próximo triage | Slack #ops |
| SEV-4 | Criar issue GitHub com label `low-priority` | — |

### Step 4: Comunicação Inicial
Postar no canal apropriado:
```
🚨 [SEV-{N}] {Resumo em 1 linha}
- Detectado: {timestamp}
- Impacto: {descrição}
- Owner: @{pessoa}
- Status: Investigando
- Ticket: {link}
```

### Step 5: Handoff para Runbook Específico
Identificar o componente afetado e executar o runbook correspondente:
- Gateway → `runbooks/zylos-gateway.md` § Troubleshooting
- Docker/Sandbox → `runbooks/zylos-docker.md` § Debug
- Browser → `runbooks/zylos-browser.md` § Anti-Detect Issues
- Memory → `runbooks/fase2-memory-system.md` § Recovery
- DAG/Cerebelo → `runbooks/cerebelo-orchestration.md` § Recovery Ladder
- Sentinel → `runbooks/sentinel-aiops.md` § False Positive

## 5. Escalação Automática

Se SLA de resposta for excedido:
| Severidade | Após | Escalonar Para |
|-----------|------|----------------|
| SEV-1 | 10 min | Tech Lead direto (telefone) |
| SEV-2 | 30 min | SRE Lead + Engineering Manager |
| SEV-3 | 2h | Team Lead do domínio |
| SEV-4 | 8h | Auto-promover para SEV-3 |

## 6. Critérios de Encerramento

Um incidente só é encerrado quando:
- [ ] Causa raiz identificada e documentada
- [ ] Fix aplicado ou workaround validado
- [ ] Métricas retornaram ao baseline por ≥15 min
- [ ] Comunicação de resolução postada
- [ ] Post-mortem agendado (SEV-1/2 obrigatório)

## 7. Referências

- Playbook completo: [`incident-response.md`](./incident-response.md)
- Dashboard de saúde: `http://localhost:9876/api/v1/health`
- Logs do Sentinel: `~/Zylos/logs/sentinel.jsonl`
- Evidence Ledger: `zylos-cli evidence query --incident {id}`