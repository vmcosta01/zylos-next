# Playbook de Incident Response SOAR — Zylos-Next

> **Versão:** 1.0.0 | **Última revisão:** 2026-11-04 | **Classificação:** Confidencial
> **Framework:** SOAR (Security Orchestration, Automation and Response)
> **Proprietário:** Security Operations + SRE

---

## 1. Objetivo

Estabelecer processo estruturado de resposta a incidentes para a plataforma Zylos-Next,
integrando triagem automatizada via `zylos-triage`, orquestração via `zylos-soar`,
monitoramento via `zylos-sentinel`, e notificações via `notification-router`. O processo
define severidades P1–P4, thresholds de escalação humana, checklist de forensics,
e template de post-mortem com 5 Whys.

---

## 2. Escopo

### 2.1 Tipos de Incidentes Cobertos

| Categoria | Exemplos | Severidade Típica |
|---|---|---|
| Disponibilidade | Outage, degradação, timeout | P1–P2 |
| Segurança | Acesso não autorizado, data leak, privilege escalation | P1 |
| Integridade de Dados | Corrupção no memory-system, grafo petgraph inconsistente | P1–P2 |
| Performance | Latência elevada, degradação gradual | P2–P3 |
| Deploy | Canary rollback, migration failure | P2–P3 |
| Sandbox | Container escape, wasmtime crash | P1–P2 |
| Triage | Classificação incorreta sistemática | P3 |
| Comunicação | AG-UI stream drops, SSE failures | P2–P3 |

### 2.2 Componentes Envolvidos

- `zylos-triage`: Classificação de dois estágios (automated + human-in-the-loop).
- `zylos-soar`: Orquestração de workflows de resposta.
- `zylos-sentinel`: Health scanner e alert source.
- `zylos-gateway`: Fonte de logs de acesso e métricas HTTP/SSE.
- `zylos-memory-system`: Fonte de logs de auditoria e dados de agentes.
- `zylos-sandbox`: Fonte de logs de execução isolada.
- `zylos-live-comm`: Fonte de métricas de streaming.
- `notification-router`: Distribuição de alertas e escalação.

---

## 3. Definição de Severidade (P1–P4)

### P1 — Crítico (Resposta Imediata)

- **Definição:** Perda total de serviço, comprometimento de segurança, ou vazamento
  de dados pessoais.
- **Exemplos:**
  - Outage completo do `zylos-gateway` (> 5 min).
  - Comprometimento confirmado do `zylos-memory-system`.
  - Sandbox escape confirmado (container breakout).
  - Vazamento de dados pessoais (LGPD Art. 48).
  - Acesso não autorizado a secrets ou credenciais.
- **Resposta:** Imediata, 24/7. Incident Commander designado em até 5 min.
- **SLA:** Resolução ou mitigação em até 1 hora.
- **Escalação:** Automática para SRE Sênior + Security Lead + CTO.

### P2 — Alto (Resposta Urgente)

- **Definição:** Degradação significativa de serviço ou incidente de segurança
  contido.
- **Exemplos:**
  - Degradação de performance (> 50% aumento de latência p99).
  - Canary rollback durante deploy.
  - Falha de schema migration com dados inconsistentes.
  - `zylos-triage` classificando incorretamente > 10% das mensagens.
  - `zylos-sentinel` reportando falha em múltiplos componentes.
- **Resposta:** Em até 15 minutos durante horário comercial.
- **SLA:** Resolução ou mitigação em até 4 horas.
- **Escalação:** SRE Sênior + On-call engineer.

### P3 — Médio (Resposta Normal)

- **Definição:** Impacto limitado, workaround disponível.
- **Exemplos:**
  - Componente individual com error rate elevado mas dentro do SLA.
  - Feature específica degradada com fallback disponível.
  - `zylos-live-comm` com stream drops esporádicos (< 1%).
  - Logs de auditoria com gap temporário.
- **Resposta:** Em até 1 hora durante horário comercial.
- **SLA:** Resolução em até 24 horas.
- **Escalação:** SRE Pleno ou Júnior com supervisão.

### P4 — Baixo (Resposta Agendada)

- **Definição:** Inconveniência menor, sem impacto em produção.
- **Exemplos:**
  - Documentação desatualizada.
  - Alerta falso positivo recorrente (noise).
  - Melhoria de performance não urgente.
  - Limpeza de dados órfãos no memory-system.
- **Resposta:** Próxima sprint ou maintenance window.
- **SLA:** Resolução em até 2 semanas.
- **Escalação:** Backlog grooming.

---

## 4. Pré-requisitos

### 4.1 Ferramentas e Acessos

