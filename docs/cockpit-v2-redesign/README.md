# Zylos Cockpit v2.0 Redesign — Planning Artifacts

**Status:** ✅ Planejamento Completo — Pronto para Implementação
**Data:** 2026-11-18
**Manifesto:** CONCEITOS-DESENVOLVIMENTO-IA.md v3.1 (10 camadas)

---

## Ordem de Leitura Recomendada

| # | Documento | Propósito | Tempo Estimado |
|---|-----------|-----------|----------------|
| 1 | [PRD.md](./PRD.md) | Visão, personas, features, DoD, Out of Scope | 15 min |
| 2 | [ARCHITECTURE.md](./ARCHITECTURE.md) | C4 diagram, tech stack, fluxos, estrutura | 20 min |
| 3 | [DESIGN_SYSTEM.md](./DESIGN_SYSTEM.md) | Tokens, estados, springs, translucency, a11y | 25 min |
| 4 | [APP_FLOW.md](./APP_FLOW.md) | 11 telas × 6 estados, jornadas, fluxos críticos | 20 min |
| 5 | [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md) | 8 fases gateadas, tarefas atômicas, gates | 30 min |
| 6 | [TESTING.md](./TESTING.md) | 8 tipos de teste, checklists, mordida protocol | 15 min |
| 7 | [SECURITY.md](./SECURITY.md) | Threat model, CSP, tokens, egress checker | 10 min |
| 8 | [EVALUATION.md](./EVALUATION.md) | Métricas UX, benchmarks, Apple audit rubric | 10 min |
| 9 | [DECISIONS.md](./DECISIONS.md) | ADR-007 a ADR-010 (+ herdadas ADR-001-006) | 10 min |
| 10 | [HANDOFF_fase1.md](./HANDOFF_fase1.md) | Handoff estruturado para implementador Fase 1 | 5 min |

**Tempo total de leitura:** ~2.5 horas

---

## Resumo Executivo

### Problema
Backend classe mundial (5-level proofs, triple sandbox, SQL leases, AIOps sentinel, 338+ skills) com frontend nota **2.2/10** nos princípios Apple Design. Três gaps críticos: CSS duplicado, HITL placeholder textual, Phone Relay div estática.

### Solução
Redesign completo do frontend seguindo 17 princípios Apple Design, unificando design system, implementando inline approvals (ADR-001), VNC stream (ADR-003), spring animations, materiais translúcidos, command palette, e acessibilidade completa.

### Resultado Esperado
- Apple Design score: 2.2 → ≥8.0/10
- Lighthouse Mobile: ~45 → ≥90
- HITL approval time: 12s → <3s
- Bundle WASM: 1.2MB → <800KB gzipped
- 6 estados obrigatórios em todos os componentes

---

## Compliance com Manifesto v3.1

| Camada | Conceito | Artefato | Status |
|--------|----------|----------|--------|
| 1 | #1 PRD antes de código | PRD.md | ✅ |
| 1 | #2 Arquitetura definida | ARCHITECTURE.md | ✅ |
| 1 | #3 Design System contrato | DESIGN_SYSTEM.md | ✅ |
| 1 | #18 APP_FLOW obrigatório | APP_FLOW.md | ✅ |
| 1 | #19 Implementation Plan fases | IMPLEMENTATION_PLAN.md | ✅ |
| 1 | #20 DoD e Out of Scope | PRD.md §§5-6 | ✅ |
| 1 | #22 Evaluation/Benchmark | EVALUATION.md | ✅ |
| 2 | Anti-Slop #1 Prova=artefato | Gates em IMPLEMENTATION_PLAN.md | ✅ |
| 2 | Anti-Slop #4 Documente ANTES | Todos docs criados antes de código | ✅ |
| 2 | Anti-Slop #7 Out of Scope nunca omitido | PRD.md §6 + cada fase | ✅ |
| 3 | Loop Profissional | READ→UNDERSTAND→PLAN→... | ✅ |
| 5 | 15 Arquivos obrigatórios | 11/15 criados (restante são repo-level) | ✅ |
| 6 | Templates de revisão | Skills Hermes usadas | ✅ |
| 9 | Skills Hermes | prd-author, app-flow-author, implementation-plan-author | ✅ |
| 12 | Segurança camada zero | SECURITY.md | ✅ |

---

## Próximos Passos

1. **Implementador lê este README e documentos na ordem recomendada**
2. **Executa Fase 1** conforme HANDOFF_fase1.md
3. **Gera HANDOFF_fase2.md** ao completar Fase 1
4. **Itera até Fase 8** com gates verdes entre cada fase
5. **Avaliação final** contra métricas em EVALUATION.md

---

## Onde Zylos Já Supera Todos os 9 Pares

- 5-level ground proofs (nenhum par tem)
- Adversarial panel multi-juiz
- Triple sandbox (WASM+Docker+Browser)
- SQL lease task durability
- AIOps sentinel + PII detection
- 4-tier HITL escalation
- Shadow-git audit trail
- 338+ skills registry

**Gap único resolvido por este planejamento:** Frontend.

---

## Changelog

| Versão | Data | Mudança |
|--------|------|---------|
| 1.0.0 | 2026-11-18 | Planning complete; 11 artifacts; ready for Fase 1 implementation |