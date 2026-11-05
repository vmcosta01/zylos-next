# Playbook: Capacity Planning — Escala Horizontal de Agentes

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Decisões estratégicas sobre quando, como e quanto escalar o sistema Zylos-Next
**Público-alvo:** Tech Lead, Dev Senior, SRE
**Gatilho:** Métricas de uso atingem 70% dos limites, novo cliente onboarded, ou evento sazonal previsto

---

## 1. Contexto Organizacional

O Zylos-Next é um sistema de orquestração de agentes IA com execução sandboxed (WASM/Docker), comunicação em tempo real (SSE/WebSocket), e registry de skills soberano. A capacidade do sistema é determinada por:

- **Agentes concorrentes:** Limitado por `max_concurrent_agents` no config do daemon (default: 50)
- **Streams SSE ativos:** Limitado por memória do gateway (default: ~2MB/stream)
- **Skills em execução:** Limitado por sandbox pool size (WASM: CPU-bound, Docker: recursos do host)
- **Vector store queries:** Limitado por I/O do SQLite WAL (single-writer)

Missão: garantir que nenhum usuário experimente degradação de serviço sob carga prevista ou pico sazonal, sem over-provisioning desnecessário.

---

## 2. Papéis e Responsabilidades

| Papel | Responsabilidade | Decisão |
|-------|-----------------|---------|
| Tech Lead | Aprovar plano de escala, budget de infra | Go/No-go para scale-up |
| Dev Senior | Definir thresholds, implementar auto-scaling triggers | Thresholds e métricas |
| SRE / Ops | Executar scale-up/down, monitorar pós-mudança | Timing de execução |
| Product Manager | Comunicar eventos sazonais, forecast de uso | Input de demanda futura |
| Financeiro | Aprovar budget adicional se necessário | Budget cap |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Quando Iniciar Capacity Planning

Execute este playbook quando QUALQUER condição for verdadeira:

- [ ] Uso de CPU do daemon > 70% sustentado por 15 min
- [ ] Memória do gateway > 80% do alocado
- [ ] Latência p99 de AG-UI streaming > 50ms (target: <10ms)
- [ ] Fila de agentes pendentes > 10 por mais de 30s
- [ ] Novo cliente enterprise onboarded (>100 agentes esperados)
- [ ] Evento sazonal em 30 dias (ex: Black Friday, lançamento de produto)
- [ ] Vector store query latency p95 > 100ms

### 3.2 Matriz de Decisão

| Cenário | Ação Recomendada | Prazo | Aprovador |
|---------|-----------------|-------|-----------|
| Uso 70-85%, tendência estável | Otimizar código/config, adiar scale-up | 2 semanas | Dev Senior |
| Uso 85-95%, tendência crescente | Scale-up vertical (mais RAM/CPU) | 3 dias | Tech Lead |
| Uso >95% ou pico iminente | Scale-up horizontal (nós adicionais) | 24h | Tech Lead + Financeiro |
| Evento sazonal confirmado | Pre-scale 2x baseline 48h antes | 1 semana | Tech Lead + PM |
| Degradação recorrente sem causa clara | Auditoria de performance + profiling | 1 sprint | Tech Lead |
| Custo projetado excede budget | Negociar scope ou otimizar antes de escalar | Variável | Financeiro + Tech Lead |

### 3.3 Critérios de Entrada e Saída

**Entrada:** Pelo menos uma condição da §3.1 é verdadeira E métricas coletadas por ≥15 min.

**Saída:** Plano de ação aprovado com owner, prazo e métrica de sucesso definida. OU decisão documentada de "não agir agora" com reavaliação agendada.

---

## 4. Governança e Escalação

### 4.1 Níveis de Decisão

| Nível | Quem Decide | Exemplo | Tempo Máximo |
|-------|-------------|---------|--------------|
| L1 — Operacional | Dev Senior / SRE | Ajustar `max_concurrent_agents`, limpar logs | 1 hora |
| L2 — Tático | Tech Lead | Scale-up vertical, mudar provider de embedding | 24 horas |
| L3 — Estratégico | Tech Lead + Financeiro | Adicionar nós, mudar arquitetura, contratar vendor | 72 horas |
| L4 — Emergencial | Qualquer senior on-call | Mitigar outage imediato, rollback | Imediato |

### 4.2 Protocolo de Escalação

1. **L1 esgotado sem resolução em 2h** → Notificar Tech Lead via Slack/canal dedicado
2. **L2 esgotado sem resolução em 24h** → Escalar para CTO/VP Eng com relatório de impacto
3. **Outage ativo com SLA violado** → Acionar L4 imediatamente, post-mortem obrigatório em 48h

### 4.3 Comunicação entre Equipes

- **Canal primário:** `#zylos-capacity` no Slack (ou equivalente)
- **Documentação compartilhada:** Este playbook + dashboard de métricas (Grafana/Axiom)
- **Reunião semanal:** 30 min toda segunda para revisar tendências (cancelada se tudo verde)
- **Post-mortem:** Obrigatório após qualquer incidente de capacidade; publicado em `docs/postmortems/`

---

## 5. Ferramentas e Sistemas Necessários

| Ferramenta | Uso | Acesso |
|------------|-----|--------|
| Grafana / Axiom | Dashboard de métricas em tempo real | Todos devs |
| Prometheus / OpenTelemetry | Coleta de métricas do daemon/gateway | SRE |
| `zylos-cli` | Comandos operacionais (`agent list`, `daemon status`) | Todos devs |
| Terraform / Ansible | Provisionamento de infra (se cloud) | SRE |
| Slack / Teams | Comunicação e alertas | Todos |
| Notion / Confluence | Documentação deste playbook e decisões | Todos |
| GitHub Issues | Tracking de ações de capacity planning | Todos devs |

---

## 6. Formato e Publicação

- **Formato:** Markdown versionado no repositório (`docs/playbooks/capacity-planning.md`)
- **Atualização:** Após cada post-mortem, mudança de threshold, ou quarterly review
- **Review trimestral:** Tech Lead revisa métricas reais vs. thresholds definidos; ajusta se necessário
- **Onboarding:** Novo dev senior deve ler este playbook na primeira semana e confirmar compreensão com tech lead

---

## 7. Referências Cruzadas

- **Runbook relacionado:** `zylos-daemon.md` (recuperação operacional)
- **Runbook relacionado:** `fase3-live-comm.md` (métricas de streaming)
- **Playbook relacionado:** `security-incident-response.md` (se degradação for causada por ataque)
- **Arquitetura:** `ARCHITECTURE.md` (limites de design)
- **EVALUATION.md:** Métricas de performance por crate
- **HANDOFF_6.md:** Estado atual e recomendações

---

**Próxima Revisão Agendada:** Fevereiro de 2027
**Owner:** Tech Lead Zylos-Next