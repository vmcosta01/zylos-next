# SOAR: Analytics Auto-Improvement Feedback Loop

**Trigger:** Skill com taxa de sucesso abaixo do limiar (padrão < 70%) ou queda ≥ 15 pp vs janela anterior, ou latência média > 5000 ms — avaliado por `zylos_analytics::feedback::analyze`
**Severity:** Medium → Critical (conforme a taxa de sucesso)
**Auto-Remediation:** Partial (gera recomendação e sinaliza no cockpit; a evolução da skill passa pelo pipeline de skill-evolution com aprovação humana)

**Fase:** 11.10 (T11.10.5) · **Código:** `crates/zylos-analytics/src/feedback.rs` · **API:** `GET /api/v1/analytics/recommendations`

## Detection

`analyze(store, now, &FeedbackConfig)` compara a janela atual (`window_days`, padrão 7 dias) com a janela anterior de mesmo tamanho, por skill e por (skill, agente):

| Config | Padrão | Efeito |
|---|---|---|
| `window_days` | 7 | tamanho da janela |
| `min_success_rate` | 0.70 | abaixo → `evolve_skill` |
| `regression_drop` | 0.15 | queda ≥ 15 pp → `investigate_regression` |
| `max_avg_latency_ms` | 5000 | acima → `optimize_latency` |
| `min_samples` | 20 | ignora skills com poucas execuções (evita falso positivo) |

Uma skill saudável no agregado, mas ruim em um agente específico, gera `review_agent_config`.

Severidade de `evolve_skill` (relativa ao limiar): `critical` < limiar−30 pp (40% no padrão), `high` < limiar−15 pp (55%), senão `medium`. `investigate_regression`: `medium`, ou `high` se a queda ≥ 2×`regression_drop`. `optimize_latency`: `low`. `review_agent_config`: `medium`.

```bash
curl -s "http://localhost:3000/api/v1/analytics/recommendations?window_days=7&min_success_rate=0.7"
```

Resultado medido no benchmark (30 dias sintéticos, 1.02M eventos): **3 recomendações**, skill < 70% detectada: `pdf-extract` (gerada com ~55% de sucesso de propósito), em 189 ms.

## Response

### Automática
1. Recomendações aparecem no painel **Analytics → Auto-melhoria** (cor por severidade).
2. O preset `skill_evolution_summary` inclui a tabela de recomendações (JSON/CSV/PDF).

### Manual (operador)
1. Abrir a skill no **Skill Editor** / pipeline de skill-evolution (`playbooks/skill-evolution-lifecycle.md`).
2. Filtrar falhas da skill: `/analytics/events?skills=<skill>&success=false&limit=100` e ler as dimensões (canal, modelo) que concentram falhas.
3. `evolve_skill`: propor nova versão do contrato da skill; `investigate_regression`: comparar deploys/modelos da janela; `optimize_latency`: revisar timeouts e tier de modelo (`playbooks/pb-model-tier-strategy.md`); `review_agent_config`: revisar config do agente específico.
4. Após a mudança, acompanhar a tendência semanal no preset Skill Evolution Summary (coluna "Tendência", em pp).

## Guardrails

- O loop **não altera skills sozinho**: apenas recomenda. Mudanças de skill seguem o approval gate.
- `min_samples` evita recomendações com base estatística fraca.
- Limiar e janela são parâmetros da API — documente qualquer alteração em relação ao padrão.

## Verification

- [ ] Teste `feedback::tests` passa (`cargo test -p zylos-analytics feedback`)
- [ ] Skill degradada aparece com `kind = evolve_skill`
- [ ] Após a correção, a skill deixa de aparecer na janela seguinte
