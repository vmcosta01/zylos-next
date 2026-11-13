# POP — Gerenciamento de Sandbox Docker (Procedimento Operacional Padrão)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`deploy-release.md`](./deploy-release.md), [`incident-response.md`](./incident-response.md)
**Runbook Relacionado:** `runbooks/zylos-docker.md`
**Público:** SRE, DevOps, Agentes Cerebelo
**Tempo Estimado:** 5-15 minutos

---

## 1. Objetivo

Padronizar a criação, monitoramento e limpeza de containers Docker sandbox para execução de código não-confiável, garantindo isolamento de segurança e aprovação prévia via workflow HITL.

## 2. Pré-Condições

- [ ] Docker daemon rodando (`docker info` retorna OK)
- [ ] Imagem base disponível localmente (`docker images | grep zylos-runtime`)
- [ ] Approval policy configurada conforme ambiente (AutoAllow apenas em dev/staging)
- [ ] Permissão de escrita em `/var/lib/zylos/docker/`
- [ ] Resource limits definidos no config (`max_memory_mb`, `max_cpu_percent`)

**Regra:** Se qualquer pré-condição não for atendida, NÃO iniciar. Resolver pré-condição primeiro ou escalar para SRE on-call.

## 3. Procedimento

### Passo 1: Verificar Status do Docker Daemon (1 min)
```bash
docker info --format '{{.ServerVersion}}'
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' | head -20
```
**Output esperado:** Versão do servidor + lista de containers ativos (pode ser vazia).
**Se falhar:** Iniciar Docker Desktop ou `systemctl start docker`. Se persistir → escalar SEV-2.

### Passo 2: Validar Approval Policy Ativa (1 min)
```bash
# Via API do gateway
curl -s http://localhost:9876/api/v1/docker/approval-policy | jq .
```
**Output esperado:** `{"policy": "ManualConfirm"}` em produção, `{"policy": "AutoAllow"}` em dev.
**Se falhar:** Verificar config em `config/gateway.toml` § `[docker.approval]`.

### Passo 3: Criar Container Sandbox (2-5 min)
```bash
# Via CLI (passa por approval workflow automaticamente)
zylos-cli docker create \
  --agent-id agent-42 \
  --image zylos-runtime:latest \
  --memory-limit 256m \
  --cpu-limit 1.0 \
  --network none \
  --timeout 30s
```
**Output esperado:** `{"container_id": "abc123...", "status": "approved", "created_at": "..."}`
**Se falhar com timeout:** Approval pendente → verificar dashboard HITL ou Slack #approvals.
**Se falhar com deny:** Log de auditoria gerado → verificar motivo em `~/Zylos/logs/docker-audit.jsonl`.

### Passo 4: Executar Código no Container (variável)
```bash
zylos-cli docker exec --container-id abc123... --command "python script.py"
```
**Output esperado:** stdout/stderr do comando executado.
**Se falhar:** Verificar logs do container: `zylos-cli docker logs --container-id abc123... --tail 50`.

### Passo 5: Parar e Limpar Container (1 min)
```bash
zylos-cli docker stop --container-id abc123...
zylos-cli docker rm --container-id abc123...
```
**Output esperado:** `{"status": "stopped"}` e `{"status": "removed"}`.
**Se falhar:** Forçar remoção: `docker rm -f abc123...` e reportar bug.

## 4. Validação Pós-Execução

- [ ] Container parado e removido (`docker ps -a | grep abc123` retorna vazio)
- [ ] Logs de execução capturados em `~/Zylos/logs/docker/{container_id}.log`
- [ ] Audit trail registrado no Evidence Ledger
- [ ] Nenhum container órfão ativo (`docker ps --filter "label=zylos-agent" --format '{{.Names}}'`)
- [ ] Métricas de resource usage exportadas para Prometheus

## 5. Rollback

**Trigger conditions:**
- Container não responde após 30s
- Resource usage excede limites configurados
- Approval workflow travou sem resposta

**Passos de rollback:**
```bash
# 1. Forçar parada imediata
docker kill $(docker ps -q --filter "label=zylos-agent")

# 2. Remover todos os containers órfãos
docker rm -f $(docker ps -aq --filter "label=zylos-agent")

# 3. Verificar limpeza
docker ps -a --filter "label=zylos-agent" --format '{{.Names}}'
# Output esperado: (vazio)

# 4. Notificar via playbook de incident-response se SEV ≥ 2
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) DOCKER_ROLLBACK triggered" >> ~/Zylos/logs/docker-audit.jsonl
```

**Validação pós-rollback:**
- [ ] Zero containers zylos-agent ativos
- [ ] Logs de rollback registrados
- [ ] Stakeholders notificados (se SEV ≥ 2)

## 6. Quando NÃO Usar Este POP

- **Código puro computacional sem I/O** → Use WASM sandbox (`crates/zylos-sandbox/src/wasm_sandbox.rs`)
- **Container de longa duração (>1h)** → Use `runbooks/zylos-docker.md` § Long-running Containers
- **Debug de container crash loop** → Use `runbooks/zylos-docker.md` § Troubleshooting
- **Mudança de approval policy** → Use `playbooks/deploy-release.md` (requer deploy)
- **Incidente de segurança em container** → Use `playbooks/incident-response-soar.md` + SOAR workflow

## 7. Referências Cruzadas

- Runbook completo: `runbooks/zylos-docker.md`
- Playbook de deploy: `playbooks/deploy-release.md`
- SOAR workflow: `docs/soar/runbooks/container-isolation.runbook.yaml`
- Evidence Ledger: `runbooks/evidence-ledger.md`
- Logs de auditoria: `~/Zylos/logs/docker-audit.jsonl`
- Dashboard HITL: `http://localhost:9876/cockpit#/approvals`