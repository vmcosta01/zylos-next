# RB-Production-Incident-Response — Runbook de Resposta a Incidentes em Produção

**Versão:** 1.0.0
**Data:** 2026-11-12
**Plano Relacionado:** [IMPLEMENTATION_PLAN_FASE11_MEGA.md](../docs/IMPLEMENTATION_PLAN_FASE11_MEGA.md) §11.4 T11.4.10
**SOAR Workflow:** [`soar-production-alerts.md`](../docs/soar/soar-production-alerts.md)
**Playbooks Relacionados:** [`pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md), [`pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md)
**POPs Relacionados:** [`pop-health-check-validation.md`](./pop-health-check-validation.md), [`pop-wasm-source-maps.md`](./pop-wasm-source-maps.md)
**Público:** SRE, On-call Engineer, Tech Lead
**Tempo Estimado:** Variável (depende da severidade)

---

## 1. Visão Geral

Este runbook define o procedimento completo de resposta a incidentes em produção do gateway e cockpit Zylos. Cobre detecção, triagem, diagnóstico, remediação, comunicação e post-mortem. Integra-se com os alertas automáticos do SOAR (`soar-production-alerts.md`) e ferramentas de observabilidade (Sentry, Axiom).

**Objetivo:** Restaurar serviço em <30min para SEV-1, <2h para SEV-2, com comunicação clara e documentação pós-incidente.

---

## 2. Nível Senior (Tech Lead / SRE Senior)

### Matriz RACI

| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|------------|-------------|-----------|----------|
| Detecção automática | Sentinel/SOAR | SRE | Dev Pleno | All |
| Triagem inicial | On-call Engineer | Tech Lead | SRE | PM |
| Diagnóstico root cause | Dev Senior | Tech Lead | SRE | PM |
| Remediação imediata | On-call Engineer | Tech Lead | Dev Senior | All |
| Comunicação stakeholders | Tech Lead | Tech Lead | PM | All |
| Post-mortem | Dev Senior + SRE | Tech Lead | All | All |
| Implementação de fixes | Dev Pleno | Tech Lead | Dev Senior | PM |

### Critérios de Severidade

| Severidade | Definição | Response Time | Exemplo |
|-----------|-----------|--------------|---------|
| SEV-1 | Serviço completamente indisponível ou data loss | <5 min ack, <30 min restore | Health check down >3min, corrupção de dados |
| SEV-2 | Funcionalidade crítica degradada (>50% usuários afetados) | <15 min ack, <2h restore | Error rate >5%, latency p95 >10s |
| SEV-3 | Funcionalidade não-crítica degradada ou parcial | <30 min ack, <4h restore | Error rate 1-5%, feature específica quebrada |
| SEV-4 | Problema cosmético ou warning sem impacto funcional | Next business day | UI glitch, log noise, deprecated dep warning |

### Gates de Decisão

| Gate | Condição | Ação |
|------|----------|------|
| G1: Auto-remediation aprovada? | SOAR config `auto_restart_approved: true` E <2 restarts nas últimas 2h | Executar restart automático |
| G2: Rollback necessário? | Deploy recente (<24h) correlacionado com início do incidente | Rollback para versão anterior |
| G3: Escalar para PagerDuty? | SEV-1 sem ack em 5min OU SEV-2 sem resolução em 1h | Acionar on-call via PagerDuty |
| G4: Comunicar stakeholders? | SEV-1/SEV-2 confirmado | Notificar #alerts-critical + email para PM |
| G5: Post-mortem obrigatório? | SEV-1 ou SEV-2 | Criar doc post-mortem em <48h |

---

## 3. Nível Pleno (On-call Engineer / Dev Pleno)

### Fluxograma de Resposta

```
ALERTA RECEBIDO (Slack/PagerDuty/Sentry)
 │
 ├─ Verificar severidade (SEV-1/2/3/4)
 │ SEV-3/4 → Criar ticket Linear → Priorizar no backlog → END
 │ SEV-1/2 ↓
 │
 ├─ ACK alerta (Slack thread ou PagerDuty) dentro do SLA
 │
 ├─ Verificar health checks
 │ curl https://prod-url/api/v1/health
 │ curl https://prod-url/api/v1/cockpit/health
 │ HTTP 200 → Investigar métricas (Axiom/Sentry)
 │ HTTP 5xx/timeout ↓
 │
 ├─ Verificar deploy recente (<24h?)
 │ SIM → ROLLBACK IMEDIATO (§Rollback)
 │ NÃO ↓
 │
 ├─ Verificar logs no Axiom
 │ query: level=ERROR | sort by _time desc | limit 50
 │ Identificar padrão: panic? timeout? DB error? auth fail?
 │
 ├─ Verificar Sentry
 │ Novo evento com stack trace legível?
 │ SIM → Identificar função/módulo causador
 │ NÃO → Verificar se source maps foram uploaded (pop-wasm-source-maps.md)
 │
 ├─ Tentar remediação imediata
 │ Restart gateway? → systemctl restart zylos-gateway
 │ Limpar cache? → redis-cli FLUSHALL (se aplicável)
 │ Scale replicas? → kubectl scale deployment/zylos-gateway --replicas=3
 │
 ├─ Validar restauração
 │ curl https://prod-url/api/v1/health → HTTP 200?
 │ SIM → Monitorar 15min → Comunicar resolução → END
 │ NÃO → ESCALAR para Tech Lead via PagerDuty
```

