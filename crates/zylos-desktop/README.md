# zylos-desktop

Shell desktop Tauri v2 para zylos-gateway. Fornece janela nativa com webview embutido (cockpit UI), system tray com indicador de status, deep links (`zylos://`), auto-update e bridge IPC entre frontend e backend gateway.

## Quick Start

```rust
use zylos_desktop::{DesktopConfig, IpcRequest, DeepLink};

let config = DesktopConfig::load(std::path::Path::new("desktop-config.json"))
    .unwrap_or_default();

let link = DeepLink::parse("zylos://open_thread?thread_id=abc-123")?;
let req = IpcRequest::SendPrompt {
    message: "analyze logs".into(),
    thread_id: Some("t1".into()),
};
let json = serde_json::to_string(&req)?;
```

## Types

| Tipo | Descrição |
|------|-----------|
| `DesktopConfig` | Configuração da janela, gateway URL, auto-update, deep link scheme, asset paths |
| `IpcRequest` | Mensagens frontend → backend (status, prompt, skills, playbooks, actions) |
| `IpcResponse` | Respostas backend → frontend (status, ack, results, updates, errors) |
| `SkillSummary` | Resumo de skill: nome, descrição, categoria |
| `PlaybookSummary` | Resumo de playbook: id, nome, descrição, número de steps |
| `DeepLink` | Links `zylos://` parseados: `OpenThread`, `RunSkill`, `Settings`, `OAuthCallback` |
| `TrayAction` | Ações do menu tray: `ShowWindow`, `HideWindow`, `OpenDashboard`, `RestartGateway`, `CheckUpdates`, `Quit` |
| `GatewayHandle` | Handle para processo gateway embarcado (PID, uptime, stop) |
| `GatewayManager` | Gerenciador thread-safe do gateway embarcado (start/stop/status/shutdown) |
| `GatewayStatus` | Status do gateway: running, pid, uptime_secs |
| `Notification` | Notificação OS-native com urgência e action button |
| `Urgency` | Nível de urgência: `Info`, `Warning`, `Critical` |
| `NotificationManager` | Gerenciador de notificações com histórico bounded |
| `TrayStatus` | Status do tray: `Healthy`, `Warning`, `Error`, `Unknown` |
| `TrayManager` | Gerenciador thread-safe do tray (status, visibilidade, ações) |
| `Workspace` | Metadados de workspace de bot: id, nome, path, timestamps |
| `WorkspaceManager` | Gerenciador de workspaces por bot (create/list/get/delete/touch) |

## Error Variants

### ConfigError

| Variante | Causa |
|----------|-------|
| `Io(std::io::Error)` | Falha de E/S ao ler/escrever arquivo de config |
| `Parse(serde_json::Error)` | JSON inválido no arquivo de config |
| `Serialize(serde_json::Error)` | Falha ao serializar config para JSON |

### DeepLinkError

| Variante | Causa |
|----------|-------|
| `InvalidScheme` | URL não começa com `zylos://` |
| `MissingParam(name)` | Parâmetro obrigatório ausente na query string |
| `UnknownAction(action)` | Ação de deep link não reconhecida |

### GatewayError

| Variante | Causa |
|----------|-------|
| `BinaryNotFound(path)` | Binário zylos-gateway não encontrado |
| `AlreadyRunning(pid)` | Gateway já está rodando |
| `StartFailed(msg)` | Falha ao spawnar processo gateway |
| `StopFailed(msg)` | Falha ao parar gateway (graceful ou force kill) |
| `NotRunning` | Tentativa de parar gateway que não está rodando |
| `Io(std::io::Error)` | Erro de I/O |

### WorkspaceError

| Variante | Causa |
|----------|-------|
| `Io(std::io::Error)` | Erro de E/S em operações de filesystem |
| `InvalidBotId(msg)` | Bot ID contém caracteres inválidos ou tamanho fora do range |
| `AlreadyExists(id)` | Workspace já existe para este bot_id |
| `NotFound(id)` | Workspace não encontrado |
| `Json(serde_json::Error)` | Falha ao ler/escrever workspace.json |

## Feature Flags

| Feature | Efeito |
|---------|--------|
| `gui` | Habilita dependências Tauri e `gui::build_app()`. Sem ela, apenas tipos de config e protocolo IPC compilam (CI headless). |

## Fase 8: Desktop UX & Mobile Relay (2026-11-11)

### WorkspacePersistence (T8.6.1)

Persistência de estado do cockpit entre sessões via arquivo JSON local (`~/.zylos/workspace-state.json`).

- Save automático com debounce (300ms) após última mudança
- Load síncrono na inicialização; tolerante a corrupção (fallback para defaults)
- Multi-tab: cada aba tem namespace próprio de estado
- Schema versioning para migração futura

```rust
use zylos_desktop::workspace_persistence::WorkspacePersistence;

let wp = WorkspacePersistence::default_path();
let state = wp.load().await; // Carrega ou retorna default
wp.update_and_save(|s| { s.layout.theme = "dark".into(); }).await;
```

### AgentAwarenessState (T8.6.2)

Enum de estados visuais de agentes para UI em tempo real.

| Estado | Ícone | Cor CSS | Requer Atenção? |
|--------|-------|---------|-----------------|
| Idle | 💤 | `--color-muted` | Não |
| Thinking | 🤔 | `--color-warning` | Não |
| Executing | ⚡ | `--color-accent` | Não |
| WaitingApproval | ✋ | `--color-info` | **Sim** |
| Error | ❌ | `--color-error` | **Sim** |
| Completed | ✅ | `--color-success` | Não |

Inclui `AgentAwarenessTracker` com histórico ring buffer e estatísticas por estado.

### RelayServer (T8.6.3)

HTTP/WebSocket relay para comunicação mobile-to-desktop sobre LAN.

- Token-based auth com TTL configurável (default 24h)
- Rate limiting por IP (60 req/min default)
- CORS headers para clientes browser mobile
- Purge automático de tokens expirados

```bash
# Pair dispositivo
curl -X POST http://localhost:9877/api/v1/relay/pair \
  -d '{"device_name": "iPhone 15"}'
```

### MobileResponsive (T8.6.4)

Breakpoint definitions e CSS responsivo para cockpit WASM.

| Breakpoint | Largura | Layout |
|------------|---------|--------|
| Mobile | < 640px | Single column, sidebar hidden |
| Tablet | 640–1023px | 2-column adaptive |
| Desktop | ≥ 1024px | Full sidebar + multi-panel |

Gera CSS custom properties e media queries automaticamente. Suporta safe area insets para dispositivos com notch.

### Hermes Integration

Endpoints dedicados via bridge Hermes (`hermes_desktop_api`):
- `/api/v1/hermes/desktop/workspace/*` — state, save, reset
- `/api/v1/hermes/desktop/awareness/*` — states, attention, update
- `/api/v1/hermes/desktop/relay/*` — devices, pair, revoke

Cross-reference: [HANDOFF_FASE8.md](../../docs/handoffs/HANDOFF_FASE8.md) §2.8, [Runbook zylos-desktop-fase8](../../runbooks/zylos-desktop-fase8.md)

## Gaps

- `gui::build_app` não registra plugins Tauri (TODO no código)
- `NotificationManager::send` não integra com plugin Tauri notification (stub log-only)
- `TrayManager` não cria tray icon real (gerencia estado apenas)
- `GatewayManager` não suporta restart (apenas stop + start manual)
- `WorkspaceManager` não valida unicidade de nome (apenas bot_id)
- Sem validação de deep link params além de presença/ausência
