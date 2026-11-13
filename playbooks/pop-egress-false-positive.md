# POP-002: Reportar e Corrigir Falso Positivo de Egress

**Version:** 1.0.0
**Playbook:** [pb-egress-rules-management.md](./pb-egress-rules-management.md)
**Last Updated:** 2026-11-11

## Purpose

Procedimento operacional para quando o verificador de egress bloqueia ou alerta sobre texto legítimo que não contém dados sensíveis reais (falso positivo). Aplica-se tanto a patterns built-in quanto a regras customizadas.

## Trigger

- Mensagem bloqueada pelo egress checker com finding incorreto
- Usuário reporta "isso não é dado sensível" no cockpit
- Métrica de false positive rate >5% para determinada regra

## Steps

### 1. Identificar o finding problemático

1. Verificar o `EgressCheckResult` retornado pelo `/api/v1/egress/check`
2. Anotar: `category`, `rule_name`, `matched_text`, `confidence`
3. Classificar:
   - **Built-in falso positivo**: pattern correto mas contexto errado (ex: número longo que passa Luhn mas não é cartão)
   - **Custom rule falso positivo**: regex muito ampla
   - **Health keyword ambíguo**: termo técnico que coincide com keyword de saúde

### 2. Decidir ação corretiva

| Tipo | Ação | Prioridade |
|------|------|------------|
| Built-in email/phone em código-fonte | Adicionar exceção por contexto (code block, URL) | Média |
| Built-in credit card Luhn falso positivo | Refinar regex para exigir separadores ou prefixos BIN | Alta |
| Built-in secret em variável de ambiente documentada | Whitelist por bot_id + tool_name | Média |
| Custom rule regex muito ampla | Refinar pattern com anchors/boundaries | Alta |
| Health keyword em contexto médico legítimo | Reduzir confidence ou adicionar allowlist por bot | Baixa |

### 3. Implementar correção

#### Para regras customizadas:
```bash
# Atualizar regex para ser mais específica
curl -X PATCH http://localhost:3000/api/v1/egress/rules/{rule_id} \
 -H "Content-Type: application/json" \
 -d '{"pattern": "\\bPROJ-[A-Z]{2,4}-\\d{4,6}\\b"}'
```

#### Para built-ins (requer code change):
1. Abrir issue no repositório zylos-next
2. Propor refinamento do pattern em `crates/zylos-egress/src/checker.rs`
3. Adicionar teste de regressão com o caso falso positivo
4. Submeter PR com label `egress/false-positive`

### 4. Validar correção

```bash
# Testar texto original que causou falso positivo
curl -X POST http://localhost:3000/api/v1/egress/check \
 -H "Content-Type: application/json" \
 -d '{"text": "<texto original aqui>"}'
```

Verificar:
- Finding não aparece mais OU confidence reduzida abaixo de threshold
- Texto legítimo similar continua sendo detectado se for realmente sensível
- `passed: true` para o caso falso positivo específico

### 5. Documentar caso

Adicionar entrada no `TRACKRECORD_FASE9.md`:
```markdown
### YYYY-MM-DD
- **Fixed:** Falso positivo em egress checker — {descrição}
- **Rule:** {rule_name} / {category}
- **Cause:** {causa raiz}
- **Fix:** {correção aplicada}
```

## Verification

- Texto original não gera mais finding (ou gera com confidence < threshold)
- Casos verdadeiros positivos continuam sendo detectados
- Métrica de false positive rate retorna a <5% em 24h
- Nenhum novo falso positivo introduzido pela correção

## Rollback

Se correção introduziu falso negativo (deixou passar dado sensível real):
1. Reverter mudança na regra/pattern imediatamente
2. Investigar por que correção foi muito agressiva
3. Reprojetar com abordagem mais conservadora

## Escalation

| Situação | Escalar Para | Quando |
|----------|-------------|--------|
| Built-in falso positivo recorrente (>10/dia) | Platform team | Imediatamente, afeta UX de todos os usuários |
| Correção de built-in requer code change | Security team + maintainer | Via PR com review obrigatório |
| Dado sensível real vazou após correção | Incident response (SOAR) | Imediatamente, severity critical |
| Usuário não consegue enviar mensagem legítima | Support team | Após 3 tentativas falhas do usuário |