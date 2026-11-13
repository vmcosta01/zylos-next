# POP — Backup e Restore do Banco de Dados (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`disaster-recovery.md`](./disaster-recovery.md), [`incident-response.md`](./incident-response.md)
**Runbook Relacionado:** `runbooks/backup-restore.md`
**Público:** SRE, DBA, Tech Lead
**Tempo Estimado:** 10-30 minutos (backup: 5-10min, restore: 15-30min)

---

## 1. Objetivo

Padronizar procedimentos de backup incremental/full e restore completo/parcial do banco SQLite WAL do zylos-memory-system, garantindo integridade de dados, conformidade LGPD e recuperação dentro do RTO/RPO definidos.

## 2. Pré-Condições

- [ ] Acesso SSH ao host do banco de dados
- [ ] Chave de criptografia disponível (`/etc/zylos/backup-key.txt`)
- [ ] Espaço em disco suficiente (`df -h /mnt/zylos-backup` > 2x tamanho do DB)
- [ ] `sqlite3` >= 3.44 instalado (`sqlite3 --version`)
- [ ] `age` ou `gpg` disponível para criptografia
- [ ] Janela de manutenção agendada (para restore em produção)
- [ ] Stakeholders notificados (para restore em produção)

**Regra:** Se qualquer pré-condição não for atendida, NÃO iniciar. Para restore em produção sem janela agendada → escalar SEV-1 imediatamente.

## 3. Procedimento de Backup

### Passo 1: Verificar Integridade do DB Atual (2 min)
```bash
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA integrity_check;"
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA journal_mode;"
```
**Output esperado:** `ok` e `wal`.
**Se falhar:** NÃO fazer backup de DB corrompido. Escalar SEV-1 e seguir `playbooks/incident-response.md`.

### Passo 2: Executar Backup (3-8 min)
```bash
# Backup incremental (diário)
/opt/zylos-next/scripts/backup-incremental.sh

# OU backup full (semanal)
/opt/zylos-next/scripts/backup-full.sh
```
**Output esperado:** `Backup concluído: /mnt/zylos-backup/{tipo}/{data}/`
**Se falhar:** Verificar logs em `/var/log/zylos/backup.log` e espaço em disco.

### Passo 3: Validar Backup (2 min)
```bash
# Verificar checksum
sha256sum -c /mnt/zylos-backup/full/$(date +%G-W%V)/SHA256SUMS

# Verificar integridade do backup
sqlite3 /mnt/zylos-backup/full/$(date +%G-W%V)/main-full.db "PRAGMA integrity_check;"
```
**Output esperado:** `OK` e `ok`.
**Se falhar:** Backup inválido → re-executar passo 2. Se persistir → escalar SEV-2.

### Passo 4: Registrar no Evidence Ledger (1 min)
```bash
zylos-cli evidence log \
  --type backup_completed \
  --metadata "{\"path\":\"/mnt/zylos-backup/full/$(date +%G-W%V)\",\"size_mb\":$(du -sm /mnt/zylos-backup/full/$(date +%G-W%V)/main-full.db | cut -f1),\"integrity\":\"ok\"}"
```
**Output esperado:** `{"hash":"sha256:...","timestamp":"..."}`

## 4. Procedimento de Restore

### Passo 1: Parar Writers (2 min)
```bash
# Lock advisory para prevenir writes durante restore
flock -x /var/lock/zylos-writer.lock -n true || {
  echo "ERRO: Não foi possível adquirir lock. Writers ativos?"
  exit 1
}
```
**Output esperado:** (sem output = sucesso).
**Se falhar:** Identificar processos segurando o lock: `lsof /var/lock/zylos-writer.lock`.

### Passo 2: Descriptografar e Restaurar (5-15 min)
```bash
/opt/zylos-next/scripts/restore-full.sh /mnt/zylos-backup/full/2026-W45/main-full.db.age
```
**Output esperado:** `Restore concluído com sucesso. Integridade verificada: ok`
**Se falhar:** Verificar chave de descriptografia e integridade do arquivo .age.

### Passo 3: Validar Restore (3 min)
```bash
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA integrity_check;"
sqlite3 /var/lib/zylos/memory/main.db "SELECT COUNT(*) FROM memories;"
curl -s http://localhost:9876/api/v1/health | jq .
```
**Output esperado:** `ok`, contagem de registros > 0, health check healthy.
**Se falhar:** Rollback imediato → vá para § Rollback.

### Passo 4: Liberar Lock e Notificar (1 min)
```bash
flock -u /var/lock/zylos-writer.lock
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) RESTORE_COMPLETED" >> /var/log/zylos/restore-audit.log
```

## 5. Validação Pós-Execução

- [ ] Backup: checksum válido + integridade ok + registrado no ledger
- [ ] Restore: DB íntegro + health check ok + contagem de registros consistente
- [ ] Logs de auditoria registrados (`/var/log/zylos/{backup,restore}-audit.log`)
- [ ] Lock liberado (apenas para restore)
- [ ] Stakeholders notificados (restore em produção)

## 6. Rollback

**Trigger conditions:**
- Restore falhou na validação de integridade
- Health check não retorna healthy após restore
- Contagem de registros inconsistente com backup

**Passos de rollback:**
```bash
# 1. Parar gateway imediatamente
systemctl stop zylos-gateway

# 2. Restaurar backup anterior (se disponível)
if [ -f /var/lib/zylos/memory/main.db.pre-restore.bak ]; then
  cp /var/lib/zylos/memory/main.db.pre-restore.bak /var/lib/zylos/memory/main.db
fi

# 3. Reiniciar gateway
systemctl start zylos-gateway

# 4. Validar
sleep 5
curl -sf http://localhost:9876/api/v1/health || echo "FALHA: Gateway não recuperou"

# 5. Escalar SEV-1
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) RESTORE_ROLLBACK triggered" >> /var/log/zylos/restore-audit.log
```

**Validação pós-rollback:**
- [ ] Gateway healthy
- [ ] DB íntegro (versão anterior)
- [ ] Incidente SEV-1 aberto
- [ ] Post-mortem agendado

## 7. Quando NÃO Usar Este POP

- **Backup de artefatos SOAR** → Use `runbooks/evidence-ledger.md` § Export
- **Restore parcial (por agente/skill)** → Use `runbooks/backup-restore.md` § Restore Parcial
- **Migração entre hosts** → Use `scripts/migrate-production.sh`
- **DB corrompido sem backup válido** → Use `playbooks/disaster-recovery.md` § Data Loss
- **Backup de configurações** → Use `git commit` + `playbooks/deploy-release.md`

## 8. Referências Cruzadas

- Runbook completo: `runbooks/backup-restore.md`
- Playbook de disaster recovery: `playbooks/disaster-recovery.md`
- SOAR workflow: `docs/soar/runbooks/data-recovery.runbook.yaml`
- Evidence Ledger: `runbooks/evidence-ledger.md`
- Scripts: `scripts/backup-incremental.sh`, `scripts/backup-full.sh`, `scripts/restore-full.sh`
- Logs: `/var/log/zylos/{backup,restore}-audit.log`