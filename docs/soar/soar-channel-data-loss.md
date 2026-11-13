# SOAR: Channel Data Loss Response

**Trigger:** Perda detectada de dados de canais (DB corrompido, tabelas vazias inesperadamente, erros sqlx persistentes)
**Severity:** Critical (comunicação da equipe impactada)
**Auto-Remediation:** Partial (reseed automático; mensagens perdidas requerem backup)

## Detection

### Sinais de Perda de Dados
1. **Tabelas vazias:** `SELECT COUNT(*) FROM channels` retorna 0 após startup
2. **Erros sqlx persistentes:** >10 erros/min nos logs do gateway
3. **Integrity check falha:** `PRAGMA integrity_check` != 'ok'
4. **Cockpit sem canais:** frontend mostra estado vazio sem fallback mock
5. **Arquivo DB ausente:** `channels.db` não existe em disco

### Queries de Detecção
```sql
-- Verificar se seed rodou
SELECT COUNT(*) FROM channels;

-- Verificar integridade
PRAGMA integrity_check;

-- Verificar consistência referencial
SELECT COUNT(*) FROM channel_messages 
WHERE channel_id NOT IN (SELECT id FROM channels);
```

## Containment

1. **Parar writes imediatos:** gateway continua servindo reads cacheados
2. **Ativar modo read-only:** configurar pool como read-only se DB parcialmente funcional
3. **Notificar equipe:** postar em canal alternativo (Slack/Discord) sobre indisponibilidade

## Remediation

### Cenário A: DB Corrompido
Seguir `docs/pop-channel-recovery.md` → Opção B (dump/reimport) ou C (recriar).

### Cenário B: Tabelas Vazias
```bash
# Gateway auto-executa seed_defaults() no startup
# Reiniciar gateway:
cargo run -p zylos-gateway
```

### Cenário C: Perda de Mensagens
1. Restaurar backup mais recente:
```bash
sqlite3 data/channels.db ".restore '/backup/channels-YYYYMMDD.db'"
```
2. Verificar integridade pós-restore
3. Reiniciar gateway

### Cenário D: Arquivo Ausente
Gateway recria automaticamente via `create_if_missing(true)` + migrations + seed.

## Recovery Verification

1. `curl http://localhost:3000/api/v1/channels` retorna ≥4 canais
2. Cockpit carrega lista de canais
3. Envio de mensagem funciona: `POST /channels/ch-geral/messages`
4. Join/leave funciona: `POST /channels/ch-geral/join`

## Post-Incident

1. Documentar causa raiz no audit log
2. Verificar se backups estavam funcionando
3. Atualizar monitoring thresholds se necessário
4. Revisar POP se procedimento foi insuficiente

## Prevenção
- WAL mode sempre ativo
- Backups diários via cron (ver runbook)
- Integrity check periódico
- Alertas proativos em métricas de erro