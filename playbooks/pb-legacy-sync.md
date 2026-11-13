# Playbook: Legacy System Sync (pb-legacy-sync)

**Versão:** 1.0.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `playbook-author`
**SOAR Workflow:** `soar-guardrail-enforcement.md`

---

## 1. Visão Geral

### Cenário Coberto
Sincronização de dados entre sistemas legados e o Zylos Gateway, incluindo migração incremental de registros históricos, reconciliação de estados divergentes e validação de integridade pós-sync.

### Gatilho de Ativação
- **Automático:** Cron diário às 03:00 UTC (`0 3 * * *`)
- **Manual:** Comando via cockpit SOAR Dashboard ou CLI `zylos soar trigger pb-legacy-sync`
- **Evento:** Alerta de drift detectado pelo Sentinel (`sentinel.drift.legacy_sync`)

### SLA / RTO / RPO
| Métrica | Valor |
|---------|-------|
| RTO (tempo máximo de sync) | 4 horas |
| RPO (perda máxima de dados) | 0 registros (sync idempotente) |
| Timeout aprovação HITL | **30 minutos** (obrigatório) |
| Janela de execução | 03:00–07:00 UTC (baixo tráfego) |

### Runbooks Referenciados
- [`rb-notify-slack`](../runbooks/rb-notify-slack.md) — Notificações Slack em cada fase
- [`runbooks/zylos-gateway.md`](../runbooks/zylos-gateway.md) — Health check e restart
- [`runbooks/sentinel-aiops.md`](../runbooks/sentinel-aiops.md) — Monitoramento durante sync

---

## 2. Nível Senior (Tech Lead / Arquiteto / SRE Senior)

### Matriz RACI

| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|-------------|-------------|-----------|----------|
| Aprovar início do sync | SRE On-call | Tech Lead | Product Owner | Stakeholders |
| Executar extração legacy | Data Engineer | SRE On-call | DBA | — |
| Validar integridade pós-sync | QA Engineer | Tech Lead | Data Engineer | Product Owner |
| Escalar falha crítica | SRE On-call | Engineering Manager | Tech Lead | CTO |
| Aprovar rollback | Tech Lead | Engineering Manager | SRE On-call | Stakeholders |

### Critérios de Severidade

| Severidade | Condição | Exemplo | Ação |
|------------|----------|---------|------|
| SEV-1 | Perda de dados confirmada | Registros deletados no source sem marcação | Parar sync imediato, page on-call, iniciar DR |
| SEV-2 | Drift > 1% dos registros | 500+ registros divergentes em batch de 50k | Pausar sync, notificar Tech Lead, aguardar aprovação |
| SEV-3 | Latência > 2x baseline | Sync levando 6h em vez de 3h | Notificar SRE, continuar com monitoramento reforçado |
| SEV-4 | Warning de schema deprecated | Campo legado mapeado para novo formato | Logar, continuar, criar ticket de tech debt |

### Gates de Decisão

| Gate | Condição Objetiva | Aprovador | Timeout |
|------|-------------------|-----------|---------|
| G1: Pré-sync validation | Source DB health OK + target capacity > 20% | SRE On-call | 15 min |
| G2: Post-extraction checksum | MD5(source_batch) == MD5(staging_batch) | Automático | 5 min |
| G3: HITL approval (SEV-2+) | Drift report revisado e aprovado | Tech Lead | **30 min** |
| G4: Post-sync integrity | 100% records reconciled + zero orphan refs | QA Engineer | 30 min |

### Trade-offs Documentados

| Decisão | Alternativa Rejeitada | Racional |
|---------|----------------------|----------|
| Sync batch a cada 4h | Streaming CDC em tempo real | Legacy DB não suporta CDC; batch é idempotente e auditável |
| Checksum MD5 por batch | Row-by-row comparison | Performance: MD5 batch é 100x mais rápido para >10k registros |
| HITL apenas em SEV-2+ | HITL em todo sync | Volume: 4 syncs/dia tornariam HITL gargalo operacional |
| Rollback full-batch | Rollback parcial por registro | Atomicidade: rollback parcial pode deixar estado inconsistente |

### Referências
- ADR-012: Batch sync vs CDC para sistemas legados
- Post-mortem INC-2026-089: Sync timeout sem HITL causou 2h de downtime
- [SOAR Guardrail Enforcement](soar-guardrail-enforcement.md)

---

## 3. Nível Pleno (Dev Pleno / On-call Engineer)

### Fluxograma de Decisão

```
INÍCIO
  │
  ├─ G1: Source DB health OK? ──NÃO──→ Notificar SRE → ABORTAR
  │         │SIM
  ├─ Extrair batch do legacy
  │         │
  ├─ G2: Checksum match? ──NÃO──→ Re-extrair (max 3 retries)
  │         │                       └─ Falhou 3x? → SEV-2 → G3 HITL
  │         │SIM
  ├─ Transformar + carregar no Zylos
  │         │
  ├─ Drift > 1%? ──SIM──→ G3: HITL approval (30min timeout)
  │         │NÃO            └─ Timeout? → SEV-2 → Escalar Tech Lead
  ├─ G4: Integridade pós-sync? ──NÃO──→ Rollback full-batch → SEV-2
  │         │SIM
  └─ Notificar sucesso → rb-notify-slack → FIM
```