- [ ] `zylos-triage` configurado e treinado com dataset de incidentes.
- [ ] `zylos-soar` com playbooks de incident response carregados.
- [ ] `zylos-sentinel` com alertas configurados para todos os componentes.
- [ ] `notification-router` com canais de escalação definidos.
- [ ] Acessos SSH e kubectl com privilégios de troubleshooting.
- [ ] Acesso ao vault de secrets (read-only para forensics).
- [ ] Sala de guerra (War Room) configurada no canal de comunicação.

### 4.2 Contatos e Escalação

| Role | Canal Primário | Canal Secundário |
|---|---|---|
| SRE On-call | PagerDuty | WhatsApp |
| Security Lead | Signal | Phone |
| DPO (LGPD) | Email | Phone |
| CTO | Slack | Phone |
| Product Owner | Slack | WhatsApp |

---

## 5. Procedimento de Resposta a Incidentes

### 5.1 Fase 1 — Detecção e Triagem Automática

> ** SÊNIOR:** Supervisiona a triagem. Intervém se o `zylos-triage` classificar
> incorretamente. Decide se ativa manualmente um incidente que o triage não detectou.

> ** PLENO:** Monitora alertas do `zylos-sentinel`. Valida classificações do
> `zylos-triage`. Executa primeira resposta para P3/P4.

> ** JÚNIOR:** Monitora dashboards. Reporta anomalias. Executa runbooks pré-aprovados
> para P4.

```bash
# O zylos-sentinel detecta anomalia e envia para zylos-triage
# zylos-triage classifica em dois estágios:

# Estágio 1: Classificação automática
zylos-triage classify \
  --input=<alert-payload> \
  --stage=1 \
  --output=json

# Estágio 2: Validação (automática para P3/P4, human-in-the-loop para P1/P2)
zylos-triage classify \
  --input=<stage1-result> \
  --stage=2 \
  --human-review=<required|optional> \
  --output=json
```

### 5.2 Fase 2 — Classificação e Orquestração

O `zylos-triage` classifica o incidente e o `zylos-soar` inicia o workflow correspondente:

```bash
# zylos-soar inicia workflow de incident response
zylos-soar trigger \
  --workflow=incident-response \
  --input=<triage-output> \
  --severity=<P1|P2|P3|P4> \
  --auto-escalate=true
```

**Workflow `zylos-soar` para incidentes:**

1. **Coleta de contexto:** Logs, métricas, estado dos componentes.
2. **Notificação:** Envia alerta via `notification-router` para os canais apropriados.
3. **Diagnóstico inicial:** Executa checks automatizados.
4. **Mitigação:** Executa ações pré-aprovadas (restart, rollback, rate-limit).
5. **Escalação:** Se não resolvido dentro do SLA, escala automaticamente.

```bash
# Monitorar workflow SOAR em execução
zylos-soar status --workflow-id=<workflow-id>

# Verificar logs do workflow
zylos-soar logs --workflow-id=<workflow-id> --follow

# Intervir manualmente no workflow
zylos-soar intervene --workflow-id=<workflow-id> --action=<action-name>
```

### 5.3 Fase 3 — Thresholds de Escalação Humana

| Severidade | Escalação Automática | Escalação Humana Trigger |
|---|---|---|
| P1 | SRE Sênior + Security Lead + CTO | Imediata |
| P2 | SRE Sênior + On-call | Se não mitigado em 15 min |
| P3 | On-call | Se não resolvido em 2h |
| P4 | Backlog | Sprint planning |

```bash
# Escalação manual via notification-router
notification-router send \
  --channel=pagerduty \
  --severity=P1 \
  --message="Incidente confirmado: <descrição>" \
  --incident-id=<id> \
  --escalate-to=sre-lead,security-lead,cto

# Escalação automática via zylos-soar (configurado no workflow)
zylos-soar escalate \
  --workflow-id=<workflow-id> \
  --reason="<motivo>" \
  --target=<escalation-level>
```

### 5.4 Fase 4 — Investigação e Mitigação

> ** SÊNIOR:** Lidera investigação. Coordena múltiplos engenheiros. Toma decisões
> de mitigação (rollback, shutdown, isolation). Autoriza forensics se necessário.

> ** PLENO:** Executa comandos de diagnóstico. Coleta logs e métricas. Implementa
> mitigações aprovadas pelo Sênior. Documenta ações em tempo real.

> ** JÚNIOR:** Coleta dados. Monitora métricas. Executa runbooks pré-aprovados.
> Atualiza timeline do incidente.

```bash
# Coleta de logs centralizada
zylos-logs collect \
  --component=all \
  --time-range=<start>:<end> \
  --output=forensics-bundle.tar.gz

# Análise de métricas
zylos-metrics query \
  --component=<component> \
  --query=<promql> \
  --time-range=<start>:<end>

# Estado do memory-system
zylos-memory state-snapshot --output=<snapshot-file>

# Status dos workflows SOAR ativos
zylos-soar list --status=running
```

---

## 6. Forensics Checklist

