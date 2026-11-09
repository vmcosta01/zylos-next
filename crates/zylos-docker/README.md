# zylos-docker

Isolamento sandbox baseado em Docker para agentes Zylos. Fornece containers por agente com limites de recursos, aprovação workflow para operações de risco e gerenciamento de ciclo de vida.

## Quick Start

```rust
use zylos_docker::{DockerSandbox, SandboxConfig, ApprovalWorkflow, RiskLevel};

let config = SandboxConfig {
    image: "alpine:3.19".into(),
    memory_limit: 256 * 1024 * 1024,
    network_enabled: false,
    ..Default::default()
};

let mut sandbox = DockerSandbox::new(config).await?;
sandbox.create().await?;
sandbox.start().await?;
let output = sandbox.exec(&["echo", "hello"]).await?;
sandbox.stop().await?;
sandbox.remove().await?;
```

## Types

| Tipo | Descrição |
|------|-----------|
| `DockerSandbox` | Container sandbox com lifecycle management (create/start/stop/remove/exec) |
| `SandboxConfig` | Configuração: imagem, limites de RAM/CPU, rede, mounts, workdir, env vars |
| `ContainerStatus` | Enum de estado: `Created`, `Running`, `Stopped`, `Removed`, `Error(String)` |
| `ApprovalWorkflow` | Workflow que gateia operações por nível de risco; configurável |
| `ApprovalDecision` | Resultado de avaliação: `Approved`, `Denied`, `Pending` |
| `RiskLevel` | Enum: `Low`, `Medium`, `High` — determina se aprovação é necessária |
| `DockerError` | Enum de erros (ver abaixo) |
| `Result<T>` | Alias para `std::result::Result<T, DockerError>` |

## Error Variants

| Variante | Causa |
|----------|-------|
| `DaemonUnavailable(msg)` | Docker daemon inacessível ou não rodando |
| `CreationFailed(msg)` | Configuração inválida ou falha ao puxar imagem |
| `NotFound(msg)` | Container não existe no daemon |
| `ApprovalDenied(msg)` | Operação bloqueada pelo `ApprovalWorkflow` |
| `IsolationViolation(msg)` | Tentativa de acesso a filesystem violou regras de sandbox |
| `ExecutionFailed(msg)` | Comando dentro do container falhou ou container não está running |
| `Serialization(serde_json::Error)` | Falha de serialização JSON |
| `Io(std::io::Error)` | Erro de I/O durante operações sandbox |
| `Bollard(bollard::errors::Error)` | Erro do cliente Docker Bollard |

## Gaps

- `ApprovalWorkflow::evaluate` retorna `Pending` como placeholder; integração real com policy engine/HITL não implementada
- Sem health check periódico do container
- Sem suporte a volumes read-write (apenas `readonly_mounts`)
- Sem logs streaming em tempo real
- `auto_approve_after` em `ApprovalWorkflow` é definido mas nunca avaliado temporalmente