### Tabela de Escalação

| Condição | Quem Contatar | Canal | Timeout |
|----------|---------------|-------|---------|
| Source DB unreachable | SRE On-call | PagerDuty | 5 min |
| Checksum mismatch 3x | Tech Lead | Slack #engineering | 15 min |
| HITL timeout (30min) | Engineering Manager | Phone + Slack | Imediato |
| Drift > 5% | Tech Lead + PO | Slack #incident | 10 min |
| Rollback falhou | Engineering Manager | PagerDuty SEV-1 | Imediato |

### Checklist de Comunicação

- [ ] **Pré-sync:** Notificar #engineering via `rb-notify-slack` (início estimado)
- [ ] **Durante sync:** Atualizar status no cockpit SOAR Dashboard
- [ ] **SEV-2+:** Criar thread em #incident com drift report
- [ ] **Pós-sync sucesso:** Notificar #engineering via `rb-notify-slack` (resumo)
- [ ] **Pós-sync falha:** Notificar #incident + criar ticket Jira

### Runbooks Acionáveis por Fase

| Fase | Runbook | Ação |
|------|---------|------|
| Diagnóstico | `sentinel-aiops.md` | Verificar métricas de drift e latência |
| Contenção | `zylos-gateway.md` | Health check e restart se necessário |
| Recuperação | `rb-notify-slack.md` | Notificar stakeholders do status |
| Validação | `evidence-ledger.md` | Registrar prova de integridade pós-sync |

### Métricas de Sucesso

| Métrica | Alvo | Medição |
|---------|------|---------|
| MTTR (sync failure) | < 30 min | Tempo entre falha e resolução |
| Taxa de falsos positivos drift | < 2% | Drift reports que eram ruído / total |
| Sync duration p95 | < 3h | Percentil 95 das últimas 30 execuções |
| HITL response time | < 15 min | Média de tempo de resposta em approvals |

---

## 4. Nível Júnior (Dev Júnior / Estagiário / Agente Automatizado)

### Triagem Inicial (Passo a Passo)

1. **Verificar health do source DB:**
   ```bash
   curl -s http://legacy-db.internal:8080/health | jq .status
   # Esperado: "healthy"
   # Se != "healthy" → ESCALAR IMEDIATAMENTE para SRE On-call
   ```

2. **Verificar capacidade do target:**
   ```bash
   curl -s http://localhost:3000/api/v1/phase2/status | jq .subsystems.memory_sync.total_entries
   # Se > 80% da capacidade → NÃO INICIAR, escalar para Tech Lead
   ```

3. **Verificar janela de execução:**
   - Hora atual deve estar entre 03:00–07:00 UTC
   - Se fora da janela → AGUARDAR ou obter aprovação explícita do Tech Lead

4. **Iniciar sync:**
   ```bash
   curl -X POST http://localhost:3000/api/v1/soar/events \
     -H "Content-Type: application/json" \
     -d '{"event_type":"legacy_sync.start","payload":{"batch_id":"auto"}}'
   ```

### Quando NÃO Agir e Escalar Imediatamente

- ❌ Source DB retorna erro 5xx → Escalar SRE On-call
- ❌ Drift report mostra > 5% divergência → Escalar Tech Lead
- ❌ HITL approval pendente há > 20 min → Notificar Engineering Manager
- ❌ Qualquer mensagem de erro contendo "data loss" ou "corruption" → SEV-1 imediato
- ❌ Não tem certeza sobre qualquer passo → PERGUNTAR antes de agir

### Glossário

| Termo | Significado |
|-------|-------------|
| **Drift** | Divergência entre registros no source legacy e target Zylos |
| **Batch** | Conjunto de registros extraídos e sincronizados atomicamente |
| **HITL** | Human-In-The-Loop — aprovação humana obrigatória antes de prosseguir |
| **RTO** | Recovery Time Objective — tempo máximo aceitável de recuperação |
| **RPO** | Recovery Point Objective — perda máxima aceitável de dados |
| **CDC** | Change Data Capture — captura de mudanças em tempo real |
| **Checksum** | Hash de verificação de integridade (MD5/SHA-256) |

---

## 5. Referências Cruzadas

- **Runbooks:** [`rb-notify-slack`](../runbooks/rb-notify-slack.md), [`zylos-gateway.md`](../runbooks/zylos-gateway.md), [`sentinel-aiops.md`](../runbooks/sentinel-aiops.md)
- **SOAR Workflow:** [`soar-guardrail-enforcement.md`](soar-guardrail-enforcement.md)
- **POPs Relacionados:** [`pop-guardrail-rule-update.md`](pop-guardrail-rule-update.md)
- **Dashboard:** Cockpit SOAR Dashboard → Tab "Legacy Sync"
- **Monitoramento:** Grafana Dashboard "Legacy Sync Metrics"
- **Canais:** Slack #engineering, #incident, PagerDuty
- **Última Revisão:** 2026-11-11
- **Próximo Review Obrigatório:** 2026-12-11 (30 dias) ou após próximo incidente SEV-1/SEV-2

---

## Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0.0 | 2026-11-11 | Implementation Agent | Criação inicial com timeout HITL de 30min (resolve alerta de validação) |