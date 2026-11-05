# Playbook de Deploy e Release — Zylos-Next

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Escopo:** Governança de release, critérios de promoção e rollback em produção
**Público:** Tech Lead, SRE, DevOps, Agente Orquestrador
**Runbooks Relacionados:** `runbooks/zylos-gateway.md`, `runbooks/zylos-docker.md`

---

## 1. Contexto Organizacional

### Missão do Deploy
Garantir que cada release do Zylos-Next chegue à produção com segurança, previsibilidade e capacidade de rollback instantâneo, preservando a integridade dos agentes ativos e dos dados em SQLite WAL.

### Valores Aplicados
- **Prova sobre Promessa:** Nenhum deploy sem benchmarks verdes e auditoria adversarial aprovada
- **Segurança Camada Zero:** Threat model revisado antes de cada release; capabilities validadas em staging
- **Documentação Viva:** CHANGELOG.md e HANDOFF atualizados ANTES do tag de release

### Papéis no Deploy

| Papel | Responsabilidade | Decisões | Escala Para |
|-------|-----------------|----------|-------------|
| **Release Manager** | Coordenação do ciclo de release, aprovação final | Go/No-Go para produção | Tech Lead (dúvidas técnicas) |
| **Tech Lead** | Validação de benchmarks e auditorias | Aprovar mudança de tech stack ou breaking change | Product Owner (mudança de escopo) |
| **SRE / Ops Lead** | Execução do deploy, monitoramento pós-deploy | Rollback automático se health check falhar | Release Manager (rollback manual) |
| **Auditor Adversarial** | Validação independente do artefato de release | Rejeitar release se encontrar vulnerabilidade | Ninguém (independente) |
| **Agente Orquestrador** | Automação de CI/CD, execução de smoke tests | Retry de jobs falhos, notificação de blockers | SRE (falha infra) |

---

## 2. Fluxo Estratégico de Release

### 2.1 Critérios de Entrada (Gate de Release)

Antes de qualquer deploy para produção, TODOS os itens devem estar verdes:

```
[ ] Todos os testes unitários e de integração passam (cargo test --workspace)
[ ] Benchmarks dentro do target definido em EVALUATION.md
[ ] Auditoria adversarial dupla aprovada (evals/audits/)
[ ] Security audit limpo (cargo audit + threat model review)
[ ] CHANGELOG.md atualizado com mudanças da versão
[ ] Runbooks atualizados se houver mudança operacional
[ ] Staging environment validado com smoke tests
[ ] Rollback plan documentado e testado
[ ] HANDOFF_N.md completo se for release de fase
```

**Se algum item falhar:** Release bloqueado. Sem exceções.

### 2.2 Cadência de Release

| Tipo | Frequência | Escopo | Aprovação |
|------|-----------|--------|-----------|
| **Hotfix** | Sob demanda | Correção crítica SEV-1/2 | Tech Lead + SRE |
| **Minor** | Semanal | Features, melhorias, bug fixes não-críticos | Release Manager |
| **Major (Fase)** | Por milestone | Nova fase completa, breaking changes | Tech Lead + Product Owner |
| **Canary** | Contínua | Teste de novas features com subset de usuários | Automático (feature flags) |

### 2.3 Matriz de Decisão de Rollback

```
DEPLOY CONCLUÍDO
    ↓
Health checks passam? ──NÃO──→ Rollback IMEDIATO (<2min)
    │                              ↓
    SIM                         Investigar causa raiz
    ↓                              ↓
Métricas dentro do baseline? ──NÃO──→ Rollback se degradação >20%
    │                              ↓
    SIM                         Monitorar 30min → Post-mortem
    ↓
Smoke tests passam? ──NÃO──→ Rollback + Alerta SEV-2
    │
    SIM → Release estável → Atualizar status page
```

---

## 3. Governança e Comunicação

### 3.1 Janela de Deploy

| Ambiente | Janela | Exceções |
|----------|--------|----------|
| **Staging** | Qualquer horário | — |
| **Produção** | Ter/Qui 10h–14h BRT | Hotfix SEV-1 (qualquer horário com IC) |
| **Canary** | Contínuo | Feature flag controla exposição |

**Regra:** Nunca deployar em sexta-feira após 14h ou em véspera de feriado.

### 3.2 Comunicação de Release

| Momento | Canal | Conteúdo |
|---------|-------|----------|
| Pré-deploy (30min) | #deploy-alerts | Versão, mudanças esperadas, janela |
| Durante deploy | #deploy-alerts | Progresso, health checks, métricas |
| Pós-deploy (sucesso) | #releases + Status Page | Changelog resumido, métricas validadas |
| Pós-deploy (rollback) | #incidentes | Causa, impacto, next steps |
| Post-mortem | Docs + All-hands | Lições aprendidas, action items |

### 3.3 Versionamento Semântico

- **MAJOR:** Breaking changes em traits públicas, schemas incompatíveis, nova fase
- **MINOR:** Novas features backward-compatible, novos crates funcionais
- **PATCH:** Bug fixes, melhorias de performance sem mudança de API

---

## 4. Ferramentas e Recursos

### 4.1 Pipeline CI/CD

| Stage | Ferramenta | Gate |
|-------|-----------|------|
| Build | GitHub Actions + Rust toolchain | Compilação zero-warning |
| Test | cargo test + nextest | 100% pass rate |
| Audit | cargo audit + clippy | Zero vulnerabilities, zero warnings |
| Benchmark | criterion + custom harness | Dentro do target ±10% |
| Stage Deploy | Docker + systemd | Health check verde |
| Prod Deploy | Blue-green via Docker tags | Smoke test + métricas |
| Rollback | Tag anterior + DB migration revert | <2min automatizado |

### 4.2 Feature Flags

Usadas para canary releases e mitigação de risco:
- Configuração via SQLite (`feature_flags` table)
- Toggle em runtime sem restart
- Métricas por flag (usage rate, error rate)
- Cleanup obrigatório após 2 sprints

### 4.3 Acessos Necessários

- GitHub Actions admin (trigger manual, re-run)
- Docker registry push/pull
- SSH para servidores de staging e produção
- SQLite admin para migrations e rollbacks
- Cockpit admin para feature flags

---

## 5. Adaptação por Nível de Senioridade

### Para Seniores
- Defina e revise os **critérios de entrada** periodicamente
- Decida quando abrir exceção ao gate de release (documente rationale)
- Analise tendências de deploy frequency e lead time
- Conduza post-mortems de releases problemáticos

### Para Plenos
- Execute o **pipeline CI/CD** e valide cada stage
- Escreva changelogs claros e acionáveis
- Monitore métricas pós-deploy e identifique anomalias
- Proponha melhorias na automação baseado em padrões recorrentes

### Para Júniores
- Siga o **checklist de gate de release** sem atalhos
- Execute deploys apenas em staging até ter sign-off
- Reporte qualquer warning ou flaky test imediatamente
- Use runbooks de rollback passo a passo em caso de falha

---

## 6. Referências Cruzadas

- **Playbook Estratégico Geral:** `PLAYBOOK.md`
- **Evaluation Framework:** `EVALUATION.md`
- **Security Model:** `specs/SECURITY-MODEL.md`
- **Communication Contract:** `specs/COMMUNICATION-CONTRACT.md`
- **Runbooks Operacionais:** `runbooks/README.md`
- **Handoffs de Fase:** `HANDOFF_*.md`