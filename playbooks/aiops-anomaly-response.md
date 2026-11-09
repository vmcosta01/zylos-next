# Playbook: Resposta a Anomalias AIOps — Sentinel

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Alerta SEV-1/SEV-2 do Sentinel ou degradação detectada por AIOps
**Público-alvo:** Tech Lead, SRE Senior, On-call Engineer
**SLA:** MTTR <30min para SEV-1, <2h para SEV-2

---

## 1. Visão Geral

Este playbook define **O QUÊ fazer, QUEM decide e COMO escalar** quando o Sentinel detecta anomalias. Para passos técnicos de investigação, consulte o runbook associado.

**Runbooks associados:**
- [`runbooks/sentinel-aiops.md`](../runbooks/sentinel-aiops.md) — Investigação e troubleshooting do Sentinel
- [`runbooks/monitoring-observability.md`](../runbooks/monitoring-observability.md) — Métricas e dashboards gerais

---

## 2. Nível Senior — Estratégia e Governança

### Matriz RACI
| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|-------------|-------------|-----------|----------|
| Triagem inicial | On-call Engineer | SRE Lead | — | — |
| Classificação de severidade | On-call Engineer | SRE Lead | Tech Lead | — |
| Decisão de contenção automática | SOAR Workflow | SRE Lead | Tech Lead | Product Owner |
| Escalação para SEV-1 | SRE Lead | CTO | Tech Lead | Stakeholders |
| Post-mortem | Tech Lead | SRE Lead | On-call Engineer | Todos |
| Recalibração de thresholds | SRE Lead | Tech Lead | Data Scientist | — |

### Critérios de Severidade
| Severity | Condição | Exemplo | Tempo Máximo de Resolução |
|----------|----------|---------|--------------------------|
| SEV-1 | Serviço crítico indisponível ou dados corrompidos | Gateway down, evidence ledger integrity fail | 30min |
| SEV-2 | Degradação significativa mas serviço funcional | Latência p95 >5s, fila acumulando | 2h |
| SEV-3 | Degradação menor ou componente não-crítico | Dashboard lento, métricas atrasadas | 8h |
| SEV-4 | Anomalia estatística sem impacto visível | Z-score 2.0-2.5 isolado | Próxima sprint |

### Gates de Decisão
| Gate | Condição | Ação se Aprovado | Ação se Reprovado |
|------|----------|-----------------|-------------------|
| G1: Triagem | Alerta é verdadeiro positivo? | Prosseguir para G2 | Marcar falso positivo + feedback |
| G2: Contenção | Contenção automática é segura? | SOAR executa contenção | Escalar para humano |
| G3: Recuperação | Serviço restaurado e estável? | Fechar incidente | Reverter para G2 |
| G4: Post-mortem | Root cause identificada? | Documentar + action items | Agendar deep-dive |

### Trade-offs Documentados
- **Auto-contenção vs confirmação humana:** Auto-contenção ativada apenas para SEV-1 com confiança >95%. Demais casos requerem aprovação humana para evitar dano colateral.
- **Recalibração automática vs manual:** Recalibração automática permitida apenas fora de maintenance windows. Durante deploy, thresholds são congelados.

---

## 3. Nível Pleno — Fluxo de Decisão

### Fluxograma de Resposta
```
Alerta Sentinel → G1: Verdadeiro positivo?
  ├─ NÃO → Feedback false_positive → Fim
  └─ SIM → Classificar severidade
       ├─ SEV-1 → Page on-call + SOAR auto-triage → G2
       ├─ SEV-2 → Slack alert + ticket → Investigar → G2
       ├─ SEV-3 → Log warning → Investigar quando possível
       └─ SEV-4 → Anotar métrica → Review semanal

G2: Contenção segura?
  ├─ SIM → SOAR executa → Monitorar → G3
  └─ NÃO → Escalar humano → Decisão manual → G3

G3: Serviço estável?
  ├─ SIM → Documentar → G4
  └─ NÃO → Reverter para G2
```

### Tabela de Escalação
| Condição | Quem Contatar | Canal | Timeout |
|----------|--------------|-------|---------|
| SEV-1 confirmado | SRE Lead | PagerDuty | 5min |
| SEV-1 sem resolução 15min | Tech Lead | Phone call | Imediato |
| SEV-2 sem resolução 1h | Tech Lead | Slack DM | 15min |
| Dados financeiros/PII envolvidos | Compliance Officer | Email urgente | 30min |
| Múltiplos SEV-1 simultâneos | CTO | Phone + Slack | Imediato |

### Checklist de Comunicação
- [ ] Slack #incidents atualizado com status a cada 15min (SEV-1)
- [ ] Stakeholders informados via email em 30min (SEV-1)
- [ ] Status page atualizada se impacto externo
- [ ] Post-mortem agendado em 24h após resolução

---

## 4. Nível Júnior — Triagem Inicial

### Passo a Passo: Recebi um Alerta do Sentinel

1. **Verificar se é verdadeiro positivo**
   ```bash
   zylos sentinel alerts --alert-id <ID>
   zylos sentinel correlate --alert-id <ID> --window 30m
   ```
   Se correlaciona com outros eventos → provavelmente verdadeiro.

2. **Classificar severidade** usando tabela acima.

3. **Se SEV-3/SEV-4:** Documentar e monitorar. Não acionar ninguém.

4. **Se SEV-1/SEV-2:** Acionar escalação conforme tabela.

5. **NUNCA:** Ignorar alerta SEV-1, mesmo que pareça falso positivo. Sempre documentar.

### Glossário
- **Z-Score:** Medida estatística de quão distante uma métrica está da média histórica
- **SOAR:** Security Orchestration, Automation and Response — sistema de automação
- **MTTR:** Mean Time to Resolve — tempo médio de resolução
- **Contenção:** Ação para limitar dano (ex: isolar agente, reduzir rate limit)

---

## 5. Referências Cruzadas

- **Runbook principal:** [`runbooks/sentinel-aiops.md`](../runbooks/sentinel-aiops.md)
- **SOAR Workflow:** [`docs/soar/runbooks/sentinel-auto-triage.runbook.yaml`](../docs/soar/runbooks/sentinel-auto-triage.runbook.yaml)
- **Playbook relacionado:** [`incident-response.md`](incident-response.md)
- **Dashboard:** Grafana → "Sentinel Anomaly Detection"
- **Próxima revisão:** 2027-02-05 ou após próximo SEV-1
