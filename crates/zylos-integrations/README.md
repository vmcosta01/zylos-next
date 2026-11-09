# zylos-integrations

[![Crates.io](https://img.shields.io/crates/v/zylos-integrations.svg)](https://crates.io/crates/zylos-integrations)
[![Docs.rs](https://docs.rs/zylos-integrations/badge.svg)](https://docs.rs/zylos-integrations)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Catálogo de integrações SaaS OAuth2 para o **zylos-gateway**. Fornece armazenamento criptografado de tokens, refresh automático com jitter, especificações declarativas de providers via YAML, executor de ações e adaptadores de canal (WeChat Work, Feishu/Lark).

## Instalação

Adicione ao `Cargo.toml`:

```toml
[dependencies]
zylos-integrations = "0.1"
```

### Variáveis de ambiente obrigatórias

| Variável | Descrição |
|---|---|
| `ZYLOS_VAULT_KEY` | Chave AES-256 em hex (64 caracteres / 32 bytes). Usada para criptografar tokens no SQLite. |
| `ZYLOS_DATA_DIR` | *(Opcional)* Diretório onde o banco `integration_tokens.db` será criado. Padrão: `$DATA_DIR/zylos`. |

Sem `ZYLOS_VAULT_KEY`, o crate usa uma chave de desenvolvimento (todos zeros) e emite um warning — **nunca use isso em produção**.

## Uso rápido

### Inicializar o registry e listar providers

```rust
use zylos_integrations::IntegrationRegistry;
use std::path::Path;

#[tokio::main]
async fn main() -> zylos_integrations::Result<()> {
    // Carrega todos os *.yaml do diretório de specs
    let registry = IntegrationRegistry::from_specs_dir(
        Path::new("./integrations/specs")
    ).await?;

    for spec in registry.list().await {
        println!("{} ({})", spec.display_name, spec.slug);
    }

    Ok(())
}
```

### Fluxo OAuth2 PKCE completo

```rust
// 1. Iniciar o fluxo — retorna URL para redirecionar o usuário
let session = registry.initiate_oauth("gmail", "http://localhost:8080/callback").await?;
println!("Autorize em: {}", session.authorization_url);

// 2. Após o callback, trocar o código por tokens
let bundle = registry.complete_oauth(
    &session.session_id,
    "authorization_code_from_callback",
    "user-123",
).await?;

println!("Token obtido para {}", bundle.provider_slug);
```

### Executar uma ação com auto-refresh

```rust
let result = registry.execute_action(
    "user-123",
    "linear",
    "create_issue",
    serde_json::json!({
        "title": "Bug report",
        "description": "Something broke"
    }),
).await?;

println!("{:?}", result);
```

O executor verifica automaticamente se o token expirou, faz refresh transparente e persiste o novo token antes de executar a ação.

### Revogar tokens

```rust
registry.revoke("user-123", "gmail").await?;
```

## Arquitetura dos módulos

```
src/
├── lib.rs          # IntegrationRegistry — API principal
├── manifest.rs     # IntegrationSpec, ActionSpec, SpecRegistry (YAML loader)
├── oauth/mod.rs    # OAuthFlow (PKCE), TokenBundle, OAuthSession
├── db/mod.rs       # TokenStore (SQLite + AES-256-GCM), audit log
├── executor.rs     # ActionExecutor — dispatch HTTP + PII scanning
├── composio.rs     # ComposioAdapter (feature-flagged, 2000+ apps)
├── channel.rs      # ChannelAdapter trait + WeChat Work / Feishu adapters
└── error.rs        # IntegrationError (thiserror)
```

### `manifest` — Especificações declarativas

Carrega arquivos YAML que descrevem cada provider OAuth2 e suas ações disponíveis.

```rust
pub struct IntegrationSpec {
    pub slug: String,
    pub display_name: String,
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub actions: Vec<ActionSpec>,
    pub requires_approval: bool,
    pub rate_limit_rpm: u64,
}
```

### `oauth` — Motor OAuth2 PKCE

Gerencia sessões pendentes, troca de códigos e refresh de tokens. Sessões expiram após 1 hora e são limpas automaticamente.

```rust
pub struct TokenBundle {
    pub provider_slug: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub scopes: Vec<String>,
    pub token_type: String,
}
```

### `db` — Storage criptografado

SQLite WAL com tokens criptografados via AES-256-GCM. Nonce aleatório de 12 bytes prependido ao ciphertext. Audit log com hash SHA-256 de cada ação executada.

### `executor` — Dispatch de ações

Constrói requisições HTTP com bearer auth, substitui `{param}` em templates de URL, injeta headers específicos por provider (Notion-Version, Linear GraphQL) e escaneia o body de saída por PII antes do envio.

### `composio` — Marketplace de apps (feature-flagged)

Adapter para o catálogo Composio com cache TTL configurável. Quando desabilitado, todas as chamadas retornam `ComposioError::Disabled` para fallback gracioso.

### `channel` — Adaptadores de mensagens

Trait `ChannelAdapter` unificada para plataformas de mensageria corporativa:

```rust
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    fn platform_id(&self) -> &'static str;
    async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult>;
    async fn list_users(&self, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<UserInfo>>;
    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo>;
    async fn list_chats(&self, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<ChatInfo>>;
    async fn list_departments(&self, parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>>;
    async fn list_calendar_events(&self, start_time: &str, end_time: &str) -> Result<Vec<CalendarEvent>>;
    async fn health_check(&self) -> Result<bool>;
}
```

Implementações incluídas: `wechat_work::WeChatWorkAdapter`, `feishu::FeishuAdapter`.

## Configuração de providers via YAML

Crie arquivos `.yaml` no diretório de specs:

```yaml
slug: linear
display_name: Linear
auth_url: https://linear.app/oauth/authorize
token_url: https://api.linear.app/oauth/token
client_id: "${LINEAR_CLIENT_ID}"
client_secret: "${LINEAR_CLIENT_SECRET}"
redirect_uri: http://localhost:8080/callback
scopes:
  - read
  - write
requires_approval: false
rate_limit_rpm: 100
actions:
  - id: create_issue
    description: Create a new issue
    method: POST
    url_template: https://api.linear.app/graphql
    requires_approval: false
    body_schema:
      type: object
      properties:
        title: { type: string }
        description: { type: string }

  - id: list_issues
    description: List issues in a team
    method: GET
    url_template: https://api.linear.app/issues?team={team_id}
    requires_approval: false
```

Campos opcionais: `scopes` (default vazio), `actions` (default vazio), `requires_approval` (default `false`), `rate_limit_rpm` (default `0` = ilimitado).

## Segurança

### Criptografia de tokens

- **Algoritmo:** AES-256-GCM com nonce aleatório de 12 bytes
- **Chave:** Derivada da variável `ZYLOS_VAULT_KEY` (hex, 32 bytes)
- **Storage:** SQLite WAL; tokens nunca persistidos em plaintext
- **Memória:** Chave envolvida em `Zeroizing<[u8; 32]>` para limpeza segura

### PII stripping em requisições de saída

O `ActionExecutor` escaneia o body de cada requisição antes do envio usando padrões regex embutidos:

- SSN (`XXX-XX-XXXX`)
- Cartão de crédito (sequências de 13–19 dígitos)
- Endereços de email

Achados são logados no audit trail com `decided_by = "sentinel"`. Em produção, integre com `zylos-sentinel` no nível do gateway para bloqueio ativo.

### HITL (Human-in-the-Loop)

Ações ou providers marcados com `requires_approval: true` retornam `IntegrationError::ApprovalRequired` antes de qualquer chamada HTTP, exigindo aprovação explícita.

### Audit trail

Toda execução de ação é registrada na tabela `integration_audit_log` com:
- `user_id`, `provider_slug`, `action`
- `decided_by` (`auto` ou `sentinel`)
- `decision_hash` (SHA-256 de `action:slug:result`)
- Timestamp Unix

## Feature flags

| Feature | Default | Descrição |
|---|---|---|
| `composio` | ❌ | Habilita o adapter Composio para 2000+ apps SaaS |

```toml
[dependencies]
zylos-integrations = { version = "0.1", features = ["composio"] }
```

Quando `composio` está desabilitado, `ComposioAdapter` ainda pode ser instanciado, mas todas as chamadas retornam `ComposioError::Disabled`.

## Testes

```bash
# Testes unitários
cargo test -p zylos-integrations

# Com feature composio
cargo test -p zylos-integrations --features composio
```

Testes incluem:
- Criação do registry com diretório vazio
- Serialização/deserialização de `IntegrationSpec`, `OutboundMessage`, `ComposioApp`
- Health check de adapters sem token (deve retornar `false`)
- Comportamento disabled do Composio adapter
- MessageId display e conversão From<String>

## Licença

MIT — veja [LICENSE](../../LICENSE).