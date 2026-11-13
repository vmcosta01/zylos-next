# POP — Consulta e Exportação do Evidence Ledger (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`evidence-chain-of-custody.md`](./evidence-chain-of-custody.md), [`adversarial-audit.md`](./adversarial-audit.md)
**Runbook Relacionado:** `runbooks/evidence-ledger.md`
**Público:** Auditor, Compliance, SRE, Tech Lead
**Tempo Estimado:** 5-15 minutos

---

## 1. Objetivo

Padronizar a consulta, verificação de integridade e exportação de evidências do Evidence Ledger para auditorias, post-mortems e conformidade regulatória, garantindo cadeia de custódia válida e dados íntegros.

## 2. Pré-Condições

- [ ] Crate `zylos-evidence-ledger` compilado e acessível
- [ ] Storage backend acessível (PostgreSQL para hot tier, S3 para warm/cold)
- [ ] Permissão de leitura no ledger (role: `auditor` ou `admin`)
- [ ] Chave de verificação HMAC disponível (para validação de assinaturas)
- [ ] Ferramenta de exportação instalada (`zylos-cli ledger`)

**Regra:** Se storage backend estiver inacessível, verificar conectividade antes de iniciar. Para cold tier (Glacier), solicitar restore antecipado (horas de latência).

## 3. Procedimento

### Passo 1: Verificar Status do Ledger (1 min)
```bash
zylos-cli ledger status
```
**Output esperado:** `{"tier":"hot","entries":42857,"last_merkle_root":"abc123...","integrity":"valid"}`
**Se falhar:** Verificar conectividade PostgreSQL/S3. Se `integrity != valid` → escalar SEV-1 imediatamente.

### Passo 2: Consultar Evidências (2-5 min)
```bash
# Por workflow
zylos-cli ledger query --workflow-id <ID> --limit 100 --format table

# Por agente
zylos-cli ledger query --agent-id agent-42 --from 2026-11-01 --to 2026-11-10

# Por tag
zylos-cli ledger query --tag security-incident --format jsonl

# Por intervalo temporal
zylos-cli ledger query --from "2026-11-10T00:00:00Z" --to "2026-11-10T23:59:59Z"
```
**Output esperado:** Tabela/JSONL com `entry_id`, `timestamp`, `type`, `hash`, `metadata`.
**Se vazio:** Verificar filtros. Se certeza de que deveria haver dados → verificar tier (hot/warm/cold).

### Passo 3: Verificar Integridade da Chain (2-5 min)
```bash
# Verificação completa (pode demorar em datasets grandes)
zylos-cli ledger verify-integrity --from 2026-11-01

# Verificação rápida (apenas Merkle roots)
zylos-cli ledger verify-integrity --quick --from 2026-11-01
```
**Output esperado:** `{"status":"valid","entries_checked":1234,"merkle_root":"def456...","verified_at":"..."}`
**Se `status != valid`:** ⚠️ CORRUPTION DETECTED → Isolar shard afetado, acionar `playbooks/incident-response.md` § SEV-1, NÃO usar dados para auditoria até resolução.

### Passo 4: Exportar para Auditoria (3-10 min)
```bash
# JSONL (machine-readable)
zylos-cli ledger export --from 2026-11-01 --to 2026-11-10 --format jsonl --output audit_2026-W45.jsonl

# CSV (spreadsheet)
zylos-cli ledger export --workflow-id <ID> --format csv --output workflow_report.csv

# PDF (formal audit report)
zylos-cli ledger export --from 2026-11-01 --format pdf --output compliance_report.pdf

# Com assinatura criptográfica (para evidência legal)
zylos-cli ledger export --from 2026-11-01 --format jsonl --sign --output signed_audit.jsonl
```
**Output esperado:** Arquivo criado no path especificado + hash SHA-256 impresso no stdout.
**Se falhar com timeout:** Dataset muito grande → reduzir intervalo ou usar `--stream` flag.

### Passo 5: Validar Export (1 min)
```bash
# Verificar hash do arquivo exportado
sha256sum audit_2026-W45.jsonl

# Comparar com hash impresso no passo anterior
# Devem ser idênticos
```
**Output esperado:** Hash corresponde ao output do passo 4.
**Se diferente:** Arquivo corrompido durante transferência → re-exportar.

## 4. Validação Pós-Execução

- [ ] Query retornou resultados esperados (contagem consistente)
- [ ] Integrity check passou (`status: valid`)
- [ ] Export gerado com hash válido
- [ ] Arquivo de export acessível e legível
- [ ] Log de auditoria registrado (`zylos-cli evidence log --type ledger_export`)
- [ ] Se export assinado: assinatura verificada com chave pública

## 5. Rollback

**Trigger conditions:**
- Integrity check falhou (corruption detectada)
- Export incompleto ou corrompido
- Dados inconsistentes entre tiers (hot vs warm)

**Passos de rollback:**
```bash
# 1. Parar todas as consultas ativas
zylos-cli ledger cancel-all-queries

# 2. Isolar shard corrompido (se integrity check falhou)
zylos-cli ledger isolate-shard --shard-id <ID> --reason "corruption_detected"

# 3. Restaurar de backup (se disponível)
# Seguir pop-backup-restore.md § Procedimento de Restore

# 4. Notificar Compliance e Legal (se dados de auditoria afetados)
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) LEDGER_ROLLBACK shard=<ID>" >> /var/log/zylos/ledger-audit.log

# 5. Escalar SEV-1 se corruption confirmada
```

**Validação pós-rollback:**
- [ ] Shard isolado ou restaurado
- [ ] Integrity check passa no dataset recuperado
- [ ] Stakeholders (Compliance, Legal) notificados
- [ ] Post-mortem agendado

## 6. Quando NÃO Usar Este POP

- **Inserir novas evidências** → Use API do crate `zylos-evidence-ledger` (append-only via código)
- **Modificar entradas existentes** → ❌ PROIBIDO (ledger é append-only por design)
- **Deletar evidências** → Use `zylos-cli ledger retention purge` (apenas após expiry LGPD)
- **Verificar artefatos SOAR específicos** → Use `runbooks/soar-incident-response.md` § Audit Trail
- **Backup do ledger completo** → Use `pop-backup-restore.md` (este POP é apenas para consulta/export)
- **Migração entre storage tiers** → Use `zylos-cli ledger compact` (automatizado)

## 7. Referências Cruzadas

- Runbook completo: `runbooks/evidence-ledger.md`
- Playbook de cadeia de custódia: `playbooks/evidence-chain-of-custody.md`
- Playbook de auditoria adversarial: `playbooks/adversarial-audit.md`
- SOAR workflow: `docs/soar/runbooks/forensic-evidence-collection.runbook.yaml`
- POP de backup/restore: `pop-backup-restore.md`
- Logs de auditoria: `/var/log/zylos/ledger-audit.log`
- Chave de verificação: `/etc/zylos/ledger-verify-key.pub`