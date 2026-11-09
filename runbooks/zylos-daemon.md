# Runbook: Zylos Daemon — Diagnóstico e Recuperação

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Daemon não inicia, crash recorrente, alta utilização de recursos, ou agentes órfãos
**Público-alvo:** SRE / DevOps / Platform Engineer
**Pré-requisitos:** Acesso ao host, systemd/supervisor, logs do daemon

---

## 1. Arquitetura

O daemon é o processo background responsável por:
- Gerenciar ciclo de vida dos agentes locais
- Manter conexão persistente com o gateway (WebSocket/gRPC)
- Executar heartbeat e health checks
- Bufferizar tarefas quando gateway está offline
- Coletar métricas e telemetry

## 2. Verificação de Saúde

```bash
# Status do serviço
systemctl status zylos-daemon

# PID e uptime
pgrep -a zylos-daemon

# Health endpoint local
curl -s http://localhost:9090/health | jq .

# Métricas
curl -s http://localhost:9090/metrics | head -20
```

## 3. Troubleshooting

### 3.1 Daemon não inicia
```bash
# Logs completos
journalctl -u zylos-daemon --no-pager -n 100

# Verificar porta em uso
ss -tlnp | grep 9090

# Verificar permissões
ls -la /var/lib/zylos/
ls -la /var/log/zylos/

# Testar config
zylos-daemon --config /etc/zylos/daemon.yaml --dry-run
```

### 3.2 Crash recorrente (restart loop)
```bash
# Contar restarts nas últimas 2h
journalctl -u zylos-daemon --since "2 hours ago" | grep -c "Started"

# Core dump analysis
coredumpctl list zylos-daemon
coredumpctl dump zylos-daemon | gdb /usr/bin/zylos-daemon

# Memory leak check
valgrind --leak-check=full zylos-daemon --foreground
```

### 3.3 Alta utilização de CPU/Memória
```bash
# Top processos
top -p $(pgrep zylos-daemon)

# Profile heap
curl -s http://localhost:9090/debug/pprof/heap > heap.prof
go tool pprof heap.prof

# Verificar agentes órfãos
zylos-daemon inspect --orphans

# Limpar cache
zylos-daemon cache prune --older-than 24h
```

### 3.4 Gateway desconectado
```bash
# Status da conexão
zylos-daemon connection status

# Forçar reconexão
zylos-daemon connection reconnect

# Verificar DNS/TLS
dig gateway.example.com
openssl s_client -connect gateway.example.com:443 </dev/null

# Modo offline (buffer local)
zylos-daemon mode offline
zylos-daemon queue flush  # quando gateway voltar
```

### 3.5 Agentes órfãos
```bash
# Listar órfãos
zylos-daemon agents --orphaned

# Adotar órfãos
zylos-daemon agents adopt --all

# Terminar órfãos
zylos-daemon agents kill --orphaned --force
```

## 4. Manutenção

### Rotação de logs
```bash
# Manual
logrotate -f /etc/logrotate.d/zylos-daemon

# Verificar tamanho
du -sh /var/log/zylos/*
```

### Update sem downtime
```bash
# Graceful reload
systemctl reload zylos-daemon

# Ou hot-swap binary
zylos-daemon upgrade --channel stable
```

## 5. Validação Final

- [ ] `systemctl status zylos-daemon` → active (running)
- [ ] Health endpoint retorna 200
- [ ] Conexão com gateway estabelecida
- [ ] Zero agentes órfãos
- [ ] Métricas sendo exportadas

## 6. Referências

- Playbook: `playbooks/incident-response.md` (para SEV-1/SEV-2)
- SOAR: `docs/soar/runbooks/zylos-daemon.runbook.yaml`
- Monitoring: `runbooks/monitoring-observability.md`