### Tabela de Escalação

| Condição | Quem Contatar | Canal | Timeout |
|----------|--------------|-------|---------|
| SEV-1 sem ack em 5min | Tech Lead on-call | PagerDuty | Imediato |
| SEV-2 sem resolução em 1h | Tech Lead + Dev Senior | PagerDuty + Slack | Imediato |
| Suspeita de security breach | Security Reviewer | Slack #security + PagerDuty | Imediato |
| Data loss ou corrupção | Tech Lead + DBA | PagerDuty | Imediato |
| Falso positivo recorrente | SRE | Slack #infra | 24h |

### Checklist de Comunicação

- [ ] ACK alerta no canal original (Slack thread ou PagerDuty)
- [ ] Se SEV-1/2: postar em #alerts-critical com resumo inicial
- [ ] Atualizar status a cada 15min durante incidente ativo
- [ ] Após resolução: postar resumo com causa raiz e tempo de downtime
- [ ] Se SEV-1/2: enviar email para PM e stakeholders em <1h
- [ ] Criar ticket Linear com tag `incident` e link para post-mortem

### Comandos de Diagnóstico Rápido

```bash
# Health checks
curl -s https://prod-url/api/v1/health | python3 -m json.tool
curl -s https://prod-url/api/v1/cockpit/health | python3 -m json.tool
curl -s https://prod-url/gateway/ready | python3 -m json.tool

# Logs recentes no Axiom (via CLI ou dashboard)
# Query: service="zylos-gateway" AND level="ERROR" | sort by _time desc | limit 20

# Métricas de latência e error rate
# Query Axiom: summarize count() by status, bin(_time, 5m) | where status >= 500

# Verificar processos do gateway (se acesso SSH disponível)
ps aux | grep zylos-gateway
journalctl -u zylos-gateway --since "30 min ago" --no-pager

# Kubernetes (se aplicável)
kubectl get pods -l app=zylos-gateway
kubectl logs -l app=zylos-gateway --tail=100
kubectl top pods -l app=zylos-gateway
```

---

## 4. Nível Júnior (Dev Júnior / Estagiário)

### Passo a Passo: Triagem Inicial

#### Passo 1: Verificar alerta (2 min)
- Ler mensagem no Slack/PagerDuty
- Identificar severidade (SEV-1/2/3/4) conforme matriz §2
- Se SEV-3/4: criar ticket Linear e parar aqui
- Se SEV-1/2: continuar para Passo 2

#### Passo 2: ACK e notificar (1 min)
- Responder na thread do Slack: "🔍 Investigando [SEV-X] - [breve descrição]"
- Se PagerDuty: clicar "Acknowledge"
- Se SEV-1 e sozinho: acionar Tech Lead via PagerDuty IMEDIATAMENTE

#### Passo 3: Health check rápido (2 min)
```bash
curl -s https://prod-url/api/v1/health
```
**Output esperado:** `{"status": "ok", ...}`
**Se falhar:** Anotar status code e mensagem de erro. Continuar para Passo 4.

#### Passo 4: Verificar deploy recente (2 min)
```bash
# GitHub: verificar último merge/deploy
gh run list --repo vmcosta/zylos-next --limit 5
```
**Se deploy <24h:** Provável regressão. Escalar para Dev Senior/Tech Lead com link do deploy.
**Se nenhum deploy recente:** Continuar para Passo 5.

#### Passo 5: Coletar evidências (5 min)
- Screenshot do Sentry (novo evento?)
- Screenshot do Axiom (picos de error/latency?)
- Output dos health checks
- Timestamp de início do alerta

#### Passo 6: Escalar com contexto (2 min)
Postar no #alerts-critical ou responder thread existente:
```
🚨 [SEV-X] Incidente em investigação
- Início: [timestamp]
- Sintoma: [descrição breve]
- Health check: [status]
- Deploy recente: [sim/não, link]
- Evidências: [links/screenshots]
- Próximo passo: [diagnóstico/rollback/escalar]
```

### Regras de Quando NÃO Agir e Escalar Imediatamente

- ❌ **NÃO** tente corrigir código em produção diretamente
- ❌ **NÃO** faça rollback sem confirmar que deploy recente é a causa
- ❌ **NÃO** ignore alertas SEV-1 mesmo que pareçam falso positivo
- ❌ **NÃO** reinicie serviços sem verificar logs primeiro (pode perder evidências)
- ✅ **SEMPRE** documente tudo na thread do incidente
- ✅ **SEMPRE** escale SEV-1/2 se não tiver certeza da causa raiz
- ✅ **SEMPRE** preserve logs/evidências antes de qualquer ação destrutiva

