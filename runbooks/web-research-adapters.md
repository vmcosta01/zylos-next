# Runbook: Web Research Skill Adapters — Operação

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Pesquisa web solicitada por agente ou usuário
**Público-alvo:** Agent Operator / Developer
**Pré-requisitos:** API keys configuradas (PARALLEL_API_KEY, EXA_API_KEY, FIRECRAWL_API_KEY)

---

## 1. Providers

| Provider | Endpoint | Rate Limit | Auth |
|----------|----------|-----------|------|
| Parallel | api.parallel.ai/v1 | 30 RPM | Bearer token |
| Exa | api.exa.ai/v1 | 20 RPM | Bearer token |
| Firecrawl | api.firecrawl.dev/v1 | 15 RPM | Bearer token |

## 2. Proof Grounding Pipeline

| Stage | Proof Level | Descrição |
|-------|------------|-----------|
| Search results | Observed | Resultados brutos da API |
| Extracted content | Confirmed | Conteúdo extraído e verificado cross-source |
| Synthesized answer | Verified | Resposta sintetizada com adversarial panel (opcional) |

## 3. Fallback Chain

Ordem: Parallel → Exa → Firecrawl.

```python
from skills.web_research_fallback import search_with_fallback
result = await search_with_fallback("query", num_results=10)
```

Se todos falharem, retorna `{"error": "All providers failed", "fallback_errors": [...]}`.

## 4. Configuração de API Keys

```bash
export PARALLEL_API_KEY="pk_..."
export EXA_API_KEY="exa_..."
export FIRECRAWL_API_KEY="fc_..."
```

## 5. Troubleshooting

| Erro | Causa | Solução |
|------|-------|---------|
| RuntimeError: API_KEY not set | Variável de ambiente ausente | Exportar key |
| 429 Too Many Requests | Rate limit excedido | Aguardar ou trocar provider |
| Timeout | API lenta | Fallback chain tenta próximo provider |

## 6. Referências

- Skills: `skills/web-research-{parallel,exa,firecrawl,fallback}/`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §4.3
