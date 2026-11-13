# zylos-computer-view

> Computer View Panel para Zylos Cockpit v2 — gerenciamento de sessões de visualização remota de desktop do agente com controle humano.

## Purpose

Permitir que o usuário visualize e controle o desktop/browser do agente em tempo real via VNC/WebSocket proxy, com suporte a take/release control com lease temporário e fallback para screenshots estáticos quando VNC indisponível. Inspirado em OpenMausBot (`CanvasComputers.tsx`), OpenDots (`ComputerPanel.tsx`).

## Architecture

```
┌─────────────────┐ ┌──────────────────┐ ┌─────────────┐
│ Cockpit Panel │────▶│ SessionManager │────▶│ DashMap │
│ /api/v1/computer│ │ (in-memory) │ │ (concurrent) │
└─────────────────┘ └──────────────────┘ └─────────────┘
 ▲
 ┌─────────┼─────────┐
 ▼ ▼ ▼
 VNC Proxy Screenshot Lease Mgmt
```

### Módulos

| Módulo | Responsabilidade |
|--------|-----------------|
| `schema` | Tipos: `ComputerSession`, `SessionState`, `ControlLease`, `ScreenshotData`, requests/responses |
| `session` | `ComputerSessionManager`: CRUD de sessões, leases com timeout, cleanup, screenshot placeholder |

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/computer/sessions` | Lista sessões ativas |
| POST | `/api/v1/computer/sessions` | Cria nova sessão de computer view |
| POST | `/api/v1/computer/sessions/:id/take-control` | Usuário assume controle (com lease) |
| POST | `/api/v1/computer/sessions/:id/release-control` | Usuário devolve controle ao agente |
| DELETE | `/api/v1/computer/sessions/:id` | Encerra sessão |
| GET | `/api/v1/computer/sessions/:id/screenshot` | Screenshot estático (fallback) |

## Session States

| Estado | Descrição | Ativo? |
|--------|-----------|--------|
| `connecting` | Sessão criada, aguardando conexão VNC | Sim |
| `active` | Stream ativo, agente operando | Sim |
| `user_control` | Usuário assumiu controle via lease | Sim |
| `paused` | Sessão pausada (agente esperando) | Não |
| `terminated` | Sessão encerrada | Não |
| `error` | Erro na conexão | Não |

## Control Lease

- Duração padrão: 5 minutos (configurável via `ComputerSessionManager::new(minutes)`)
- Expiração automática via `expire_leases()` (chamar periodicamente)
- Apenas um usuário por vez pode ter controle ativo
- Release explícito via `release_control()` ou expiry automático
- Campo `reason` opcional para auditoria

## Testing

```bash
cargo test -p zylos-computer-view
```

- **23 testes unitários** cobrindo: create/get session, update state, take control, take control on inactive fails, release control, release wrong user fails, terminate session, list active sessions, list bot sessions, cleanup stale sessions, generate placeholder screenshot, screenshot nonexistent fails, active count, default lease duration, session state roundtrip, is_active, control lease expiry/not expired, serialization
- **Coverage target:** >90% das funções públicas

## Dependencies

| Crate | Version | Reason |
|-------|---------|--------|
| serde | workspace | Serialização JSON |
| serde_json | workspace | Parsing de request/response |
| sqlx | 0.8 (sqlite) | Persistência (futuro) |
| uuid | 1.x | Geração de IDs únicos |
| chrono | workspace | Timestamps ISO 8601, lease expiry |
| tracing | workspace | Logging estruturado |
| thiserror | workspace | Erros tipados |
| tokio | workspace | Runtime async |
| base64 | 0.22 | Encoding de screenshots |
| dashmap | 6.x | Mapa concorrente para sessões |

## Related Documents

- **Playbook:** [pb-computer-view-setup.md](../../playbooks/pb-computer-view-setup.md)
- **Runbook:** [rb-zylos-computer-view.md](../../runbooks/rb-zylos-computer-view.md)
- **POP:** [pop-computer-control-emergency.md](../../playbooks/pop-computer-control-emergency.md)
- **SOAR:** [soar-computer-control-breach.md](../../docs/soar/soar-computer-control-breach.md)
- **Implementation Plan:** [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §3.4
- **Document Compliance:** [DOCUMENT_COMPLIANCE_FASE9.md](../../docs/DOCUMENT_COMPLIANCE_FASE9.md)