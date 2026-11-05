# Runbook: Monitoring & Observability — Métricas, Logs e Alertas

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Escopo:** Guia operacional para monitoramento, coleta de métricas, análise de logs e configuração de alertas do Zylos-Next
**Público-alvo:** Dev Pleno, Dev Junior, SRE, Tech Lead
**Gatilho:** Alerta disparado, degradação de performance detectada, investigação de incidente, configuração de novo dashboard, auditoria de observabilidade
**Playbook relacionado:** `security-incident-response.md` (correlação com eventos de segurança)

---

## 1. Visão Geral

O stack de observabilidade do Zylos-Next cobre três pilares:

- **Métricas:** Prometheus/Grafana para métricas de sistema e negócio
- **Logs:** Estruturados em JSON, agregados via Loki/Axiom
- **Traces:** OpenTelemetry para rastreamento distribuído de requests entre componentes

### Fontes de Dados

| Componente | Métricas | Logs | Traces |
|------------|----------|------|--------|
| zylos-daemon | `/metrics` endpoint | `~/.local/share/zylos/logs/daemon.log` | OTLP exporter |
| zylos-cockpit | Browser Performance API | Console errors (Sentry) | Não aplicável |
| zylos-live-comm | SSE connection count, event rate | Gateway logs | OTLP exporter |
| Sandbox (WASM) | Fuel consumption, memory usage | Audit trail | Span por skill execution |

---

## 2. Pré-requisitos

| Requisito | Uso | Acesso |
|-----------|-----|--------|
| Grafana | Dashboards e alertas | Todos (read-only), SRE (admin) |
| Prometheus/Loki/Axiom | Backend de métricas e logs | SRE |
| Sentry | Error tracking do cockpit | Devs |
| `zylos metrics` | Métricas locais do daemon | Devs |
| `jq` / `fx` | Parsing de logs JSON | Devs |

### Acessos Necessários

- **Grafana:** Login via SSO ou credenciais locais
- **Loki/Axiom:** API key para queries programáticas
- **Servidor de produção:** SSH ou kubectl para acessar logs in-loco se agregador falhar

---

## 3. Consultas Essenciais

### 3.1 Métricas de Saúde do Daemon

```promql
# Uptime do daemon (deve ser > 99.9%)
up{job="zylos-daemon"}

# Taxa de erros por segundo
rate(zylos_daemon_errors_total[5m])

# Agentes ativos
zylos_daemon_active_agents

# Latência p99 de spawn de agente
histogram_quantile(0.99, rate(zylos_agent_spawn_duration_seconds_bucket[5m]))

# Consumo de memória RSS
process_resident_memory_bytes{job="zylos-daemon"}
```

### 3.2 Queries de Logs (Loki/Axiom)

```logql
# Erros nas últimas 2 horas
{job="zylos-daemon"} |= "error" | json | line_format "{{.timestamp}} {{.level}} {{.message}}"

# Panics recentes
{job="zylos-daemon"} |= "panic" | json

# Skill executions lentas (>5s)
{job="zylos-daemon"} |= "skill_exec_complete" | json | duration_ms > 5000

# Sandbox violations
{job="zylos-daemon"} |= "sandbox_violation" | json
```

### 3.3 Métricas Locais via CLI

```bash
# Snapshot de métricas
zylos metrics --format json | jq '.active_agents, .uptime_seconds'

# Stream contínuo
zylos metrics --watch

# Exportar para arquivo (análise offline)
zylos metrics --format prometheus > metrics.prom
```

---

## 4. Alertas e Thresholds

### 4.1 Alertas Críticos (PagerDuty/Slack)

| Alerta | Condição | Severidade | Ação Imediata |
|--------|----------|------------|---------------|
| Daemon Down | `up == 0` por >2min | P0 | Runbook `zylos-daemon.md` §4.1 |
| Error Rate Alto | `rate(errors) > 0.1` por >5min | P1 | Investigar logs, considerar rollback |
| Memory Leak | `RSS > 2GB` por >10min | P1 | Restart graceful, coletar heap profile |
| Disk Full | `disk_usage > 90%` | P1 | Limpar logs antigos, expandir volume |
| SSE Connection Drop | `connections < baseline * 0.5` | P2 | Verificar gateway, rede, browser clients |

### 4.2 Alertas de Warning (Slack apenas)

