# Playbook de Onboarding de Desenvolvedores — Zylos-Next

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Escopo:** Integração técnica e cultural de novos devs (Júnior/Pleno/Sênior)
**Público:** Tech Lead, Mentores, Novos Devs, Agente Orquestrador
**Runbooks Relacionados:** `runbooks/fase0-foundation.md`, `runbooks/README.md`

---

## 1. Contexto Organizacional

### Missão do Onboarding
Transformar um novo integrante em um contribuidor produtivo e alinhado à cultura "Prova sobre Promessa" em até 2 semanas, com autonomia progressiva baseada em gates de competência validados.

### Valores Aplicados no Onboarding
- **Soberania:** Novo dev deve entender POR QUE não usamos frameworks externos antes de aprender COMO usar nosso stack
- **Documentação Viva:** Onboarding é o primeiro teste de qualidade da documentação; se o dev travar, a doc está errada
- **Loop Profissional:** Novo dev pratica READ → UNDERSTAND → PLAN → IMPLEMENT → TEST desde o dia 1

### Papéis no Onboarding

| Papel | Responsabilidade | Decisões | Escala Para |
|-------|-----------------|----------|-------------|
| **Mentor Direto** | Acompanhamento diário, code review inicial, desbloqueio técnico | Aprovar PRs de treino, ajustar ritmo | Tech Lead (dev estagnado >3 dias) |
| **Tech Lead** | Validação de gates de competência, alinhamento arquitetural | Promover nível de acesso, aprovar primeira task real | Product Owner (problemas culturais) |
| **Agente Orquestrador** | Provisionamento de ambiente, verificação de setup, tasks guiadas | Retry de setup falho, sugestão de runbook | Mentor (setup incompleto >4h) |
| **Buddy Cultural** | Integração social, dúvidas não-técnicas, feedback informal | Sugerir ajustes de processo | RH/Tech Lead (conflitos) |

---

## 2. Fluxo Estratégico de Onboarding

### 2.1 Jornada de 4 Semanas

| Semana | Foco | Gate de Saída | Artefato Entregue |
|--------|------|---------------|-------------------|
| **1** | Setup + Cultura + Fase 0-1 | Ambiente verde, quiz cultura >80% | PR de fix em docs ou typo |
| **2** | Fase 2-3 + Primeiro Bug Real | Teste verde em sandbox isolada | Fix de bug SEV-3/4 com review |
| **3** | Fase 4-5 + Feature Pequena | Benchmark dentro do target | Feature completa com testes |
| **4** | Autonomia + Primeira Task Solo | Code review aprovado sem rework maior | Task de sprint entregue |

### 2.2 Matriz de Competência por Nível

| Competência | Júnior (Semana 1-2) | Pleno (Semana 3-4) | Sênior (Semana 1-2) |
|-------------|---------------------|--------------------|---------------------|
| Setup local | Guiado por runbook | Independente | Valida e melhora runbook |
| Debug básico | Com mentor lado-a-lado | Independente | Ensina outros |
| Testes unitários | Copia padrão existente | Cria novos cenários | Define estratégia de teste |
| Arquitetura | Entende fluxo de dados | Propõe melhorias locais | Desenha novos componentes |
| Auditoria | Observa processo | Participa como refutador | Lidera auditoria adversarial |
| Documentação | Atualiza glossário | Escreve seções de runbook | Revisa e aprova docs |

### 2.3 Critérios de Aceleração/Retenção

```
DEV COMPLETOU GATE ANTES DO PRAZO?
    ↓
SIM → Oferecer task mais complexa + mentoria avançada
    ↓
NÃO → Investigar causa:
    ├── Técnica → Pair programming intensivo + runbook específico
    ├── Cultural → Conversa com buddy + ajuste de expectativas
    └── Motivacional → Feedback franco + realinhamento de carreira
    ↓
Após 1 semana de intervenção → Reavaliar fit com Tech Lead
```

---

## 3. Governança e Comunicação

### 3.1 Ritmos de Acompanhamento

| Ritual | Frequência | Participantes | Objetivo |
|--------|-----------|---------------|----------|
| Daily Check-in | Diário (15min) | Dev + Mentor | Desbloqueio rápido, ajuste de rota |
| Weekly Review | Semanal (1h) | Dev + Mentor + TL | Validação de gate, feedback estruturado |
| Retro de Onboarding | Fim semana 2 e 4 | Dev + Mentor + Buddy | Melhorar processo para próximos |
| Graduação | Fim semana 4 | Time todo | Celebração + apresentação do dev |

