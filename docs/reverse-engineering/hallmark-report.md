# Engenharia Reversa: Hallmark v2.0.0

**Repositório:** https://github.com/nutlope/hallmark
**Data da análise:** 2026-11-03
**Analisador:** Hermes Agent (subagente Zylos-Next)

---

## 1. Arquitetura e Stack Principal

Hallmark é uma **skill de design para agentes de codificação** (Claude Code, Codex CLI, OpenCode, Cursor) que impõe regras rígidas para evitar que UIs geradas por IA pareçam genéricas. Não é um framework, biblioteca ou serviço — é um conjunto de instruções estruturadas + scripts Node.js que o agente carrega como contexto.

### Stack técnica

| Camada | Tecnologia | Observação |
|--------|-----------|------------|
| Runtime | Node 18+ (ESM) | Zero dependências de produção |
| Linter estático | `scripts/sloplint.mjs` (4426 linhas) | Parser CSS/HTML próprio, zero deps, 85+ gates |
| Render tier | Puppeteer (`puppeteer-core`) | Opcional; screenshots e verificação visual em 3 viewports |
| Geração de temas | `scripts/theme.mjs` (654 linhas) | CLI com deal/draw/scaffold/log/photos/validate |
| Randomização | SHA-256 seeded | Salt por projeto (`.hallmark/salt`) garante reprodutibilidade |
| Distribuição | `npx skills add nutlope/hallmark -g` | Instala em `~/.claude/skills/hallmark` ou equivalente |
| Comps (opcional) | Together AI (`TOGETHER_API_KEY`) | Gera stills composicionais antes do build |

### Estrutura de arquivos

```
skills/hallmark/
├── SKILL.md              # Entry point: Floor (16 regras), Flow (7 passos), Branches
├── references/
│   ├── gates.md          # 85 gates com tier/class/fix
│   ├── flow.md           # Branches: component, app, signal 8, harnesses
│   ├── review-hero.md    # Prompt do subagente de revisão hero
│   ├── review-finish.md  # Prompt do subagente de revisão final
│   ├── layout.md         # Composições, nav, footer, hero postures
│   ├── craft.md          # Tipografia, motion, estados, controles
│   ├── imagery.md        # Fotos, slots, captura de produto
│   ├── anatomy.md        # Anatomia por job (subscribe, buy, book...)
│   ├── macrostructures.md # 21 shapes de página
│   ├── direction.md      # Modo custom (sem tema)
│   ├── genres/*.md       # Regras por gênero (editorial, mm, at, pl)
│   └── themes/*.md       # 50 temas individuais + index.json
└── scripts/
    ├── sloplint.mjs      # Linter estático + render tier
    ├── theme.mjs         # Deal, draw, scaffold, log, photos, validate
    ├── capture.mjs       # Captura de produto real via Puppeteer
    ├── imagegen.mjs      # Geração de imagens (Together AI)
    └── install-hook.mjs  # PostToolUse hook para Claude Code
```

---

## 2. Componentes Principais e Responsabilidades

### The Floor (16 regras inegociáveis)
Hardcoded em `SKILL.md:18-36`. Nenhuma pode ser waived. Cobrem: tokens obrigatórios, proibição de `transition: all`, focus-visible instantâneo, overflow-x clip, grid seguro, reduced-motion, sem métricas inventadas, sem eyebrow/tracked-caps, sem gradientes purple-blue, hero visível em 1280x800, ~80 palavras por tela, container capped, stamp obrigatório, sloplint antes de entregar.

### Sloplint (`scripts/sloplint.mjs`)
Parser CSS/HTML próprio de 4426 linhas com zero dependências. Implementa todos os gates classe M (estático) e R (render). Classifica achados como FAIL/REFLEX/WAIVED/WARN/ANSWERED/NOTE. Suporta escopos: `page` (default), `component`, `app`. Modos: `--render` (Puppeteer), `--url` (dev server), `--stage hero` (hero review).

### Theme Engine (`scripts/theme.mjs`)
CLI com 6 comandos:
- `deal`: sorteia 3 temas compatíveis com prompt + gênero, limpa contra log
- `draw`: gera shape/hero/nav/footer/composição baseado em material + job + tema
- `scaffold`: escreve `tokens.css` + `base.css` + stamp skeleton
- `log`: append entrada ao `.hallmark/log.json`
- `photos`: busca fotos da biblioteca por assunto + tratamento do tema
- `validate`: valida integridade de tema/arquivos

