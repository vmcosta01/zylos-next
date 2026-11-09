# zylos-desktop

**Fase:** 2.1 - Desktop Shell (Tauri v2 + SolidJS)
**Status:** ✅ Implementado (302 LOC Rust + SolidJS frontend)

## Descricao

Aplicacao desktop nativa construida com Tauri v2 (Rust backend) e SolidJS (TypeScript frontend). Embedda o `zylos-gateway` como processo filho, gerencia system tray, deep links, onboarding wizard e IPC bridge.

## Arquitetura

```
zylos-desktop/
├── src-tauri/
│   ├── src/main.rs          # Tauri app setup, IPC commands (341 LOC)
│   ├── Cargo.toml           # Tauri v2 + zylos-desktop crate dep
│   └── tauri.conf.json      # Window config, bundler, permissions
├── src/
│   ├── App.tsx              # Root SolidJS component
│   ├── index.tsx            # Entry point
│   ├── ipc.ts               # Tauri invoke wrappers
│   └── components/
│       ├── ChatPanel.tsx    # Agent chat interface
│       ├── OnboardingWizard.tsx  # First-run setup
│       └── StatusBar.tsx    # Gateway status + agent count
├── package.json             # SolidJS + Tauri deps
└── vite.config.ts           # Vite bundler config
```

## Features

- ✅ Embedded gateway lifecycle (auto-start/stop)
- ✅ System tray com menu contextual
- ✅ Deep links: `zylos://chat/{bot_id}`, `zylos://approve/{request_id}`
- ✅ Onboarding wizard (first-run)
- ✅ IPC bridge (10 comandos)
- ✅ Auto-update check
- ✅ Cross-platform: macOS (.dmg), Windows (.msi), Linux (.AppImage/.deb)

## IPC Commands

| Comando | Descricao |
|---------|-----------|
| `get_status` | Gateway health + agent count |
| `send_prompt` | Dispatch prompt to agent |
| `list_skills` | Fetch registered skills |
| `list_playbooks` | Fetch available playbooks |
| `execute_action` | Run provider action |
| `open_external` | Open URL in browser |
| `minimize_to_tray` | Hide to system tray |
| `check_update` | Check for app updates |
| `is_first_run` | Check onboarding status |
| `complete_onboarding` | Save gateway URL + mark done |

## Build

```bash
# Dev mode (hot reload)
npm run tauri dev

# Production build
npm run tauri build

# Apenas frontend (sem Rust)
npm run dev
```

## Configuracao

Localizacao: `~/Zylos/desktop-config.json`

```json
{
  "gateway_url": "http://localhost:3000",
  "agents_root": "~/Zylos/agents"
}
```

## Deep Links

- `zylos://chat/{bot_id}` - Abre chat com agente especifico
- `zylos://approve/{request_id}` - Abre tela de aprovacao HITL

## Dependencias

### Rust (Tauri backend)
- `tauri` 2.0 (window management, IPC)
- `tauri-plugin-shell` 2.0 (process spawning)
- `reqwest` 0.12 (HTTP client to gateway)
- `tokio` 1.40 (async runtime)
- `zylos-desktop` crate (tipos compartilhados)

### TypeScript (SolidJS frontend)
- `solid-js` 1.9 (reactive UI)
- `@tauri-apps/api` 2.0 (IPC client)
- `vite` 6.0 (bundler)