### 3.2 Feedback Estruturado

Modelo SBI (Situation-Behavior-Impact) obrigatório:
- **Situação:** "No PR #123 de ontem..."
- **Behavior:** "...você pulou a etapa de benchmark..."
- **Impact:** "...isso causou regressão de 15% na latência que só foi pega em staging."

**Regra:** Todo feedback negativo deve vir acompanhado de sugestão concreta de melhoria.

### 3.3 Escalação de Problemas

| Situação | Quem Acionar | Tempo Máximo |
|----------|-------------|--------------|
| Dev sem acesso/ferramentas | Agente Orquestrador | 2h |
| Dev travado tecnicamente | Mentor | 4h |
| Dev desmotivado/conflito | Buddy + TL | 24h |
| Gate não atingido após 2 tentativas | Tech Lead | 48h |
| Incompatibilidade cultural grave | TL + PO | Imediato |

---

## 4. Ferramentas e Recursos

### 4.1 Checklist de Setup (Dia 1)

- [ ] Rust toolchain instalado (`rustup show` confirma 1.82+)
- [ ] Clone do repo + `cargo build --workspace` verde
- [ ] SQLite CLI instalado + acesso ao DB de dev
- [ ] Docker/Podman funcionando + permissões corretas
- [ ] Chaves de API configuradas no keyring (não em .env!)
- [ ] Acesso aos canais de comunicação (#onboarding, #dev, #incidentes)
- [ ] Leitura obrigatória: PLAYBOOK.md, MEMORY.md, SECURITY.md
- [ ] Execução guiada de `runbooks/fase0-foundation.md` §Setup

### 4.2 Tasks de Treino Progressivas

| Ordem | Task | Runbook | Complexidade |
|-------|------|---------|--------------|
| 1 | Fix typo em docs | — | Trivial |
| 2 | Adicionar teste unitário faltante | `fase1-bot-system.md` §Testes | Baixa |
| 3 | Reproduzir bug conhecido em sandbox | `fase4-sandbox-isolation.md` | Média |
| 4 | Implementar skill simples | `fase5-skills-registry.md` | Média |
| 5 | Otimizar hot path com benchmark | `EVALUATION.md` | Alta |

### 4.3 Materiais de Referência Obrigatórios

- `PLAYBOOK.md` — Cultura e governança
- `ARCHITECTURE.md` — Topologia dos sistemas
- `SECURITY.md` — Threat model e capabilities
- `DECISIONS.md` — ADRs históricas (entender o "porquê")
- `runbooks/README.md` — Índice operacional
- `evals/audits/` — Exemplos de auditoria adversarial

---

## 5. Adaptação por Nível de Senioridade

### Para Seniores (sendo onboarded)
- Foque em **entender decisões arquiteturais** (leia DECISIONS.md inteiro)
- Valide se os runbooks fazem sentido para alguém com sua experiência
- Proponha melhorias no processo de onboarding baseado em experiências passadas
- Seu gate de saída é contribuir com uma melhoria sistêmica, não apenas completar tasks

### Para Plenos (sendo onboarded)
- Siga a jornada de 4 semanas conforme planejado
- Use o mentor como acelerador, não como muleta
- Documente dificuldades encontradas para melhorar o onboarding futuro
- Seu gate é autonomia em tasks de complexidade média

### Para Júniores (sendo onboarded)
- Não pule etapas do checklist de setup mesmo que pareça trivial
- Pergunte SEMPRE que tiver dúvida; "pergunta boba" não existe neste time
- Pratique o loop profissional em micro-tasks antes de pegar bugs reais
- Seu gate é demonstrar compreensão dos valores e execução disciplinada

### Para Mentores
- Prepare o ambiente ANTES do dia 1 do dev
- Tenha as 3 primeiras tasks de treino pré-selecionadas
- Faça pair programming nas primeiras 4h, não apenas code review assíncrono
- Documente padrões de dificuldade recorrentes para melhorar runbooks

---

## 6. Referências Cruzadas

- **Playbook Estratégico Geral:** `PLAYBOOK.md`
- **Runbook de Foundation:** `runbooks/fase0-foundation.md`
- **Índice de Runbooks:** `runbooks/README.md`
- **Security Model:** `specs/SECURITY-MODEL.md`
- **Decisões Arquiteturais:** `DECISIONS.md`
- **Avaliação e Benchmarks:** `EVALUATION.md`