# Runbook de Backup e Restore — Zylos-Next

## Objetivo

Garantir a recuperação completa e segura dos dados persistidos no sistema de memória (`zylos-memory-system`, SQLite WAL), com conformidade LGPD, criptografia em repouso e testes mensais de restauração.

## Escopo

- Backup incremental (diário) e full (semanal) dos bancos SQLite WAL.
- Restore parcial (por agente, skill ou intervalo de tempo) e completo.
- Retenção conforme LGPD: 30 dias hot, 90 dias cold, exclusão definitiva.
- Criptografia at-rest de todos os artefatos de backup.
- Testes mensais obrigatórios de restore em ambiente isolado.

## Pré-requisitos

- Acesso SSH ao host onde roda o `zylos-memory-system`.
- Chave de criptografia simétrica (AES-256-GCM) armazenada no cofre (Vault/Secrets Manager).
- `sqlite3` >= 3.44 instalado no host de backup.
- `gpg` ou `age` disponível para criptografia dos artefatos.
- Bucket S3-compatível ou volume NFS montado em `/mnt/zylos-backup`.
- Cron ou systemd timer configurado com permissão de escrita nos diretórios de backup.

---

## Procedimento

### 1. Schedule de Backups

| Tipo       | Frequência | Horário (BRT) | Retenção        |
|------------|------------|----------------|-----------------|
| Incremental | Diário     | 02:00          | 30 dias (hot)   |
| Full        | Semanal    | Domingo 03:00  | 90 dias (cold)  |

### 2. Backup Incremental Diário

> **🟢 Júnior** — Execute o script agendado via cron. Não modifique parâmetros sem aprovação.

```bash
# /opt/zylos-next/scripts/backup-incremental.sh
#!/usr/bin/env bash
set -euo pipefail

DB_PATH="/var/lib/zylos/memory/main.db"
BACKUP_DIR="/mnt/zylos-backup/incremental/$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# SQLite WAL backup consistente usando .backup command
sqlite3 "$DB_PATH" ".backup '${BACKUP_DIR}/main-snapshot.db'"

# Capturar WAL delta (apenas páginas modificadas desde o último snapshot)
sqlite3 "$DB_PATH" "PRAGMA wal_checkpoint(TRUNCATE);"

# Compactar e criptografar
tar czf "${BACKUP_DIR}/backup.tar.gz" -C "$BACKUP_DIR" main-snapshot.db
age -r "$(cat /etc/zylos/backup-pubkey.txt)" \
    -o "${BACKUP_DIR}/backup.tar.gz.age" \
    "${BACKUP_DIR}/backup.tar.gz"

# Upload para cold storage após 30 dias
find /mnt/zylos-backup/incremental/ -mtime +30 -exec mv {} /mnt/zylos-backup/cold/ \;
```

```cron
# /etc/cron.d/zylos-backup
0 2 * * * root /opt/zylos-next/scripts/backup-incremental.sh >> /var/log/zylos/backup.log 2>&1
```

### 3. Backup Full Semanal

> **🟡 Pleno** — Valide o checksum do backup full e registre no ledger de auditoria.

```bash
# /opt/zylos-next/scripts/backup-full.sh
#!/usr/bin/env bash
set -euo pipefail

DB_PATH="/var/lib/zylos/memory/main.db"
BACKUP_DIR="/mnt/zylos-backup/full/$(date +%G-W%V)"
mkdir -p "$BACKUP_DIR"

# Parar writers temporariamente (lock advisory)
flock -x /var/lock/zylos-writer.lock -c "
  sqlite3 '$DB_PATH' '.backup \"${BACKUP_DIR}/main-full.db\"'
"

# Verificação de integridade obrigatória
INTEGRITY=$(sqlite3 "${BACKUP_DIR}/main-full.db" "PRAGMA integrity_check;")
if [ "$INTEGRITY" != "ok" ]; then
  echo "FALHA DE INTEGRIDADE: $INTEGRITY" >&2
  exit 1
fi

# Checksum SHA-256
sha256sum "${BACKUP_DIR}/main-full.db" > "${BACKUP_DIR}/SHA256SUMS"

# Criptografia at-rest
age -r "$(cat /etc/zylos/backup-pubkey.txt)" \
    -o "${BACKUP_DIR}/main-full.db.age" \
    "${BACKUP_DIR}/main-full.db"

# Rotação: deletar full backups com mais de 90 dias
find /mnt/zylos-backup/full/ -mtime +90 -type f -delete

echo "Backup full concluído: ${BACKUP_DIR}"
```

```cron
0 3 * * 0 root /opt/zylos-next/scripts/backup-full.sh >> /var/log/zylos/backup.log 2>&1
```

