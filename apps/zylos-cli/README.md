# zylos-cli

**Fase:** 7 - CLI Admin
**Status:** ✅ Implementado (468 LOC, 1 arquivo Rust, 10 testes)

## Descricao

Interface de linha de comando para administracao do sistema Zylos. Permite gerenciar agentes, submeter tarefas, consultar status, visualizar logs e disparar workflows SOAR diretamente do terminal via API REST do `zylos-gateway`.

## Quick Start

```bash
# Verifica saude do gateway e agentes
zylos status

# Lista agentes ativos
zylos agent list

# Submete tarefa para execucao
zylos task submit spec.yaml

# Lista playbooks SOAR disponiveis
zylos soar list-playbooks

# Dispara evento SOAR
zylos soar submit-event incident.json

# Exibe configuracao ativa
zylos config show

# Ultimas 50 linhas de log
zylos logs --tail 50
```

## Comandos

| Comando | Descricao |
|---------|-----------|
| `status` | Verifica saude do gateway e agentes |
| `agent list` | Lista agentes ativos |
| `agent start <id>` | Inicia agente especifico |
| `agent stop <id>` | Para agente gracefulmente |
| `agent inspect <id>` | Inspeciona detalhes do agente |
| `task submit <spec>` | Submete tarefa para execucao (YAML/JSON) |
| `task status <id>` | Consulta status de tarefa |
| `task cancel <id>` | Cancela tarefa pendente ou em execucao |
| `soar list-playbooks` | Lista playbooks SOAR disponiveis |
| `soar submit-event <path>` | Submete evento SOAR (JSON/YAML) |
| `config show` | Exibe configuracao ativa |
| `config validate <path>` | Valida arquivo de configuracao sem aplicar |
| `logs [--tail N]` | Stream ou tail de logs (padrao: 20 linhas) |

## Configuracao

O CLI se comunica com o `zylos-gateway` via REST API. Configure a URL do gateway via:

- **Flag CLI:** `--gateway_url http://gw:8080`
- **Variavel de ambiente:** `ZYLOS_GATEWAY_URL`
- **Padrao:** `http://localhost:3000`

Formato de output controlado com `--format json` ou `--format table` (padrao: `table`).

## Arquitetura

```
zylos-cli/
├── src/
│   └── main.rs          # Entry point com clap derive, 8 comandos implementados
└── Cargo.toml
```

## Dependencias

- `clap` 4.6 (CLI argument parsing)
- `reqwest` 0.12 (HTTP client para gateway)
- `tokio` 1.40 (async runtime)
- `serde` + `serde_json` (serializacao)
- `serde_yaml` 0.9 (parsing de specs YAML)
- `tracing` + `tracing-subscriber` (logging)
- `anyhow` (error handling)
- `toml` 0.8 (validacao de config TOML)

## Testes

8 testes unitarios cobrindo:
- Parsing de CLI com argumentos padrao e customizados
- Validacao de configuracao TOML/JSON/YAML
- Comandos de agente e task

## Runbook

- [`zylos-cli.runbook.yaml`](../../docs/soar/runbooks/zylos-cli.runbook.yaml)
- [`zylos-cli.md`](../../runbooks/zylos-cli.md)
