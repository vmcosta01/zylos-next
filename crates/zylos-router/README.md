# zylos-router

**Fase:** 7B.2.1 — Padrões CORTEX
**Status:** ⚠️ Parcial (~30% do plano)

## Descrição

Roteamento aterrado com coleira (leash), alvos sagrados e proposta de mudança. Garante que agentes não desviem do escopo definido e nunca inventem capacidades.

## Tipos Principais

- `Capability` — Capacidade registrada com keywords, embedding e track-record
- `RouteResult` — Resultado de roteamento com score e tipo de match
- `ChangeProposal` — Proposta de mudança com risco e aprovação humana obrigatória
- `LeashConfig` — Configuração de distância máxima e alvos sagrados
- `RiskLevel` — Low/Medium/High/Critical

## Gaps vs Master Plan

- ❌ Fallback general-purpose quando score=0
- ❌ `is_real()` garantia dura nunca inventa capacidade
- ❌ `propose_change()` com requires_human=true sempre
- ❌ Alvos sagrados (freios/verificadores) com gate reforçado
- ❌ Desempate por track-record medido (feature flag opt-in)
- ❌ `route_com_proposta()` com cluster fallbacks + dedup persistente

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/router.mjs` (140 linhas)
