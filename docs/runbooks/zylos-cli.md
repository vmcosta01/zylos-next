# Runbook: zylos-cli — Operação e Troubleshooting

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Autor:** Hermes Agent (default profile)
**Gatilho:** Usuário reporta erro no CLI, comando não responde, ou output inesperado
**Público-alvo:** Devs pleno e junior

---

## 1. Descrição e Contexto

O `zylos-cli` é a interface primária do usuário com o sistema Zylos. Ele orquestra agentes, gerencia skills, exibe status do daemon e opera o cockpit. Qualquer falha aqui bloqueia o usuário final imediatamente.

Este runbook cobre diagnóstico, recuperação e validação para os problemas mais comuns em produção e desenvolvimento.

---

## 2. Pré-requisitos

- Binário `zylos` acessível no `$PATH` (ou caminho conhecido do build)
- Permissão de leitura em `~/.config/zylos/` e `~/.local/share/zylos/`
- Daemon rodando (verificar com `zylos daemon status`)
- Acesso ao log do daemon: `~/.local/share/zylos/logs/daemon.log`
- Rust toolchain instalada (para rebuild local se necessário)

---

## 3. Diagnóstico Rápido (Primeiros 60 segundos)

Execute na ordem. Pare quando identificar a causa raiz.

```bash
# 1. Verificar se o binário responde
zylos --version
# Esperado: "zylos-cli X.Y.Z"
# Se falhar: binário corrompido ou PATH errado → vá para §4.1

# 2. Verificar status do daemon
zylos daemon status
# Esperado: "running (pid XXXX)"
# Se "not running": → vá para §4.2
# Se "unreachable": socket morto → vá para §4.3

# 3. Verificar último erro no log
tail -50 ~/.local/share/zylos/logs/daemon.log | grep -i "error\|panic\|fatal"
# Se houver panic: → vá para §4.4
# Se houver timeout: → vá para §4.5

# 4. Testar comando simples
zylos agent list
# Esperado: lista JSON de agentes
# Se hang >5s: deadlock ou socket travado → vá para §4.3
```

---

## 4. Procedimentos de Recuperação

### 4.1 Binário não encontrado ou corrompido

```bash
# Verificar PATH
which zylos
# Se vazio: adicionar ao PATH ou usar caminho completo

# Rebuild local
cd ~/zylos-next && cargo build --release --bin zylos
cp target/release/zylos ~/.local/bin/zylos

# Validar
zylos --version
```

**Validação:** `zylos --version` retorna versão sem erro.

### 4.2 Daemon não está rodando

```bash
# Tentar start normal
zylos daemon start

# Se falhar com "address already in use":
lsof -i :9847  # ou porta configurada
kill <PID_CONFLITANTE>
zylos daemon start

# Se falhar com "permission denied":
ls -la ~/.local/share/zylos/
chmod 700 ~/.local/share/zylos/
zylos daemon start
```

**Validação:** `zylos daemon status` retorna "running".

### 4.3 Socket travado / unreachable

```bash
# Remover socket stale
rm -f ~/.local/share/zylos/daemon.sock

# Reiniciar daemon
zylos daemon restart

# Testar conectividade
zylos agent list
```

**Validação:** `zylos agent list` retorna JSON dentro de 3s.

### 4.4 Panic no daemon log

```bash
# Extrair stack trace completo
grep -A 30 "panic" ~/.local/share/zylos/logs/daemon.log | tail -40

# Identificar crate/função causadora
# Exemplo: "panicked at 'unwrap on None' in skill_registry.rs:142"

# Abrir issue com stack trace + passos de reprodução
# NÃO tentar hotfix em produção sem review
```

**Validação:** Issue criada com stack trace completo. Daemon reiniciado como workaround temporário.

### 4.5 Timeout em comandos

```bash
# Verificar carga do sistema
top -bn1 | head -20
df -h ~/.local/share/zylos/

# Se disco cheio (>90%):
du -sh ~/.local/share/zylos/logs/*
# Limpar logs antigos (manter últimos 7 dias)
find ~/.local/share/zylos/logs/ -name "*.log" -mtime +7 -delete

# Se CPU saturada: identificar processo e avaliar kill/restart
```

**Validação:** `zylos agent list` responde em <3s após limpeza.

---

## 5. Caminhos Alternativos e Contingência

| Cenário | Ação Alternativa |
|---------|-----------------|
| CLI completamente inoperante | Usar API REST direta via curl (`curl localhost:9847/api/v1/agents`) |
| Daemon não recupera após restart | Coletar logs + core dump (se disponível) e escalar para dev senior |
| Comando específico falha mas outros funcionam | Isolar com `zylos <cmd> --verbose` e abrir issue com reprodução mínima |
| Versão nova introduziu regressão | Rollback: `cp ~/.local/bin/zylos.bak ~/.local/bin/zylos` (se backup existir) |

---

## 6. Critério de Validação Final

Após qualquer procedimento de recuperação, TODOS devem passar:

```bash
zylos --version                    # ✅ Retorna versão
zylos daemon status                # ✅ "running"
zylos agent list                   # ✅ JSON válido em <3s
zylos skill eval --dry-run test    # ✅ Sem panic/error
```

Se algum falhar após 2 tentativas: escalar para dev senior com logs completos.

---

## 7. Protocolo Pós-execução

1. Registrar incidente no log de operações (`~/.local/share/zylos/logs/ops.log`)
2. Se foi bug novo: criar issue no repositório com tag `cli-bug`
3. Se foi configuração: atualizar este runbook com novo cenário
4. Simular tabletop trimestralmente: escolher 2 cenários aleatórios e executar do zero

---

## 8. Referências Cruzadas

- **Playbook relacionado:** Incident Response (`docs/playbooks/security-incident-response.md`)
- **Arquitetura:** `docs/architecture/phase3-agent-orchestration.md`
- **Configuração:** `~/.config/zylos/config.toml`
- **HANDOFF relevante:** `HANDOFF_3.md` (Skills Registry)