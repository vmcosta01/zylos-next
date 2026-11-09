# Runbook: Zylos Proof — Verificação de Provas e Evidências

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Necessidade de verificar integridade de evidência ou validar prova de execução
**Público-alvo:** Auditor / Compliance Officer / Agent Operator
**Pré-requisitos:** Crate `zylos-proof` compilado, acesso ao evidence ledger

---

## 1. Visão Geral

O módulo Proof garante integridade e verificabilidade de todas as ações do sistema:
- Hash chain imutável de evidências
- Assinatura criptográfica de outputs de agentes
- Verificação cross-source (proof grounding)
- Integração com evidence-ledger para auditoria
- Certificados de conformidade automáticos

**Playbook associado:** [`playbooks/evidence-chain-of-custody.md`](../playbooks/evidence-chain-of-custody.md)

---

## 2. Nível Senior

### Modelo de Confiança
| Proof Level | Definição | Verificação |
|-------------|-----------|-------------|
| Observed | Dado bruto coletado | Hash + timestamp |
| Confirmed | Cross-validado com ≥2 fontes | Hash + signature + source_count ≥2 |
| Verified | Adversarial panel aprovou | Hash + signature + panel_signatures ≥2 |
| Certified | Compliance check passou | Todos acima + compliance_stamp |

### Decisões
| Decisão | Racional |
|---------|----------|
| SHA-256 para hashing | Balance entre segurança e performance; NIST approved |
| Ed25519 para assinaturas | Chaves curtas, verificação rápida, post-quantum resistant (hybrid) |
| Merkle tree para batch verification | O(log n) proofs vs O(n) linear |

---

## 3. Nível Pleno

### Comandos
```bash
# Verificar integridade de evidência
zylos proof verify --evidence-id <ID>

# Gerar certificado de conformidade
zylos proof certify --workflow-id <ID> --standard SOC2

# Listar provas pendentes de verificação
zylos proof pending --last 24h

# Exportar chain de evidências para auditoria
zylos proof export --from 2026-11-01 --format jsonl --output audit.jsonl

# Validar assinatura de output de agente
zylos proof validate-signature --agent-id <ID> --output-hash <HASH>
```

### Troubleshooting
| Sintoma | Causa | Solução |
|---------|-------|---------|
| Verification failed: hash mismatch | Evidência alterada pós-coleta | Investigar chain of custody; possível tampering |
| Signature invalid | Chave rotacionada ou agente comprometido | Verificar key rotation log; isolar agente |
| Missing source for Confirmed level | Fonte secundária indisponível | Aguardar ou降级 para Observed com nota |
| Certificate generation timeout | Ledger sobrecarregado | Retry com backoff; verificar ledger health |

---

## 4. Nível Júnior

### Verificar Integridade de Evidência (Passo a Passo)

1. **Obter ID da evidência**
   ```bash
   zylos proof list --workflow-id <ID> --format table
   ```

2. **Executar verificação**
   ```bash
   zylos proof verify --evidence-id <EVIDENCE_ID>
   ```
   Output esperado: `{"status": "valid", "level": "confirmed", "sources": 3}`

3. **Se inválida:**
   - NÃO prosseguir com uso da evidência
   - Acionar playbook de incident response
   - Documentar no evidence-ledger como "compromised"

4. **Se válida mas nível insuficiente:**
   - Verificar se fontes adicionais estão disponíveis
   - Solicitar re-coleta se necessário

### Regras
- Nunca confiar em evidência com status "invalid"
- Sempre verificar antes de usar em decisão crítica
- Documentar toda verificação no audit log

---

## 5. Referências Cruzadas

- **Playbook:** [`playbooks/evidence-chain-of-custody.md`](../playbooks/evidence-chain-of-custody.md)
- **SOAR Workflow:** [`docs/soar/runbooks/verification-proof.runbook.yaml`](../docs/soar/runbooks/verification-proof.runbook.yaml)
- **Crate:** `crates/zylos-proof/src/lib.rs`
- **Runbook relacionado:** [`evidence-ledger.md`](evidence-ledger.md), [`fase7a-proof-verification.md`](fase7a-proof-verification.md)
