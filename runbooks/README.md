# 📚 Runbooks & Playbooks — Zylos-Next (Fases 0–7)

**Última Atualização:** 2 de novembro de 2026
**Cobertura:** Fase 0 → Fase 7 (completa)
**Estrutura:** 3 níveis por runbook (Senior / Pleno / Júnior)

---

## 🗂️ Índice de Runbooks

### Runbooks de Fase

| Fase | Arquivo | Crate(s) | Testes | Status |
|------|---------|----------|--------|--------|
| **0** | [`runbooks/fase0-foundation.md`](./fase0-foundation.md) | Workspace Cargo, toolchain | ✅ | Completo |
| **1** | [`runbooks/fase1-bot-system.md`](./fase1-bot-system.md) | `zylos-bot-system` | 21/21 ✅ | Completo |
| **2** | [`runbooks/fase2-memory-system.md`](./fase2-memory-system.md) | `zylos-memory-system` | 28/28 ✅ | Completo |
| **3** | [`runbooks/fase3-live-comm.md`](./fase3-live-comm.md) | `zylos-live-comm`, `zylos-mobile` | 20/20 ✅ | Completo |
| **3** | [`runbooks/fase3-verification-proof.md`](./fase3-verification-proof.md) | `zylos-verification` | 42/42 ✅ | Completo |
| **4** | [`runbooks/fase4-sandbox-isolation.md`](./fase4-sandbox-isolation.md) | `zylos-sandbox`, `zylos-search-mcp` | 21/21 ✅ | Completo |
| **5** | [`runbooks/fase5-skills-registry.md`](./fase5-skills-registry.md) | `zylos-skills` | 107/107 ✅ | Completo |
| **6** | [`runbooks/fase6-live-communication.md`](./fase6-live-communication.md) | `zylos-live-comm`, `zylos-cockpit` | 60/60 ✅ | Completo |

### Runbooks de Sprint (Fase 4)

| Sprint | Arquivo | Tópico | Status |
|--------|---------|--------|--------|
| 4.2.1 | [`runbooks/browser-headless-anti-detect.md`](./browser-headless-anti-detect.md) | Browser anti-detecção | 7/7 ✅ |
| 4.2.2 | [`runbooks/webmcp-spec-compliance.md`](./webmcp-spec-compliance.md) | WebMCP spec compliance | 6/6 ✅ |
| 4.2.3 | [`runbooks/parallel-search-mcp.md`](./parallel-search-mcp.md) | Parallel Search MCP | 8/8 ✅ |

### Runbooks de Crate/App Individual

| Componente | Arquivo | Tipo | Status |
|------------|---------|------|--------|
| `zylos-gateway` | [`runbooks/zylos-gateway.md`](./zylos-gateway.md) | App (API Gateway) | ✅ Novo |
| `zylos-docker` | [`runbooks/zylos-docker.md`](./zylos-docker.md) | Crate (Sandbox Docker) | ✅ Novo |
| `zylos-browser` | [`runbooks/zylos-browser.md`](./zylos-browser.md) | Crate (Browser Anti-Detect) | ✅ Novo |
| `zylos-mobile` | [`runbooks/zylos-mobile.md`](./zylos-mobile.md) | Crate (Mobile MCP) | ✅ Novo |
| `zylos-core` | [`runbooks/zylos-core.md`](./zylos-core.md) | Crate (Tipos Compartilhados) | ✅ Novo |
| `zylos-vlm-ocr` | [`runbooks/zylos-vlm-ocr.md`](./zylos-vlm-ocr.md) | Crate (OCR Sidecar) | ✅ Novo |

### Runbooks Específicos de Task

| Task | Arquivo | Tópico | Status |
|------|---------|--------|--------|
| VLM OCR | [`runbooks/vlm-ocr-sidecar.md`](./vlm-ocr-sidecar.md) | SVG parser + OCR engine | 17/17 ✅ |

---

## 🎯 Como Usar Este Índice

### Para Devs Seniores
Comece pela seção "NÍVEL SENIOR" de cada runbook. Foco em:
- Decisões arquiteturais e alternativas rejeitadas
- Benchmarks e critérios de performance
- Auditorias de segurança e isolamento
- Contratos de API e invariants

### Para Devs Plenos
Comece pela seção "NÍVEL PLENO". Foco em:
- Procedimentos de implementação e validação
- Cenários de integração multi-crate
- Checklists de testes e qualidade
- Otimização de hot paths

### Para Devs Júniores
Comece pela seção "NÍVEL JÚNIOR". Foco em:
- Execução passo-a-passo com comandos exatos
- Glossário de termos técnicos
- Regras claras de quando escalar
- Exemplos executáveis sem modificação

---

## 🔗 Dependências Entre Fases

```
Fase 0 (Foundation)
    ↓
Fase 1 (BOT_SYSTEM) ──────────────────────┐
    ↓                                      │
Fase 2 (MEMORY_SYSTEM) ←──────────────────┤
    ↓                                      │
Fase 3 (VERIFICATION_PROOF) ←─────────────┤
    ↓                                      │
Fase 4 (SANDBOX_ISOLATION) ←──────────────┘
```

**Regra:** Cada fase depende da anterior estar completa e auditada. Não pule fases.

---

## 📋 Convenções