| Alerta | Condição | Ação |
|--------|----------|------|
| Latência Degradada | p99 > 100ms por >15min | Investigar carga, profiling |
| Eval Timeout Rate > 5% | Judge LLM lento | Trocar provider ou aumentar timeout |
| Sandbox Fuel Exhaustion > 1% | Skills mal otimizadas | Review das skills afetadas |
| Log Volume Spike | > 10x baseline | Possível loop de erro, investigar |

---

## 5. Troubleshooting

### 5.1 Grafana dashboard vazio

**Causa:** Prometheus não está scrapando o target.

**Diagnóstico:**
```bash
# Verificar targets no Prometheus
curl http://prometheus:9090/api/v1/targets | jq '.data.activeTargets[] | select(.labels.job=="zylos-daemon")'

# Testar endpoint de métricas diretamente
curl http://zylos-daemon:9847/metrics
```

**Solução:** Verificar `prometheus.yml` → scrape_configs → job_name e relabel_configs.

### 5.2 Logs não aparecem no Loki

**Causa:** Promtail/Filebeat não está lendo o caminho correto.

**Diagnóstico:**
```bash
# Verificar posição do cursor
cat /var/lib/promtail/positions.yaml | grep zylos

# Testar leitura manual
tail -f ~/.local/share/zylos/logs/daemon.log | loki-logcli --stdin
```

**Solução:** Atualizar `promtail.yml` → scrape_configs → static_configs → targets.

### 5.3 Métricas inconsistentes entre fontes

**Causa:** Clock skew ou labels diferentes.

**Solução:**
```bash
# Verificar clock do servidor
date -u
timedatectl status

# Comparar labels
curl http://prometheus:9090/api/v1/series?match[]=zylos_daemon_active_agents | jq
```

### 5.4 Alerta falso positivo

**Diagnóstico:**
1. Verificar query no Grafana → Edit → Preview
2. Confirmar que condição persiste além do for duration
3. Checar se houve deploy recente que mudou métrica baseline

**Solução:** Ajustar threshold ou adicionar `absent()` clause para targets opcionais.

---

## 6. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| Grafana/Prometheus down | `zylos metrics` local + logs via `tail`/`grep` |
| Loki/Axiom indisponível | Logs diretos no filesystem + `jq` para parsing |
| Sentry fora do ar | Console do browser + source maps locais |
| OTLP collector down | Traces perdidos; focar em métricas e logs |
| Métricas corrompidas | Reset de counters via `zylos daemon restart` |

---

## 7. Critério de Validação Final

Após configurar ou corrigir observabilidade, TODOS devem passar:

- [ ] `curl http://zylos-daemon:9847/metrics` retorna métricas válidas no formato Prometheus
- [ ] Grafana dashboard "Zylos Overview" exibe dados atualizados (<1min delay)
- [ ] Query de log retorna resultados recentes no Loki/Axiom
- [ ] Alerta de teste dispara corretamente (via `amtool test-alert`)
- [ ] `zylos metrics` local concorda com métricas remotas (±5%)

Se algum falhar após 2 tentativas: escalar para SRE com evidências (screenshots, curl outputs).

---

## 8. Protocolo Pós-execução

1. Documentar nova query/dashboard no repositório (`docs/dashboards/`)
2. Se novo alerta criado: adicionar ao `alertmanager.yml` e testar
3. Se incidente resolvido via observabilidade: registrar lições no post-mortem
4. Revisar thresholds trimestralmente com base em dados reais
5. Simular tabletop de observabilidade mensalmente: escolher cenário e rastrear do alerta à causa raiz

---

## 9. Referências Cruzadas

- **Playbook:** `capacity-planning.md` (métricas para decisões de escala)
- **Playbook:** `security-incident-response.md` (logs de auditoria e traces forenses)
- **Runbook relacionado:** `zylos-daemon.md` (recuperação quando métricas indicam falha)
- **Runbook relacionado:** `deploy-cicd.md` (métricas pós-deploy)
- **HANDOFF relevante:** `HANDOFF_4.md` (Audit Trail da sandbox)
- **Configuração:** `prometheus.yml`, `loki.yml`, `grafana/dashboards/zylos.json`
- **ADR relevante:** ADR-035 (Observability Stack Selection)

---

**Próxima Revisão Agendada:** Dezembro de 2026
**Owner:** SRE Zylos-Next