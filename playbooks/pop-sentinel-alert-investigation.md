# POP — Investigação de Alertas do Sentinel (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`aiops-anomaly-response.md`](./aiops-anomaly-response.md), [`sop-incident-triage.md`](./sop-incident-triage.md)
**Runbook Relacionado:** `runbooks/sentinel-aiops.md`
**Público:** SRE On-Call, DevOps Pleno+, Agentes Sentinel
**Tempo Estimado:** 10-30 minutos

---

## 1. Objetivo

Padronizar a investigação de alertas gerados pelo módulo Sentinel/AIOps, desde a triagem inicial até a classificação como verdadeiro positivo ou falso positivo, garantindo MTTR dentro do SLA e alimentação contínua do pipeline de recalibração.

## 2. Pré-Condições

- [ ] Acesso ao dashboard Grafana ("Sentinel Anomaly Detection")
- [ ] CLI `zylos-cli` configurada com credenciais válidas
- [ ] Prometheus/Grafana acessível (`curl -sf http://localhost:9090/-/healthy`)
- [ ] Permissão de leitura em métricas e alertas do Sentinel
- [ ] Canal Slack #alerts monitorado (para notificações em tempo real)

**Regra:** Se Prometheus/Grafana estiver inacessível, verificar health do stack de observabilidade antes de iniciar. Sem métricas, investigação é impossível → escalar SEV-2.

## 3. Procedimento

### Passo 1: Receber e Classificar Alerta (2 min)
```bash
# Listar alertas ativos por severidade
zylos-cli sentinel alerts --severity SEV-1 --last 1h --format table
zylos-cli sentinel alerts --severity SEV-2 --last 4h --format table
```
**Output esperado:** Tabela com `alert_id`, `component`, `metric`, `zscore`, `timestamp`, `status`.
**Se vazio:** Verificar filtro de tempo/severidade. Se certeza de que deveria haver alertas → verificar scraper health.

### Passo 2: Obter Detalhes do Alerta (1 min)
```bash
zylos-cli sentinel alert get --alert-id <ALERT_ID> --format json
```
**Output esperado:** JSON completo com `context`, `correlated_events`, `threshold_config`, `raw_metric_values`.
**Se falhar:** Alert ID inválido ou expirado. Listar alertas novamente (passo 1).

### Passo 3: Correlacionar com Outros Componentes (3-5 min)
```bash
zylos-cli sentinel correlate --alert-id <ALERT_ID> --window 30m --format table
```
**Output esperado:** Tabela de eventos relacionados em outros componentes (gateway, memory, docker, etc.).
**Se vazio:** Anomalia isolada (provável falso positivo ou componente específico). Prosseguir para passo 4.
**Se múltiplos correlatos:** Possível causa raiz sistêmica → priorizar investigação cross-componente.

### Passo 4: Verificar Métricas Brutas no Grafana (5-10 min)
1. Acesse dashboard "Sentinel Anomaly Detection"
2. Filtre pelo componente e janela temporal do alerta
3. Confirme visualmente a anomalia no gráfico
4. Compare com baseline histórico (mesmo horário/dia da semana anterior)
5. Verifique se há padrões sazonais ou picos conhecidos

**Output esperado:** Gráfico mostrando desvio claro da baseline OU comportamento normal dentro de variação esperada.
**Se gráfico não carregar:** Verificar conectividade Grafana → `curl -sf http://localhost:3000/api/health`.

### Passo 5: Classificar e Agir (2-5 min)

#### Se VERDADEIRO POSITIVO:
```bash
# Confirmar alerta
zylos-cli sentinel feedback --alert-id <ALERT_ID> --label true_positive --note "Causa raiz identificada: {descrição}"

# Acionar playbook correspondente ao componente afetado
# Ex: gateway → runbooks/zylos-gateway.md § Troubleshooting
# Ex: memory → runbooks/fase2-memory-system.md § Recovery

# Se SEV-1/2, seguir sop-incident-triage.md
```

#### Se FALSO POSITIVO:
```bash
# Marcar como falso positivo (alimenta recalibração automática)
zylos-cli sentinel feedback --alert-id <ALERT_ID> --label false_positive --note "Motivo: {explicação}"

# Se padrão recorrente, ajustar threshold
zylos-cli sentinel config set --component <COMPONENT> --zscore-threshold <NOVO_VALOR>
```

