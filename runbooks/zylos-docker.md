# Runbook — zylos-docker (Sandbox Docker com Approval Workflow)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `crates/zylos-docker`
**Dependências:** `bollard`, `serde`, `tokio`
**Fase:** 4 (SANDBOX_ISOLATION) → 7 (END_TO_END_INTEGRATION)

---

## 1. Visão Geral

O `zylos-docker` gerencia containers Docker isolados por agente, com workflow de aprovação antes da criação e limpeza automática. É a camada de sandbox para código não-confiável que requer acesso a filesystem ou rede controlada.

**Módulos:**
- `container.rs` — Criação, start, stop, logs, exec em containers
- `approval.rs` — Workflow de aprovação (allow/deny/timeout) antes de criar container
- `error.rs` — Tipos de erro unificados
- `lib.rs` — Re-exports públicos

**Integração:** Usado pelo `zylos-sandbox` como backend alternativo ao WASM quando o agente precisa de syscalls reais (filesystem, rede, processos).

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que Docker além de WASM?
WASM isola computação pura, mas não cobre:
- Acesso a filesystem real (uploads, downloads, geração de arquivos)
- Networking controlado (webhooks, APIs externas)
- Processos filho (compiladores, interpretadores)
Docker fornece isolamento de kernel + cgroups + namespaces para esses casos.

### 2.2 Approval Workflow
Todo container passa por aprovação explícita ANTES da criação:
```
Request → ApprovalCheck → [ALLOW|DENY|TIMEOUT] → Create/Drop
```
- **ALLOW:** Container criado imediatamente
- **DENY:** Request rejeitada, log de auditoria gerado
- **TIMEOUT:** Request expira após TTL configurável (default 30s), tratada como DENY

Isso previne criação acidental de containers por agentes comprometidos ou prompts maliciosos.

### 2.3 Segurança
- Containers rodam com `--rm` (auto-cleanup on stop)
- Network mode configurável (`bridge`, `none`, custom)
- Sem privilégios elevados (`--privileged=false` default)
- Resource limits via cgroups (CPU, memory)
- Logs capturados para auditoria pós-execução

### 2.4 Trade-offs vs WASM
| Aspecto | WASM | Docker |
|---------|------|--------|
| Cold start | <100ms | 500ms-2s |
| Isolamento | Memory-safe, no syscalls | Kernel-level, syscalls permitidas |
| Overhead | ~1MB RAM | ~50-200MB RAM |
| Use case | Computação pura | I/O, networking, processos |

**Regra:** Preferir WASM sempre que possível. Docker apenas quando WASM não cobre o requisito.

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-docker
# Testes unitários não requerem Docker daemon rodando
# Testes de integração requerem: docker info
```

### 3.2 API pública
```rust
use zylos_docker::{ContainerManager, ApprovalPolicy};

// Criar manager com policy padrão
let manager = ContainerManager::new(ApprovalPolicy::AutoAllow);

// Solicitar criação (passa por approval)
let container_id = manager.create("agent-42", "python:3.12-slim").await?;

// Executar comando
let output = manager.exec(&container_id, ["python", "-c", "print('hello')"]).await?;

// Parar e limpar
manager.stop(&container_id).await?;
```

### 3.3 Approval Policies
- `AutoAllow` — Aprova automaticamente (dev/staging apenas)
- `ManualConfirm` — Requer confirmação humana via CLI/dashboard
- `Timeout(u64)` — Auto-deny após N segundos sem resposta
- `DenyAll` — Bloqueia tudo (maintenance mode)

### 3.4 Checklist operacional
- [ ] Docker daemon rodando (`docker info` retorna OK)
- [ ] Imagens base disponíveis localmente (`docker images`)
- [ ] Approval policy configurada conforme ambiente
- [ ] Resource limits definidos para evitar runaway containers
- [ ] Logs sendo coletados para auditoria

### 3.5 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| `Connection refused` | Docker daemon parado | `systemctl start docker` ou Docker Desktop |
| `Permission denied` | Usuário sem acesso ao socket | Adicionar usuário ao grupo `docker` |
| `Image not found` | Imagem não puxada | `docker pull <image>` |
| `Approval timeout` | Policy ManualConfirm sem resposta | Verificar dashboard/CLI ou mudar policy |
| Container órfão | Stop falhou | `docker rm -f <container_id>` |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-docker
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes unitários
```bash
cargo test -p zylos-docker --lib
```
**Esperado:** Todos passam (testes unitários mockam Docker).

### 4.3 Verificar Docker disponível
```bash
docker info
```
**Esperado:** Output com Server Version, Storage Driver, etc. Se falhar, Docker não está rodando.

### 4.4 Quando escalar
- Erro de compilação → verificar dependências no Cargo.toml (`bollard`, `serde`)
- Teste de integração falha → verificar se Docker daemon está acessível
- Approval timeout → verificar policy configurada ou dashboard de aprovação
- Container não para → usar `docker rm -f` manualmente e reportar bug

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-docker/src/`
- **Runbook Fase 4:** `runbooks/fase4-sandbox-isolation.md`
- **Security model:** `specs/SECURITY-MODEL.md` §Sandbox Boundaries
- **HANDOFF_4:** `HANDOFF_4.md` (quando criado)
- **WASM alternative:** `crates/zylos-sandbox/src/wasm_sandbox.rs`