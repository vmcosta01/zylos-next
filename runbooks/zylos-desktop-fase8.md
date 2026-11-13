# Runbook: zylos-desktop (Fase 8 Update)

**Versão:** 2.0
**Data:** 2026-11-11
**Autor:** Implementation Agent (Fase 8)
**Skill:** `runbook-builder`
**Módulos Novos:** `workspace_persistence`, `agent_awareness`, `relay_server`, `mobile_responsive`

---

## 1. Visão Geral

O crate `zylos-desktop` fornece o shell Tauri v2 para o zylos-gateway. A Fase 8 adicionou 4 módulos operacionais críticos:

| Módulo | Função | Porta/Path |
|--------|--------|------------|
| `workspace_persistence` | Salva/restaura estado do cockpit entre sessões | `~/.zylos/workspace-state.json` |
| `agent_awareness` | Estados visuais de agentes para UI (Idle/Thinking/Executing/etc.) | IPC + WebSocket |
| `relay_server` | HTTP/WebSocket relay para mobile-to-desktop | `:9877` |
| `mobile_responsive` | Breakpoints e CSS responsivo para cockpit WASM | Injected stylesheet |

---

## 2. Workspace Persistence

### 2.1 Localização do Arquivo

```
Windows: %LOCALAPPDATA%\zylos\workspace-state.json
macOS:   ~/Library/Application Support/zylos/workspace-state.json
Linux:   ~/.local/share/zylos/workspace-state.json
```

### 2.2 Reset Manual

Se o estado estiver corrompido:

```bash
# Backup primeiro
cp workspace-state.json workspace-state.json.bak

# Reset via API
curl -X POST http://localhost:9876/api/v1/desktop/workspace/reset \
  -H "Authorization: Bearer $TOKEN"

# Ou delete manual
rm workspace-state.json
# Próximo start cria default automaticamente
```

### 2.3 Debounce Behavior

- Save automático com **300ms debounce** após última mudança
- Múltiplas mudanças rápidas = apenas 1 escrita no disco
- Save explícito via `POST /api/v1/desktop/workspace/save` força escrita imediata

### 2.4 Schema Migration

Campo `schema_version` (atual: 1). Se versão futura detectada, load tenta best-effort e loga warning. Rollback: restaurar backup `.bak`.

---

## 3. Agent Awareness States

### 3.1 Estados e Significados

| Estado | Ícone | Cor CSS | Requer Atenção? |
|--------|-------|---------|-----------------|
| Idle | 💤 | `--color-muted` | Não |
| Thinking | 🤔 | `--color-warning` | Não |
| Executing | ⚡ | `--color-accent` | Não |
| WaitingApproval | ✋ | `--color-info` | **Sim** |
| Error | ❌ | `--color-error` | **Sim** |
| Completed | ✅ | `--color-success` | Não |

### 3.2 Monitoramento via API

```bash
# Todos os estados atuais
curl http://localhost:9876/api/v1/desktop/agents/states \
  -H "Authorization: Bearer $TOKEN"

# Apenas agentes requerendo atenção
curl http://localhost:9876/api/v1/desktop/agents/attention \
  -H "Authorization: Bearer $TOKEN"

# Estatísticas
curl http://localhost:9876/api/v1/desktop/agents/stats \
  -H "Authorization: Bearer $TOKEN"
```

### 3.3 Histórico de Transições

```bash
# Últimas 20 transições
curl "http://localhost:9876/api/v1/desktop/agents/history?limit=20" \
  -H "Authorization: Bearer $TOKEN"
```

Ring buffer de 100 eventos por default. Eventos mais antigos são descartados automaticamente.

---

## 4. Relay Server (Mobile-to-Desktop)

### 4.1 Configuração Default

```yaml
bind_addr: "0.0.0.0"
port: 9877
gateway_url: "http://127.0.0.1:9876"
token_ttl_secs: 86400  # 24h
rate_limit_per_minute: 60
cors_enabled: true
```

### 4.2 Pairing de Dispositivo

```bash
curl -X POST http://localhost:9877/api/v1/relay/pair \
  -H "Content-Type: application/json" \
  -d '{"device_name": "iPhone 15 Pro"}'
```