Quando um incidente de segurança é confirmado (P1 ou P2 com componente de segurança):

### 6.1 Preservação de Evidências

- [ ] Isolar o componente afetado (network policy, shutdown graceful).
- [ ] Coletar snapshot de memória (se aplicável e autorizado).
- [ ] Preservar logs completos (application, access, audit, system).
- [ ] Capturar estado do `zylos-memory-system` (SQLite dump + WAL).
- [ ] Preservar imagens de container envolvidas.
- [ ] Capturar tráfego de rede (se aplicável e autorizado pelo DPO).
- [ ] Registrar timeline precisa de todas as ações.

### 6.2 Análise

- [ ] Identificar vetor de ataque (logs de acesso, audit trail).
- [ ] Determinar escopo do comprometimento (quais dados, quais agentes).
- [ ] Identificar persistência (backdoors, tokens vazados).
- [ ] Verificar se outros componentes foram afetados.
- [ ] Correlacionar com threat intelligence feeds.

### 6.3 Contenção e Erradicação

- [ ] Revogar credenciais comprometidas.
- [ ] Rotacionar secrets afetados no vault.
- [ ] Patch ou remover vulnerabilidade explorada.
- [ ] Limpar dados persistentes comprometidos.
- [ ] Validar integridade do `zylos-memory-system` (grafo petgraph).

```bash
# Forensics bundle
zylos-forensics collect \
  --incident-id=<id> \
  --components=<list> \
  --include-logs=true \
  --include-memory-dump=true \
  --include-network-capture=false \
  --output=forensics-<id>.tar.gz

# Validar integridade do grafo após incidente de segurança
zylos-memory graph-validate --deep --check-for-tampering

# Revogar tokens comprometidos
zylos-auth revoke --incident-id=<id> --scope=all

# Rotacionar secrets
zylos-vault rotate --incident-id=<id> --affected-secrets=<list>
```

---

## 7. Rollback

### 7.1 Rollback de Mitigação

Se a mitigação aplicada piorar a situação:

```bash
# Reverter ação de mitigação
zylos-soar rollback --workflow-id=<workflow-id> --step=<step-number>

# Desfazer isolamento de rede
zylos-network restore --incident-id=<id>

# Restaurar configuração anterior
zylos-config restore --component=<component> --version=<previous>
```

### 7.2 Rollback de Deploy Relacionado

Se o incidente está correlacionado com um deploy recente:

```bash
# Rollback do último deploy
zylos-canary rollback --deployment=<deploy-id> --reason="incident-<id>"

# Rollback de schema migration
zylos-memory migrate rollback --target=<previous-version>
```

> ** SÊNIOR:** Decide rollback. Comunica stakeholders. Inicia post-mortem.

> ** PLENO:** Executa rollback. Valida estado estável. Coleta dados para forensics.

> ** JÚNIOR:** Monitora métricas pós-rollback. Atualiza timeline.

---

## 8. Verificação e Encerramento

### 8.1 Critérios de Resolução

- [ ] Causa raiz identificada e documentada.
- [ ] Mitigação aplicada e estável por 30+ minutos.
- [ ] Métricas normalizadas (dentro do baseline).
- [ ] Zero alertas relacionados por 15 minutos.
- [ ] Forensics concluído (se aplicável).
- [ ] Dados do `zylos-memory-system` íntegros.
- [ ] Comunicação LGPD executada (se aplicável).

### 8.2 Procedimento de Encerramento

```bash
# Encerrar workflowzylos-soar close --workflow-id=<workflow-id> --resolution=<resolution>

# Atualizar status do incidente
zylos-incident update --id=<incident-id> --status=resolved

# Enviar comunicação de encerramento
notification-router send \
  --channel=all-stakeholders \
  --message="Incidente <id> resolvido. Post-mortem agendado para <data>." \
  --incident-id=<id>

# Agendar post-mortem (obrigatório para P1/P2)
zylos-incident schedule-postmortem \
  --id=<incident-id> \
  --deadline=72h \
  --participants=sre,security,product,management
```

---

## 9. Post-Mortem Template — 5 Whys

### 9.1 Informações Básicas

| Campo | Valor |
|---|---|
| Incident ID | |
| Data/Hora Início | |
| Data/Hora Resolução | |
| Duração Total | |
| Severidade | |
| Incident Commander | |
| Autor do Post-Mortem | |

### 9.2 Resumo Executivo

_Descrição concisa do que aconteceu, impacto e resolução. Máximo 5 linhas._

### 9.3 Timeline

| Horário | Evento |
|---|---|
| HH:MM | Primeiro alerta do `zylos-sentinel` |
| HH:MM | `zylos-triage` classifica como P_ |
| HH:MM | Incident Commander designado |
| HH:MM | War room aberta |
| HH:MM | Mitigação aplicada |
| HH:MM | Incidente resolvido |
| HH:MM | Comunicação enviada |

