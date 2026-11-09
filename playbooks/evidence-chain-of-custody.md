# Playbook: Cadeia de Custódia de Evidências

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Incidente de segurança, auditoria externa, ou necessidade de preservação de evidências
**Público-alvo:** Compliance Officer, Auditor, Tech Lead, SRE
**SLA:** Preservação imediata; export em <1h para auditoria urgente

---

## 1. Visão Geral

Este playbook define a estratégia de preservação, verificação e exportação de evidências para garantir integridade legal e conformidade. Para operações técnicas, consulte os runbooks associados.

**Runbooks associados:**
- [`runbooks/evidence-ledger.md`](../runbooks/evidence-ledger.md) — Consulta e exportação do ledger
- [`runbooks/proof-verification.md`](../runbooks/proof-verification.md) — Verificação de integridade e certificados

---

## 2. Nível Senior — Governança de Evidências

### Matriz RACI
| Atividade | Responsible | Accountable | Consulted | Informed |
|-----------|-------------|-------------|-----------|----------|
| Preservação de evidências | SRE Lead | Compliance Officer | Tech Lead | Legal |
| Verificação de integridade | Auditor | Compliance Officer | SRE Lead | — |
| Export para auditoria externa | Compliance Officer | Legal | SRE Lead | CTO |
| Decisão de retenção/extensão | Compliance Officer | Legal | SRE Lead | — |
| Investigação de tampering | Security Lead | CTO | Compliance Officer | Legal |

### Requisitos Legais
| Requisito | Implementação | Verificação |
|-----------|--------------|-------------|
| Imutabilidade | Append-only ledger + hash chain | `zylos ledger verify-integrity` |
| Rastreabilidade | Timestamp + agent_id + workflow_id em cada entry | Query por qualquer campo |
| Retenção mínima 1 ano | Policy hot/warm/cold configurada | `zylos ledger retention show` |
| Export auditável | Formato JSONL assinado + checksum | Verificar assinatura no export |

### Gates de Decisão
| Gate | Condição | Ação |
|------|----------|------|
| G1: Preservação | Incidente confirmado? | Congelar retenção + snapshot imediato |
| G2: Integridade | Hash chain válida? | Prosseguir com análise; se inválida → incident response |
| G3: Export | Solicitação autorizada? | Gerar export assinado; se não → negar + log |
| G4: Descarte | Período de retenção expirou + sem hold legal? | Executar purge seguro |

---

## 3. Nível Pleno — Procedimentos

### Preservação de Emergência
```
1. zylos ledger snapshot --tag "incident-YYYY-MM-DD" --reason "<descrição>"
2. zylos ledger retention extend --tag "incident-YYYY-MM-DD" --duration 365d
3. zylos ledger verify-integrity --tag "incident-YYYY-MM-DD"
4. Notificar Compliance Officer via email com snapshot ID
```

### Export para Auditoria
```
1. Verificar autorização (ticket de auditoria ou email do Compliance)
2. zylos ledger export --from <START> --to <END> --format jsonl --output audit.jsonl
3. zylos proof certify --file audit.jsonl --standard SOC2
4. Enviar arquivo + certificado via canal seguro
5. Log da exportação no audit trail
```

### Tabela de Escalação
| Situação | Quem Contatar | Canal | Timeout |
|----------|--------------|-------|---------|
| Integrity check failed | Security Lead + Compliance | PagerDuty | 5min |
| Solicitação de export sem autorização | Compliance Officer | Email | 1h |
| Storage tier inacessível | SRE Lead | Slack | 30min |
| Hold legal ativo + tentativa de purge | Legal + Compliance | Email urgente | Imediato |

---

## 4. Nível Júnior — Operações Básicas

### Consultar Evidências de Incidente

1. **Listar snapshots disponíveis**
   ```bash
   zylos ledger snapshots list --tag "incident-*"
   ```

2. **Verificar integridade antes de usar**
   ```bash
   zylos ledger verify-integrity --tag "<SNAPSHOT_TAG>"
   ```
   Se inválido → NÃO usar → escalar imediatamente.

3. **Exportar se autorizado**
   ```bash
   zylos ledger export --tag "<SNAPSHOT_TAG>" --format jsonl --output report.jsonl
   ```

### Regras Críticas
- **NUNCA** modificar ou deletar evidências
- **SEMPRE** verificar integridade antes de confiar
- **SEMPRE** documentar quem acessou e quando
- Se dúvida sobre autorização → perguntar ao Compliance Officer

---

## 5. Referências Cruzadas

- **Runbooks:** [`runbooks/evidence-ledger.md`](../runbooks/evidence-ledger.md), [`runbooks/proof-verification.md`](../runbooks/proof-verification.md)
- **SOAR Workflow:** [`docs/soar/runbooks/forensic-evidence-collection.runbook.yaml`](../docs/soar/runbooks/forensic-evidence-collection.runbook.yaml)
- **Playbook relacionado:** [`incident-response.md`](incident-response.md)
- **Próxima revisão:** 2027-02-05 ou após auditoria externa
