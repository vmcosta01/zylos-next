# SOP — Deploy Seguro com Canary (Standard Operating Procedure)

**Versão:** 1.0
**Data:** 2026-11-10
**Playbook Relacionado:** [`deploy-release.md`](./deploy-release.md)
**Runbooks Relacionados:** `zylos-gateway.md`, `canary-deployment.md`, `backup-restore.md`
**Público:** SRE, DevOps, Tech Lead
**Tempo Estimado:** 15-30 min (dependendo do tamanho do deploy)

---

## 1. Objetivo

Executar deploy de nova versão do zylos-gateway usando estratégia canary com rollback automático, garantindo zero downtime e validação progressiva antes de promoção total.

## 2. Pré-Condições Obrigatórias

- [ ] Todos os testes passando: `cargo test --workspace --all-features`
- [ ] Clippy limpo: `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Benchmarks dentro do target: `cargo bench -p zylos-memory-system --bench memory_system`
- [ ] Backup do DB realizado: `cp data/zylos.db data/zylos.db.pre-deploy.bak`
- [ ] Changelog atualizado e tag criada: `git tag v{X.Y.Z}`
- [ ] Approval HITL obtido (SEV-1/2 ou mudança breaking)
- [ ] War room aberta se deploy fora de janela (09h-17h BRT)

## 3. Procedimento Canary

### Fase 1: Build e Validação (5 min)
```bash
# Build release
cargo build --release -p zylos-gateway

# Validar binário
./target/release/zylos-gateway --version
./target/release/zylos-gateway --validate-config --config config/gateway.toml

# Gerar checksum
sha256sum ./target/release/zylos-gateway > ./target/release/zylos-gateway.sha256
```

### Fase 2: Deploy Canary (5% do tráfego)
```bash
# Iniciar instância canary em porta alternativa
./target/release/zylos-gateway \
  --config config/gateway-canary.toml \
  --port 9877 \
  --metrics-label canary &

# Aguardar health check
for i in $(seq 1 30); do
  curl -sf http://localhost:9877/api/v1/health && break
  sleep 1
done

# Roteamento: 5% do tráfego para canary
# (via load balancer, nginx, ou gateway interno)
echo "Canary active at $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> ~/Zylos/logs/deploy-audit.log
```

### Fase 3: Observação (10 min mínimo)
Monitorar métricas canary vs stable:
```bash
# Comparar error rates
curl -s http://localhost:9877/api/v1/metrics | jq .error_rate  # canary
curl -s http://localhost:9876/api/v1/metrics | jq .error_rate  # stable

# Comparar latência p99
curl -s http://localhost:9877/api/v1/metrics | jq .latency_p99
curl -s http://localhost:9876/api/v1/metrics | jq .latency_p99

# Verificar logs de erro
journalctl -u zylos-gateway-canary --since "10 min ago" | grep -i error
```

**Critérios de promoção:**
- Error rate canary ≤ stable + 0.1%
- Latência p99 canary ≤ stable × 1.1
- Zero panics/crashes nos logs
- Zero alerts do Sentinel

### Fase 4: Promoção Gradual
| Tempo | % Tráfego Canary | Ação |
|-------|-----------------|------|
| T+0 | 5% | Iniciar observação |
| T+10min | 25% | Se métricas OK |
| T+20min | 50% | Se métricas OK |
| T+25min | 100% | Promover canary → stable |
| T+30min | — | Desligar instância antiga |

### Fase 5: Rollback Automático

Se qualquer critério falhar durante observação:
```bash
# Parar canary imediatamente
pkill -f "zylos-gateway.*canary"

# Restaurar tráfego 100% para stable
# (load balancer já deve ter feito isso automaticamente)

# Log de rollback
echo "ROLLBACK at $(date -u +%Y-%m-%dT%H:%M:%SZ) reason: {motivo}" >> ~/Zylos/logs/deploy-audit.log

# Notificar via playbook de incident-response se SEV ≥ 2
```

## 4. Pós-Deploy

### Checklist de Validação Final
- [ ] Health check estável por ≥15 min
- [ ] Métricas dentro do baseline
- [ ] Cockpit carregando corretamente
- [ ] Agentes conectando normalmente
- [ ] Evidence Ledger registrando eventos
- [ ] Notificação de deploy enviada ao Slack #releases

### Documentação
```bash
# Registrar no Evidence Ledger
zylos-cli evidence log \
  --type deployment \
  --metadata '{"version":"v{X.Y.Z}","strategy":"canary","duration_min":30,"rollback":false}'

# Atualizar changelog
echo "## v{X.Y.Z} ($(date +%Y-%m-%d))" >> CHANGELOG.md
```

## 5. Janelas de Deploy

| Tipo | Janela | Aprovação Necessária |
|------|--------|---------------------|
| Hotfix SEV-1 | Qualquer horário | Tech Lead + SRE Lead |
| Release normal | Seg-Qui 09h-17h BRT | Tech Lead |
| Release major | Terça 10h-14h BRT | Tech Lead + Engineering Manager |
| Experimental | Sexta 14h-16h BRT | Owner do componente |

**Proibido:** Deploy sexta após 16h, fins de semana, feriados (exceto hotfix SEV-1).

## 6. Referências

- Playbook completo: [`deploy-release.md`](./deploy-release.md)
- Runbook canary: `runbooks/canary-deployment.md`
- Runbook backup: `runbooks/backup-restore.md`
- Gateway runbook: `runbooks/zylos-gateway.md`
- Logs de deploy: `~/Zylos/logs/deploy-audit.log`