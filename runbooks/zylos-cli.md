# Runbook: Zylos CLI — Operação e Troubleshooting

**Versão:** 1.0
**Data:** 2026-11-05
**Autor:** Hermes Agent
**Gatilho:** Usuário reporta erro na CLI, comando falha, ou necessidade de manutenção
**Público-alvo:** Support / DevOps / End User
**Pré-requisitos:** Zylos CLI instalada (`zylos --version`), acesso ao gateway

---

## 1. Comandos Principais

| Comando | Descrição | Output Esperado |
|---------|-----------|-----------------|
| `zylos status` | Verifica saúde do gateway e agentes | JSON com status de cada componente |
| `zylos agent list` | Lista agentes ativos | Tabela com ID, nome, status, uptime |
| `zylos agent start <id>` | Inicia agente específico | Confirmação + PID |
| `zylos agent stop <id>` | Para agente gracefulmente | Confirmação de shutdown |
| `zylos task submit <spec>` | Submete tarefa para execução | Task ID + queue position |
| `zylos task status <id>` | Consulta status de tarefa | Estado atual + progresso |
| `zylos config show` | Exibe configuração ativa | YAML formatado |
| `zylos logs --tail N` | Últimas N linhas de log | Output stream |

## 2. Troubleshooting

### 2.1 CLI não responde / timeout
```bash
# Verificar se gateway está acessível
zylos status --timeout 5

# Se falhar, verificar conexão
curl -s http://localhost:8080/health || echo "Gateway inacessível"

# Restartar daemon local
zylos daemon restart
```

### 2.2 Autenticação falha
```bash
# Verificar token
zylos auth status

# Renovar token
zylos auth login --gateway-url https://gateway.example.com

# Verificar variáveis de ambiente
env | grep ZYLOS_
```

### 2.3 Agente não inicia
```bash
# Logs detalhados
zylos agent logs <agent-id> --level debug

# Verificar recursos
zylos agent inspect <agent-id> --resources

# Validar spec
zylos agent validate <spec-file.yaml>
```

### 2.4 Tarefa presa em pending
```bash
# Verificar fila
zylos queue status

# Forçar requeue
zylos task requeue <task-id>

# Cancelar e re-submeter
zylos task cancel <task-id>
zylos task submit <spec-file.yaml>
```

## 3. Configuração

Arquivo: `~/.config/zylos/config.yaml`

```yaml
gateway:
  url: https://gateway.example.com
  timeout: 30s
auth:
  token_path: ~/.config/zylos/token.json
logging:
  level: info
  format: json
```

Override via env: `ZYLOS_GATEWAY_URL`, `ZYLOS_LOG_LEVEL`.

## 4. Validação Final

- [ ] `zylos status` retorna healthy
- [ ] `zylos agent list` mostra agentes esperados
- [ ] `zylos task submit` aceita spec válida
- [ ] Logs acessíveis via `zylos logs`

## 5. Referências

- Playbook: `playbooks/deploy-release.md` (para updates de CLI)
- SOAR: `docs/soar/runbooks/zylos-cli.runbook.yaml`
- Docs: `docs/IMPLEMENTATION_PLAN.md` §Fase 7
