# Playbook: Stakeholder Communication — Comunicação Externa e Interna

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Protocolo estratégico para comunicação com stakeholders internos e externos durante incidentes, releases, mudanças de pricing, onboarding de clientes enterprise e eventos sazonais
**Público-alvo:** Tech Lead, Product Manager, Dev Senior, Customer Success
**Gatilho:** Incidente P0/P1, release com breaking change, mudança de termos/preço, onboarding de cliente enterprise, evento sazonal previsto, ou solicitação de status report por liderança

---

## 1. Contexto Organizacional

O Zylos-Next é um sistema de orquestração de agentes IA com múltiplos públicos:

| Stakeholder | Interesse Primário | Canal Preferencial | Frequência |
|-------------|-------------------|-------------------|------------|
| **Usuários finais (devs)** | Funcionalidade, estabilidade, docs | GitHub Issues, Discord, changelog | Contínua |
| **Clientes enterprise** | SLA, segurança, suporte dedicado | Email, call mensal, portal dedicado | Mensal + ad-hoc |
| **Liderança técnica (CTO/VP)** | Roadmap, riscos, budget, métricas | Slack, reunião semanal, dashboard | Semanal |
| **Financeiro / Legal** | Custos, contratos, compliance, ToS | Email, reunião mensal | Mensal |
| **Security / Compliance** | Auditorias, vulnerabilidades, LGPD/GDPR | Canal seguro, relatório trimestral | Trimestral + ad-hoc |
| **Parceiros / Vendors** | Integrações, co-marketing, suporte técnico | Email, call técnica | Variável |
| **Comunidade open-source** | Contribuições, roadmap público, transparência | GitHub, blog, Discord | Release + contínua |

Missão: garantir que cada stakeholder receba a informação certa, no momento certo, pelo canal certo, com tom e nível de detalhe adequados. Comunicação ruim gera perda de confiança, churn e crises evitáveis.

---

## 2. Papéis e Responsabilidades

| Papel | Responsabilidade | Decisão |
|-------|-----------------|---------|
| Product Manager | Definir messaging, aprovar comunicação externa, gerenciar expectativas de clientes | Tom e timing de comunicações externas |
| Tech Lead | Validar precisão técnica, aprovar detalhes de incidentes/releases | Conteúdo técnico de comunicações |
| Customer Success | Comunicar clientes enterprise diretamente, coletar feedback pós-comunicação | Personalização por cliente |
| Dev Senior | Fornecer dados técnicos precisos, revisar drafts de changelogs e post-mortems | Precisão factual |
| Security Engineer | Revisar comunicações sobre incidentes de segurança, validar que não expõem vetores | Segurança do conteúdo |
| Marketing / Comms | Formatar e distribuir comunicações públicas (blog, redes sociais) | Formato e distribuição |
| Liderança (CTO/VP) | Aprovar comunicações de alto impacto (pricing, outage prolongado, pivot estratégico) | Go/No-go final |

---

## 3. Fluxo de Decisão Estratégica

### 3.1 Quando Iniciar Comunicação Proativa

Execute este playbook quando QUALQUER condição for verdadeira:

- [ ] Incidente P0/P1 ativo ou resolvido nas últimas 24h
- [ ] Release com breaking change ou feature significativa pronta para publicação
- [ ] Mudança de pricing, termos de serviço ou política de privacidade
- [ ] Onboarding de cliente enterprise com requisitos customizados
- [ ] Evento sazonal em 30 dias que pode afetar disponibilidade ou performance
- [ ] Solicitação explícita de status report por liderança ou cliente
- [ ] Vulnerabilidade de segurança corrigida que requer notificação proativa
- [ ] Pivot de vendor/dependência crítica que afeta funcionalidade visível
- [ ] Métrica de SLA violada ou próxima do threshold (>90% do limite)

### 3.2 Matriz de Decisão por Cenário

| Cenário | Audiência Primária | Timing | Aprovador | Template |
|---------|-------------------|--------|-----------|----------|
| Incidente P0 ativo | Todos os afetados | Imediato + updates a cada 30min | IC + Tech Lead | §6.1 Alerta Inicial |
| Incidente P0 resolvido | Todos os afetados | <24h após resolução | Tech Lead + PM | §6.2 Resolução |
| Post-mortem publicado | Interno + clientes enterprise | <72h após incidente | Security Eng + Tech Lead | §6.3 Post-mortem Summary |
| Release com breaking change | Usuários finais + devs | 2 semanas antes + dia do release | PM + Tech Lead | §6.4 Changelog |
| Nova feature significativa | Usuários + comunidade | Dia do release | PM + Marketing | §6.5 Feature Announcement |
| Mudança de pricing | Clientes enterprise + financeiro | 30 dias antes | Financeiro + CTO | §6.6 Pricing Change |
| Onboarding enterprise | Cliente + equipe interna | Kickoff + milestones | CS + Tech Lead | §6.7 Onboarding Update |
| Evento sazonal previsto | Ops + clientes enterprise | 2 semanas antes + durante | SRE + PM | §6.8 Seasonal Prep |
| Status report solicitado | Liderança | Conforme solicitado | Tech Lead | §6.9 Status Report |
| Vulnerabilidade corrigida | Clientes enterprise + security | <7 dias após fix | Security Eng + PM | §6.10 Security Advisory |

