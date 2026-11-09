# Runbook: Finance CSV Skill — Operação e Troubleshooting

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Usuário solicita importação de extrato financeiro ou categorização falha
**Público-alvo:** Support / End User
**Pré-requisitos:** Python 3.10+, skill finance-csv instalada

---

## 1. Formatos Suportados

| Formato | Extensão | Parser | Auto-detect |
|---------|----------|--------|-------------|
| CSV genérico | .csv | parse_csv() | Header keywords |
| OFX | .ofx | parse_ofx() | XML tags |
| QFX | .qfx | parse_ofx() | XML tags |

## 2. Bank Presets (Brasil)

Nubank, Inter, Itaú, Bradesco, Santander, BB, Caixa, C6, PicPay, MercadoPago + generic fallback.

Auto-detecção via header keywords. Override manual: `parse_csv(path, bank_preset="nubank")`.

## 3. Categorização

### Keyword Rules (9 categorias)
alimentação, transporte, moradia, saúde, educação, lazer, compras, serviços, transferências, outros.

### ML Classifier (futuro)
Modelo em `models/categorizer.pkl`. Fallback para keyword rules quando ausente.

## 4. Detecção de Recorrência

Transações com mesmo valor (±5%) e intervalo 25-35 dias são marcadas como `is_recurring=True`.

## 5. Output

`generate_summary()` retorna dict com:
- total_income, total_expense, net_balance
- recurring_expense
- by_category[], by_month[]

## 6. Troubleshooting

| Sintoma | Causa | Solução |
|---------|-------|---------|
| Transações ignoradas | Formato de data não reconhecido | Adicionar preset em BANK_PRESETS |
| Categoria errada | Keyword ambígua | Ajustar KEYWORD_RULES no classifier.py |
| Encoding error | Arquivo não UTF-8 | Converter: `iconv -f LATIN1 -t UTF-8` |

## 7. Referências

- Skill: `skills/finance-csv/`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §4.2