Resposta: `{"token": "zrly_...", "expires_at": 1700086400}`

**IMPORTANTE:** Token é exibido **apenas uma vez**. Armazenar com segurança.

### 4.3 Listar Dispositivos Ativos

```bash
curl http://localhost:9877/api/v1/relay/devices \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### 4.4 Revogar Token

Ver `playbooks/pop-relay-token-revoke.md` para procedimento completo.

### 4.5 Rate Limiting

- 60 requests/minuto/IP por default
- IPs bloqueados retornam `429 Too Many Requests`
- Window reseta automaticamente após 60s
- Verificar status: `GET /api/v1/relay/rate-limits`

### 4.6 Segurança

- Tokens prefixados com `zrly_` para fácil identificação
- Purge automático de tokens expirados a cada 24h
- CORS configurável via `cors_origins` no config
- Bind em `0.0.0.0` aceita LAN; usar firewall para restringir se necessário

---

## 5. Mobile Responsive

### 5.1 Breakpoints

| Breakpoint | Largura | Layout |
|------------|---------|--------|
| Mobile | < 640px | Single column, sidebar hidden, bottom nav |
| Tablet | 640–1023px | 2-column, sidebar optional |
| Desktop | ≥ 1024px | Full sidebar + multi-panel grid |

### 5.2 CSS Variables Injected

O módulo gera CSS custom properties automaticamente:

```css
--sidebar-visible: 0|1;
--grid-columns: 1|2|3;
--stack-panels: 0|1;
--font-scale: 0.9|1.0;
--touch-target-min: 44|40|32px;
--safe-area-top/right/bottom/left: Npx;
```

### 5.3 Safe Area Insets

Para dispositivos com notch/Dynamic Island:

```bash
# Viewport state atual inclui safe area
curl http://localhost:9876/api/v1/desktop/viewport \
  -H "Authorization: Bearer $TOKEN"
```

Resposta inclui `safe_area: {top, right, bottom, left}` em pixels.

### 5.4 Accessibility

- `prefers-reduced-motion`: desabilita animações
- `prefers-color-scheme`: dark/light automático
- Touch targets mínimos por breakpoint (44px mobile, 32px desktop)

---

## 6. Troubleshooting

### 6.1 Workspace State Corrompido

**Sintoma:** Cockpit não carrega ou mostra layout quebrado
**Diagnóstico:** `cat workspace-state.json | jq .` — se falhar, JSON inválido
**Fix:** Delete arquivo, restart app. Default state criado automaticamente.

### 6.2 Relay Token Expirado

**Sintoma:** Mobile app retorna 401
**Diagnóstico:** `GET /api/v1/relay/devices` — verificar `expires_at`
**Fix:** Re-pair dispositivo. Tokens não podem ser renovados, apenas recriados.

### 6.3 Agent Stuck in "Thinking"

**Sintoma:** Agente permanece em Thinking > 5 minutos
**Diagnóstico:** `GET /api/v1/desktop/agents/attention`
**Fix:** Verificar logs do gateway. Pode indicar deadlock ou timeout de tool call.

### 6.4 Mobile Layout Quebrado

**Sintoma:** Elementos sobrepostos ou fora da viewport no mobile
**Diagnóstico:** Verificar `--safe-area-*` variables no DevTools
**Fix:** Ajustar `SafeAreaInsets` no desktop config ou adicionar padding CSS.

---

## 7. Cross-References

- **HANDOFF:** `docs/handoffs/HANDOFF_FASE8.md` §2.8
- **POPs:** `pop-relay-token-revoke.md`, `pop-guardrail-rule-update.md`
- **Playbook:** `skill-evolution-lifecycle.md`
- **Código:** `crates/zylos-desktop/src/{workspace_persistence,agent_awareness,relay_server,mobile_responsive}.rs`
- **Runbook anterior:** `runbooks/zylos-desktop.md` (v1.x, pré-Fase 8)

---

## 8. Histórico

| Versão | Data | Autor | Mudança |
|--------|------|-------|---------|
| 1.0 | 2026-11-07 | Implementation Agent | Criação inicial (pré-Fase 8) |
| 2.0 | 2026-11-11 | Implementation Agent | Adição módulos Fase 8 (workspace, awareness, relay, responsive) |