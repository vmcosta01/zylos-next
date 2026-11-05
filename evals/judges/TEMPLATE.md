# LLM-as-Judge Template — Zylos-Next

## Metadata
- **Eval:** <eval-name>
- **Judge Model:** <model>
- **Date:** YYYY-MM-DD

## Prompt do Juiz

```
Você é um juiz adversarial avaliando a qualidade de <output-type>.

Critérios (score 1-5 cada):
1. **Correção**: O output satisfaz todos os requisitos?
2. **Completude**: Nada essencial foi omitido?
3. **Grounding**: Claims são suportados por evidência verificável?
4. **Formatação**: Output segue o schema/formato especificado?
5. **Anti-Slop**: Ausência de filler, hedge, afirmações não verificadas?

Instruções:
- Busque REFUTE, não confirmar
- Score 1-2 requer citação de falha específica
- Score 4-5 requer citação de evidência positiva
- Se incerto, marque como NEEDS_HUMAN_REVIEW

Output JSON:
{
  "scores": {"correcao": N, "completude": N, "grounding": N, "formatacao": N, "anti_slop": N},
  "overall_pass": bool,
  "refutations": ["..."],
  "evidence": ["..."],
  "needs_human_review": bool
}
```

## Dataset

| Input | Expected Output | Judge Score | Notes |
|-------|----------------|-------------|-------|
| ... | ... | - | ... |

## Thresholds

- **Pass**: overall_pass=true AND min(score) >= 3
- **Fail**: overall_pass=false OR any score <= 2
- **Review**: needs_human_review=true
