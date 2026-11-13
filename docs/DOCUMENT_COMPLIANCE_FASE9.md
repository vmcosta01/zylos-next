# Compliance Documental — Fase 9 (Cockpit v2)

**Versão:** 1.0.0
**Data:** 2026-11-11
**Base:** IMPLEMENTATION_PLAN_DETAILED.md + Manifesto v3.1 (10 camadas)
**Status:** ⚠️ GAPS IDENTIFICADOS — documentos abaixo devem ser criados ANTES ou DURANTE implementação

---

## 1. Gap Analysis: Documentos Existentes vs Necessários

### 1.1 Handoffs

| Documento | Status | Ação Necessária |
|-----------|--------|-----------------|
| `HANDOFF_FASE8.md` | ✅ Existe | — |
| `HANDOFF_FASE9.md` | ❌ Faltando | Criar ao final da Fase 9 com resumo executivo, módulos entregues, testes, known issues |
| `fase8-to-9-handoff.md` | ❌ Faltando | Criar AGORA como ponte entre fases (contexto, decisões pendentes, riscos herdados) |

### 1.2 READMEs por Crate Novo

| Crate | README | Status | Ação |
|-------|--------|--------|------|
| `zylos-approval` | `crates/zylos-approval/README.md` | ❌ Faltando | Criar com: propósito, API endpoints, schema, exemplos, testes |
| `zylos-egress` | `crates/zylos-egress/README.md` | ❌ Faltando | Criar com: patterns detectados, regras custom, performance benchmarks |
| `zylos-brief` | `crates/zylos-brief/README.md` | ❌ Faltando | Criar com: trigger schedule, fontes de dados, fallback behavior |
| `zylos-spaces` | `crates/zylos-spaces/README.md` | ❌ Faltando | Criar com: CRUD ops, markdown support, autosave, export |
| `zylos-model-config` | `crates/zylos-model-config/README.md` | ❌ Faltando | Criar com: tier strategy, reasoning effort, provider list |
| `zylos-computer-view` | `crates/zylos-computer-view/README.md` | ❌ Faltando | Criar com: VNC proxy, WebSocket protocol, fallback static |
| `zylos-audit-log` | `crates/zylos-audit-log/README.md` | ❌ Faltando | Criar com: schema, retention policy, export CSV, stats API |

### 1.3 Playbooks (Procedimentos Estratégicos)

| Playbook | Vinculado a | Status | Ação |
|----------|-------------|--------|------|
| `pb-approval-gate-setup.md` | Task 1.1 | ❌ Faltando | Criar: como configurar approval rules, categorias block/warn, expiry |
| `pb-egress-rules-management.md` | Task 1.2 | ❌ Faltando | Criar: como adicionar/remover regras custom, testar patterns |
| `pb-morning-brief-config.md` | Task 2.1 | ❌ Faltando | Criar: como ajustar horário, fontes de dados, timezone |
| `pb-model-tier-strategy.md` | Task 2.3 | ❌ Faltando | Criar: como definir cheapest/balanced/fastest por bot |
| `pb-spaces-onboarding.md` | Task 2.5 | ❌ Faltando | Criar: como criar primeiro space, importar docs, organizar páginas |
| `pb-computer-view-setup.md` | Task 2.4 | ❌ Faltando | Criar: como provisionar VM/container, configurar VNC, take/release control |

### 1.4 POPs (Procedimentos Operacionais Padrão)

| POP | Vinculado a | Status | Ação |
|-----|-------------|--------|------|
| `pop-approval-decide.md` | Task 1.1 | ❌ Faltando | Criar: passo-a-passo para decidir approval (allow once/always/refuse), rollback |
| `pop-egress-false-positive.md` | Task 1.2 | ❌ Faltando | Criar: como reportar falso positivo, ajustar regra, reenviar mensagem |
| `pop-audit-log-export.md` | Task 1.4 | ❌ Faltando | Criar: como exportar audit log CSV, filtrar por período/bot, arquivar |
| `pop-brief-regenerate.md` | Task 2.1 | ❌ Faltando | Criar: como forçar regeneração do brief, verificar fontes falhas |
| `pop-space-page-recovery.md` | Task 2.5 | ❌ Faltando | Criar: como recuperar página deletada, restaurar versão anterior |
| `pop-computer-control-emergency.md` | Task 2.4 | ❌ Faltando | Criar: como tomar controle emergencial, kill session, investigar ação suspeita |
| `pop-model-fallback.md` | Task 2.3 | ❌ Faltando | Criar: como trocar modelo quando primary falha, verificar quota |