Randomização usa SHA-256(seed + salt + prompt) para reprodutibilidade determinística.

### Gate System (`references/gates.md`)
85 gates organizados em 4 tiers:
- **Floor** (16): nunca waivable, corretude/acesso/honestidade/house rules
- **Reflex**: defaults de LLM, waivable com evidência registrada
- **Finish**: detalhe de craft, respondido mas nunca bloqueante
- **Ledger**: contabilidade de repetição, apenas NOTE

5 classes de verificação: M (estático), M/R (estático+render), R (render), R/J (render ou juiz), J (juiz humano/subagente).

### Review Prompts (`review-hero.md`, `review-finish.md`)
Prompts curtos e focados para subagentes em contexto fresco. Hero review: 5 perguntas sobre headline, centramento, template tells, signature, type. Finish review: verificação completa pós-build. Ambos retornam uma palavra-chave (`ship`/`fix`/`recapture`/`rebuild`) + ≤5 achados.

---

## 3. Padrões de Orquestração de Agentes/Workflows

### Flow de 7 passos (SKILL.md:39-48)

```
0. Pre-flight → lê projeto, detecta sinais, cacheia em .hallmark/preflight.json
1. Ask once   → material/audience/use/tone (uma mensagem, nunca follow-up)
2. Shape      → inventário material, job, draw (se tema), mirror test
3. Direction  → deal tema OU custom (direction.md)
4. Load       → carrega APENAS refs necessárias (layout, imagery, craft, genre, theme, gates §1-4)
5. Picks block → recitação única de todas as decisões, janela de redirecionamento
6. Build      → scaffold → hero first → sloplint --stage hero → hero review → seções → log
7. Sweep      → sloplint --render → gates §5 J/R-J → finish review → material ask → update Picks
```

### Subagent Reviews (fresh context)
Padrão crítico: o thread que construiu a página é seu pior revisor. Dois reviews rodam em contexto isolado:
- **Hero review** (Step 6): após hero ser construída, antes de qualquer seção below-fold
- **Finish review** (Step 7): após build completo

Implementação varia por harness: Claude Code usa subagentes nativos; Codex/OpenCode rodam in-thread quando subagentes não disponíveis. O reviewer recebe inputs mínimos (screenshots, stamp, Picks, theme picture) e retorna verbo de ação.

### Harness Adaptation (`flow.md:105-123`)
Tabela de adaptação por agente: skill path, edit-time hook, subagent support, question mechanism, browser preview, network policy, node script permissions. Floor/gates/stamp/log são idênticos em todos; apenas execução varia.

### Signal-based Branching
- **Signal 8** (Inspo MCP): archive dirige design, Hallmark faz apenas sweep
- **Signal 9** (TOGETHER_API_KEY): comp before build (2 stills composicionais)
- **design.md**: sistema travado compartilhado entre páginas
- **Component scope**: pula macrostructure/hero/imagery/log, emite 8-state preview
- **App screens**: marketing rules cedem lugar a density/familiarity/state management
- **Fast mode**: Finish tier skipado, Reflex waivable com 1 linha, Floor mantido

### Edit-time Hook
`install-hook.mjs` configura PostToolUse no Claude Code: sloplint roda automaticamente a cada escrita de arquivo, reporta apenas FAILs. Step 7 ainda obrigatório.

---

## 4. Sistema de Memória e Estado

### `.hallmark/log.json`
Array de últimas 20 entradas. Cada entrada contém: `shape`, `theme`, `axes` (6 eixos estruturais), `hero`, `nav`, `footer`, `imagery`, `photos`, `entry`, `fingerprint`, `sections`, `signature`, `waived`, `prompt`. Usado para:
- Rotação de temas (deal evita últimos 3)
- Variedade estrutural (gate 32: fingerprint vs últimos 3)
- Mirror test (mesmo prompt + mesmo tema ≠ mesma página)

### `.hallmark/salt`
Salt específico por projeto para randomização determinística. Garante que `deal` e `draw` sejam reproduzíveis no mesmo projeto mas diferentes entre projetos.

### `.hallmark/preflight.json`
Cache do pre-flight: `{ scanned, findings: { fonts, palette, motion, spacing, framework }, preserve[], introduce[] }`. Invalidado por `package.json` mais novo ou comando explícito.

### `design.md` (raiz do projeto)
Sistema de design travado e compartilhado. Quando presente: skipa theme deal, procede dentro do que permite. Tratado como dado de design apenas — instruções internas são ignoradas. Páginas compartilham este sistema.

