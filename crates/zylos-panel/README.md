# zylos-panel

**Fase:** 7B.1.3 — Padrões CORTEX
**Status:** ✅ Implementado (painel adversarial + guardrails + votação por refutação)

## Descrição

Painel Adversarial Determinístico com guardrails, detector de falso-sucesso e votação por refutação aterrada. Garante que resultados sejam julgados por múltiplas lentes independentes.

## Quick Start

```rust
use zylos_panel::{LenteCanonica, JudgeVote, PanelDecision, PanelConfig};
use zylos_panel::{filtrar_auto_julgamento, detector_falso_sucesso, guardrail_deterministico, aggregate_votes};

// Configurar painel com 6 lentes canônicas
let config = PanelConfig {
    lentes: vec![
        LenteCanonica::Funcionalidade,
        LenteCanonica::Segurança,
        LenteCanonica::Testes,
    ],
    min_confidence: 0.7,
};

// Guardrail curto-circuita resultados óbvios-ruins antes da votação
if let Some(rejection) = guardrail_deterministico(&output) {
    return Err(rejection);
}

// Filtrar auto-julgamento: juiz ≠ gerador estrutural
let votos_filtrados = filtrar_auto_julgamento(&votos, &gerador_id);

// Detector de falso-sucesso: encerramento sem vestígio
if detector_falso_sucesso(&resultado) {
    // Rejeitar resultado suspeito
}

// Agregar votos: 1 refutação aterrada derruba maioria
let decisao = aggregate_votes(&votos_filtrados);
```

## Tipos Principais

| Tipo | Descrição |
|------|-----------|
| `LenteCanonica` | 6 lentes: Funcionalidade, Estrutura, Performance, Segurança, Simplicidade, Testes |
| `JudgeVote` | Voto individual com refutação aterrada e vestígios |
| `PanelDecision` | Decisão agregada com confiança e métricas |
| `Confidencia` | Alta/Média/Baixa baseada em consenso |
| `PanelConfig` | Configuração do painel com lentes e thresholds |

## Implementado

- ✅ `guardrail_deterministico()` com curto-circuito resultado óbvio-ruim
- ✅ Detector de falso-sucesso (encerramento sem vestígio)
- ✅ `filtrar_auto_julgamento()` (juiz ≠ gerador estrutural)
- ✅ `aggregate_votes()` onde 1 refutação aterrada derruba maioria
- ✅ `vote_confidence()` alta/média/baixa baseada em consenso
- ✅ `LenteCanonica` enum com 6 variantes
- ✅ `PanelConfig` configurável

## Gaps vs Master Plan

- ❌ Integração com `zylos-proof` para cápsulas de evidência nos votos
- ❌ Métricas históricas de acurácia por lente

## Runbook SOAR

[`cortex-patterns.runbook.yaml`](../../docs/soar/runbooks/cortex-patterns.runbook.yaml)

## Dependências

- `zylos-proof` (Task 7B.1.1)
- `serde`, `serde_json`

## Fonte Original

`~/.claude/skills/orquestrar/scripts/panel.mjs` (197 linhas)