### 1.5 Runbooks (Procedimentos Técnicos de Operação)

| Runbook | Vinculado a | Status | Ação |
|---------|-------------|--------|------|
| `rb-zylos-approval.md` | Crate zylos-approval | ❌ Faltando | Criar: deploy, migration, health check, troubleshooting |
| `rb-zylos-egress.md` | Crate zylos-egress | ❌ Faltando | Criar: deploy, rule reload, performance tuning, false positive triage |
| `rb-zylos-brief.md` | Crate zylos-brief | ❌ Faltando | Criar: cron setup, source connectivity check, manual trigger |
| `rb-zylos-spaces.md` | Crate zylos-spaces | ❌ Faltando | Criar: backup/restore pages, markdown lint, orphan cleanup |
| `rb-zylos-model-config.md` | Crate zylos-model-config | ❌ Faltando | Criar: provider health check, quota monitoring, failover test |
| `rb-zylos-computer-view.md` | Crate zylos-computer-view | ❌ Faltando | Criar: VNC proxy start/stop, WebSocket debug, screenshot fallback |
| `rb-zylos-audit-log.md` | Crate zylos-audit-log | ❌ Faltando | Criar: retention job, archive to S3, stats query optimization |
| `rb-cockpit-v2-deploy.md` | Cockpit v2 geral | ❌ Faltando | Criar: trunk build, WASM size check, design-tokens validation |

### 1.6 SOAR Workflows (Orquestração de Segurança Automatizada)

| Workflow | Trigger | Status | Ação |
|----------|---------|--------|------|
| `soar-egress-block-response.md` | Egress checker bloqueia PII | ❌ Faltando | Criar: notificar usuário, log incidente, sugerir redação alternativa |
| `soar-approval-timeout.md` | Approval expira sem decisão | ❌ Faltando | Criar: cancelar ação, notificar bot, escalar se crítico |
| `soar-audit-anomaly.md` | Audit log detecta padrão suspeito | ❌ Faltando | Criar: alertar admin, pausar bot, gerar relatório forense |
| `soar-computer-control-breach.md` | Controle não autorizado detectado | ❌ Faltando | Criar: revogar sessão, notificar usuário, snapshot forense |

### 1.7 POTs (Procedimentos Operacionais Táticos)

| POT | Contexto | Status | Ação |
|-----|----------|--------|------|
| `pot-approval-rule-tuning.md` | Ajuste fino de regras após falso positivo/negativo | ❌ Faltando | Criar: análise de padrões, ajuste regex, validação A/B |
| `pot-egress-pattern-update.md` | Adição de novo pattern de PII/secret | ❌ Faltando | Criar: pesquisar regex, testar contra corpus, deploy gradual |
| `pot-model-benchmark.md` | Comparação de modelos para tier strategy | ❌ Faltando | Criar: benchmark latency/custo/qualidade, atualizar frontier |
| `pot-space-migration.md` | Migração de docs legados para Spaces | ❌ Faltando | Criar: converter formato, validar links, rollback plan |

### 1.8 Track Record / Decision Log

| Documento | Status | Ação |
|-----------|--------|------|
| `docs/DECISIONS_FASE9.md` | ❌ Faltando | Criar: log de todas as decisões arquiteturais da Fase 9 com racional e alternativas consideradas |
| `docs/TRACKRECORD_FASE9.md` | ❌ Faltando | Criar: registro cronológico de implementações, blockers, resoluções, métricas |
| Atualizar `MODULE_DOCUMENTATION_STATUS.md` | ⚠️ Parcial | Adicionar 7 novos crates da Fase 9 na tabela |

### 1.9 Documentação Transversal