### 9.4 Impacto

- **Usuários afetados:** _Número e porcentagem_
- **Dados afetados:** _Tipo e volume_
- **Receita perdida:** _Estimativa_
- **SLA violado:** _Qual e por quanto_
- **LGPD notificação:** _Sim/Não, data_

### 9.5 Os 5 Porquês

#### Problema: _[descrever o problema raiz]_

1. **Por que** o problema ocorreu?
   - _Resposta 1_
2. **Por que** [resposta 1]?
   - _Resposta 2_
3. **Por que** [resposta 2]?
   - _Resposta 3_
4. **Por que** [resposta 3]?
   - _Resposta 4_
5. **Por que** [resposta 4]?
   - _**Causa raiz:** Resposta 5_

### 9.6 Action Items

| Ação | Responsável | Deadline | Prioridade |
|---|---|---|---|
| _[ação corretiva]_ | _[nome]_ | _[data]_ | P_ |
| _[ação preventiva]_ | _[nome]_ | _[data]_ | P_ |
| _[melhoria de monitoramento]_ | _[nome]_ | _[data]_ | P_ |
| _[atualização de runbook]_ | _[nome]_ | _[data]_ | P_ |

### 9.7 Lições Aprendidas

- **O que funcionou bem?**
  - _[exemplo: zylos-triage detectou em < 1 min]_
- **O que poderia ser melhorado?**
  - _[exemplo: comunicação com stakeholders demorou]_
- **O que faremos diferente na próxima vez?**
  - _[exemplo: automatizar ação X via zylos-soar]_

---

## 10. Integração com notification-router

### 10.1 Canais de Notificação

| Severidade | Canal Primário | Canal Secundário | Delay de Escalação |
|---|---|---|---|
| P1 | PagerDuty + War Room | Phone (CTO + Security) | 5 min |
| P2 | PagerDuty + Slack | WhatsApp (On-call) | 15 min |
| P3 | Slack | Email | 2 horas |
| P4 | Slack (canal de backlog) | — | Próxima sprint |

### 10.2 Configuração de Alertas

```bash
# Configurar rota de notificação para incidentes P1
notification-router route create \
  --name=p1-incident \
  --filter="severity=P1" \
  --channels=pagerduty,war-room,phone \
  --escalation-delay=5m \
  --escalation-targets=sre-lead,security-lead,cto

# Configurar rota para P2
notification-router route create \
  --name=p2-incident \
  --filter="severity=P2" \
  --channels=pagerduty,slack \
  --escalation-delay=15m \
  --escalation-targets=sre-lead,on-call

# Configurar digest diário de P3/P4
notification-router route create \
  --name=p3p4-digest \
  --filter="severity=P3 OR severity=P4" \
  --channels=slack \
  --digest=daily \
  --digest-time=09:00
```

---

## 11. Integração com zylos-sentinel

### 11.1 Health Checks como Fonte de Incidentes

O `zylos-sentinel` é a fonte primária de detecção de incidentes de disponibilidade
e performance:

```bash
# Configurar sentinel para gerar alertas compatíveis com zylos-triage
zylos-sentinel config alert-format=zylos-triage-compatible

# Definir thresholds por componente
zylos-sentinel config set \
  --component=zylos-gateway \
  --metric=http_5xx_rate \
  --threshold=0.01 \
  --severity-mapping="0.01:P3,0.05:P2,0.1:P1"

zylos-sentinel config set \
  --component=zylos-gateway \
  --metric=latency_p99 \
  --threshold=2.0 \
  --severity-mapping="2.0:P3,3.0:P2,5.0:P1"

# Integrar com zylos-soar
zylos-sentinel config set \
  --component=all \
  --on-alert=trigger-zylos-soar \
  --soar-workflow=incident-response
```

### 11.2 Post-Incident Health Validation

Após resolução de incidente, validar que o `zylos-sentinel` não reporta mais anomalias:

```bash
zylos-sentinel full-health-check \
  --cluster=production \
  --components=all \
  --expect=healthy \
  --timeout=300
```

---

## 12. Referências

- `zylos-triage` — Documentação de classificação de dois estágios
- `zylos-soar` — Documentação de orquestração de workflows
- `zylos-sentinel` — Documentação de health scanning e alertas
- `notification-router` — Skill de roteamento de notificações
- `zylos-gateway` — Documentação de logs de acesso e métricas
- `zylos-memory-system` — Documentação de auditoria e forensics
- Disaster Recovery playbook — `/playbooks/disaster-recovery.md`
- Canary Deployment runbook — `/runbooks/canary-deployment.md`
- LGPD — Lei 13.709/2018, Art. 46–48
- NIST SP 800-61 Rev. 2 — Computer Security Incident Handling Guide