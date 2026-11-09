# zylos-vault

[![Crates.io](https://img.shields.io/crates/v/zylos-vault.svg)](https://crates.io/crates/zylos-vault)
[![Documentation](https://docs.rs/zylos-vault/badge.svg)](https://docs.rs/zylos-vault)
[![License](https://img.shields.io/crates/l/zylos-vault.svg)](#license)
[![CI](https://github.com/paulo/zylos-next/actions/workflows/ci.yml/badge.svg)](../../actions)

Armazenamento seguro de credenciais com criptografia AES-256-GCM e derivação de chave via Argon2id. A chave mestra nunca é persistida em disco — ela é derivada a cada desbloqueio a partir de uma passphrase do usuário combinada com um salt armazenado no vault. Todos os segredos são zeroizados da memória após o uso, e toda operação possui trilha de auditoria completa.

## Instalação

Adicione ao seu `Cargo.toml`:

```toml
[dependencies]
zylos-vault = "0.1"
```

### Dependências principais

| Crate        | Propósito                                |
| ------------ | ---------------------------------------- |
| `aes-gcm`    | Criptografia autenticada AES-256-GCM     |
| `argon2`     | Derivação de chave Argon2id (OWASP rec.) |
| `rusqlite`   | Backend SQLite com WAL mode              |
| `zeroize`    | Zeroização segura de memória             |
| `serde`      | Serialização de tags e metadados         |
| `chrono`     | Timestamps Unix em auditoria             |
| `uuid`       | IDs únicos para registros de auditoria   |
| `thiserror`  | Tipos de erro ergonômicos                |
| `tracing`    | Logs estruturados                        |

## Uso rápido

### Abrir ou criar um vault

```rust
use std::path::Path;
use zylos_vault::Vault;

// Cria um novo vault ou abre um existente.
// A passphrase é usada para derivar a chave mestra via Argon2id.
let vault = Vault::open(Path::new("./my-vault.db"), "minha-passphrase-segura")?;
```

### Armazenar um segredo

```rust
// Chave única, valor em bytes, tags para filtragem, identificador do chamador
vault.store_secret(
    "stripe/api-key",
    b"sk_live_abc123xyz",
    &["prod", "payments"],
    "deploy-script",
)?;
```

### Recuperar um segredo

```rust
match vault.retrieve_secret("stripe/api-key", "app-server")? {
    Some(data) => println!("Chave recuperada: {} bytes", data.len()),
    None => println!("Segredo não encontrado"),
}
```

### Listar chaves (com filtro opcional por tag)

```rust
// Todas as chaves
let all = vault.list_keys(None)?;

// Apenas chaves com a tag "prod"
let prod_keys = vault.list_keys(Some("prod"))?;

for entry in &prod_keys {
    println!("{} (tags: {:?})", entry.key, entry.tags);
}
```

### Excluir um segredo

```rust
vault.delete_secret("stripe/api-key", "admin-cli")?;
// Retorna VaultError::NotFound se a chave não existir
```

### Rotacionar a chave mestra

Re-criptografa todos os segredos com uma nova passphrase em uma transação atômica:

```rust
let mut vault = Vault::open(Path::new("./my-vault.db"), "passphrase-antiga")?;
vault.rotate_key("passphrase-antiga", "passphrase-nova", "security-admin")?;
// A instância atual já usa a nova chave para operações subsequentes
```

### Consultar trilha de auditoria

```rust
let records = vault.audit_log(50)?; // Últimas 50 entradas, mais recentes primeiro

for record in &records {
    println!(
        "[{}] {} -> {} por {} (sucesso: {})",
        record.timestamp, record.action, record.target_key,
        record.caller, record.success
    );
}
```

## API pública

### `Vault`

| Método | Descrição |
| --- | --- |
| `Vault::open(path: &Path, passphrase: &str) -> Result<Vault>` | Abre ou cria um vault. Deriva a chave mestra e verifica contra o token de verificação armazenado. |
| `store_secret(key, value, tags, caller) -> Result<()>` | Criptografa e armazena um segredo. Sobrescreve se a chave já existir (upsert). |
| `retrieve_secret(key, caller) -> Result<Option<Vec<u8>>>` | Descriptografa e retorna um segredo. `None` se não encontrado. |
| `list_keys(tag_filter: Option<&str>) -> Result<Vec<VaultEntry>>` | Lista todas as chaves, opcionalmente filtradas por substring de tag. |
| `delete_secret(key, caller) -> Result<()>` | Remove um segredo. Erro se não existir. |
| `rotate_key(old_passphrase, new_passphrase, caller) -> Result<()>` | Re-criptografa todos os segredos com nova passphrase em transação atômica. |
| `audit_log(limit: usize) -> Result<Vec<AuditRecord>>` | Retorna as últimas N entradas de auditoria, ordenadas por timestamp decrescente. |

### Tipos

```rust
pub struct VaultEntry {
    pub key: String,
    pub tags: Vec<String>,
    pub created_at: i64,   // Unix timestamp
    pub updated_at: i64,   // Unix timestamp
}

pub struct AuditRecord {
    pub id: String,         // UUID v4
    pub timestamp: i64,     // Unix timestamp
    pub action: String,     // "store" | "retrieve" | "delete" | "rotate_key"
    pub target_key: String, // "*" para rotate_key
    pub caller: String,
    pub success: bool,
}

pub enum VaultError {
    Crypto(String),
    Database(rusqlite::Error),
    KeyDerivation(String),
    NotFound(String),
    AuthFailed,
    Io(std::io::Error),
    Serialization(serde_json::Error),
}
```

## Arquitetura

```
┌─────────────────────────────────────────────┐
│                  Vault API                   │
│  open · store · retrieve · list · delete     │
│  rotate_key · audit_log                      │
├─────────────────────────────────────────────┤
│           MasterKey (ZeroizeOnDrop)          │
│  Argon2id(passphrase + salt) → [u8; 32]     │
│  Nunca persistida · Verificada via token     │
├──────────────────┬──────────────────────────┤
│   AES-256-GCM    │      SQLite (WAL)        │
│  Nonce aleatório │  secrets · meta · audit  │
│  por operação    │  Índices em timestamp    │
└──────────────────┴──────────────────────────┘
```

### Schema do banco de dados

- **`meta`** — Salt (16 bytes) e token de verificação da chave mestra
- **`secrets`** — Chave (PK), ciphertext, nonce (12 bytes), tags (JSON), timestamps
- **`audit_log`** — ID (UUID), timestamp, ação, chave-alvo, chamador, sucesso; indexado por timestamp

## Segurança

- **Criptografia**: AES-256-GCM com nonce aleatório de 12 bytes gerado via `rand::thread_rng()` para cada operação
- **Derivação de chave**: Argon2id v1.3 com parâmetros OWASP recomendados (19 MiB memória, 2 iterações, paralelismo 1)
- **Zeroização**: `MasterKey` implementa `ZeroizeOnDrop` — a chave é apagada da memória quando o `Vault` é liberado
- **Verificação de passphrase**: Token de verificação criptografado armazenado no vault; falha silenciosa retorna `VaultError::AuthFailed` sem revelar se o vault existe
- **Auditoria obrigatória**: Toda operação (store, retrieve, delete, rotate) gera registro imutável com identidade do chamador e resultado
- **Transações atômicas**: Rotação de chave re-criptografa todos os segredos dentro de uma transação SQLite — falha parcial reverte completamente
- **WAL mode**: Habilitado para leituras concorrentes sem bloqueio de escrita

### Considerações

- O arquivo `.db` contém dados criptografados, mas o salt e nonces são visíveis — isso é by design e seguro
- Backups do arquivo do vault devem ser tratados como material sensível
- A passphrase é o único ponto de confiança; perda da passphrase = perda irreversível dos dados
- Para ambientes multi-thread, use uma instância por thread ou sincronize externamente (`Mutex<Arc<Vault>>`)

## Testes

```bash
cargo test -p zylos-vault
```

O suite inclui 10 testes cobrindo:

- Store e retrieve round-trip
- Retrieve de chave inexistente (retorna `None`)
- Listagem com e sem filtro de tag
- Delete e erro ao deletar inexistente
- Passphrase incorreta retorna `AuthFailed`
- Rotação de chave com verificação pós-rotação
- Trilha de auditoria com ordem e campos corretos
- Upsert (sobrescrita de segredo existente)
- Valor vazio (zero bytes)
- Segredo grande (1 MB)

## Licença

Este projeto é licenciado sob os termos da licença MIT ou Apache-2.0, à sua escolha. Consulte os arquivos `LICENSE-MIT` e `LICENSE-APACHE` no repositório raiz para detalhes.