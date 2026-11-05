# Runbook — zylos-mobile (Mobile MCP Server)

**Versão:** 1.0
**Data:** 2 de novembro de 2026
**Crate:** `crates/zylos-mobile`
**Dependências:** `serde`, `serde_json`, `tokio`
**Fase:** 3 (LIVE_COMM_SYSTEM) → 7 (END_TO_END_INTEGRATION)

---

## 1. Visão Geral

O `zylos-mobile` implementa um servidor MCP (Model Context Protocol) leve para integração com dispositivos móveis. Expõe tools via JSON-RPC sobre stdio ou HTTP, permitindo que agentes Zylos interajam com sensores, câmera, GPS e notificações do dispositivo.

**Módulos:**
- `mcp_server.rs` — Servidor MCP com dispatch de tools/list e tools/call
- `error.rs` — Tipos de erro unificados (McpError com code/message/data)
- `lib.rs` — Re-exports públicos e trait MobileCapability

**Tools expostas:**
- `device/info` — Informações do dispositivo (modelo, OS, bateria)
- `camera/capture` — Captura de foto/vídeo
- `location/get` — Geolocalização atual
- `notification/send` — Envio de notificação push local
- `sensor/read` — Leitura de acelerômetro, giroscópio, luminosidade

---

## 2. Nível Senior — Arquitetura e Decisões

### 2.1 Por que MCP para mobile?
MCP é o protocolo padrão do ecossistema Hermes para tool discovery e invocation. Usar MCP em vez de REST/gRPC customizado garante:
- Compatibilidade nativa com Hermes Agent e Cockpit
- Auto-discovery de capabilities via `tools/list`
- Schema validation automática de inputs/outputs
- Streaming de eventos via `events/subscribe`

### 2.2 Transporte
Suporta dois modos:
- **stdio:** Para integração direta com Hermes Agent (pipe stdin/stdout)
- **HTTP SSE:** Para Cockpit web e clientes remotos (endpoint `/mcp`)

Modo é selecionado via env `ZYLOS_MOBILE_TRANSPORT` (default: `stdio`).

### 2.3 Segurança
- Cada tool requer capability declaration no manifest do agente
- Rate limiting por tool (configurável, default 10 req/s)
- Camera/location requerem approval explícito do usuário no dispositivo
- Sem acesso a filesystem do dispositivo sem sandbox

### 2.4 Trade-offs
| Aspecto | Nossa Abordagem | Alternativa Rejeitada |
|---------|----------------|----------------------|
| Protocolo | MCP (JSON-RPC) | gRPC (overhead, sem auto-discovery) |
| Transporte | stdio + HTTP SSE | WebSocket only (sem pipe integration) |
| Approval | Per-tool no device | Global toggle (granularidade insuficiente) |
| Schema | JSON Schema draft-7 | Protobuf (sem introspecção dinâmica) |

---

## 3. Nível Pleno — Operação e Integração

### 3.1 Build e testes
```bash
cd C:/Users/Paulo/zylos-next
cargo test -p zylos-mobile
# 5 testes: mcp_tools_list, mcp_tools_call_valid, mcp_tools_call_invalid, error_serialization, ping_pong
```

### 3.2 Execução stdio
```bash
# Via Hermes Agent (auto-spawn)
hermes agent add --mcp-server "zylos-mobile"

# Manual (debug)
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | cargo run -p zylos-mobile
```

### 3.3 Execução HTTP
```bash
ZYLOS_MOBILE_TRANSPORT=http ZYLOS_LISTEN=0.0.0.0:3001 cargo run -p zylos-mobile
# Endpoint: POST http://localhost:3001/mcp
# Health: GET http://localhost:3001/health
```

### 3.4 Variáveis de ambiente
| Variável | Default | Descrição |
|----------|---------|-----------|
| `ZYLOS_MOBILE_TRANSPORT` | `stdio` | Modo de transporte (`stdio` ou `http`) |
| `ZYLOS_LISTEN` | `0.0.0.0:3001` | Endereço HTTP (apenas modo http) |
| `ZYLOS_MOBILE_RATE_LIMIT` | `10` | Req/s por tool |
| `RUST_LOG` | `info` | Filtro de tracing |

### 3.5 Checklist operacional
- [ ] Transporte configurado conforme ambiente (stdio para agent, http para cockpit)
- [ ] Rate limit ajustado para carga esperada
- [ ] Approval flow testado no dispositivo alvo
- [ ] Schema das tools validado contra spec MCP
- [ ] Logs capturados para auditoria de chamadas

### 3.6 Troubleshooting
| Sintoma | Causa provável | Solução |
|---------|---------------|---------|
| `Method not found` | Tool não registrada | Verificar `register_tool()` no init |
| `Invalid params` | Schema mismatch | Validar input contra JSON Schema da tool |
| `Timeout` | Device não respondeu | Verificar conexão/approval pendente |
| `Rate limited` | Excedeu req/s | Aumentar `ZYLOS_MOBILE_RATE_LIMIT` ou throttle client |
| `Transport error` | Pipe quebrado / porta ocupada | Reiniciar server ou mudar porta |

---

## 4. Nível Júnior — Execução Guiada

### 4.1 Verificar se compila
```bash
cd C:/Users/Paulo/zylos-next
cargo check -p zylos-mobile
```
**Esperado:** `Finished` sem erros.

### 4.2 Rodar testes
```bash
cargo test -p zylos-mobile
```
**Esperado:** `test result: ok. 5 passed`.

### 4.3 Testar tools/list via stdio
```bash
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | cargo run -p zylos-mobile
```
**Esperado:** JSON com array de tools e seus schemas.

### 4.4 Quando escalar
- Erro de compilação → verificar dependências (`serde`, `serde_json`, `tokio`)
- Teste falha → ler mensagem, comparar com expected no código
- Tool não aparece em list → verificar registro no `mcp_server.rs`
- Device não responde → verificar app mobile instalado e permissões concedidas
- Rate limit falso positivo → ajustar `ZYLOS_MOBILE_RATE_LIMIT`

---

## 5. Referências Cruzadas

- **Código fonte:** `crates/zylos-mobile/src/`
- **Runbook Fase 3:** `runbooks/fase3-live-comm.md`
- **Spec comunicação:** `specs/COMMUNICATION-CONTRACT.md` §MCP
- **HANDOFF_3:** Documentação Fase 3
- **MCP spec:** https://modelcontextprotocol.io/specification