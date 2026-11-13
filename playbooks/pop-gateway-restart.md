# POP — Restart Seguro do Gateway (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`deploy-release.md`](./deploy-release.md), [`incident-response.md`](./incident-response.md)
**Runbook Relacionado:** `runbooks/zylos-gateway.md`
**Público:** SRE, DevOps, Agentes Cerebelo
**Tempo Estimado:** 3-5 minutos

---

## 1. Objetivo

Executar restart do zylos-gateway com zero downtime percebido pelo usuário, preservando sessões ativas e garantindo rollback automático em caso de falha.

## 2. Pré-Condições

- [ ] Acesso SSH/local ao host do gateway
- [ ] Permissão de escrita em `~/Zylos/config/gateway.toml`
- [ ] Cockpit acessível para verificação pós-restart
- [ ] Backup recente do SQLite WAL (`cp data/zylos.db data/zylos.db.bak`)

## 3. Procedimento

### Passo 1: Verificar Estado Atual
```bash
# Health check
curl -s http://localhost:9876/api/v1/health | jq .

# Contar sessões ativas
curl -s http://localhost:9876/api/v1/sessions/count | jq .count

# Salvar estado atual
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) PRE_RESTART sessions=$(curl -s http://localhost:9876/api/v1/sessions/count | jq .count)" >> ~/Zylos/logs/restart-audit.log
```

### Passo 2: Drenar Conexões (Graceful Shutdown)
```bash
# Enviar SIGTERM para graceful shutdown (timeout 30s)
kill -SIGTERM $(pgrep -f zylos-gateway)

# Aguardar drain
sleep 5

# Verificar se processo ainda existe
if pgrep -f zylos-gateway > /dev/null; then
  echo "WARN: Gateway still running after 5s, waiting..."
  sleep 10
fi
```

### Passo 3: Reiniciar
```bash
# Iniciar nova instância
cd ~/Zylos && ./zylos-gateway --config config/gateway.toml &

# Aguardar health check positivo (max 30s)
for i in $(seq 1 30); do
  if curl -sf http://localhost:9876/api/v1/health > /dev/null 2>&1; then
    echo "✅ Gateway healthy after ${i}s"
    break
  fi
  sleep 1
done
```

### Passo 4: Validação Pós-Restart
```bash
# Verificar health
curl -s http://localhost:9876/api/v1/health | jq .

# Verificar sessões recuperadas
curl -s http://localhost:9876/api/v1/sessions/count | jq .count

# Log de auditoria
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) POST_RESTART sessions=$(curl -s http://localhost:9876/api/v1/sessions/count | jq .count)" >> ~/Zylos/logs/restart-audit.log
```

## 4. Rollback Automático

Se health check falhar após 30s:
```bash
# Matar instância nova
pkill -f zylos-gateway

# Restaurar backup do DB se necessário
if [ -f ~/Zylos/data/zylos.db.bak ]; then
  cp ~/Zylos/data/zylos.db.bak ~/Zylos/data/zylos.db
fi

# Reiniciar versão anterior (se disponível)
# Ou notificar SEV-2 via playbook de incident-response
echo "❌ ROLLBACK TRIGGERED at $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> ~/Zylos/logs/restart-audit.log
```

## 5. Critérios de Sucesso

- [ ] Health check retorna `{"healthy": true}` em ≤30s
- [ ] Sessões ativas ≥ contagem pré-restart (ou diferença justificada)
- [ ] Nenhum erro nos logs nos primeiros 60s pós-restart
- [ ] Cockpit carrega corretamente

## 6. Quando NÃO Usar Este POP

- **DB corrompido** → Use `runbooks/backup-restore.md` primeiro
- **Config inválida** → Valide com `zylos-gateway --validate-config` antes
- **Upgrade de versão** → Use `playbooks/deploy-release.md` (canary/blue-green)
- **SEV-1 ativo** → Siga `playbooks/incident-response.md` § War Room

## 7. Referências

- Runbook completo: `runbooks/zylos-gateway.md`
- Playbook de deploy: `playbooks/deploy-release.md`
- Logs de restart: `~/Zylos/logs/restart-audit.log`
- Evidence Ledger: `zylos-cli evidence log --type gateway_restart`