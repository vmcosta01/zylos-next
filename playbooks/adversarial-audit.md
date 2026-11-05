# Playbook de Auditoria Adversarial — Zylos-Next

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Escopo:** Governança do processo de dupla auditoria, critérios de refutação e aprovação de artefatos críticos
**Público:** Tech Lead, Auditores Adversariais, Devs Sênior/Pleno, Agente Orquestrador
**Runbooks Relacionados:** Todos os runbooks de fase (`runbooks/fase*-*.md`)

---

## 1. Contexto Organizacional

### Missão da Auditoria Adversarial
Garantir que nenhum artefato crítico seja promovido sem ter sido ativamente refutado por um auditor independente, eliminando viés de confirmação e garantindo que claims sejam suportados por evidências verificáveis.

### Valores Aplicados
- **Prova sobre Promessa:** O auditor busca REFUTE, não confirmar; ausência de refutação ≠ aprovação automática
- **Segurança Camada Zero:** Vulnerabilidades de segurança são bloqueio imediato, independente de severidade percebida
- **Documentação Viva:** Cada auditoria gera registro permanente em `evals/audits/`; lições aprendidas retroalimentam runbooks

### Papéis na Auditoria

| Papel | Responsabilidade | Decisões | Escala Para |
|-------|-----------------|----------|-------------|
| **Auditor Gerador** | Cria artefato seguindo loop profissional, responde a refutações | Ajustar artefato baseado em feedback | Tech Lead (discordância irreconciliável) |
| **Auditor Refutador** | Busca ativamente falhas, inconsistências, missing evidence | Rejeitar artefato, exigir re-trabalho | Ninguém (independente por design) |
| **Tech Lead** | Arbitragem final em disputas, aprovação de exceções | Override de bloqueio (com rationale documentado) | Product Owner (mudança de critério) |
| **Agente Orquestrador** | Coordenação logística, verificação de independência | Bloquear promoção se auditor = gerador | Tech Lead (falta de auditores disponíveis) |

**Regra Inviolável:** Auditor ≠ Gerador SEMPRE. Mesmo agente/pessoa não pode auditar próprio trabalho.

---

## 2. Fluxo Estratégico de Auditoria

### 2.1 Artefatos Sujeitos a Auditoria Dupla

| Categoria | Exemplos | Frequência |
|-----------|----------|------------|
| **Handoffs de Fase** | `HANDOFF_*.md` | Todo handoff |
| **Specs Críticas** | `specs/DESIGN.md`, `SECURITY-MODEL.md`, `COMMUNICATION-CONTRACT.md` | Toda mudança |
| **Benchmarks** | `EVALUATION.md`, relatórios de performance | Antes de promoção |
| **Runbooks Novos** | Qualquer runbook criado ou modificado significativamente | Na criação/revisão |
| **Code Merges** | PRs que alteram traits públicas ou security model | Todo merge |
| **Post-Mortems** | Relatórios de incidente SEV-1/2 | Após resolução |

### 2.2 Matriz de Decisão de Auditoria

```
ARTEFATO SUBMETIDO
    ↓
Auditor é independente? ──NÃO──→ BLOQUEAR + Notificar Tech Lead
    │                                  ↓
    SIM                             Designar novo auditor
    ↓
Refutador encontra issues? ──SIM──→ Gerador corrige → Re-auditoria
    │                                  ↓
    NÃO                             Issues resolvidas? ──NÃO──→ Rejeitar
    ↓                                  ↓
Ambos assinam APPROVED            SIM → Re-auditoria → Aprovar se verde
    ↓
Artefato promovido
```

### 2.3 Critérios de Refutação Obrigatórios

Todo refutador deve verificar estes 7 pontos ANTES de assinar:

| # | Critério | Como Verificar | Bloqueia Se |
|---|----------|----------------|-------------|
| 1 | Evidência existe | Links para benchmarks, testes, logs | Claim sem prova |
| 2 | Benchmarks são reproduzíveis | Comandos exatos + ambiente especificado | Resultado não-reproduzível |
| 3 | Threat model foi considerado | Referência a SECURITY-MODEL.md | Mudança sem análise de risco |
| 4 | Docs estão atualizadas | Diff mostra docs no mesmo commit | Código sem docs |
| 5 | Error handling está completo | 6 estados documentados por componente | Missing error paths |
| 6 | Out of Scope está definido | Seção presente e explícita | Escopo implícito |
| 7 | Anti-slop filters passaram | Checklist dos 7 princípios preenchido | Qualquer filtro falhou |

---

## 3. Governança e Comunicação

### 3.1 SLAs de Auditoria

| Tipo de Artefato | Tempo Máximo de Auditoria | Escalação Se Exceder |
|------------------|---------------------------|----------------------|
| Handoff de Fase | 24h | Tech Lead assume ou designa |
| Spec Crítica | 12h | Tech Lead decide prioridade |
| Benchmark | 8h | Agente Orquestrador cobra |
| Runbook | 4h | Mentor/TL faz review rápido |
| Code Merge | 2h (business hours) | Auto-approve com flag de risco |
| Post-Mortem | 24h | IC escreve draft, auditor revisa |

### 3.2 Formato de Relatório de Auditoria

Todo relatório em `evals/audits/` segue este template:

```markdown
# Auditoria: {Nome do Artefato} v{Versão}

**Auditor Gerador:** {nome/agente}
**Auditor Refutador:** {nome/agente}
**Data:** {YYYY-MM-DD}
**Status:** {APPROVED | REJECTED | NEEDS_REVISION}

## Claims Verificados
- [ ] Claim 1: {descrição} → Evidência: {link/path}
- [ ] Claim 2: ...

## Refutações Encontradas
1. {Descrição da falha} → Impacto: {alto/médio/baixo} → Resolução: {como foi corrigido}
2. ...

## Decisão Final
{Rationale para APPROVED/REJECTED}

## Assinaturas
- Gerador: {assinatura/data}
- Refutador: {assinatura/data}
```

### 3.3 Disputas e Arbitragem

Quando gerador e refutador discordam:

1. **Documentar ambos os lados** no relatório de auditoria
2. **Tech Lead revisa** em até 4h
3. **Decisão documentada** em DECISIONS.md como ADR
4. **Perdedor atualiza** artefato conforme decisão
5. **Re-auditoria rápida** (1h) para validar correção

**Regra:** Disputas não-resolvidas em 24h escalam automaticamente para Product Owner.

---

## 4. Ferramentas e Recursos

### 4.1 Stack de Verificação

| Ferramenta | Uso | Quando Usar |
|-----------|-----|-------------|
| `cargo test --workspace` | Validação de testes unitários/integração | Todo code merge |
| `cargo bench` | Reprodução de benchmarks | Antes de promoção |
| `cargo audit` | Verificação de vulnerabilidades | Todo release |
| `tracing-subscriber` logs | Validação de comportamento runtime | Debug de claims |
| SQLite audit log | Verificação de eventos de segurança | Auditoria de segurança |
| Minimax/GPT-4o API | Segunda opinião adversarial (automatizada) | Triagem inicial |

### 4.2 Checklists por Tipo de Artefato

**Para Handoffs:**
- [ ] Todas as 5 seções do HANDOFF_N.md preenchidas
- [ ] Métricas de sucesso da fase atingidas (link para EVALUATION.md)
- [ ] Lições aprendidas documentadas
- [ ] Gaps conhecidos listados com prioridade
- [ ] Próxima fase tem runbook criado

**Para Specs:**
- [ ] JSON Schema validado contra exemplos reais
- [ ] Breaking changes documentadas e versionadas
- [ ] Threat model atualizado se spec muda security boundary
- [ ] Referências cruzadas para runbooks relevantes

**Para Benchmarks:**
- [ ] Ambiente de teste especificado (hardware, OS, Rust version)
- [ ] Warm-up iterations documentadas
- [ ] Statistical significance report (p-value, confidence interval)
- [ ] Comparison com baseline anterior
- [ ] Raw data disponível para reprodução

### 4.3 Diretório de Auditorias

```
evals/audits/
├── fase0-fase1-audit.md          # Auditoria combinada Fase 0-1
├── fase0-fase1-audit-v3-glm.md   # Re-auditoria com modelo diferente
├── fase2-audit-adversarial-minimax.md
├── fase2-audit-adversarial-v4-independent.md
├── fase4-audit-isolated.md
├── fase4-audit-adversarial.md
├── fase5-audit-isolated.md
├── fase5-audit-adversarial.md
├── fase6-audit-isolated.md
└── fase6-audit-adversarial.md
```

**Convenção:** `{fase}-{tipo}-audit{-variante}.md`

---

## 5. Adaptação por Nível de Senioridade

### Para Seniores (como Auditores)
- Foque em **refutações sistêmicas**, não apenas erros pontuais
- Identifique padrões de falha recorrente e proponha melhorias nos runbooks
- Valide se os critérios de auditoria ainda fazem sentido para o estado atual do projeto
- Mentore auditores plenos/júniores em técnicas de refutação eficaz

### Para Plenos (como Auditores)
- Execute o **checklist completo** sem atalhos
- Documente refutações com evidência clara e impacto quantificado
- Peça second opinion ao sênior quando unsure sobre severidade de issue
- Pratique escrever relatórios que sejam acionáveis, não apenas descritivos

### Para Júniores (como Observadores/Aprendizes)
- **Observe** o processo antes de participar ativamente (primeiras 2 auditorias)
- Leia relatórios anteriores em `evals/audits/` para entender padrão de qualidade
- Faça perguntas durante a auditoria; aprendizado > produtividade nesta fase
- Nunca seja auditor refutador de artefato que você ajudou a criar

### Para Geradores (todos os níveis)
- Prepare o artefato **como se fosse ser refutado** (porque será)
- Inclua evidência proativa para cada claim; não espere o auditor pedir
- Responda a refutações com dados, não com opiniões
- Se discordar da refutação, documente seu lado claramente e escale

---

## 6. Referências Cruzadas

- **Playbook Estratégico Geral:** `PLAYBOOK.md` §4.2 Dupla Auditoria Adversarial
- **Princípios Anti-Slop:** `PLAYBOOK.md` §4.1
- **Security Model:** `specs/SECURITY-MODEL.md`
- **Evaluation Framework:** `EVALUATION.md`
- **Decisões Arquiteturais:** `DECISIONS.md`
- **Índice de Runbooks:** `runbooks/README.md`
- **Handoffs de Fase:** `HANDOFF_*.md`