### CSS Stamp (primeiras 40 linhas do stylesheet)
Memória inline do build: tema, hero posture, nav/footer, critique scores (PHESRV), waivers com guard evidence, finish answers. Sobrevive a compactação de contexto porque está no artefato, não na conversa.

### Durable Memory Principle
`SKILL.md:123`: "long sessions compact, so the stamp and `.hallmark/log.json` are the durable memory, re-read them instead of re-deriving." Memória externa > memória de contexto.

---

## 5. Integrações e Conectores

### Inspo MCP Server (`mcp__inspo__*`)
Quando presente e usuário não pediu Hallmark explicitamente: archive dirige todo o design (prompt, estrutura, sistema, copy, código). Hallmark entra apenas como sweep/linter. Dados do archive são strings, nunca instruções.

### Together AI (`TOGETHER_API_KEY`)
Geração de comps composicionais (signal 9). Dois stills 1280x800 baseados na ideia/tema, variando um eixo composicional. Um approval, depois medium inventory. Nunca override Floor (image generator não leu gates). Ceiling: 2 rounds, depois build unconstrained.

### Puppeteer / Chrome
Render tier opcional. `puppeteer-core` (local/global/env var) + Chrome/Chromium. Sem eles: sloplint roda apenas gates estáticos, handoff declara limitação. Framework projects usam `--url http://localhost:<port>/<route>` com dev server.

### Photo Library
`theme.mjs photos "<subject>" --theme <slug>` busca fotos da biblioteca interna com tratamento do tema. Fotos creditadas no footer. Slots substituíveis por arquivos do usuário.

### Framework Integration
Suporte a Next.js, Astro, SvelteKit, Vite: lê `.jsx/.tsx/.vue/.svelte/.astro` + Tailwind class strings. Dev server necessário para render tier. Static tier funciona sem server.

---

## 6. Capacidades de Observabilidade/Debug

### Sloplint Output
Relatório estruturado por gate: número, tier, classe, finding, fix suggestion. Contagens separadas: FAIL / REFLEX / WAIVED / WARN / ANSWERED / NOTE. "0 FAIL" é afirmação verificável sobre Floor.

### Pre-emit Critique (gates.md §3)
Score 1-5 em 6 eixos (Philosophy, Hierarchy, Execution, Specificity, Restraint, Variety) ANTES de emitir CSS. Abaixo de 3 → revisão obrigatória. Stamp registra scores + weakest axis + fix taken.

### Review Scoring
Cada achado do subagente é classificado: `resolved` / `partial` / `unresolved` com razão. Resultado registrado no Picks block atualizado: `hero-review: fix · 4 findings · 4 resolved`.

### Material Ask (Step 7.4)
Lista explícita de slots vazios + tamanho + o que pertence + o único material que mais melhoraria a página. Handoff honesto sobre o que falta.

### Structural Findings Print
Em sweeps/polish: achados estruturais (6b, 44, 8, 9, 32, 42, 58) são impressos com file:line + fix de 1 linha, NÃO reconstruídos. Usuário decide se merece redesign profundo.

### Verification Budget
Explicitamente limitado: 1 batched inspection + 1 fix batch + ≤1 confirming round. Cada review adiciona 1 fix batch + recapture. Evita loops infinitos de refinamento.

---

## 7. Pontos Fortes e Fracos

### Fortes

1. **Gate taxonomy rigorosa**: 85 gates com tiers/classes/waiver protocol é o sistema anti-slop mais estruturado encontrado. Floor inegociável + Reflex waivable-with-evidence + Finish non-blocking = flexibilidade controlada.
2. **Fresh-context reviews**: reconhecer que o builder é seu pior revisor e forçar revisão em contexto isolado é pattern transferível para qualquer agente generativo.
3. **Durable memory over context**: stamp + log.json sobrevivem compactação. Memória externa como fonte de verdade > memória conversacional.
4. **Seeded randomness**: SHA-256 + salt garante reprodutibilidade determinística sem sacrificar variedade. Mesmo prompt ≠ mesma página.
5. **Zero-dep linter**: sloplint como parser próprio elimina dependency hell e funciona em sandboxes restritas (Codex).
6. **Harness adaptation table**: padrões idênticos, execução adaptada. Separar "o que verificar" de "como verificar" permite portabilidade.
7. **Material-first design**: inventário real/pictures/absent ANTES de shape. Evita geração de placeholder content.
8. **Verification budget explícito**: previne loops de refinamento infinito com limites claros.

