# 🧬 Conceitos de Desenvolvimento com IA — Manifesto Operacional (v3.1)

> **ESTE ARQUIVO É INVIOLÁVEL.** Todo agente, sessão, workflow ou orquestração que envolva escrita, modificação, revisão ou auditoria de código DEVE cumprir **TODAS** as seções deste manifesto. Violação de qualquer parte = trabalho inválido, independente de resultado funcional.
>
> ⚠️ **ATENÇÃO:** Este manifesto NÃO é apenas "22 conceitos". Ele é um sistema operacional completo de desenvolvimento com IA composto por **10 camadas obrigatórias**. Ler apenas os conceitos canônicos e ignorar as demais camadas = violação.
>
> Fontes: Vibe Coding MD Files Guide · Vibe Coding Complete Guide (33 steps) · App Planning Guide (4 Prompts) · Beginner Template Pack · vibe-coding-context-kit · criar_estrutura.py · estrutura-projeto-agente-ia.md · Requisitos Victor Costa · LICOES.md · CLAUDE.md
>
> Última atualização: 2026-11-10 · Versão: 3.1.0

---

## Estrutura Completa do Manifesto (10 Camadas Obrigatórias)

Este documento é composto por **10 camadas interdependentes**. Todas são obrigatórias e enforceáveis. Nenhuma camada pode ser ignorada ou tratada como opcional.