| Documento | Status | Ação |
|-----------|--------|------|
| Atualizar `DESIGN_SYSTEM.md` | ⚠️ Parcial | Adicionar tokens de approval states, priority levels, agent identity cores, computer view |
| Atualizar `IMPLEMENTATION_PLAN.md` (master) | ⚠️ Parcial | Referenciar IMPLEMENTATION_PLAN_DETAILED.md como anexo da Fase 9 |
| Atualizar `AGENTS.md` | ⚠️ Parcial | Adicionar auto-invocation rules para novas skills (approval-author, egress-author, etc.) |
| Atualizar `CONCEITOS-DESENVOLVIMENTO-IA.md` | ⚠️ Parcial | Adicionar conceitos novos: HITL gates, egress checking, morning brief pattern |
| Atualizar gateway `README.md` | ⚠️ Parcial | Adicionar endpoints novos: /approvals/*, /egress/*, /brief/*, /spaces/*, /audit/* |
| Atualizar cockpit `README.md` | ⚠️ Parcial | Adicionar novos componentes, tabs, design system v2 |

---

## 2. Matriz de Rastreabilidade: Task → Documentos Obrigatórios

| Task | Crate | README | Playbook | POP | Runbook | SOAR | POT | Handoff | Track |
|------|-------|--------|----------|-----|---------|------|-----|---------|-------|
| 1.1 Approval Gates | zylos-approval | ✅ | pb-approval-gate-setup | pop-approval-decide | rb-zylos-approval | soar-approval-timeout | pot-approval-rule-tuning | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 1.2 Egress Checker | zylos-egress | ✅ | pb-egress-rules-management | pop-egress-false-positive | rb-zylos-egress | soar-egress-block-response | pot-egress-pattern-update | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 1.3 Password Masking | (gateway middleware) | — | — | — | rb-zylos-gateway (update) | — | — | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 1.4 Audit Log | zylos-audit-log | ✅ | — | pop-audit-log-export | rb-zylos-audit-log | soar-audit-anomaly | — | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 2.1 Morning Brief | zylos-brief | ✅ | pb-morning-brief-config | pop-brief-regenerate | rb-zylos-brief | — | — | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 2.2 Task Triage | (cockpit component) | — | — | — | rb-cockpit-v2-deploy (update) | — | — | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 2.3 Multi-Model | zylos-model-config | ✅ | pb-model-tier-strategy | pop-model-fallback | rb-zylos-model-config | — | pot-model-benchmark | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 2.4 Computer View | zylos-computer-view | ✅ | pb-computer-view-setup | pop-computer-control-emergency | rb-zylos-computer-view | soar-computer-control-breach | — | HANDOFF_FASE9 | TRACKRECORD_FASE9 |
| 2.5 Spaces/Docs | zylos-spaces | ✅ | pb-spaces-onboarding | pop-space-page-recovery | rb-zylos-spaces | — | pot-space-migration | HANDOFF_FASE9 | TRACKRECORD_FASE9 |

---

## 3. Templates de Documentos (Estrutura Obrigatória)

### 3.1 Template: README de Crate

```markdown
# {crate-name}

> {one-line description}

## Purpose
{why this crate exists, what problem it solves}

## Architecture
{key structs, traits, modules — with mermaid diagram if complex}

## API Endpoints (if applicable)
| Method | Path | Description |
|--------|------|-------------|
| GET | /api/v1/{resource} | List all |

## Schema
{SQL migrations or Rust struct definitions}

## Configuration
{env vars, config file options, feature flags}

## Testing
{how to run tests, coverage targets, key test scenarios}

## Dependencies
{key deps and why they were chosen}

## Related Documents
- Playbook: [pb-{name}.md](../../playbooks/pb-{name}.md)
- Runbook: [rb-{name}.md](../../runbooks/rb-{name}.md)
- POP: [pop-{name}.md](../../playbooks/pop-{name}.md)
- SOAR: [soar-{name}.md](../soar/soar-{name}.md)
```

### 3.2 Template: Playbook

```markdown
# PB-{NUMBER}: {Title}

**Version:** 1.0.0
**Owner:** {team/person}
**Last Updated:** {date}
**Related POPs:** [pop-{name}.md]
**Related Runbooks:** [rb-{name}.md]

## Objective
{what this playbook achieves, business context}

## Prerequisites
{what must be true before starting}

## Steps
1. {step with concrete commands/actions}
2. {step}
3. {step}

## Decision Points
{where human judgment is needed, with criteria}

## Rollback Procedure
{how to undo if something goes wrong}

## SLA/RTO/RPO
- **SLA:** {availability target}
- **RTO:** {recovery time objective}
- **RPO:** {recovery point objective}

## References
{links to related docs, external resources}
```

### 3.3 Template: POP

```markdown
# POP-{NUMBER}: {Title}

**Version:** 1.0.0
**Playbook:** [pb-{name}.md]
**Last Updated:** {date}

## Purpose
{when and why to use this procedure}

## Trigger
{what event initiates this POP}

## Steps
1. {atomic step}
2. {atomic step}
3. {atomic step}

## Verification
{how to confirm success}

## Rollback
{mandatory — how to undo each step}

## Escalation
{when and how to escalate to human}
```

### 3.4 Template: SOAR Workflow

```markdown
# SOAR: {Workflow Name}

**Trigger:** {event that activates this workflow}
**Severity:** {critical/high/medium/low}
**Auto-Remediation:** {yes/no/partial}

## Detection
{how the trigger is detected, signal sources}

## Containment
{immediate actions to limit impact}

## Investigation
{steps to understand root cause}

## Remediation
{actions to fix the issue}

## Recovery
{steps to restore normal operation}

## Post-Incident
{documentation, lessons learned, prevention}

## Bound POPs
- [pop-{name}.md] — {when invoked}

## Bound Runbooks
- [rb-{name}.md] — {when invoked}
```

### 3.5 Template: Handoff

```markdown
# Handoff: Fase {N} → Fase {N+1}

**Date:** {date}
**Author:** {name}

## Executive Summary
{3-5 sentences on what was delivered}

## Modules Delivered
| Module | Status | Tests | Notes |
|--------|--------|-------|-------|
| {name} | ✅ Complete | {count} | {notes} |

## Known Issues
{bugs, limitations, tech debt carried forward}

## Decisions Made
{key architectural decisions with rationale}

## Risks for Next Phase
{what could go wrong, mitigations}

## Environment State
{versions, configs, running services}

## Documentation Index
{links to all docs created/updated in this phase}
```

### 3.6 Template: Track Record

```markdown
# Track Record — Fase {N}

## Chronological Log

### {YYYY-MM-DD}
- **Implemented:** {what was done}
- **Blocker:** {what stopped progress}
- **Resolution:** {how blocker was resolved}
- **Tests Added:** {count and type}
- **Decision:** {any architectural decision made}
- **Metric:** {performance, coverage, build time delta}

### {YYYY-MM-DD}
...

## Metrics Summary
| Metric | Start | End | Delta |
|--------|-------|-----|-------|
| Test count | {n} | {m} | +{delta} |
| Build time | {t1} | {t2} | {delta} |
| Warnings | {w1} | {w2} | {delta} |
| Coverage | {c1}% | {c2}% | {delta} |
```

---

## 4. Checklist de Compliance por Gate

### Gate Fase 1 (Segurança)

- [ ] `HANDOFF_FASE9.md` criado (mesmo que parcial, atualizado ao final)
- [ ] `fase8-to-9-handoff.md` criado
- [ ] `crates/zylos-approval/README.md` criado
- [ ] `crates/zylos-egress/README.md` criado
- [ ] `crates/zylos-audit-log/README.md` criado
- [ ] `playbooks/pb-approval-gate-setup.md` criado
- [ ] `playbooks/pb-egress-rules-management.md` criado
- [ ] `playbooks/pop-approval-decide.md` criado
- [ ] `playbooks/pop-egress-false-positive.md` criado
- [ ] `playbooks/pop-audit-log-export.md` criado
- [ ] `runbooks/rb-zylos-approval.md` criado
- [ ] `runbooks/rb-zylos-egress.md` criado
- [ ] `runbooks/rb-zylos-audit-log.md` criado
- [ ] `soar/soar-egress-block-response.md` criado
- [ ] `soar/soar-approval-timeout.md` criado
- [ ] `soar/soar-audit-anomaly.md` criado
- [ ] `docs/DECISIONS_FASE9.md` criado
- [ ] `docs/TRACKRECORD_FASE9.md` criado
- [ ] `MODULE_DOCUMENTATION_STATUS.md` atualizado com 3 novos crates
- [ ] `DESIGN_SYSTEM.md` atualizado com approval/priority tokens
- [ ] Gateway `README.md` atualizado com novos endpoints

### Gate Fase 2 (Produtividade)

- [ ] `crates/zylos-brief/README.md` criado
- [ ] `crates/zylos-model-config/README.md` criado
- [ ] `crates/zylos-computer-view/README.md` criado
- [ ] `crates/zylos-spaces/README.md` criado
- [ ] `playbooks/pb-morning-brief-config.md` criado
- [ ] `playbooks/pb-model-tier-strategy.md` criado
- [ ] `playbooks/pb-computer-view-setup.md` criado
- [ ] `playbooks/pb-spaces-onboarding.md` criado
- [ ] `playbooks/pop-brief-regenerate.md` criado
- [ ] `playbooks/pop-model-fallback.md` criado
- [ ] `playbooks/pop-computer-control-emergency.md` criado
- [ ] `playbooks/pop-space-page-recovery.md` criado
- [ ] `runbooks/rb-zylos-brief.md` criado
- [ ] `runbooks/rb-zylos-model-config.md` criado
- [ ] `runbooks/rb-zylos-computer-view.md` criado
- [ ] `runbooks/rb-zylos-spaces.md` criado
- [ ] `runbooks/rb-cockpit-v2-deploy.md` criado
- [ ] `soar/soar-computer-control-breach.md` criado
- [ ] `pot/pot-model-benchmark.md` criado
- [ ] `pot/pot-space-migration.md` criado
- [ ] `MODULE_DOCUMENTATION_STATUS.md` atualizado com 4 novos crates
- [ ] `DESIGN_SYSTEM.md` atualizado com computer view tokens
- [ ] Cockpit `README.md` atualizado com novos componentes/tabs
- [ ] `TRACKRECORD_FASE9.md` atualizado com entradas da Fase 2

### Gate Final (Fase 9 Completa)

- [ ] `HANDOFF_FASE9.md` completo com resumo executivo
- [ ] Todos os 7 READMEs de crates criados
- [ ] Todos os 6 playbooks estratégicos criados
- [ ] Todos os 7 POPs operacionais criados
- [ ] Todos os 7 runbooks técnicos criados
- [ ] Todos os 4 SOAR workflows criados
- [ ] Todos os 4 POTs táticos criados
- [ ] `DECISIONS_FASE9.md` com todas as ADRs
- [ ] `TRACKRECORD_FASE9.md` com log cronológico completo
- [ ] `MODULE_DOCUMENTATION_STATUS.md` com todos os 7 crates
- [ ] `DESIGN_SYSTEM.md` v2 completo
- [ ] Gateway + Cockpit READMEs atualizados
- [ ] `AGENTS.md` com auto-invocation rules para novas skills
- [ ] `CONCEITOS-DESENVOLVIMENTO-IA.md` atualizado
- [ ] Cross-linking bidirecional verificado entre todos os documentos

---

## 5. Skills Hermes para Automação Documental

As seguintes skills devem ser invocadas automaticamente durante a implementação:

| Skill | Trigger | Uso na Fase 9 |
|-------|---------|---------------|
| `handoff-author` | Final de fase | Gerar HANDOFF_FASE9.md |
| `sop-author` | Nova tarefa estratégica | Gerar playbooks pb-* |
| `pop-author` | Novo procedimento operacional | Gerar POPs pop-* |
| `runbook-builder` | Novo crate/módulo | Gerar runbooks rb-* |
| `soar-orchestration` | Novo workflow de segurança | Gerar SOARs soar-* |
| `spec-author` | Nova especificação técnica | Gerar specs antes de codar |
| `architecture-author` | Nova decisão arquitetural | Gerar ADRs em DECISIONS_FASE9.md |
| `testing-checklist-author` | Novo componente/crate | Gerar TESTING.md por módulo |
| `project-memory-author` | Início de fase | Gerar MEMORY.md, DECISIONS.md, TASKS.md |

---

## 6. Prioridade de Criação

### Imediato (ANTES de começar implementação)
1. `fase8-to-9-handoff.md` — contexto de transição
2. `docs/DECISIONS_FASE9.md` — log de decisões desde o início
3. `docs/TRACKRECORD_FASE9.md` — log cronológico desde o início
4. Templates de README/playbook/POP/runbook aprovados

### Durante Fase 1
5. READMEs dos 3 crates (approval, egress, audit-log)
6. Playbooks e POPs de segurança
7. Runbooks dos 3 crates
8. SOARs de segurança
9. Atualizar MODULE_DOCUMENTATION_STATUS.md

### Durante Fase 2
10. READMEs dos 4 crates (brief, model-config, computer-view, spaces)
11. Playbooks e POPs de produtividade
12. Runbooks dos 4 crates + cockpit deploy
13. SOAR de computer control
14. POTs de benchmark e migration
15. Atualizar DESIGN_SYSTEM.md, gateway/cockpit READMEs

### Ao Final
16. HANDOFF_FASE9.md completo
17. Verificação de cross-linking bidirecional
18. Atualizar AGENTS.md e CONCEITOS-DESENVOLVIMENTO-IA.md