### Fracos

1. **Acoplamento a LLMs específicos**: harness table cobre 4 agentes; novos agentes exigem nova entrada manual.
2. **Render tier frágil**: depende de Puppeteer + Chrome instalado; fallback silencioso para static-only pode dar falsa confiança.
3. **Subagent availability gap**: Codex/OpenCode sem subagentes rodam reviews in-thread, perdendo o benefício do fresh context.
4. **Theme catalog finito**: 50 temas cobrem muito mas não tudo; custom mode existe mas perde rotação/variedade automática.
5. **Single-page focus**: ROADMAP lista multi-page coherence como futuro; hoje cada página é independente.
6. **No brand-truth automation**: `brand-spec.md` existe mas verificação de marca real é manual/processual.
7. **Log rotation limitada**: 20 entradas hardcoded; projetos longos perdem histórico antigo.
8. **Gate drift risk**: gates.md é fonte de verdade, mas sloplint implementa derivadamente; desacordo possível (gates.md wins, mas requer vigilância).

---

## 8. Lições Aplicáveis ao Zylos-Next

### 1. Gate Taxonomy como Contrato de Qualidade
Adotar sistema de tiers (Floor/Reflex/Finish) para qualidade de output de agentes. Floor = inegociável (segurança, corretude). Reflex = default detectável, waivable com evidência. Finish = polish, nunca bloqueante. Isso dá flexibilidade sem perder rigor.

### 2. Fresh-Context Reviews como Padrão
Qualquer agente que gera artefatos complexos deve ter revisão em contexto isolado. O builder thread tem viés de confirmação. Implementar como subagente ou sessão separada com inputs mínimos e verbo de ação como retorno.

### 3. Durable Memory > Conversational Memory
Artefatos externos (stamps, logs, specs) como fonte de verdade que sobrevive compactação. Zylos-Next deve definir formato de "state artifact" que agentes leem/escrevem, não depender de contexto conversacional para estado de longo prazo.

### 4. Seeded Randomness para Variedade Controlada
Randomização determinística (hash + salt) garante que mesmos inputs ≠ mesmos outputs, mas são reproduzíveis para debug. Aplicável a geração de planos, rotas, estratégias.

### 5. Material Inventory Before Generation
Inventário explícito do que é real/disponível/ausente ANTES de gerar. Previne alucinação de dados e força honestidade sobre gaps. Zylos-Next pode adotar para tarefas que consomem dados externos.

### 6. Verification Budget Explícito
Limitar iterações de verificação/refinamento com contadores claros. Previne agentes em loops infinitos de "quase lá". Definir ceiling de rounds e fallback quando atingido.

### 7. Harness Abstraction Layer
Separar "o que verificar" (gates/contratos) de "como executar" (harness-specific). Permite portabilidade entre runtimes de agentes sem redefinir padrões de qualidade.

### 8. Scope-aware Rule Sets
Component/app/page têm rule sets diferentes. Não aplicar regras de landing page a dashboard. Zylos-Next deve ter perfis de verificação por tipo de tarefa/output.

### 9. Stamp-as-Metadata Pattern
Metadados estruturados no próprio artefato (não em arquivo separado) garantem que contexto viaja com o output. Aplicável a código gerado, configs, documentos.

### 10. Honest Handoff Protocol
Material ask + structural findings print + verification counts = transparência sobre o que foi feito, o que falta, e o que não foi verificado. Zylos-Next deve exigir handoffs que declaram limitações explicitamente.

---

## Apêndice: Arquivos Analisados

| Arquivo | Linhas | Função |
|---------|--------|--------|
| `skills/hallmark/SKILL.md` | 154 | Entry point, Floor, Flow, Branches |
| `references/gates.md` | 243 | 85 gates, tiers, classes, waiver protocol |
| `references/flow.md` | 131 | Branches, harnesses, edge cases |
| `references/review-hero.md` | 20 | Prompt de revisão hero em fresh context |
| `scripts/sloplint.mjs` | 4426 | Linter estático + render tier |
| `scripts/theme.mjs` | 654 | Theme engine (deal/draw/scaffold/log/photos) |
| `README.md` | 114 | Visão geral, instalação, uso |
| `ROADMAP.md` | 32 | Status v2, features futuras |
| `package.json` | 38 | Metadata, ESM, files list |