### 4. Verificação de Integridade

> **🟢 Júnior** — Execute a verificação pós-backup e reporte qualquer resultado diferente de `ok`.

```bash
# Integridade do banco principal
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA integrity_check;"
# Saída esperada: ok

# Integridade do backup
sqlite3 /mnt/zylos-backup/full/$(date +%G-W%V)/main-full.db "PRAGMA integrity_check;"

# Quick check (mais rápido, menos exaustivo)
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA quick_check;"

# Verificar WAL mode ativo
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA journal_mode;"
# Saída esperada: wal

# Verificar page_size e foreign_keys
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA page_size; PRAGMA foreign_key_check;"
```

### 5. Restore Completo

> **🔴 Senior** — Apenas Seniors devem executar restores em produção. Confirme janela de manutenção e comunique o time antes de iniciar.

```bash
# /opt/zylos-next/scripts/restore-full.sh
#!/usr/bin/env bash
set -euo pipefail

BACKUP_FILE="${1:?Uso: restore-full.sh <caminho-backup.age>}"
DB_PATH="/var/lib/zylos/memory/main.db"
RESTORE_DIR="/tmp/zylos-restore-$(date +%s)"
mkdir -p "$RESTORE_DIR"

# 1. Descriptografar
age -d -i /etc/zylos/backup-key.txt "$BACKUP_FILE" > "${RESTORE_DIR}/main-restored.db"

# 2. Verificar integridade antes de aplicar
INTEGRITY=$(sqlite3 "${RESTORE_DIR}/main-restored.db" "PRAGMA integrity_check;")
if [ "$INTEGRITY" != "ok" ]; then
  echo "Backup corrompido: $INTEGRITY" >&2
  rm -rf "$RESTORE_DIR"
  exit 1
fi

# 3. Parar o zylos-memory-system
systemctl stop zylos-memory-system

# 4. Backup de segurança do estado atual
cp "$DB_PATH" "${DB_PATH}.pre-restore-$(date +%s)"

# 5. Aplicar restore
cp "${RESTORE_DIR}/main-restored.db" "$DB_PATH"

# 6. Recriar WAL e SHM
rm -f "${DB_PATH}-wal" "${DB_PATH}-shm"
sqlite3 "$DB_PATH" "PRAGMA wal_checkpoint(RESTART);"

# 7. Reiniciar serviço
systemctl start zylos-memory-system

# 8. Verificar health
sleep 5
curl -sf http://localhost:3000/api/v1/ready || { echo "Falha no health check"; exit 1; }

rm -rf "$RESTORE_DIR"
echo "Restore completo com sucesso"
```

### 6. Restore Parcial

> **🟡 Pleno** — Use restore parcial para recuperar dados específicos sem afetar o banco inteiro.

```bash
# Restaurar apenas skills de um agente específico
sqlite3 /tmp/zylos-partial.db <<'SQL'
ATTACH DATABASE '/mnt/zylos-backup/full/2026-W44/main-full.db' AS backup;
ATTACH DATABASE '/var/lib/zylos/memory/main.db' AS live;

-- Copiar skills do agente alvo
INSERT OR REPLACE INTO live.skills
SELECT * FROM backup.skills
WHERE agent_id = 'agent-0042';

-- Copiar memórias associadas (30 dias)
INSERT OR REPLACE INTO live.memories
SELECT * FROM backup.memories
WHERE agent_id = 'agent-0042'
  AND created_at >= datetime('now', '-30 days');

DETACH DATABASE backup;
DETACH DATABASE live;
SQL

# Verificar integridade após restore parcial
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA integrity_check;"
sqlite3 /var/lib/zylos/memory/main.db "PRAGMA foreign_key_check;"
```

### 7. Retenção LGPD

> **🔴 Senior** — A exclusão definitiva de dados pessoais deve ser aprovada pelo DPO e registrada no ledger de auditoria.

```
Política de Retenção:
┌──────────┬─────────────┬────────────────────────────────────────────┐
│ Camada   │ Retenção    │ Ação                                       │
├──────────┼─────────────┼────────────────────────────────────────────┤
│ Hot      │ 30 dias     │ Acesso imediato, storage local/NVMe        │
│ Cold     │ 90 dias     │ S3 Glacier / storage econômico             │
│ Delete   │ > 90 dias   │ Exclusão definitiva (shred + crypto-shred) │
└──────────┴─────────────┴────────────────────────────────────────────┘
```

