# Handoff — Fase 1: Design System Unificado

**Versão:** 1.0.0
**Data:** 2026-11-18
**De:** Planejamento (Subagent)
**Para:** Implementador Fase 1
**Conceito Manifesto:** #8 (Orquestração multi-agente com handoff estruturado)

---

## 1. Contexto Aprovado

O planejamento completo do Cockpit v2.0 foi produzido seguindo todas as 10 camadas do manifesto CONCEITOS-DESENVOLVIMENTO-IA.md v3.1. Os seguintes documentos foram criados e aprovados como contrato:

- ✅ PRD.md — Visão, personas, features, DoD, Out of Scope
- ✅ ARCHITECTURE.md — C4 diagram, tech stack, fluxos de dados, estrutura de arquivos
- ✅ DESIGN_SYSTEM.md — Tokens completos, 6 estados, springs, translucency, acessibilidade
- ✅ APP_FLOW.md — 11 telas mapeadas com 6 estados cada
- ✅ IMPLEMENTATION_PLAN.md — 8 fases gateadas com tarefas atômicas
- ✅ TESTING.md — Checklist 8 tipos + mordida protocol
- ✅ SECURITY.md — Threat model, CSP, token policy, egress checker
- ✅ EVALUATION.md — Métricas UX, benchmarks WASM, Apple audit rubric
- ✅ DECISIONS.md — ADR-007 a ADR-010 (herdadas ADR-001 a ADR-006)

**A Fase 1 (Design System Unificado) está pronta para execução.**

---

## 2. Artefatos Gerados

| Arquivo | Path | Tamanho | Status |
|---------|------|---------|--------|
| PRD.md | `docs/cockpit-v2-redesign/PRD.md` | ~15KB | ✅ Completo |
| ARCHITECTURE.md | `docs/cockpit-v2-redesign/ARCHITECTURE.md` | ~15KB | ✅ Completo |
| DESIGN_SYSTEM.md | `docs/cockpit-v2-redesign/DESIGN_SYSTEM.md` | ~21KB | ✅ Completo |
| APP_FLOW.md | `docs/cockpit-v2-redesign/APP_FLOW.md` | ~13KB | ✅ Completo |
| IMPLEMENTATION_PLAN.md | `docs/cockpit-v2-redesign/IMPLEMENTATION_PLAN.md` | ~23KB | ✅ Completo |
| TESTING.md | `docs/cockpit-v2-redesign/TESTING.md` | ~11KB | ✅ Completo |
| SECURITY.md | `docs/cockpit-v2-redesign/SECURITY.md` | ~9KB | ✅ Completo |
| EVALUATION.md | `docs/cockpit-v2-redesign/EVALUATION.md` | ~8KB | ✅ Completo |
| DECISIONS.md | `docs/cockpit-v2-redesign/DECISIONS.md` | ~6KB | ✅ Completo |
| HANDOFF_fase1.md | `docs/cockpit-v2-redesign/HANDOFF_fase1.md` | Este arquivo | ✅ Completo |
| README.md | `docs/cockpit-v2-redesign/README.md` | ~3KB | ✅ Índice |

**Todos os artefatos estão em:** `C:/Users/Paulo/zylos-next/docs/cockpit-v2-redesign/`

---

## 3. Blockers Conhecidos

| # | Blocker | Severidade | Mitigação |
|---|---------|------------|-----------|
| B1 | Imagem phone-relay não analisável por vision tool | Baixa | Descrição textual obtida via segunda tentativa; suficiente para planejamento |
| B2 | leptos-spring compatibilidade com Leptos 0.7 não verificada | Média | ADR-007 prevê custom solver como fallback; decidir na Fase 3 |
| B3 | CSS atual não auditado linha-a-linha | Baixa | Tarefa 1.1 do Implementation Plan cobre auditoria completa |
| B4 | Backend endpoints VNC NÃO implementados: `/api/v1/computer-view/:session/lease`, `/api/v1/screenshots/:session/latest` | **Alta** | **Criar endpoints no gateway ANTES da Fase 5 frontend** |

---

## 4. Próximo Passo Esperado

### Executar Fase 1: Design System Unificado

**Objetivo:** Eliminar conflitos CSS e estabelecer `design-tokens.css` como single source of truth.

**Tarefas (copiadas do IMPLEMENTATION_PLAN.md):**

1. Auditar `design-tokens.css` e `style.css` existentes → relatório de conflitos
2. Escrever `styles/design-tokens.css` completo (DESIGN_SYSTEM.md §12)
3. Criar `styles/base.css` com reset/utilities
4. Migrar componentes existentes para `var()` tokens
5. Eliminar `style.css`
6. Adicionar CI lint rule (zero hardcoded colors)
7. Atualizar `index.html` para fonts + CSS

**Gate de verificação:**
```bash
test ! -f styles/style.css
grep -r '#[0-9a-fA-F]\{6\}' src/ | wc -l | grep '^0$'
trunk build --profile wasm-release
```

**Estimativa:** 2-3 dias

---

## 5. Perguntas Abertas

| # | Pergunta | Para Quem | Prioridade |
|---|----------|-----------|------------|
| Q1 | Existem componentes inline com cores hardcoded além dos identificados na auditoria inicial? | Implementador Fase 1 | Alta |
| Q2 | Inter font deve ser self-hosted ou manter Google Fonts CDN? (Offline support vs bundle size) | Tech Lead | Média |
| Q3 | Há preferência por stylelint config específica ou usar preset recomendado? | Implementador Fase 1 | Baixa |
| Q4 | CI pipeline já tem stage para lint CSS ou precisa adicionar? | DevOps | Média |

---

## 6. Recomendações para o Implementador

1. **Leia DESIGN_SYSTEM.md §12 ANTES de começar** — contém o CSS completo copy-paste ready
2. **Não pule a auditoria (tarefa 1.1)** — descobrir conflitos desconhecidos agora evita retrabalho
3. **Commit pequeno por tarefa** — cada tarefa numerada = um commit mínimo
4. **Visual regression baseline ANTES de migrar** — screenshot de todas as telas atuais para comparar após migração
5. **Teste em viewport mobile (375px)** — CSS migration frequentemente quebra responsividade
6. **Atualize TASKS.md e MEMORY.md ao final da fase** — Conceito #7 exige documentos vivos
7. **Gere HANDOFF_fase2.md ao completar** — próximo implementador precisa do mesmo protocolo

---

## 7. Compliance Checklist do Handoff

| Requisito Manifesto #8 | Status |
|------------------------|--------|
| Contexto aprovado documentado | ✅ Seção 1 |
| Artefatos gerados com paths | ✅ Seção 2 |
| Blockers conhecidos listados | ✅ Seção 3 |
| Próximo passo esperado claro | ✅ Seção 4 |
| Perguntas abertas identificadas | ✅ Seção 5 |
| Formato salvo em disco (HANDOFF_fase1.md) | ✅ Este arquivo |

---

## 8. Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 1.0.0 | 2026-11-18 | Initial handoff Fase 1; planning artifacts complete; ready for implementation |