#### Se INCONCLUSIVO:
```bash
# Marcar para revisão posterior
zylos-cli sentinel feedback --alert-id <ALERT_ID> --label needs_review --note "Dados insuficientes, requer análise manual"

# Criar ticket para investigação profunda
zylos-cli task create --title "Investigar alerta <ALERT_ID>" --priority medium --tag sentinel
```

### Passo 6: Documentar e Encerrar (2 min)
```bash
# Log de investigação
zylos-cli evidence log \
 --type sentinel_investigation \
 --metadata "{\"alert_id\":\"<ALERT_ID>\",\"classification\":\"{true_positive|false_positive|needs_review}\",\"investigator\":\"{nome}\",\"duration_min\":{tempo},\"root_cause\":\"{descrição}\"}"

# Se verdadeiro positivo e SEV ≥ 2, agendar post-mortem
zylos-cli task create --title "Post-mortem: <resumo incidente>" --priority high --tag post-mortem
```
**Output esperado:** Hash da evidência registrada + ID da task criada (se aplicável).

## 4. Validação Pós-Execução

- [ ] Alerta classificado (true_positive / false_positive / needs_review)
- [ ] Feedback registrado no Sentinel (`zylos-cli sentinel feedback`)
- [ ] Métricas brutas verificadas no Grafana
- [ ] Correlação cross-componente analisada
- [ ] Evidência de investigação registrada no Evidence Ledger
- [ ] Se true_positive: playbook acionado e/ou incidente aberto
- [ ] Se false_positive: threshold ajustado (se recorrente)
- [ ] Se SEV ≥ 2: post-mortem agendado

## 5. Rollback

**Trigger conditions:**
- Classificação incorreta identificada posteriormente (ex: falso positivo era na verdade verdadeiro)
- Threshold ajustado causou excesso de novos falsos positivos
- Investigação incompleta (dados perdidos ou corrompidos)

**Passos de rollback:**
```bash
# 1. Reverter classificação (se necessário)
zylos-cli sentinel feedback --alert-id <ALERT_ID> --label reclassified --note "Reclassificado de {original} para {novo}: {motivo}"

# 2. Reverter ajuste de threshold (se aplicável)
zylos-cli sentinel config set --component <COMPONENT> --zscore-threshold <VALOR_ANTERIOR>

# 3. Reabrir incidente (se foi fechado prematuramente)
zylos-cli incident reopen --incident-id <ID> --reason "Classificação incorreta"

# 4. Log de rollback
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) SENTINEL_ROLLBACK alert=<ALERT_ID> reason={motivo}" >> /var/log/zylos/sentinel-audit.log

# 5. Notificar SRE Lead se rollback afetou múltiplos alertas
```

**Validação pós-rollback:**
- [ ] Classificação corrigida no Sentinel
- [ ] Threshold revertido (se aplicável)
- [ ] Incidente reaberto (se aplicável)
- [ ] Log de rollback registrado
- [ ] SRE Lead notificado (se impacto amplo)

## 6. Quando NÃO Usar Este POP

- **Alerta de segurança (breach, vazamento)** → Use `playbooks/incident-response-soar.md` + SOAR workflow
- **Configuração inicial do Sentinel** → Use `runbooks/sentinel-aiops.md` § Setup
- **Recalibração completa do modelo** → Use `zylos-cli sentinel recalibrate --last 7d` (procedimento separado)
- **Criação de novo detector** → Use `crates/zylos-sentinel/src/lib.rs` § Custom Detectors
- **Silenciar alerta durante maintenance** → Use `zylos-cli sentinel silence --duration 2h --reason "deploy"`
- **Export de métricas para análise offline** → Use `runbooks/sentinel-aiops.md` § Export

## 7. Referências Cruzadas

- Runbook completo: `runbooks/sentinel-aiops.md`
- SOP de triagem de incidentes: `sop-incident-triage.md`
- Playbook de resposta AIOps: `playbooks/aiops-anomaly-response.md`
- SOAR workflow: `docs/soar/runbooks/sentinel-auto-triage.runbook.yaml`
- Dashboard Grafana: "Sentinel Anomaly Detection"
- Evidence Ledger: `pop-evidence-ledger-query.md`
- Logs de auditoria: `/var/log/zylos/sentinel-audit.log`
- Configuração do Sentinel: `config/sentinel.toml`