```bash
# Exclusão LGPD por titular (crypto-shred + delete)
# Identificar chaves do titular
TENANT_ID="${1:?Uso: lgpd-delete.sh <tenant-id>}"

# 1. Crypto-shred: rotacionar a chave de criptografia do tenant
# (dados se tornam irrecuperáveis mesmo que o arquivo persista)
vault write transit/keys/zylos-tenant-${TENANT_ID}/rotate

# 2. Delete físico dos registros
sqlite3 /var/lib/zylos/memory/main.db <<SQL
DELETE FROM memories WHERE tenant_id = '${TENANT_ID}';
DELETE FROM conversations WHERE tenant_id = '${TENANT_ID}';
DELETE FROM skills WHERE tenant_id = '${TENANT_ID}';
VACUUM;
SQL

# 3. Registrar no ledger de auditoria
echo "$(date -Iseconds) LGPD_DELETE tenant=${TENANT_ID} operator=$(whoami)" \
  >> /var/log/zylos/lgpd-audit.log
```

### 8. Teste Mensal de Restore (Obrigatório)

> **🔴 Senior** — Agende e execute o teste de restore na primeira semana de cada mês. Documente o resultado.

```bash
# /opt/zylos-next/scripts/monthly-restore-test.sh
#!/usr/bin/env bash
set -euo pipefail

TEST_ENV="/opt/zylos-test"
BACKUP_LATEST=$(ls -t /mnt/zylos-backup/full/*/main-full.db.age | head -1)

# Provisionar ambiente isolado
docker compose -f "$TEST_ENV/docker-compose.yml" down -v 2>/dev/null || true
docker compose -f "$TEST_ENV/docker-compose.yml" up -d zylos-test-db

sleep 10

# Restaurar no ambiente de teste
age -d -i /etc/zylos/backup-key.txt "$BACKUP_LATEST" \
  | docker exec -i zylos-test-db sqlite3 /data/test.db

# Validações
INTEGRITY=$(docker exec zylos-test-db sqlite3 /data/test.db "PRAGMA integrity_check;")
ROW_COUNT=$(docker exec zylos-test-db sqlite3 /data/test.db "SELECT count(*) FROM memories;")
SCHEMA_VER=$(docker exec zylos-test-db sqlite3 /data/test.db "PRAGMA user_version;")

echo "=== RELATÓRIO DE TESTE MENSAL ==="
echo "Data: $(date -Iseconds)"
echo "Backup testado: $BACKUP_LATEST"
echo "Integridade: $INTEGRITY"
echo "Total de memórias: $ROW_COUNT"
echo "Versão do schema: $SCHEMA_VER"

if [ "$INTEGRITY" = "ok" ] && [ "$ROW_COUNT" -gt 0 ]; then
  echo "STATUS: ✅ APROVADO"
  echo "APROVADO $(date +%Y%m%d) $BACKUP_LATEST" >> /var/log/zylos/restore-tests.log
else
  echo "STATUS: ❌ REPROVADO — AÇÃO IMEDIATA NECESSÁRIA"
  echo "FALHOU $(date +%Y%m%d) $BACKUP_LATEST" >> /var/log/zylos/restore-tests.log
  exit 1
fi

# Cleanup
docker compose -f "$TEST_ENV/docker-compose.yml" down -v
```

---

## Rollback

| Cenário                          | Ação                                                    |
|----------------------------------|---------------------------------------------------------|
| Restore falhou no health check   | `cp ${DB_PATH}.pre-restore-* $DB_PATH` e reiniciar     |
| Backup corrompido detectado      | Restaurar do último backup full válido (max 7 dias)     |
| WAL corrompido pós-restore       | `rm -f *.db-wal *.db-shm` + `PRAGMA wal_checkpoint`    |
| Perda de chave de criptografia   | Acionar DPO, declarar incidente LGPD, crypto-shred      |

---

## Verificação

1. `sqlite3 main.db "PRAGMA integrity_check;"` → deve retornar `ok`.
2. `systemctl status zylos-memory-system` → active (running).
3. `curl -s http://localhost:3000/api/v1/ready` → `{"status":"ready"}`.
4. `curl -s http://localhost:3000/api/v1/health` → todos os checks green.
5. `/var/log/zylos/restore-tests.log` → último teste com status APROVADO há menos de 31 dias.
6. `find /mnt/zylos-backup/ -name "*.age" -mtime +90` → nenhum arquivo (rotação funcionando).
7. Ledger de auditoria em `/var/log/zylos/lgpd-audit.log` com entradas recentes de operações de exclusão.

---

## Referências

- [SQLite WAL Documentation](https://www.sqlite.org/wal.html)
- [SQLite Online Backup API](https://www.sqlite.org/backup.html)
- LGPD — Lei nº 13.709/2018, Art. 15 (Término do tratamento)
- [age encryption tool](https://github.com/FiloSottile/age)
- Zylos-Next Architecture: `zylos-memory-system` crate (SQLite WAL + petgraph)