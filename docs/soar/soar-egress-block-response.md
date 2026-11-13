# SOAR: Egress Block Response

**Trigger:** Egress checker bloqueia mensagem contendo PII/secrets/dados sensíveis
**Severity:** High (potencial vazamento de dados)
**Auto-Remediation:** Partial (bloqueia envio, notifica usuário; não remove dado da fonte)

## Detection

O egress checker é invocado em dois pontos:

1. **Frontend (Leptos WASM):** Antes de enviar mensagem via SSE/WebSocket no ChatPanel
2. **Backend (Gateway):** Como segunda linha de defesa no endpoint `/api/v1/egress/check`

Ambos retornam `EgressCheckResult { passed: false, findings: [...], blocked_categories: [...] }` quando dados sensíveis são detectados.

Sinais de bloqueio:
- Frontend: modal de confirmação aparece no cockpit com lista de findings
- Backend: HTTP 403 ou response com `passed: false` no endpoint de envio de mensagem
- Audit log: entrada com `egress_blocked = true`

## Containment

1. **Bloquear envio imediatamente** — mensagem NÃO deve sair do browser/gateway
2. **Registrar no audit log:**
   ```sql
   INSERT INTO action_audit_log (id, bot_id, action_type, approval_status, arguments_hash, created_at)
   VALUES (?, ?, 'message_send', 'egress_blocked', SHA256(?), datetime('now'))
   ```
3. **Notificar usuário** via cockpit (modal inline no chat) com:
   - Lista de dados sensíveis detectados (categoria + texto redigido)
   - Opções: "Editar mensagem", "Enviar versão redigida", "Cancelar"
4. **Publicar evento** `EgressBlocked { text_hash, categories, bot_id }` no EventBus

## Investigation

1. Verificar se é falso positivo:
   - O dado detectado é realmente sensível?
   - O contexto justifica a presença (ex: CPF em formulário de cadastro legítimo)?
   - A confiança do finding é alta (>0.9) ou baixa (<0.7)?

2. Se falso positivo recorrente:
   - Seguir [pop-egress-false-positive.md](../../playbooks/pop-egress-false-positive.md)
   - Ajustar regra ou pattern conforme procedimento

3. Se verdadeiro positivo:
   - Verificar se o bot foi instruído a incluir dado sensível
   - Verificar se há vazamento sistemático (mesmo bot, mesma categoria, múltiplas ocorrências)
   - Escalar para security team se padrão suspeito

## Remediation

### Automática
- Mensagem bloqueada no ponto de origem (frontend ou backend)
- Usuário recebe opção de enviar versão redigida automaticamente
- Audit log registra tentativa bloqueada com hash dos argumentos

### Manual (se padrão recorrente)
1. Revisar instruções do bot para evitar inclusão de PII desnecessário
2. Adicionar whitelist por bot_id + tool_name se uso legítimo confirmado
3. Refinar regex/pattern para reduzir falsos positivos sem comprometer segurança
4. Atualizar training materials do bot com exemplos de mensagens seguras

## Recovery

1. Usuário edita mensagem removendo/substituindo dado sensível
2. Nova verificação de egress passa (`passed: true`)
3. Mensagem enviada normalmente
4. Se usuário escolheu "Enviar versão redigida", sistema substitui automaticamente e envia

## Post-Incident

| Item | Ação |
|------|------|
| Documentação | Atualizar `rb-zylos-egress.md` §Troubleshooting se nova causa raiz |
| Rules | Ajustar categorias bloqueantes ou adicionar exceções se necessário |
| Training | Atualizar instruções do bot para evitar PII em outputs |
| Metrics | Adicionar métrica `egress_block_rate` ao dashboard de monitoring |
| Alerting | Configurar alerta se block rate >30% em 24h (possível falso positivo massivo) |

## Bound POPs

- [pop-egress-false-positive.md](../../playbooks/pop-egress-false-positive.md) — quando bloqueio é falso positivo

## Bound Runbooks

- [rb-zylos-egress.md](../../runbooks/rb-zylos-egress.md) — troubleshooting e performance tuning

## Metrics

| Métrica | Target | Alert Threshold |
|---------|--------|-----------------|
| Block rate (24h) | <5% | >30% |
| False positive rate | <2% | >10% |
| Avg time to resolve (user edit) | <2 min | >10 min |
| Redacted send rate | >50% dos blocks | <20% (usuários estão cancelando em vez de editar) |
| Secret category blocks | 0 tolerância | Qualquer ocorrência requer investigação imediata |