### Glossário

| Termo | Definição |
|-------|-----------|
| SEV-1/2/3/4 | Classificação de severidade (1=crítico, 4=cosmético) |
| ACK | Reconhecimento formal de que alerta foi visto e está sendo tratado |
| Rollback | Reverter para versão anterior estável do software |
| Post-mortem | Documento analisando causa raiz, impacto e ações preventivas |
| MTTR | Mean Time To Recovery — tempo médio para restaurar serviço |
| False positive | Alerta disparado sem incidente real |

---

## 5. Procedimento de Rollback

### Trigger Conditions
- Deploy recente (<24h) correlacionado temporalmente com início do incidente
- Health check falha consistentemente após deploy
- Error rate spike coincide com timestamp do deploy

### Passos de Rollback

#### Opção A: Cloudflare Pages (Cockpit WASM)
```bash
# Listar deployments anteriores
wrangler pages deployment list --project-name=zylos-cockpit

# Promover deployment anterior para produção
wrangler pages deployment promote <deployment-id> --project-name=zylos-cockpit
```

#### Opção B: Gateway (VPS/Kubernetes)
```bash
# VPS com systemd
sudo systemctl stop zylos-gateway
git checkout <tag-anterior>
cargo build --release -p zylos-gateway
sudo cp target/release/zylos-gateway /usr/local/bin/
sudo systemctl start zylos-gateway

# Kubernetes
kubectl rollout undo deployment/zylos-gateway
kubectl rollout status deployment/zylos-gateway
```

### Validação Pós-Rollback
- [ ] `curl https://prod-url/api/v1/health` → HTTP 200
- [ ] `curl https://prod-url/api/v1/cockpit/health` → HTTP 200
- [ ] Sentry: zero novos eventos em 5min
- [ ] Axiom: error rate retorna ao baseline
- [ ] Comunicar resolução na thread do incidente

---

## 6. Post-Mortem Template

Criar documento em `docs/postmortems/YYYY-MM-DD-<titulo>.md` dentro de 48h para SEV-1/2:

```markdown
# Post-Mortem: [Título Descritivo]

**Data:** YYYY-MM-DD
**Severidade:** SEV-X
**Duração:** HH:MM - HH:MM (X minutos)
**Impacto:** [Número de usuários/serviços afetados]
**Autor:** [Nome]

## Resumo
[2-3 frases descrevendo o que aconteceu]

## Timeline
| Hora | Evento |
|------|--------|
| HH:MM | Primeiro alerta disparado |
| HH:MM | ACK por [nome] |
| HH:MM | Causa raiz identificada |
| HH:MM | Remediação aplicada |
| HH:MM | Serviço restaurado |

## Causa Raiz
[Descrição técnica detalhada do que causou o incidente]

## Impacto
- Usuários afetados: X
- Downtime total: X minutos
- Dados perdidos: Sim/Não (detalhes)
- Receita perdida: R$ X (se aplicável)

## O Que Funcionou Bem
- [Lista de ações/processos que ajudaram na resolução]

## O Que Pode Melhorar
- [Lista de gaps identificados na resposta]

## Ações Preventivas
| Ação | Owner | Prazo | Ticket |
|------|-------|-------|--------|
| [Descrição] | [Nome] | YYYY-MM-DD | LINEAR-XXX |

## Lições Aprendidas
- [Insights para prevenir incidentes similares]
```

---

## 7. Referências Cruzadas

### SOAR Workflows
- [`soar-production-alerts.md`](../docs/soar/soar-production-alerts.md) — Regras de alerta automático

### Playbooks
- [`pb-sentry-integration.md`](../playbooks/pb-sentry-integration.md) — Error tracking setup
- [`pb-axiom-logging-setup.md`](../playbooks/pb-axiom-logging-setup.md) — Logs estruturados setup
- [`pb-cloudflare-pages-setup.md`](../playbooks/pb-cloudflare-pages-setup.md) — Deploy/rollback Cloudflare

### POPs
- [`pop-health-check-validation.md`](./pop-health-check-validation.md) — Validação de health checks
- [`pop-wasm-source-maps.md`](./pop-wasm-source-maps.md) — Source maps para debugging
- [`pop-wasm-build-troubleshoot.md`](./pop-wasm-build-troubleshoot.md) — Diagnóstico de build WASM

### Dashboards
- Sentry: https://sentry.io → zylos-gateway / zylos-cockpit
- Axiom: https://app.axiom.co → zylos-gateway-logs
- Cockpit tab Sentinel: anomalias em tempo real

### Canais de Comunicação
- Slack: `#alerts-critical` (SEV-1/2), `#alerts-zylos` (SEV-3), `#dev` (geral)
- PagerDuty: Tech Lead on-call
- Linear: Board "Incidents" para tickets pós-incidente

---

*Documento gerado conforme skill `runbook-builder`. Próxima revisão: após primeiro incidente real em produção ou drill simulado.*