### 3.3 Critérios de Entrada e Saída

**Entrada:** Pelo menos uma condição da §3.1 é verdadeira E audiência identificada.

**Saída:** Comunicação concluída quando TODOS forem verdadeiros:
- [ ] Mensagem enviada para todas as audiências relevantes
- [ ] Feedback coletado (se aplicável) e respondido em <24h
- [ ] Documentação atualizada (changelog, FAQ, runbook) se necessário
- [ ] Métrica de engajamento/resposta registrada (open rate, replies, tickets criados)
- [ ] Lições aprendidas adicionadas a este playbook se houve falha na comunicação

---

## 4. Governança e Escalação

### 4.1 Níveis de Decisão

| Nível | Quem Decide | Exemplo | Tempo Máximo |
|-------|-------------|---------|--------------|
| L1 — Rotineira | PM / Dev Senior | Changelog de release menor, update de status interno | 24 horas |
| L2 — Sensível | Tech Lead + PM | Comunicação de incidente P1, feature announcement público | 4 horas |
| L3 — Crítica | Tech Lead + PM + Liderança | Incidente P0, mudança de pricing, disclosure de vulnerabilidade | 1 hora |
| L4 — Emergencial | Qualquer senior on-call + IC | Alerta inicial de outage ativo, rollback urgente | Imediato |

### 4.2 Protocolo de Escalação

1. **Draft pronto mas aprovador indisponível >30min** → Escalar para próximo nível hierárquico; documentar tentativa
2. **Cliente enterprise reclama de comunicação inadequada** → CS + PM revisam template e personalizam; resposta em <2h
3. **Comunicação pública contém erro factual** → Retração/correção imediata + post-mortem interno em 48h
4. **Vazamento de informação confidencial via comunicação** → Acionar Security Incident Response playbook imediatamente

### 4.3 Comunicação entre Equipes Internas

- **Canal primário:** `#zylos-comms` no Slack (ou equivalente) para alinhamento pré-publicação
- **Revisão obrigatória:** Toda comunicação externa passa por pelo menos 2 reviewers (1 técnico + 1 de produto/comms)
- **Approval trail:** Toda comunicação L2+ tem registro escrito de quem aprovou e quando (Slack thread, email, ou ticket)
- **Post-publish review:** Após comunicações críticas, reunir equipe por 15min para avaliar eficácia e gaps

---

## 5. Ferramentas e Sistemas Necessários

| Ferramenta | Uso | Acesso |
|------------|-----|--------|
| Slack / Teams | Comunicação interna e rascunhos | Todos |
| Email corporativo | Comunicação formal com clientes e liderança | PM, CS, Liderança |
| GitHub Releases / Changelog | Publicação de notas de release | Dev Senior, PM |
| Notion / Confluence | Documentação de templates e histórico | Todos |
| CRM (HubSpot/Salesforce) | Tracking de comunicações com clientes enterprise | CS, PM |
| Mailchimp / SendGrid | Comunicações em massa para usuários | Marketing, PM |
| Grafana / Axiom | Dados para status reports e incident updates | SRE, Tech Lead |
| Figma / Canva | Assets visuais para announcements públicos | Marketing |
| Calendly / Google Calendar | Agendamento de calls com clientes e liderança | CS, PM |

---

## 6. Templates de Comunicação

### 6.1 Alerta Inicial de Incidente (Interno + Clientes Enterprise)

```
Assunto: 🚨 [Zylos-Next] Incidente em Andamento — [Breve Descrição]

Status: INVESTIGANDO
Severidade: [P0/P1]
Detectado: [YYYY-MM-DD HH:MM UTC]
Impacto: [Descrição factual do que está afetado]
Workaround disponível: [Sim/Não — se sim, descrever]
Próximo update: [HH:MM UTC +30min]

Equipe já está atuando. Updates serão enviados neste canal a cada 30 minutos.
Para urgências: responder este email ou contatar [canal de emergência].
```

### 6.2 Resolução de Incidente

