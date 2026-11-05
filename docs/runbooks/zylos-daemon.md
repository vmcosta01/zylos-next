# Runbook: zylos-daemon — Operação e Troubleshooting

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Gatilho:** Daemon não responde, agentes não executam, logs mostram erro, ou health check falha
**Público-alvo:** Devs pleno e junior

---

## 1. Descrição e Contexto

O `zylos-daemon` é o processo background que gerencia o ciclo de vida dos agentes, orquestra skills, mantém o registry e expõe a API interna via socket Unix/TCP. Se o daemon falha, nenhum agente executa e o CLI perde funcionalidade.

Este runbook cobre diagnóstico, recuperação e validação para falhas do daemon em produção e desenvolvimento.

---

## 2. Pré-requisitos

- Acesso ao host onde o daemon roda
- Permissão de leitura/escrita em `~/.local/share/zylos/`
- Acesso aos logs: `~/.local/share/zylos/logs/daemon.log`
- Binário `zylos` no PATH (para comandos de gerenciamento)
- Conhecimento básico de systemd (se gerenciado como serviço)

---

## 3. Diagnóstico Rápido (Primeiros 60 segundos)

Execute na ordem. Pare quando identificar a causa raiz.

```bash
# 1. Verificar se o processo está vivo
pgrep -f zylos-daemon
# Se vazio: daemon não está rodando → vá para §4.1

# 2. Verificar status via CLI
zylos daemon status
# Esperado: "running (pid XXXX)"
# Se "unreachable": socket stale → vá para §4.2

# 3. Verificar saúde da API interna
curl -s --unix-socket ~/.local/share/zylos/daemon.sock http://localhost/health
# Esperado: {"status":"ok"}
# Se timeout ou erro: → vá para §4.3

# 4. Verificar erros recentes no log
tail -100 ~/.local/share/zylos/logs/daemon.log | grep -iE "error|panic|fatal|oom"
# Se panic/OOM: → vá para §4.4
```

---

## 4. Procedimentos de Recuperação

### 4.1 Daemon não está rodando

```bash
# Tentar start normal
zylos daemon start

# Se falhar com "address already in use":
lsof -i :9847  # porta TCP configurada
kill <PID_CONFLITANTE>
zylos daemon start

# Se falhar com "permission denied":
ls -la ~/.local/share/zylos/
chmod 700 ~/.local/share/zylos/
zylos daemon start

# Se gerenciado por systemd:
systemctl --user start zylos-daemon
systemctl --user status zylos-daemon
```

**Validação:** `zylos daemon status` retorna "running".

### 4.2 Socket stale / unreachable

```bash
# Remover socket antigo
rm -f ~/.local/share/zylos/daemon.sock

# Reiniciar daemon
zylos daemon restart

# Testar conectividade
curl -s --unix-socket ~/.local/share/zylos/daemon.sock http://localhost/health
```

**Validação:** Health check retorna `{"status":"ok"}` em <2s.

### 4.3 Health check falha mas processo existe

```bash
# Verificar uso de recursos
ps aux | grep zylos-daemon
free -h
df -h ~/.local/share/zylos/

# Se memória >90%: possível leak ou carga excessiva
# Se disco cheio: limpar logs antigos
find ~/.local/share/zylos/logs/ -name "*.log" -mtime +7 -delete

# Reiniciar graceful
zylos daemon restart

# Se persistir: coletar métricas e escalar
```

**Validação:** Health check retorna ok após restart.

### 4.4 Panic ou OOM no log

```bash
# Extrair stack trace completo
grep -B 5 -A 30 "panic\|OOM" ~/.local/share/zylos/logs/daemon.log | tail -50

# Identificar padrão:
# - "unwrap on None" → bug de código, abrir issue
# - "out of memory" → aumentar limites ou investigar leak
# - "broken pipe" → cliente desconectou, geralmente benigno

# Workaround imediato: restart
zylos daemon restart

# Se panic recorrente: habilitar core dumps
ulimit -c unlimited
echo "/tmp/core.%e.%p" | sudo tee /proc/sys/kernel/core_pattern
# Reproduzir e coletar core dump para análise
```

**Validação:** Issue criada com stack trace. Daemon estável após restart.

---

## 5. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| Daemon não recupera após 3 restarts | Coletar logs + métricas, escalar para dev senior |
| Socket corrompe recorrentemente | Migrar para TCP (`daemon.bind = "tcp://127.0.0.1:9847"` no config) |
| OOM frequente | Aumentar limite de memória ou reduzir `max_concurrent_agents` |
| Versão nova instável | Rollback: `cp ~/.local/bin/zylos.bak ~/.local/bin/zylos && zylos daemon restart` |
| Logs crescem descontroladamente | Configurar rotação: `logrotate` ou `daemon.log_max_size` no config |

---

## 6. Critério de Validação Final

Após qualquer procedimento de recuperação, TODOS devem passar:

```bash
pgrep -f zylos-daemon                          # ✅ PID existe
zylos daemon status                            # ✅ "running"
curl -s --unix-socket ~/.local/share/zylos/daemon.sock http://localhost/health  # ✅ {"status":"ok"}
zylos agent list                               # ✅ Responde em <3s
tail -20 ~/.local/share/zylos/logs/daemon.log  # ✅ Sem novos errors/panics
```

Se algum falhar após 2 tentativas: escalar com logs completos e métricas de sistema.

---

## 7. Protocolo Pós-execução

1. Registrar incidente em `~/.local/share/zylos/logs/ops.log`
2. Se bug novo: criar issue com tag `daemon-bug` + stack trace
3. Se configuração: atualizar este runbook e `config.toml.example`
4. Simular tabletop mensalmente: escolher cenário aleatório e executar do zero
5. Revisar limites de recursos trimestralmente com base em métricas reais

---

## 8. Referências Cruzadas

- **Runbook relacionado:** `zylos-cli.md` (interface primária)
- **Playbook relacionado:** `security-incident-response.md` (se breach suspeito)
- **Configuração:** `~/.config/zylos/config.toml` → seção `[daemon]`
- **Arquitetura:** `docs/architecture/phase3-agent-orchestration.md`
- **HANDOFF relevante:** `HANDOFF_4.md` (Daemon Lifecycle)