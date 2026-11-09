# Runbook: Zylos Evidence Ledger — Registro Imutável de Evidências

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Necessidade de consultar, auditar ou exportar cadeia de evidências
**Público-alvo:** Auditor / Compliance / SRE
**Pré-requisitos:** Crate `zylos-evidence-ledger` compilado, storage backend acessível

---

## 1. Visão Geral

O Evidence Ledger é o registro imutável e append-only de todas as evidências do sistema:
- Hash chain com Merkle roots periódicas
- Retenção configurável por policy (hot/warm/cold)
- Query por workflow, agente, timestamp, ou tag
- Export para formatos de auditoria (JSONL, CSV, PDF)
- Integração com zylos-proof para verificação

**Playbook associado:** [`playbooks/evidence-chain-of-custody.md`](../playbooks/evidence-chain-of-custody.md)

---

## 2. Nível Senior

### Storage Tiers
| Tier | Retenção | Backend | Query Latency |
|------|----------|---------|---------------|
| Hot | 7 dias | PostgreSQL + index | <10ms |
| Warm | 90 dias | S3-compatible object store | <500ms |
| Cold | 1+ ano | Glacier/archival | Horas |

### Decisões
| Decisão | Racional |
|---------|----------|
| Append-only | Imutabilidade é requisito legal para auditoria |
| Merkle root a cada 1000 entries | Balance entre integridade e overhead |
| Sharding por workflow_id | Evita hot partitions em workloads pesados |

---

## 3. Nível Pleno

### Comandos
```bash
# Consultar evidências
zylos ledger query --workflow-id <ID> --limit 100

# Verificar integridade da chain
zylos ledger verify-integrity --from 2026-11-01

# Exportar para auditoria
zylos ledger export --from 2026-11-01 --to 2026-11-05 --format jsonl --output audit.jsonl

# Ver políticas de retenção
zylos ledger retention show

# Forçar migração hot→warm
zylos ledger compact --older-than 7d
```

### Troubleshooting
| Sintoma | Causa | Solução |
|---------|-------|---------|
| Query timeout | Partition muito grande | Verificar sharding; executar compact |
| Integrity check failed | Corruption ou tampering | Isolar shard; restaurar de backup; acionar incident response |
| Export incompleto | Warm/cold tier inacessível | Verificar conectividade S3/Glacier |
| Write latency alta | Disk I/O saturado | Escalar storage ou mover para tier mais rápido |

---

## 4. Nível Júnior

### Consultar Evidências de Workflow

1. **Listar evidências**
   ```bash
   zylos ledger query --workflow-id <ID> --format table
   ```

2. **Verificar integridade**
   ```bash
   zylos ledger verify-integrity --workflow-id <ID>
   ```
   Output esperado: `{"status": "valid", "entries_checked": 42, "merkle_root": "abc123..."}`

3. **Exportar para revisão externa**
   ```bash
   zylos ledger export --workflow-id <ID> --format pdf --output report.pdf
   ```

### Regras
- Nunca modificar entradas existentes (append-only)
- Sempre verificar integridade antes de usar em auditoria
- Exportar evidências críticas imediatamente após incidente

---

## 5. Referências Cruzadas

- **Playbook:** [`playbooks/evidence-chain-of-custody.md`](../playbooks/evidence-chain-of-custody.md)
- **SOAR Workflow:** [`docs/soar/runbooks/forensic-evidence-collection.runbook.yaml`](../docs/soar/runbooks/forensic-evidence-collection.runbook.yaml)
- **Crate:** `crates/zylos-evidence-ledger/src/lib.rs`
- **Runbook relacionado:** [`proof-verification.md`](proof-verification.md)
