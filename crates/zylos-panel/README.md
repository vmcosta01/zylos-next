# zylos-panel

**Fase:** 7B.1.3 — Padrões CORTEX
**Status:** ⚠️ Parcial (~30% do plano)

## Descrição

Painel Adversarial Determinístico com guardrails, detector de falso-sucesso e votação por refutação aterrada. Garante que resultados sejam julgados por múltiplas lentes independentes.

## Tipos Principais

- `LenteCanonica` — 6 lentes: Funcionalidade, Estrutura, Performance, Segurança, Simplicidade, Testes
- `JudgeVote` — Voto individual com refutação aterrada e vestígios
- `PanelDecision` — Decisão agregada com confiança e métricas
- `Confidencia` — Alta/Média/Baixa baseada em consenso

## Gaps vs Master Plan

- ❌ `guardrail_deterministico()` com curto-circuito resultado óbvio-ruim
- ❌ Detector de falso-sucesso (encerramento sem vestígio)
- ❌ `filtrar_auto_julgamento()` (juiz ≠ gerador estrutural)
- ❌ `aggregate_votes()` onde 1 refutação aterrada derruba maioria
- ❌ `vote_confidence()` alta/média/baixa baseada em consenso

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)

## Fonte Original

`~/.claude/skills/orquestrar/scripts/panel.mjs` (197 linhas)