```
Assunto: ✅ [Zylos-Next] Incidente Resolvido — [Breve Descrição]

Status: RESOLVIDO
Resolvido em: [YYYY-MM-DD HH:MM UTC]
Duração total: [X horas Y minutos]
Causa raiz: [1-2 frases técnicas acessíveis]
Ações tomadas: [Lista curta de correções aplicadas]
Prevenção: [Medidas para evitar recorrência]
Post-mortem: Será publicado em [URL/data prevista]

Serviço operando normalmente. Se ainda experiencing issues, responder este email.
```

### 6.3 Post-mortem Summary (Clientes Enterprise)

```
Assunto: 📋 [Zylos-Next] Post-mortem: [Título do Incidente]

Data do incidente: [YYYY-MM-DD]
Duração: [Xh Ym]
Impacto: [Métrica concreta: X usuários afetados, Y requests falharam]

LINHA DO TEMPO:
- HH:MM — Detecção
- HH:MM — Contenção
- HH:MM — Resolução
- HH:MM — Recuperação completa

CAUSA RAIZ:
[2-3 parágrafos técnicos acessíveis]

AÇÕES CORRETIVAS:
✅ [Ação 1 — concluída]
🔄 [Ação 2 — em progresso, ETA: data]
📋 [Ação 3 — planejada para sprint X]

PREVENÇÃO:
[Como isso não vai acontecer novamente]

Dúvidas? Responder este email ou agendar call: [link Calendly]
```

### 6.4 Changelog / Release Notes

```markdown
## Zylos-Next v[X.Y.Z] — [YYYY-MM-DD]

### 🆕 Novidades
- [Feature 1]: [descrição curta + link para docs]
- [Feature 2]: [descrição curta + link para docs]

### ⚠️ Breaking Changes
- [Change 1]: [o que mudou + migration guide]
- [Change 2]: [o que mudou + migration guide]

### 🐛 Correções
- [Fix 1]: [descrição do bug e como foi resolvido]
- [Fix 2]: [descrição do bug e como foi resolvido]

### 📈 Performance
- [Improvement 1]: [métrica antes → depois]

### 🔒 Segurança
- [CVE-XXXX-YYYY]: [descrição + fix aplicado]

**Upgrade guide:** [link]
**Full changelog:** [link GitHub]
```

### 6.5 Feature Announcement (Público)

```
Assunto: 🎉 Novo no Zylos-Next: [Nome da Feature]

Olá!

Estamos animados em anunciar [Feature Name], agora disponível na versão [X.Y.Z].

O QUE É:
[2-3 frases explicando o valor para o usuário]

POR QUE IMPORTA:
[Problema que resolve + benefício tangível]

COMO USAR:
[Quick start ou link para tutorial]

EXEMPLO:
[Code snippet, screenshot ou GIF demonstrando]

Disponível agora em [link]. Feedback? Responder este email ou abrir issue no GitHub.
```

### 6.6 Mudança de Pricing / Termos

```
Assunto: ⚠️ Atualização Importante: [Pricing/Termos] do Zylos-Next

Prezado(a) [Nome/Cliente],

Estamos escrevendo para informar sobre uma mudança em nossos [preços/termos] que entrará em vigor em [data, ≥30 dias].

O QUE ESTÁ MUDANDO:
[Descrição clara e específica da mudança]

POR QUE:
[Justificativa transparente: custos de infra, novas features, sustentabilidade]

IMPACTO PARA VOCÊ:
[Como afeta especificamente este cliente/plano]

OPÇÕES:
- [Opção 1: aceitar novos termos]
- [Opção 2: migrar para plano alternativo]
- [Opção 3: cancelar até data sem penalidade]

DÚVIDAS OU PREOCUPAÇÕES?
Agende uma call conosco: [link Calendly]
Ou responda este email — estamos aqui para ajudar.

Atenciosamente,
[Nome], [Cargo]
```

### 6.7 Onboarding Update (Cliente Enterprise)

```
Assunto: 📊 Zylos-Next Onboarding: Status Update #[N] — [Cliente]

Período: [data início] a [data fim]
Fase atual: [Discovery / Setup / Testing / Go-live]

CONCLUÍDO ESTA SEMANA:
✅ [Marco 1]
✅ [Marco 2]

PRÓXIMOS PASSOS:
🔄 [Marco 3 — ETA: data]
📋 [Marco 4 — ETA: data]

BLOQUEIOS / RISCOS:
⚠️ [Bloqueio 1 — ação necessária por: quem]
ℹ️ [Risco 1 — mitigação proposta]

MÉTRICAS DE PROGRESSO:
- Skills configuradas: X/Y
- Agentes testados: X/Y
- Integrações validadas: X/Y

Próxima call: [data/hora] | [link]
Contato direto: [nome/email do CS]
```

