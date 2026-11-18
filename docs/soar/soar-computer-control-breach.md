# SOAR: Computer Control Breach Response

**Trigger:** Controle não autorizado detectado em sessão de computer view (usuário desconhecido assume controle, agente opera fora do lease, ou ação suspeita durante controle humano)
**Severity:** Critical (potencial comprometimento de desktop do agente)
**Auto-Remediation:** Partial (revoga lease, pausa sessão; não reverte ações executadas)

## Detection

Sinais de breach no computer view:

1. **Lease por usuário desconhecido:** `control_lease.user_id` não corresponde a nenhum usuário autorizado
2. **Ação durante controle humano sem lease ativo:** Agente executa comandos no desktop enquanto `state == user_control` mas sem lease válido
3. **Take control sem reason:** Requisição de take-control com `reason` vazio ou genérico ("test", "debug") em horário não-trabalho
4. **Múltiplos take-controls em curto período:** >3 take-controls na mesma sessão em <5 minutos (possível disputa ou automação maliciosa)
5. **Navegação suspeita durante controle:** Agente acessa URLs fora da whitelist durante sessão ativa
6. **Screenshot mostra conteúdo sensível:** OCR/detecção de PII em screenshots capturados

### Queries de detecção
```rust
// Via SessionManager API
let sessions = session_manager.list_active_sessions();
for session in sessions {
 if let Some(ref lease) = session.control_lease {
 // Verificar se user_id é autorizado
 if !is_authorized_user(&lease.user_id) {
 alert_breach(&session, "unauthorized_user");
 }
 // Verificar se lease expirou mas estado ainda é user_control
 if lease.is_expired() && session.state == "user_control" {
 alert_breach(&session, "expired_lease_still_controlling");
 }
 }
}
```

## Containment

1. **Revogar lease imediatamente:**
 ```rust
 session_manager.release_control(session_id, "system_auto_revoke")?;
 ```
2. **Pausar sessão** para prevenir novas ações:
 ```rust
 session_manager.update_state(session_id, SessionState::Paused)?;
 ```
3. **Capturar screenshot forense** do estado atual do desktop:
 ```rust
 let screenshot = session_manager.capture_screenshot(session_id)?;
 // Em produção: capturar screenshot real via VNC proxy
 save_forensic_evidence(session_id, &screenshot);
 ```
4. **Notificar admin/security team** via messaging connector com detalhes:
 - Session ID, bot ID, user ID do lease
 - Timestamp do take-control e duração
 - Reason fornecido (se houver)
 - Screenshot forense anexado
5. **Publicar evento** `ComputerControlBreach { session_id, bot_id, user_id, breach_type }` no EventBus

## Investigation

1. **Verificar logs de acesso:**
 - Quem fez o take-control? (user_id, IP, timestamp)
 - Houve autenticação válida? (token/session cookie)
 - O user_id corresponde a conta legítima ou foi forjado?

2. **Analisar ações durante o breach:**
 - Revisar screenshots/VNC recording do período
 - Listar URLs acessadas, arquivos abertos, comandos executados
 - Correlacionar com audit log de ações do bot no mesmo período

3. **Classificar causa raiz:**
 - **Credencial comprometida:** Token/session hijacked → rotacionar credenciais
 - **Bug de autorização:** Lease concedido sem verificação de user_id → corrigir validação
 - **Usuário legítimo mas não autorizado:** Funcionário sem permissão de computer view → ajustar RBAC
 - **Automação maliciosa:** Script/bot automatizando take-controls → bloquear IP/token
 - **Falso positivo:** Usuário autorizado esqueceu de fazer release → ajustar treinamento

## Remediation

### Automática
- Lease revogado e sessão pausada
- Screenshot forense capturado e armazenado
- Alerta enviado ao security team
- Evento publicado no EventBus para auditoria

### Manual (após investigação)
1. **Se credencial comprometida:** Rotacionar todos os tokens de computer view, forçar re-auth de todos os usuários
2. **Se bug de autorização:** Corrigir validação de user_id em `take_control()`, adicionar teste de regressão
3. **Se usuário não autorizado:** Revogar acesso, adicionar à lista de bloqueio, notificar RH se funcionário
4. **Se automação maliciosa:** Bloquear IP/range, adicionar rate limiting no endpoint take-control
5. **Se falso positivo:** Documentar exceção, ajustar treinamento de usuários

## Recovery

1. Após causa raiz resolvida, despausar sessão:
 ```rust
 session_manager.update_state(session_id, SessionState::Active)?;
 ```
2. Verificar que agente retoma operação normal
3. Monitorar por 24h para confirmar que breach não recurs
4. Atualizar regras de detecção se necessário

## Post-Incident

| Item | Ação |
|------|------|
| Documentação | Atualizar `rb-zylos-computer-view.md` §Troubleshooting com novo cenário |
| Auth | Adicionar verificação de user_id em take_control se faltava |
| Rate Limiting | Adicionar limite de take-controls por sessão/usuário/hora |
| Forensics | Implementar gravação contínua de VNC stream para replay forense |
| Training | Atualizar POP de controle emergencial com lições aprendidas |
| Metrics | Adicionar métrica `breach_rate` ao dashboard de monitoring |

## Bound POPs

- [pop-computer-control-emergency.md](../../playbooks/pop-computer-control-emergency.md) — procedimento de take/release control legítimo

## Bound Runbooks

- [rb-zylos-computer-view.md](../../runbooks/rb-zylos-computer-view.md) — troubleshooting e monitoring

## Metrics

| Métrica | Target | Alert Threshold |
|---------|--------|-----------------|
| Time to detect (TTD) | <1 min | >5 min |
| Time to contain (TTC) | <30s | >2 min |
| Breach rate (30d) | 0 | Qualquer ocorrência requer investigação |
| False positive rate | <5% | >20% (regras de detecção muito agressivas) |
| Mean time to resolve (MTTR) | <1h | >4h |