| # | Camada | Conteúdo | Obrigatoriedade |
|---|--------|----------|-----------------|
| 1 | **Conceitos Canônicos** | 22 regras fundamentais de desenvolvimento com IA (#1 PRD → #22 Evaluation) | OBRIGATÓRIA — base de todas as demais |
| 2 | **Princípios Anti-Slop** | 7 filtros de qualidade que previnem entregas medíocres | OBRIGATÓRIA — aplica-se a toda entrega |
| 3 | **Loop Profissional** | Ciclo READ→UNDERSTAND→PLAN→IMPLEMENT→TEST→REVIEW→FIX→COMMIT→UPDATE DOCS | OBRIGATÓRIO — pular etapas = trabalho inválido |
| 4 | **Penalidade** | Violação = artefato descartado e refeito; sem exceções | OBRIGATÓRIA — enforcement universal |
| 5 | **Arquivos Obrigatórios** | 15 documentos que todo projeto DEVE ter + quando criar cada um | OBRIGATÓRIA — checklist de conformidade |
| 6 | **Templates de Prompts** | 6 prompts estruturados para revisão de PRD, APP_FLOW, Fase, Spec, Handoff, Benchmark | OBRIGATÓRIA — usar antes de aprovar artefatos |
| 7 | **Matriz de Rastreabilidade** | Mapeamento Fonte F1-F7 → Conceito → Diferencial CÓRTEX | OBRIGATÓRIA — auditoria de cobertura |
| 8 | **Integração com Harness** | Scripts, hooks, skills e ferramentas que enforce os conceitos automaticamente | OBRIGATÓRIA — automação do compliance |
| 9 | **Skills Hermes Documentais** | 14 skills (8 canônicas + 6 operacionais) que implementam os conceitos como procedimentos | OBRIGATÓRIA — carregar skill antes de criar documento |
| 10 | **Protocolo de Evolução + Lições** | Versionamento semântico, processo de atualização, lições aprendidas | OBRIGATÓRIA — garante que o manifesto nunca fique obsoleto |

**Regra de Leitura:** Agentes DEVEM ler este documento na ordem das camadas acima. Parar na camada 1 (Conceitos Canônicos) e ignorar as camadas 2-10 = violação do manifesto. Cada camada adiciona constraints, automações e verificações que tornam as camadas anteriores enforceáveis na prática.

---

## Camada 1: Os 22 Conceitos Canônicos

### 1. PRD Antes de Código (Product Requirements Document)
**Regra:** Nenhum código é escrito sem um PRD que defina WHAT + WHY. O PRD contém seções obrigatórias:
- **Visão do Produto** e problema a resolver
- **Target Users Específicos** (persona com nome, dor, comportamento atual — nunca "todos")
- **Features Core** e fluxos de usuário (referência a APP_FLOW.md)
- **Research Summary** (o que foi investigado antes de definir)
- **Definition of Done** (critérios objetivos e verificáveis de completude)
- **Out of Scope** (lista explícita do que NÃO será feito nesta versão)
- **Métricas de Sucesso** mensuráveis
- **Requisitos funcionais/UX/performance**

**Arquivo:** `PRD.md`
**Fontes:** F1 Steps 2,7 · F2 Step 7 · F3 §1 · F4 TRD · F5
**Diferencial CÓRTEX:** Seções DoD e Out of Scope são obrigatórias e revisadas por auditor adversarial.
**Contra-exemplo:** PRD sem "Out of Scope" = scope creep garantido. PRD com "usuários: todos" = produto para ninguém.

### 2. Arquitetura Definida Antes da Implementação
**Regra:** A arquitetura do sistema (HOW) é documentada antes de qualquer feature. Inclui: tech stack, estrutura de pastas, fluxo de dados, banco/armazenamento, serviços externos, deploy, escalabilidade, padrões de API (paginação, cache), e decisões de feature flags/toggles. UML ou diagrama de fluxo obrigatório para sistemas complexos.
**Arquivo:** `ARCHITECTURE.md`
**Fontes:** F2 Step 11 · F3 §4 · F6 · F7
**Diferencial CÓRTEX:** UML obrigatório para sistemas complexos. Decisões arquiteturais absorvem conceitos de loja de apps (#14 antigo) e paginação backend (#18 antigo).
**Contra-exemplo:** Arquitetura sem diagrama = agente inventa conexões. Feature flag sem documentação arquitetural = toggle órfão.

### 3. Design System como Contrato Visual
**Regra:** Tokens e estados são definidos ANTES da UI. O design system contém obrigatoriamente:

**Tabela de Tokens:**
| Token | Valor | Uso |
|-------|-------|-----|
| Cores primárias/secundárias/neutras | Hex/RGB/HSL | Branding, feedback, fundos |
| Tipografia (família, tamanhos, pesos) | px/rem | Hierarquia visual |
| Espaçamento (escala modular) | px/rem | Consistência espacial |
| Sombras (elevações) | box-shadow | Profundidade |
| Border-radius | px | Cantos |
| Breakpoints responsivos | px | Adaptação |

**Estados de UI Obrigatórios por Componente:**
- **Loading** (skeleton/spinner)
- **Empty** (zero data, estado inicial)
- **Error** (falha recuperável, mensagem clara)
- **Success** (confirmação positiva)
- **Validation** (erro de input inline)
- **Disabled** (ação indisponível, razão visível)

O agente segue o design system; nunca inventa estilo ad-hoc. Labels e tooltips em todos os elementos interativos são obrigatórios (acessibilidade não é feature futura).
**Arquivo:** `DESIGN_SYSTEM.md` ou `DESIGN.md`
**Fontes:** F1 Step 4 · F2 Step 12 · F3 §3 · F4 TESTING · F5
**Diferencial CÓRTEX:** Tabela de tokens + 6 estados obrigatórios documentados. Acessibilidade (#16 antigo) absorvida aqui.
**Contra-exemplo:** Design system sem tabela de cores = agente inventa paleta. Componente sem estado de erro = tela branca em falha.

### 4. Regras do Agente (AGENTS.md / RULES.md)
**Regra:** Instruções explícitas para o agente: contexto do projeto, arquivos obrigatórios antes de codar, convenções de código, regras de segurança, comandos disponíveis, boundaries (o que NÃO mudar sem aprovação). Sem isso, o agente chuta. Enforced por hooks automáticos.
**Arquivo:** `AGENTS.md`, `RULES.md`, ou `.cursor/rules/*.mdc`
**Fontes:** F2 Step 13 · F3 §2
**Diferencial CÓRTEX:** Hooks automáticos enforce regras no momento certo (scope-guard, verificar-gate).
**Contra-exemplo:** AGENTS.md genérico ("seja cuidadoso") = inútil. Regra sem hook = sugestão ignorada.

### 5. Spec Antes de Gerar Código
**Regra:** Para cada feature/tarefa, escreve-se uma especificação técnica antes de gerar código. A spec define: contexto, tarefa, arquivos relevantes, constraints, critérios de aceitação testáveis, testes esperados, out of scope da tarefa. Nunca "build my app" — sempre spec granular.
**Fontes:** F1 Step 4 · F2 Steps 20,21 · F4 TRD · F5
**Diferencial CÓRTEX:** Critérios de aceitação devem ser testáveis (não subjetivos).
**Contra-exemplo:** Spec diz "deve funcionar rápido" sem métrica = critério vazio. Spec sem arquivos listados = agente adivinha escopo.

### 6. TDD com IA no Loop
**Regra:** Testes são escritos ANTES ou EM PARALELO ao código, não depois. O ciclo é: Understand → Plan → Implement → Test → Review → Fix → Commit. Teste verde do caminho feliz NÃO basta — ataque bordas, round-trip, isca real. Mordida obrigatória: reverta o conserto e exija vermelho.

**TESTING.md Estruturado (obrigatório):**
| Tipo | Checklist | Status |
|------|-----------|--------|
| Happy Path | Fluxo principal funciona | ☐ |
| Validação | Inputs inválidos tratados | ☐ |
| Erro | Falhas recuperáveis | ☐ |
| Permissões | Acesso negado funciona | ☐ |
| Responsividade | Mobile/tablet/desktop | ☐ |
| Acessibilidade | Keyboard/screen reader | ☐ |
| Regressão | Features antigas intactas | ☐ |
| Performance | Render < threshold definido | ☐ |

Otimizações de renderização (#17 antigo) são critérios de teste, não conceito separado.
**Arquivo:** `TESTING.md`
**Fontes:** F2 Steps 18,22,23 · F4 TESTING · F5
**Diferencial CÓRTEX:** Mordida + TESTING.md estruturado com checklist verificável.
**Contra-exemplo:** Teste só do caminho feliz = falso positivo. Sem teste de regressão = feature nova quebra antiga.

### 7. Contexto Baseado em Grafos / Documentos Vivos
**Regra:** O contexto do projeto não é um prompt gigante — é um conjunto de documentos interligados (PRD ↔ Architecture ↔ Design ↔ Rules ↔ Tasks ↔ Decisions ↔ Memory). O agente lê os docs relevantes ANTES de cada tarefa. Docs desatualizados = agente trabalhando com contexto falso.

**Estrutura Mínima de Memory.md:**
```markdown
## Decisions
- [DATA] Decisão X porque Y

## Current State
- O que está funcionando
- O que está em progresso

## Known Issues
- Bugs abertos com impacto

## Lessons Learned
- O que aprendemos nesta sprint

## Failed Approaches
- O que tentamos e NÃO funcionou (evitar repetição)

## Useful Commands
- Comandos frequentes do projeto
```

**Task Board (TASKS.md):**
- **Next** — priorizado
- **In Progress** — máximo 3 itens
- **Completed** — com link para artefato
- **Blocked** — com razão e dependência
- **Backlog** — futuro

**Arquivos:** `MEMORY.md`, `DECISIONS.md`, `TASKS.md`
**Fontes:** F2 Steps 13,16,33 · F4 · F5 · F7 src/memory
**Diferencial CÓRTEX:** Template mínimo definido. Failed approaches documentados.
**Contra-exemplo:** Memory.md como dump de conversa = inútil para próxima sessão. Task board sem seção "Blocked" = blockers invisíveis.

### 8. Orquestração Multi-Agente com Handoff Estruturado
**Regra:** Tarefas complexas são decompostas e distribuídas para múltiplos agentes especializados trabalhando em paralelo. O orquestrador coordena, prova e agrega. Nunca um único agente faz tudo quando a tarefa exige lentes diversas.

**Protocolo de Handoff (obrigatório entre fases/agentes):**
A mensagem de handoff é artefato salvo em disco (`HANDOFF_[fase].md`) contendo:
1. **Contexto Aprovado** — o que foi validado até aqui
2. **Artefatos Gerados** — paths dos arquivos produzidos
3. **Blockers Conhecidos** — problemas identificados
4. **Próximo Passo Esperado** — o que o receptor deve fazer
5. **Perguntas Abertas** — dúvidas pendentes

O agente receptor confirma entendimento antes de executar.
**Arquivo:** `HANDOFF_[fase].md`
**Fontes:** F2 · F6 · F7
**Diferencial CÓRTEX:** Track-record medido (agent-track.mjs) + handoff como artefato provável.
**Contra-exemplo:** Handoff verbal em conversa = contexto perdido. Agente seguinte repete trabalho já feito = desperdício.

### 9. Code Review Automático com Gate de Merge
**Regra:** Todo PR passa por code review automático que impede merge se encontrar falha crítica. O review verifica: correção, arquitetura, segurança, error handling, acessibilidade, responsividade, performance, duplicação. Juiz ≠ gerador.
**Fontes:** F2 Step 24 · Workflow Victor
**Diferencial CÓRTEX:** Juiz≠gerador + hook automático (verificar-gate.mjs bloqueia declaração de "pronto" com stubs/TODOs abertos).
**Contra-exemplo:** Auto-merge sem review = bug em produção. Review pelo mesmo agente que escreveu = viés confirmado.

### 10. Análise de Qualidade de Código Escrito por IA
**Regra:** Código gerado por IA passa por análise de qualidade estruturada: teste de carga, complexidade ciclomática, estrutura de dependência, cobertura de testes. "Compilou" ≠ "está correto". Auditoria isolada (agente-auditar) obrigatória.
**Fontes:** F2 Steps 22-23 · Workflow Victor
**Diferencial CÓRTEX:** Auditoria isolada (agente-auditar) + proof.mjs + panel.mjs. Protocolo de refutação: REFUTE, não confirme.
**Contra-exemplo:** "Os testes passam" sem auditoria isolada (agente-auditar) = confiança cega. Qualidade medida por quem escreveu = conflito de interesse.

### 11. Agentes de Monitoramento Contínuo
**Regra:** Agentes autônomos verificam diariamente: aplicação funcionando, backup do banco, respostas a usuários, status de assinaturas. Relatório diário consolidado. Não é opcional — é parte do sistema.
**Fontes:** F2 · Workflow Victor · Requisito Victor
**Diferencial CÓRTEX:** ops-sentinel.mjs + P6-04-health-check.mjs + relatório consolidado.
**Contra-exemplo:** Monitoramento só quando algo quebra = reação tardia. Sem relatório consolidado = sinais perdidos.

### 12. Segurança como Camada Zero (Não Afterthought)
**Regra:** Segurança é definida ANTES do código: regras de acesso, travas de banco, proibição de senhas hardcoded, validação de input server-side, autorização verificada no backend, secrets em env vars. SECURITY.md é documento de planejamento, não de auditoria pós-fato.
**Arquivo:** `SECURITY.md`, `.env.example`
**Fontes:** F2 Steps 15,18 · F3 §2 Security · Requisito Victor
**Diferencial CÓRTEX:** Camada zero + enforced por scope-guard hook (bloqueia escrita de segredos).
**Contra-exemplo:** Segurança adicionada depois = vulnerabilidade já exposta. .env committado = vazamento.

### 13. Sistema Multiusuário com Regras de Acesso
**Regra:** Todo sistema tem definição explícita de quem pode fazer o quê. Roles, permissões, boundaries. Documentado e enforced server-side.
**Fonte:** Requisito Victor
**Diferencial CÓRTEX:** Enforced server-side (nunca só frontend).
**Contra-exemplo:** Permissão só no frontend = bypass trivial. Role sem documentação = agente inventa acesso.

### 14. Dashboard como Sistema Operacional Vivo
**Regra:** O dashboard não é página estática — é SO vivo com biblioteca viva. Métricas em tempo real, status de agentes, saúde do sistema, logs, alertas. Se não está visível, não existe.
**Fonte:** Requisito Victor
**Diferencial CÓRTEX:** Integração com ops-sentinel e health-check scripts.
**Contra-exemplo:** Dashboard com dados stale = falsa sensação de controle. Métrica sem alerta = ruído.

### 15. Git como Rede de Segurança
**Regra:** Commits pequenos e frequentes. Branches por feature. Nunca commit direto na main. Mensagens descritivas. Git protege contra erros da IA — rollback é sempre possível.
**Fontes:** F1 Step 8 · F2 Steps 8,26 · F5
**Diferencial CÓRTEX:** Commits atômicos + branches isoladas.
**Contra-exemplo:** Commit gigante com 20 arquivos = rollback impossível. Main branch sem proteção = caos.

### 16. Deploy em Estágios (Nunca Direto em Produção)
**Regra:** LOCAL → PREVIEW → QA → PRODUCTION. Preview deployment para toda branch. QA testing no preview. Produção só após verificação completa. Variáveis de ambiente separadas por estágio.
**Fontes:** F2 Steps 28-31
**Diferencial CÓRTEX:** Pipeline LOCAL→PREVIEW→QA→PROD via deploy-estagios.mjs.
**Contra-exemplo:** Deploy direto em prod = risco desnecessário. Preview sem QA = teste em produção disfarçado.

### 17. Documentação Viva e Atualizada
**Regra:** TASKS.md, MEMORY.md, DECISIONS.md, README.md, ARCHITECTURE.md são atualizados conforme o projeto evolui. Documentação que descreve um sistema que não existe mais é pior que ausência de documentação. O agente precisa de contexto preciso. Atualizada a cada sessão.
**Fontes:** F2 Step 33 · F3 · F4 · F5
**Diferencial CÓRTEX:** Atualização a cada sessão, não "quando der tempo".
**Contra-exemplo:** README descrevendo v1 quando estamos em v3 = agente trabalha com contexto falso.

### 18. User Journey como Documento Obrigatório (APP_FLOW.md)
**Regra:** Todo projeto tem APP_FLOW.md mapeando: entrada → telas → ações → respostas → estados (loading/empty/error/success/validation/disabled) → saída/recuperação. Cada tela listada com seus estados obrigatórios. Fluxo testável manualmente antes de codar.
**Arquivo:** `APP_FLOW.md`
**Fontes:** F1 Steps 2-4 · F2 §DESIGN.md · F4 §02 · F5
**Integração CÓRTEX:** Insumo para proof.mjs (prova de cobertura de estados) e agente-frontend (escopo de telas).
**Contra-exemplo:** Começar a codar telas sem APP_FLOW.md = trabalho inválido. Tela implementada sem estado de erro definido = bug de UX.

### 19. Plano de Implementação por Fases (IMPLEMENTATION_PLAN.md)
**Regra:** Features são decompostas em fases com: tarefas numeradas, dependências, entregável verificável, critérios de aceitação, out-of-scope explícito. Uma fase de cada vez. Verificação passa (gate) antes da próxima fase. Nunca "build everything at once". Vertical slices (fatias completas) são preferidas sobre camadas horizontais.
**Arquivo:** `IMPLEMENTATION_PLAN.md`
**Fontes:** F1 Step 4 · F2 Steps 11-12 · F4 §03 · F5
**Integração CÓRTEX:** Fases são unidades de orquestração (#8). Gate usa verificar-gate hook. Cada fase gera artefato provável. Vertical slices (#19 antigo) absorvido aqui.
**Contra-exemplo:** Agente recebe "build the entire app" e tenta implementar tudo = violação. Fase N+1 começa sem gate verde na fase N = violação.

### 20. Definition of Done e Out of Scope Explícitos
**Regra:** Toda spec, PRD e Implementation Plan contêm: (1) "Definition of Done" — critérios objetivos e verificáveis de completude; (2) "Out of Scope" — lista explícita do que NÃO será feito nesta versão. Sem DoD = trabalho infinito. Sem Out of Scope = scope creep. Ambos revisados pelo auditor adversarial.
**Fontes:** F1 Step 2 · F2 Step 7 §Out of Scope · F4 TRD §8 · F5
**Integração CÓRTEX:** DoD é insumo para panel.mjs. Out of Scope verificado por scope-guard hook.
**Contra-exemplo:** Spec sem seção "Out of Scope" = agente inventa features. DoD diz "funciona" sem critério testável = declaração vazia.

### 21. Estrutura Padronizada de Projeto de Agente IA
**Regra:** Projetos de agente IA seguem estrutura modular:
```
src/agents/      # Definição de agentes
src/memory/      # Gestão de memória (curto/longo prazo)
src/workflows/   # Orquestração e DAGs
src/tools/       # Ferramentas disponíveis
src/evaluation/  # Datasets e métricas
src/app/         # Interface/aplicação
config/          # Configurações
scripts/         # Automações
docs/            # Documentação
```
Scaffolding automatizado via script.
**Arquivo:** `PROJECT_STRUCTURE.md` ou scaffold automático
**Fontes:** F6 (criar_estrutura.py) · F7 (estrutura-projeto-agente-ia.md)
**Integração CÓRTEX:** Estrutura alinha com órgãos (agents=execução, memory=cérebro/tálamo, workflows=orquestrar, evaluation=auditar/verificar).
**Contra-exemplo:** Projeto de agente com tudo em um único arquivo = violação. Memória misturada com lógica de agente = violação.

### 22. Avaliação e Benchmarking de Agentes
**Regra:** Todo agente tem métricas definidas e medidas: taxa de sucesso, fidelidade, latência, custo por tarefa, cobertura de testes. Dataset de avaliação versionado. Benchmarks rodados antes de promover agente para produção. "Funciona" ≠ "é bom o suficiente".
**Arquivo:** `EVALUATION.md` ou `src/evaluation/`
**Fontes:** F2 Steps 22-23 · F7 §src/evaluation
**Integração CÓRTEX:** Alimenta agent-track.mjs. Auditor adversarial valida métricas. Métricas são artefatos prováveis.
**Contra-exemplo:** Agente promovido sem benchmark = violação. Métrica "parece bom" sem número = violação.

---

## Camada 2: Princípios Anti-Slop

1. **Nunca aceite "parece funcionar" como prova.** Prova = artefato verificável (teste verde, screenshot, log, métrica).
2. **Nunca comece a codar sem spec aprovada.** Codar sem spec = construir sem planta. O retrabalho custa 3-10x mais.
3. **Nunca pule o estado de erro.** Se você não definiu o que acontece quando algo falha, você não definiu o que acontece.
4. **Nunca documente depois.** Documentação pós-fato descreve um sistema que já mudou. Documente ANTES como contrato.
5. **Nunca confie em memória de conversa.** Se não está em disco (Memory.md, DECISIONS.md, HANDOFF.md), não existe na próxima sessão.
6. **Nunca promova agente sem benchmark.** "Funciona no meu teste" ≠ "funciona em produção". Métricas versionadas são obrigatórias.
7. **Nunca omita o Out of Scope.** O que você NÃO vai fazer é tão importante quanto o que vai. Sem isso, o escopo cresce infinitamente.

---

## Camada 3: O Loop Profissional (Obrigatório para Toda Feature)

```
READ → UNDERSTAND → PLAN → IMPLEMENT → TEST → REVIEW → FIX → COMMIT → UPDATE DOCS
```

Este loop é INEGOCIÁVEL. Pular etapas = trabalho inválido.

---

## Camada 4: Penalidade

Violação de qualquer conceito = trabalho inválido. O artefato produzido é descartado e refeito seguindo os conceitos. Não há exceções.

---

## Camada 5: Arquivos Obrigatórios por Projeto

| Arquivo | Propósito | Quando Criar | Conceito |
|---------|-----------|-------------|----------|
| `PRD.md` | O que estamos construindo e por quê | Antes de qualquer código | #1 |
| `ARCHITECTURE.md` | Como o sistema funciona (UML/fluxo) | Antes de qualquer código | #2 |
| `DESIGN_SYSTEM.md` | Contrato visual (tokens, componentes, estados) | Antes de qualquer UI | #3 |
| `AGENTS.md` / `RULES.md` | Como a IA deve trabalhar neste projeto | Antes de delegar à IA | #4 |
| `APP_FLOW.md` | Jornada do usuário (telas, ações, estados) | Antes de qualquer UI | #18 |
| `IMPLEMENTATION_PLAN.md` | Fases de construção com verificação | Antes de codar | #19 |
| `TASKS.md` | Board estruturado (Next/Progress/Done/Blocked/Backlog) | Início, atualizado continuamente | #7 |
| `DECISIONS.md` | ADRs (Architecture Decision Records) | Sempre que decisão técnica for tomada | #7 |
| `MEMORY.md` | Estado atual, lições, failed approaches | Início, atualizado a cada sessão | #7 |
| `TESTING.md` | Checklist de testes por tipo | Antes de implementar | #6 |
| `SECURITY.md` | Requisitos de segurança | Antes de qualquer código | #12 |
| `.env.example` | Variáveis de ambiente necessárias | Setup inicial | #12 |
| `EVALUATION.md` | Métricas e benchmarks de agentes | Antes de promover agente | #22 |
| `HANDOFF_[fase].md` | Mensagem de handoff entre fases/agentes | Ao completar cada fase | #8 |
| `PROJECT_STRUCTURE.md` | Estrutura padronizada do projeto | Setup inicial (ou scaffold auto) | #21 |

---

## Camada 6: Templates de Prompts para Revisão

### Review de PRD
"Leia o PRD.md e responda: (1) O problema é real e específico? (2) Os usuários-alvo estão definidos com nome e dor? (3) O escopo negativo está explícito? (4) Há critérios de sucesso mensuráveis? (5) O que falta para começar a codar?"

### Transformar APP_FLOW em Checklist de Telas
"Leia o APP_FLOW.md e gere uma tabela: Tela | Estados Obrigatórios | Critérios de Aceitação | Dependências. Marque estados não definidos como GAP."

### Planejar Fase de Implementação
"Com base no IMPLEMENTATION_PLAN.md, detalhe a Fase N: (1) Liste tarefas atômicas. (2) Identifique dependências. (3) Defina entregável verificável. (4) Liste riscos. (5) Estime complexidade. Não adicione tarefas fora do escopo da fase."

### Review de Spec Antes de Implementar
"Leia a spec e verifique: (1) Todos os arquivos relevantes estão listados? (2) Constraints estão explícitas? (3) Critérios de aceitação são testáveis? (4) Out of scope está definido? (5) Há ambiguidade que exija clarificação antes de codar?"

### Handoff entre Agentes
"Gere mensagem de handoff contendo: (1) Contexto aprovado. (2) Artefatos gerados com paths. (3) Blockers conhecidos. (4) Próximo passo esperado. (5) Perguntas abertas. Formato: markdown estruturado, salvo em HANDOFF_[fase].md."

### Benchmark de Agente
"Execute o dataset de avaliação em EVALUATION.md contra o agente X. Reporte: taxa de sucesso, latência p95, custo médio, falhas categorizadas. Compare com baseline anterior. Se regressão > 5%, bloqueie promoção."

---

## Camada 7: Matriz de Rastreabilidade: Fonte → Conceito

| Conceito | F1 | F2 | F3 | F4 | F5 | F6 | F7 | Diferencial CÓRTEX |
|----------|----|----|----|----|----|----|----|--------------------|
| #1 PRD | ✓ | ✓ | ✓ | ✓ | ✓ | | | Seções obrigatórias expandidas |
| #2 Arquitetura | | ✓ | ✓ | | | ✓ | ✓ | UML obrigatório para sistemas complexos |
| #3 Design System | ✓ | ✓ | ✓ | ✓ | ✓ | | | Template de tokens + estados obrigatórios |
| #4 Regras do Agente | | ✓ | ✓ | | | | | Enforced por hooks automáticos |
| #5 Spec Antes de Código | ✓ | ✓ | | ✓ | ✓ | | | Critérios de aceitação testáveis |
| #6 TDD | | ✓ | | ✓ | ✓ | | | Mordida + TESTING.md estruturado |
| #7 Documentos Vivos | | ✓ | | ✓ | ✓ | | | Template mínimo definido |
| #8 Orquestração | | ✓ | | | | ✓ | ✓ | Track-record + handoff estruturado |
| #9 Code Review Gate | | ✓ | | | | | | Juiz≠gerador + hook automático |
| #10 Qualidade IA | | ✓ | | | | | ✓ | Auditoria isolada (agente-auditar) + proof.mjs |
| #11 Monitoramento | | ✓ | | | | | ✓ | Relatório diário consolidado |
| #12 Segurança | | ✓ | ✓ | | | | | Camada zero + SECURITY.md |
| #13 Multiusuário | | | | | | | | Enforced server-side |
| #14 Dashboard Vivo | | | | | | | | Integração com ops-sentinel |
| #15 Git | ✓ | ✓ | | | ✓ | | | Commits pequenos + branches |
| #16 Deploy Estágios | | ✓ | | | | | ✓ | LOCAL→PREVIEW→QA→PROD |
| #17 Documentação Viva | | ✓ | ✓ | ✓ | ✓ | | | Atualizada a cada sessão |
| #18 User Journey | ✓ | ✓ | | ✓ | ✓ | | | APP_FLOW.md obrigatório |
| #19 Implementation Plan | ✓ | ✓ | | ✓ | ✓ | | ✓ | Fases gateadas + verificação |
| #20 DoD + Out of Scope | ✓ | ✓ | | ✓ | ✓ | | | Revisado por auditoria isolada (agente-auditar) |
| #21 Estrutura Agente IA | | | | | | ✓ | ✓ | Alinhada com órgãos CÓRTEX |
| #22 Evaluation/Benchmark | | ✓ | | | | | ✓ | Dataset versionado + métricas |

---

## Camada 8: Integração com o Harness Existente

- **LICOES.md** → Armadilhas e becos que refinam a aplicação destes conceitos
- **CLAUDE.md** → Constituição inviolável (prova por artefato, juiz≠gerador, sigilo)
- **orquestrar skill** → Motor de orquestração multi-agente (Conceitos #8, #19)
- **proof.mjs** → Prova estruturada por artefato (Conceitos #6, #10, #20)
- **panel.mjs** → Painel adversarial de juízes (Conceitos #10, #20)
- **zen.mjs** → Delegação de código para motor especializado (Conceito #4)
- **agent-track.mjs** → Track-record medido para escolha de agentes (Conceitos #8, #22)
- **hooks** → Gatilhos automáticos que enforce conceitos no momento certo
- **APP_FLOW.md** → Insumo para agente-frontend e proof.mjs (Conceito #18)
- **IMPLEMENTATION_PLAN.md** → Unidade de orquestração e gate de verificação (Conceito #19)
- **EVALUATION.md** → Alimenta agent-track.mjs e auditoria isolada (agente-auditar) (Conceito #22)
- **HANDOFF_[fase].md** → Protocolo de transição entre fases/agentes (Conceito #8)
- **Princípios Anti-Slop** → Filtro de qualidade antes de qualquer entrega
- **Protocolo de Evolução** → Garante que este documento nunca fique obsoleto

## Camada 9: Skills Hermes Documentais

### Skills de Documentação Operacional (Zylos-Next)

Skills especializadas criadas para garantir conformidade com melhores práticas de mercado (Cutover, OpServices, Belago, Elastic, SolarWinds, TecnetOne):

- **runbook-builder** → Gera workflows multi-agente reutilizáveis como runbooks com estrutura 3-tier (Conceitos #5 Spec, #6 TDD, #7 Docs Vivos)
- **playbook-author** → Cria playbooks estratégicos com RACI, gates de decisão e cross-linking tridirecional (Conceitos #1 PRD, #2 Arquitetura, #7 Docs Vivos)
- **sop-author** → Cria SOPs com estrutura obrigatória de 5 seções e matriz de decisão (Conceitos #5 Spec, #7 Docs Vivos, #19 Implementation Plan)
- **pop-author** → Cria POPs para componentes específicos com rollback obrigatório e 7 seções (Conceitos #5 Spec, #12 Segurança)
- **soar-orchestration** → Projeta workflows SOAR, event schemas, auto-triage determinística e audit trail imutável (Conceitos #12 Segurança, #22 Evaluation)
- **aiops-observability** → Configura métricas, alertas, dashboards e detecção de anomalias integrados ao SOAR (Conceitos #22 Evaluation, #12 Segurança)

### Skills Hermes de Documentos Canônicos (Manifesto v3.1)

Skills que implementam diretamente os 22 conceitos canônicos como procedimentos automatizados:

- **prd-author** → Cria PRDs com 8 seções obrigatórias incluindo DoD e Out of Scope (Conceito #1)
- **architecture-author** → Cria ARCHITECTURE.md com UML obrigatório e tabela de decisões (Conceito #2)
- **design-system-author** → Cria DESIGN_SYSTEM.md com tabela de tokens e 6 estados de UI (Conceito #3)
- **agents-rules-author** → Cria AGENTS.md/RULES.md com regras enforceáveis e boundaries (Conceito #4)
- **spec-author** → Cria specs técnicas granulares com critérios testáveis antes de codar (Conceito #5)
- **testing-checklist-author** → Cria TESTING.md com checklist estruturado de 8 tipos de teste (Conceito #6)
- **project-memory-author** → Cria MEMORY.md, DECISIONS.md e TASKS.md estruturados (Conceito #7)
- **handoff-author** → Cria HANDOFF_[fase].md com protocolo estruturado de 5 seções (Conceito #8)
- **continuous-monitoring** → Configura agentes sentinelas de monitoramento diário (Conceito #11)
- **access-control-author** → Define roles, permissões e boundaries server-side (Conceito #13)
- **dashboard-author** → Cria dashboards como SO vivo com métricas em tempo real (Conceito #14)
- **staged-deploy-author** → Define pipeline LOCAL→PREVIEW→QA→PROD com gates (Conceito #16)
- **implementation-plan-author** → Cria IMPLEMENTATION_PLAN.md com fases gateadas e vertical slices (Conceito #19)
- **acceptance-criteria-author** → Define DoD e Out of Scope testáveis por spec/PRD (Conceito #20)
- **project-structure-author** → Cria PROJECT_STRUCTURE.md com scaffold modular padrão (Conceito #21)
- **app-flow-author** → Cria APP_FLOW.md com jornada do usuário e estados obrigatórios por tela (Conceito #18)

### Skills Hermes de Camadas Estruturais (Manifesto v3.1)

Skills que implementam as camadas não-conceituais do manifesto como procedimentos automatizados:

- **anti-slop-filter** → Valida entregas contra os 7 princípios anti-slop (Camada 2)
- **professional-loop** → Enforce o ciclo READ→...→UPDATE DOCS em toda tarefa (Camada 3)
- **review-prompts** → Aplica os 6 templates de revisão obrigatórios antes de aprovar artefatos (Camada 6)

### Skills Hermes de Orquestração Avançada

Skills nativas do ecossistema Hermes que implementam protocolos operacionais avançados de verificação e coordenação:

- **mordida-protocol** → Prova por mutação com 4 diagnósticos e sha256 antes/depois (Conceito #6 TDD)
- **isolated-audit** → Auditoria isolada contexto-zero que tenta REFUTAR, juiz≠gerador (princípio fundamental)
- **subagent-driven-dev** → Decomposição em subagentes com single-writer e contexto isolado (coordenação multi-agente)

**Exemplo de conformidade:** A documentação operacional do Zylos-Next (`runbooks/`, `playbooks/`, `docs/soar/`) foi construída seguindo estas skills e atingiu 100% de alinhamento com 29 critérios de qualidade em análise comparativa (`GAP_ANALYSIS_FASE7B_7C.md`).

---

## Camada 10: Protocolo de Evolução e Lições Aprendidas

### Protocolo de Evolução deste Documento

Este documento NÃO é estático. Ele evolui conforme:
1. **Novas fontes** são analisadas (repositórios, guias, papers)
2. **Lições aprendidas** em sessões reais (via LICOES.md e evoluir skill)
3. **Mudanças no harness** (novos hooks, agentes, skills)
4. **Auditorias periódicas** (trimestral, via agente-auditar)

**Processo de atualização:**
1. Identificar lacuna (nova fonte, lição, auditoria)
2. Propor mudança neste documento (PR ou edição direta)
3. Validar com auditoria isolada (agente-auditar, juiz≠gerador)
4. Atualizar matriz de rastreabilidade
5. Comunicar mudança via MEMORY.md e DECISIONS.md

**Versionamento:** Este documento segue versionamento semântico (MAJOR.MINOR.PATCH).
- MAJOR: conceito removido ou redefinido
- MINOR: novo conceito ou expansão significativa
- PATCH: correção de texto, exemplo, ou referência

---

## Lições Aprendidas na Criação de Skills Canônicas (2026-11-10)

1. **Descrição de skill tem limite de 60 caracteres.** O `skill_manage` rejeita descrições maiores e faz rollback de todo o batch. Escreva a descrição como trigger + comportamento em uma frase curta; detalhes vão no corpo da skill.
2. **Batch de creates é atômico.** Se um item falha, todos os anteriores são revertidos. Valide descrições antes de submeter batches grandes.
3. **Skills existentes podem ter nomes desatualizados.** Sempre verifique com `skills_list` antes de referenciar em documentação externa. Nomes como `runbook-authoring` e `playbook-authoring` foram substituídos por `runbook-builder` e `playbook-author`.
4. **Cross-linking entre skills deve ser bidirecional.** Atualizar `related_skills` em ambas as direções (ex: `playbook-author` referencia `sop-author` E `sop-author` referencia `playbook-author`). Links unidirecionais criam gaps de descoberta.
5. **Manifesto e README devem ser atualizados na mesma sessão da criação das skills.** Postergar gera divergência entre o que existe e o que está documentado.

---