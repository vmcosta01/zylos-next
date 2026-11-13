# 📚 Playbooks Estratégicos — Zylos-Next

**Última Atualização:** 2 de novembro de 2026
**Escopo:** Guias de tomada de decisão, governança e fluxo estratégico
**Público:** Tech Lead, Arquiteto, Product Owner, SRE, Auditores, Devs Todos os Níveis

---

## 🎯 Diferença: Playbook vs Runbook

| Dimensão | Playbook | Runbook |
|----------|----------|---------|
| **Foco** | Estratégia, decisão, governança ("o quê", "quando", "quem") | Operação, execução técnica ("como fazer") |
| **Público Primário** | Seniores, Tech Lead, PO | Plenos, Júniores, Agentes Orquestradores |
| **Conteúdo** | Papéis, SLAs, critérios de escalação, matriz de decisão | Comandos, scripts, passo-a-passo, validação |
| **Atualização** | Quando há mudança estratégica ou post-mortem | Quando há mudança operacional ou técnica |
| **Exemplo** | "Quando declarar SEV-1" | "Como restartar o gateway em <5min" |

**Regra:** Todo playbook referencia runbooks operacionais correspondentes. Nunca crie um sem o outro.

---

## 🗂️ Índice de Playbooks

| Domínio | Arquivo | Escopo | Status |
|---------|---------|--------|--------|
| **Estratégico Geral** | [`PLAYBOOK.md`](../PLAYBOOK.md) | Governança global, RACI, valores, gates de fase | ✅ v1.0 |
| **Resposta a Incidentes** | [`playbooks/incident-response.md`](./incident-response.md) | Detecção, triagem, contenção, recuperação, post-mortem | ✅ Novo |
| **Deploy e Release** | [`playbooks/deploy-release.md`](./deploy-release.md) | Gates de release, rollback, versionamento, feature flags | ✅ Novo |
| **Onboarding de Devs** | [`playbooks/onboarding-devs.md`](./onboarding-devs.md) | Jornada 4 semanas, gates de competência, mentoria | ✅ Novo |
| **Auditoria Adversarial** | [`playbooks/adversarial-audit.md`](./adversarial-audit.md) | Dupla auditoria, refutação, aprovação de artefatos críticos | ✅ Novo |
| **SOP Triagem Incidentes** | [`playbooks/sop-incident-triage.md`](./sop-incident-triage.md) | Procedimento padrão de triagem, classificação SEV, roteamento | ✅ Novo |
| **POP Restart Gateway** | [`playbooks/pop-gateway-restart.md`](./pop-gateway-restart.md) | Procedimento operacional de restart seguro com rollback | ✅ Novo |
| **SOP Deploy Canary** | [`playbooks/sop-canary-deploy.md`](./sop-canary-deploy.md) | Deploy canary com promoção gradual e rollback automático | ✅ Novo |
| **POP Docker Sandbox** | [`playbooks/pop-docker-sandbox.md`](./pop-docker-sandbox.md) | Criação, monitoramento e limpeza de containers sandbox | ✅ Novo |
| **POP Backup/Restore DB** | [`playbooks/pop-backup-restore.md`](./pop-backup-restore.md) | Backup incremental/full e restore do SQLite WAL | ✅ Novo |
| **POP Evidence Ledger** | [`playbooks/pop-evidence-ledger-query.md`](./pop-evidence-ledger-query.md) | Consulta, integridade e exportação de evidências | ✅ Novo |
| **POP Sentinel Alert** | [`playbooks/pop-sentinel-alert-investigation.md`](./pop-sentinel-alert-investigation.md) | Investigação e classificação de alertas AIOps | ✅ Novo |

---

## 🔗 Matriz Playbook → Runbook

| Playbook | Runbooks Relacionados |
|----------|----------------------|
| Incident Response | `zylos-gateway.md`, `zylos-docker.md`, `zylos-browser.md`, `zylos-mobile.md`, `zylos-vlm-ocr.md` |
| Deploy e Release | `zylos-gateway.md`, `zylos-docker.md`, `fase*-*.md` (todos) |
| Onboarding | `fase0-foundation.md`, `README.md` (índice completo) |
| Auditoria Adversarial | Todos os runbooks de fase + `evals/audits/*` |
| Resposta AIOps | [`aiops-anomaly-response.md`](aiops-anomaly-response.md) | Estratégia de resposta a anomalias detectadas pelo Sentinel | ✅ Novo |
| Cadeia de Custódia | [`evidence-chain-of-custody.md`](evidence-chain-of-custody.md) | Governança de preservação e exportação de evidências | ✅ Novo |
| Orquestração Workflows | [`workflow-orchestration.md`](workflow-orchestration.md) | Decisões de arquitetura DAG e resposta a falhas sistêmicas | ✅ Novo |
| Resolução Conflitos | [`conflict-resolution.md`](conflict-resolution.md) | Estratégia de seleção de resolução multi-agente | ✅ Novo |
| Estratégico Geral | Todos os playbooks e runbooks |

---

## 📋 Convenções

### Estrutura de Cada Playbook
Todo playbook segue exatamente esta estrutura:
1. **Contexto Organizacional** — missão, valores, papéis (RACI)
2. **Fluxo Estratégico** — matrizes de decisão, critérios de entrada/saída, SLAs
3. **Governança e Comunicação** — canais, templates, rituais, escalação
4. **Ferramentas e Recursos** — stack, acessos, checklists
5. **Adaptação por Nível** — orientações específicas para Senior/Pleno/Júnior
6. **Referências Cruzadas** — links para runbooks, specs, handoffs

### Nomenclatura
- Playbook estratégico geral: `PLAYBOOK.md` (raiz)
- Playbooks de domínio: `playbooks/{dominio}.md`
- Runbooks operacionais: `runbooks/{componente}.md`

### Status
- ✅ Completo — playbook escrito e validado
- ⏳ Pendente — playbook draft, aguardando revisão
- ❌ Bloqueado — dependência estratégica não resolvida

---

## 🔄 Manutenção

Este índice deve ser atualizado sempre que:
- Um novo playbook for criado
- Um post-mortem revelar necessidade de novo domínio
- Mudança organizacional alterar papéis ou fluxos
- Playbook existente for revisado significativamente

**Responsável:** Tech Lead (revisão mensal obrigatória)

---

## ⚠️ Gaps Conhecidos

| Domínio | Prioridade | Justificativa |
|---------|-----------|---------------|
| Capacity Planning | Média | Escala horizontal de agentes ainda não documentada estrategicamente |
| Security Incident | Alta | Subconjunto de incident-response, mas precisa de protocolo específico para breaches |
| Vendor/Dependency Pivot | Baixa | Soberania reduz risco, mas plano B para crates críticos falta |
| Stakeholder Communication | Média | Templates de comunicação externa além de status page |

**Ação:** Criar estes playbooks durante Fase 7 conforme necessidade real surgir.