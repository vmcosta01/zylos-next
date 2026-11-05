# zylos-mobile

MCP server para integração mobile com agentes Zylos. Expõe ferramentas via
Model Context Protocol para clientes móveis consumirem capacidades dos agentes.

**Fase no Master Plan:** Fase 6 (LIVE_COMM_SYSTEM) — componente mobile

## Features Implementadas

### MCP Server (`mcp_server.rs` — 1194 linhas)
- MCP protocol compliance
- Tool registration e discovery
- Request/response handling
- Error types estruturados

### Error Handling (`error.rs` — 57 linhas)
- Mobile-specific error types
- Serialization-safe error variants

## API Pública

### mcp_server.rs
- `pub struct JsonRpcRequest`
- `pub struct JsonRpcResponse`
- `pub fn ok`
- `pub fn err`
- `pub struct McpErrorBody`
- `pub struct McpToolDef`
- `pub struct McpResourceDef`
- `pub struct McpPromptDef`
- `pub struct McpPromptArgument`
- `pub struct ServerCapabilities`
- `pub struct AuthConfig`
- `pub fn validate`
- `pub struct RateLimiter`
- `pub fn new`
- `pub fn check`
- `pub struct AttachmentStore`
- `pub fn new`
- `pub fn store`
- `pub fn get`
- `pub fn cleanup_expired`
- `pub struct MobileMcpServer`
- `pub fn new`
- `pub fn with_config`
- `pub fn handle_raw`
- `pub fn dispatch`
- `pub fn sse_event_stream`
- `pub fn event_bus`
- `pub fn attachments`
- `pub fn spawn_attachment_cleanup`

### error.rs
- `pub enum MobileError`

## Status vs Master Plan

| Aspecto | Status | Notas |
|---------|--------|-------|
| MCP Server | ✅ Implementado | 1194 linhas, 29 pub items |
| Error handling | ✅ Implementado | Structured errors |
| Tests | ❓ Verificar | Coverage desconhecida |

## Gaps Críticos

1. **Documentação escassa** — Module-level docs mínimos
2. **Testes** — Verificar cobertura de testes unitários/integração
3. **Mobile-specific features** — Offline sync, push notifications, battery-aware scheduling não verificados