### Estrutura de Cada Runbook
Todo runbook segue exatamente esta estrutura:
1. **Visão Geral** — contexto para todos os níveis
2. **Nível Senior** — arquitetura, decisões, auditorias, benchmarks
3. **Nível Pleno** — implementação, integração, checklists
4. **Nível Júnior** — execução guiada, glossário, escalonamento
5. **Referências Cruzadas** — links para Master Plan, auditorias, manifesto

### Nomenclatura
- Runbooks de fase: `fase{N}-{nome-sistema}.md`
- Runbooks de sprint: `{nome-tarefa}.md`
- Auditorias: `evals/audits/fase{N}-audit{-v2}.md`

### Status
- ✅ Completo — runbook escrito e validado
- ⏳ Pendente — runbook escrito mas auditoria/tests faltando
- ❌ Bloqueado — dependência externa não resolvida

---

## 📊 Métricas Globais

| Métrica | Valor |
|---------|-------|
| Runbooks de fase | 8 |
| Runbooks de sprint | 3 |
| Runbooks de crate/app | 6 |
| Runbooks de task | 1 |
| **Total de runbooks** | **18** |
| Crates documentados | 12/12 |
| Apps documentados | 1/4 (gateway; cli/daemon/cockpit cobertos por fase6) |
| Níveis de profundidade | 3 (Senior/Pleno/Júnior) |
| Auditorias realizadas | 12 (Fase 0-6 completas) |
| Total de testes cobertos | 280+ |

---

## 🔄 Manutenção

Este índice deve ser atualizado sempre que:
- Um novo runbook for criado
- Uma fase for concluída ou revisada
- Métricas de testes mudarem significativamente
- Novos sprints forem adicionados
- Novos crates/apps forem criados

**Responsável:** Tech Lead (Senior) ou qualquer dev que crie/modifique um runbook.

---

## 🔗 Playbooks Estratégicos Relacionados

Runbooks são guias operacionais ("como fazer"). Para decisões estratégicas ("o quê", "quando", "quem"), consulte os playbooks:

| Domínio | Playbook | Quando Consultar |
|---------|----------|------------------|
| Resposta a Incidentes | [`playbooks/incident-response.md`](../playbooks/incident-response.md) | Antes de executar runbooks de troubleshooting em produção |
| Deploy e Release | [`playbooks/deploy-release.md`](../playbooks/deploy-release.md) | Antes de qualquer deploy ou rollback |
| Onboarding | [`playbooks/onboarding-devs.md`](../playbooks/onboarding-devs.md) | Ao iniciar no projeto ou mentorar novos devs |
| Auditoria Adversarial | [`playbooks/adversarial-audit.md`](../playbooks/adversarial-audit.md) | Antes de submeter artefatos críticos para aprovação |
| Governança Geral | [`PLAYBOOK.md`](../PLAYBOOK.md) | Para entender valores, RACI e gates de fase |
| Desktop Installer Build | [`desktop-installer-build.md`](desktop-installer-build.md) | Build e distribuição de instaladores cross-platform (NSIS/WiX/DMG/AppImage/deb/rpm) |
| Finance CSV Skill | [`finance-csv-skill.md`](finance-csv-skill.md) | Operação da skill de importação e categorização de extratos financeiros BR |
| Web Research Adapters | [`web-research-adapters.md`](web-research-adapters.md) | Configuração e troubleshooting dos adapters Parallel/Exa/Firecrawl |
| Helm Hardened Deployment | [`helm-hardened-deployment.md`](helm-hardened-deployment.md) | Deploy do chart Helm com HPA custom, PDB, NetworkPolicy e ServiceMonitor |
| Sentinel AIOps | [`sentinel-aiops.md`](sentinel-aiops.md) | Detecção de anomalias, thresholds e recalibração do módulo AIOps |
| Cerebelo Orchestration | [`cerebelo-orchestration.md`](cerebelo-orchestration.md) | Debug de DAGs, checkpoints e recovery de workflows multi-agente |
| Proof Verification | [`proof-verification.md`](proof-verification.md) | Verificação de integridade de evidências e certificados de conformidade |
| Resolver Conflicts | [`resolver-conflicts.md`](resolver-conflicts.md) | Resolução de conflitos multi-agente via majority vote/adversarial/HITL |
| Evidence Ledger | [`evidence-ledger.md`](evidence-ledger.md) | Consulta, exportação e verificação do registro imutável de evidências |

**Regra:** Todo runbook de troubleshooting deve referenciar o playbook de incident-response. Todo runbook de deploy deve referenciar o playbook de deploy-release.

---

## ⚠️ Gaps Conhecidos (Fase 7)

Os seguintes componentes ainda precisam de runbooks dedicados antes do release v1.0:

| Componente | Prioridade | Justificativa |
|------------|-----------|---------------|
| `zylos-goals` | Baixa | Sistema de metas — crate stub, documentar quando implementar |
| `zylos-swarm-orchestrator` | Média | Orquestração de swarm — crate stub |
| `zylos-personalities` | Baixa | Sistema de personalidades — crate stub |

**Nota:** Runbooks para Desktop Installer, Finance CSV, Web Research Adapters, Helm Hardened Deployment, CLI, Daemon, Cockpit, Sentinel, Cerebelo, Proof, Resolver e Evidence-Ledger foram criados e já estão indexados acima.

**Ação:** Implementar os 3 crates stub restantes e criar seus runbooks durante a Fase 7 como parte dos critérios de saída para release v1.0.