# zylos-egress

> Verificador de egress — detecta PII, secrets e dados sensíveis antes de enviar para destinos externos.

## Purpose

Prevenir vazamento de dados sensíveis verificando texto no frontend (Leptos WASM) antes do envio e no backend como segunda linha de defesa. Detecta emails, telefones BR, CPF/CNPJ (com validação de dígitos verificadores), cartões de crédito (validação Luhn), API keys/tokens/passwords, e keywords de saúde. Suporta regras customizáveis via API REST.

Inspirado em: OpenDuet (`egress.ts`), AnyModelBot (password masking), Rerun (approval flow com verificação prévia).

## Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────┐
│ ChatPanel       │────▶│ EgressChecker    │────▶│ Gateway API │
│ (antes do envio)│     │ (WASM + regex)   │     │ /egress/*   │
└─────────────────┘     └──────────────────┘     └──────┬──────┘
                                                         │
                                              ┌──────────▼──────────┐
                                              │ EgressChecker       │
                                              │ (backend, 2ª linha) │
                                              └─────────────────────┘
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `SensitiveCategory`, `SensitiveFinding`, `EgressCheckResult`, `EgressRule` |
| `checker` | Engine de detecção com patterns built-in (LazyLock) + regras customizáveis |

## Patterns Built-in

| Pattern | Categoria | Confiança | Bloqueante? | Validação |
|---------|-----------|-----------|-------------|-----------|
| Email RFC 5322 simplificado | email | 0.95 | Não | — |
| Telefone BR (+55, DDD, 9 dígitos) | phone | 0.85 | Não | Mínimo 10 dígitos |
| CPF (XXX.XXX.XXX-XX) | brazilian_tax_id | 0.99 | Não | Dígitos verificadores mod-11 |
| CNPJ (XX.XXX.XXX/XXXX-XX) | brazilian_tax_id | 0.99 | Não | Dígitos verificadores mod-11 |
| Cartão de crédito (13-16 dígitos) | credit_card | 0.98 | **Sim** | Algoritmo Luhn |
| API key/secret/token/password | secret | 0.95 | **Sim** | Regex multi-pattern |
| GitHub PAT (ghp_*) | secret | 0.95 | **Sim** | Prefix match |
| Slack token (xox*) | secret | 0.95 | **Sim** | Prefix match |
| Google API key (AIza*) | secret | 0.95 | **Sim** | Prefix match |
| Health keywords (PT-BR) | health_data | 0.60 | Não | Keyword match |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/egress/check` | Verifica texto e retorna findings + versão redigida |
| GET | `/api/v1/egress/rules` | Lista regras customizadas |
| POST | `/api/v1/egress/rules` | Cria regra customizada |
| DELETE | `/api/v1/egress/rules/:id` | Remove regra |

## Configuration

Categorias bloqueantes padrão: `secret`, `credit_card`. Configurável via `with_blocked_categories()`.

Regras customizáveis via API ou `with_custom_rules()`. Cada regra tem: id, name, pattern (regex), category, blocks (bool), enabled (bool).

## Testing

```bash
cargo test -p zylos-egress
```

- **24 testes unitários** cobrindo: email detection, CPF válido/inválido/repetido, CNPJ válido/inválido, credit card Luhn válido/inválido, secrets (API key, GitHub PAT), health keywords, redacted text generation, custom rules, disabled rules, blocked categories configuráveis, multiple findings sorting, Luhn algorithm, CPF/CNPJ validation functions
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON para API |
| serde_json | workspace | Parsing de request/response |
| regex | 1.x | Pattern matching compilado |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |

## Performance

- Patterns compilados estaticamente via `LazyLock` (uma vez por processo)
- Check completo <5ms para textos até 10KB
- Zero alocações no hot path (regex reutilizadas)

## Related Documents

- **Playbook:** [pb-egress-rules-management.md](../../playbooks/pb-egress-rules-management.md)
- **Runbook:** [rb-zylos-egress.md](../../runbooks/rb-zylos-egress.md)
- **POP:** [pop-egress-false-positive.md](../../playbooks/pop-egress-false-positive.md)
- **SOAR:** [soar-egress-block-response.md](../../docs/soar/soar-egress-block-response.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §2.2
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)