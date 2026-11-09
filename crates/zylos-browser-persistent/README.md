# zylos-browser-persistent

Sessões de navegador persistentes por agente com isolamento de cookies/storage. Cada agente recebe seu próprio diretório de perfil com cookies, localStorage, sessionStorage e IndexedDB isolados. As sessões sobrevivem a reinícios via metadados em SQLite.

## Quick Start

```rust
use sqlx::SqlitePool;
use zylos_browser_persistent::{BrowserSessionManager, BrowserManagerConfig};

let pool = SqlitePool::connect("sqlite:browser.db").await?;
let config = BrowserManagerConfig::default();
let mgr = BrowserSessionManager::new(pool, config).await?;

let session = mgr.get_or_create_session("agent-42", &["web".into()]).await?;
mgr.store_cookies(&session.session_id, &cookies).await?;
let cookies = mgr.get_cookies(&session.session_id).await?;
mgr.destroy_session(&session.session_id).await?;
```

## Types

| Tipo | Descrição |
|------|-----------|
| `BrowserSessionManager` | Gerenciador principal; cria/retoma/destrói sessões e gerencia cookies/storage |
| `BrowserSession` | Metadados de uma sessão persistente (ID, agente, profile dir, viewport, tags) |
| `BrowserManagerConfig` | Configuração: base dir de perfis, TTL, limite de sessões, viewport padrão |
| `StoredCookie` | Cookie persistido em SQLite com todos os atributos (domain, path, secure, sameSite) |
| `StorageEntry` | Entrada de localStorage/sessionStorage/IndexedDB vinculada a uma sessão |
| `StorageType` | Enum: `Local`, `Session`, `IndexedDb` |
| `BrowserError` | Enum de erros (ver abaixo) |

## Error Variants

| Variante | Causa |
|----------|-------|
| `SessionNotFound(id)` | ID de sessão não existe no banco |
| `Database(sqlx::Error)` | Falha em operação SQLite |
| `Io(std::io::Error)` | Falha em E/S de filesystem (criar/remover dirs de perfil) |
| `Serialization(serde_json::Error)` | Falha ao serializar/desserializar metadados |
| `ProfileCreation(msg)` | Não foi possível criar o diretório de perfil no disco |
| `SessionExpired(msg)` | Sessão excedeu o TTL configurado |

## Gaps

- Sem suporte a export/import de sessões entre máquinas
- Sem limpeza automática de sessões expiradas (TTL só é checado em `get_or_create_session`)
- Sem criptografia de cookies/storage em repouso
- Sem suporte a múltiplos browsers (apenas Chromium via profile dir)
- Migrações SQL são embutidas inline; sem versionamento de schema dedicado