### 6.8 Preparação para Evento Sazonal

```
Assunto: 🗓️ Zylos-Next: Preparação para [Evento Sazonal]

Timeframe: [data início] a [data fim]
Expectativa de carga: [X% acima do baseline]

AÇÕES PRÉ-EVENTO (concluídas):
✅ Capacity scaling executado ([detalhes])
✅ Monitoring reforçado ([dashboards])
✅ Runbooks de contingência revisados
✅ Equipe on-call escalada ([escala])

DURANTE O EVENTO:
- Monitoring intensivo: [dashboard link]
- Canal de guerra: [#channel]
- Updates de status: a cada [X] horas
- Rollback plan: [link/documento]

PÓS-EVENTO:
- Scale-down gradual a partir de [data]
- Post-event review agendado para [data]
- Relatório de métricas em [data]

Dúvidas? Contatar [SRE on-call] ou responder este email.
```

### 6.9 Status Report (Liderança)

```
Assunto: 📊 Zylos-Next: Status Report Semanal — [Semana X]

RESUMO EXECUTIVO (TL;DR):
[3-5 bullets com highlights da semana]

MÉTRICAS CHAVE:
- Uptime: [X%] (target: 99.9%)
- Latência p99: [Xms] (target: <10ms)
- Agentes ativos: [X] (capacidade: Y)
- Custo mensal projetado: $[X] (budget: $Y)
- Tickets abertos: [X] (críticos: Y)

ENTREGAS DA SEMANA:
✅ [Entrega 1]
✅ [Entrega 2]

EM ANDAMENTO:
🔄 [Item 1 — % completo, ETA]
🔄 [Item 2 — % completo, ETA]

RISCOS E BLOQUEIOS:
⚠️ [Risco 1 — impacto + mitigação]
🚫 [Bloqueio 1 — ação necessária]

PRÓXIMA SEMANA:
📋 [Prioridade 1]
📋 [Prioridade 2]

DECISÕES NECESSÁRIAS:
❓ [Decisão 1 — contexto + opções + recomendação]

Dashboard completo: [link]
Dúvidas? Reply ou me procure no Slack.
```

### 6.10 Security Advisory

```
Assunto: 🔒 Zylos-Next Security Advisory: [CVE-ID / Título]

Severidade: [Crítica/Alta/Média/Baixa]
CVSS: [X.X]
Afeta versões: [X.Y.Z - A.B.C]
Corrigido em: [X.Y.Z+1]

DESCRIÇÃO:
[O que é a vulnerabilidade, sem detalhes exploratórios]

IMPACTO:
[O que um atacante poderia fazer se explorasse]

CORREÇÃO:
Atualize para versão [X.Y.Z+1] ou superior.
Migration guide: [link]

WORKAROUND (se disponível):
[Instruções temporárias até upgrade]

TIMELINE:
- Detectado: [data]
- Fix desenvolvido: [data]
- Patch release: [data]
- Disclosure pública: [data]

CRÉDITOS:
[Reporter/researcher, se autorizado]

Dúvidas de segurança? security@zylos.dev (PGP key: [link])
```

---

## 7. Formato e Publicação

- **Formato:** Markdown versionado no repositório (`docs/playbooks/stakeholder-communication.md`)
- **Templates:** Armazenados em `docs/templates/comms/` como arquivos separados para reuso
- **Atualização:** Após cada comunicação crítica, feedback negativo, ou quarterly review
- **Review trimestral:** PM + Tech Lead revisam templates, métricas de eficácia e gaps
- **Onboarding:** Novo PM/CS deve ler este playbook e praticar com 2 templates na primeira semana
- **Teste:** Simular comunicação de incidente P0 trimestralmente (tabletop com time de comms)

---

## 8. Referências Cruzadas

- **Playbook relacionado:** `security-incident-response.md` (comunicação durante incidentes de segurança)
- **Playbook relacionado:** `capacity-planning.md` (comunicação sobre escala e limites)
- **Playbook relacionado:** `vendor-dependency-pivot.md` (comunicação sobre mudanças de dependências)
- **Runbook relacionado:** `zylos-cli.md` (referência técnica para changelogs)
- **Runbook relacionado:** `zylos-cockpit.md` (referência para feature announcements de UI)
- **HANDOFF_*.md:** Contexto técnico para comunicações de release
- **SECURITY.md:** Regras de disclosure responsável
- **APP_FLOW.md:** Estados do dashboard para screenshots em announcements

---

**Próxima Revisão Agendada:** Fevereiro de 2027
**Owner:** Product Manager Zylos-Next