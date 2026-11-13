# zylos-auth

Crate de autenticação multi-tenant, JWT stateless, RBAC e primitivas CRDT para colaboração em tempo real.

## Módulos

| Módulo | Descrição |
|--------|-----------|
| `claims` | JWT Claims com tenant_id, roles e expiry |
| `tokens` | TokenService para emissão e validação HS256 |
| `roles` | Enum Role (Admin, Editor, Viewer, ServiceAccount, Custom) |
| `permissions` | Permission enum com verificação baseada em roles |
| `middleware` | Axum middleware para Bearer token validation |
| `crdt` | CrdtSyncServer — broadcast Yjs-compatible over WebSocket |
| `presence` | PresenceTracker — quem está editando agora |

## Configuração

```bash
export ZYLOS_AUTH_SECRET="sua-chave-secreta-min-32-bytes"
```

Sem a variável, usa fallback dev-only (`dev-secret-change-me-in-production`). **Nunca use em produção.**

## Uso rápido

```rust
use zylos_auth::{TokenService, TokenConfig, CreateTokenRequest};

let svc = TokenService::new(TokenConfig::default())?;
let token = svc.create_token(&CreateTokenRequest {
    user_id: "user-1".into(),
    tenant_id: "tenant-a".into(),
    roles: vec!["editor".into()],
    name: Some("Alice".into()),
})?;

let claims = svc.validate_token(&token.access_token)?;
assert!(claims.has_role("editor"));
```

## Middleware Axum

```rust
use zylos_auth::middleware::auth_middleware;
use axum::middleware;

let app = Router::new()
    .route("/protected", get(handler))
    .layer(middleware::from_fn_with_state(token_service, auth_middleware));
```

O middleware injeta `AuthContext` nas extensions do request. Use `require_auth(&request)` nos handlers.

## CRDT Sync

O `CrdtSyncServer` gerencia documentos colaborativos via broadcast channels:

```rust
use zylos_auth::CrdtSyncServer;

let server = CrdtSyncServer::new();
let mut rx = server.subscribe(&"skill-abc".into());
server.broadcast_update(&"skill-abc".into(), CrdtMessage::Update(data));
```

Protocolo wire compatível com y-sync/y-websocket (SyncStep1, SyncStep2, Update, Awareness).

## Presence

```rust
use zylos_auth::PresenceTracker;

let tracker = PresenceTracker::new();
tracker.join("doc-1", client_id, "user-a".into(), "Alice".into(), "#ff0000".into());
let users = tracker.active_users("doc-1"); // Vec<(ClientId, UserId, Name, Color)>
```

## Testes

```bash
cargo test -p zylos-auth
```

## ADR-009

Este crate implementa a opção **JWT Stateless** avaliada na ADR-009. Tokens são auto-contidos (HS256), sem session store. Revogação requer blacklist ou TTL curto + refresh tokens (ver `pop-token-revoke`).