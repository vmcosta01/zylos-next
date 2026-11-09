# zylos-sandbox

Ambientes de execução isolados para código de agentes IA. O crate fornece seis módulos complementares que cobrem desde execução WASM leve até containers Docker completos, passando por anti-detecção de navegador, perfis persistentes, injeção segura de credenciais e compartilhamento de tela via VNC.

## Diagrama de Arquitetura

```
┌─────────────────────────────────────────────────────────────────┐
│                     zylos-sandbox                               │
│                                                                 │
│  ┌──────────────┐  ┌────────────────┐  ┌─────────────────────┐  │
│  │ wasm_sandbox │  │ docker_sandbox │  │ browser_antidetect  │  │
│  │              │  │                │  │                     │  │
│  │ wasmtime     │  │ bollard        │  │ Fingerprint         │  │
│  │ snapshot/    │  │ lifecycle      │  │ generation          │  │
│  │ resume       │  │ health checks  │  │ stealth patches     │  │
│  │ fuel limits  │  │ volumes/network│  │ bot risk scoring    │  │
│  └──────┬───────┘  └───────┬────────┘  └──────────┬──────────┘  │
│         │                  │                      │             │
│         │           ┌──────┴──────────┐   ┌───────┴──────────┐  │
│         │           │browser_profile_ │   │  secure_input    │  │
│         │           │store            │   │  (Ghost Typist)  │  │
│         │           │                 │   │                  │  │
│         │           │ Chromium dirs   │   │ CredentialVault  │  │
│         │           │ owner/tag index │   │ rate limiting    │  │
│         │           │ session tracking│   │ audit trail      │  │
│         │           └─────────────────┘   └────────┬─────────┘  │
│         │                                          │             │
│         │           ┌──────────────────┐           │             │
│         └──────────►│  screen_share    │◄──────────┘             │
│                     │                  │                         │
│                     │ TigerVNC/noVNC   │                         │
│                     │ idle timeout     │                         │
│                     │ one-time tokens  │                         │
│                     └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

## Módulos

| Módulo | Descrição | Caso de uso |
|--------|-----------|-------------|
| `wasm_sandbox` | Execução WASM com snapshot/resume via wasmtime | Código leve, determinístico, com limites de recurso |
| `docker_sandbox` | Containers Docker com ciclo de vida completo | Workloads que precisam de SO completo, ferramentas nativas |
| `browser_antidetect` | Geração de fingerprint e patches stealth | Automação web sem detecção de bot |
| `browser_profile_store` | Perfis Chromium persistentes por agente | Manter cookies/login entre sessões |
| `secure_input` | Pipeline "Ghost Typist" para credenciais | Injetar secrets sem expor ao LLM ou logs |
| `screen_share` | Sessões VNC/noVNC por workspace | Inspeção visual de sandboxes em tempo real |

---

## wasm_sandbox

Execução isolada de módulos WASM com suporte a snapshot/resume, limites de recurso (memória, fuel, FDs), política de rede e replay determinístico via fingerprint SHA-256.

### Tipos principais

- **`WasmSandbox`** — instância de sandbox com métodos `call_function`, `snapshot`, `restore`
- **`SandboxConfig`** — limites de memória/fuel/FDs, paths permitidos, variáveis de ambiente, política de rede
- **`NetworkPolicy`** — allow/deny list com wildcard (`*.internal.net`) e default-deny
- **`SandboxSnapshot`** — memória linear + globals serializáveis com metadados
- **`ExecutionResult`** — valor de retorno, fuel consumido, duração, uso de memória
- **`InputFingerprint`** — hash content-addressable (módulo + args) para replay determinístico
- **`SandboxError`** — enum de erros: `Compilation`, `Instantiation`, `ResourceLimit`, `Snapshot`, `Restore`, `CallFailed`, `Shutdown`, `NetworkPolicy`

### Exemplo de uso

```rust
use zylos_sandbox::{WasmSandbox, SandboxConfig, NetworkPolicy};
use std::time::Duration;

let wasm_bytes = std::fs::read("agent_module.wasm")?;

let config = SandboxConfig {
    max_memory_bytes: 128 * 1024 * 1024, // 128 MB
    max_execution_time: Duration::from_secs(60),
    max_fuel: 2_000_000_000,
    allowed_read_paths: vec!["./data".into()],
    network_policy: NetworkPolicy {
        default_deny: true,
        allow_list: vec!["api.example.com".into()],
        deny_list: vec![],
    },
    ..Default::default()
};

let mut sandbox = WasmSandbox::new(&wasm_bytes, config)?;

// Executar função exportada
let result = sandbox.call_function("process", &[serde_json::json!(42)])?;
println!("Retorno: {:?}, Fuel: {}", result.return_value, result.fuel_consumed);

// Snapshot para retomada posterior
let snapshot = sandbox.snapshot(vec!["checkpoint-1".into()])?;

// Restaurar de um snapshot (valida hash do módulo)
let restored = WasmSandbox::restore(&snapshot, &wasm_bytes, SandboxConfig::default())?;
```

### Snapshot/Resume

O snapshot captura a memória linear completa e todos os globals exportados mutáveis. A restauração valida o hash SHA-256 do módulo WASM antes de aplicar o estado, garantindo que apenas o mesmo binário possa ser restaurado.

```rust
// Validação offline
assert!(WasmSandbox::validate_snapshot(&snapshot, &wasm_bytes));
```

> **Nota:** A execução resumida de funções após restore requer suporte async do wasmtime (planejado para Phase 5). Atualmente, o restore recupera memória e globals corretamente, mas chamadas subsequentes podem falhar devido a estado interno do wasmtime.

### Cancelamento gracioso

```rust
use tokio_util::sync::CancellationToken;

let token = CancellationToken::new();
let sandbox = WasmSandbox::new_with_cancel(&wasm_bytes, config, token.clone())?;

// Em outra task:
token.cancel();

// Chamadas subsequentes retornam SandboxError::Shutdown
assert!(sandbox.is_shutdown_requested());
```

---

## docker_sandbox

Containers Docker com gerenciamento completo de ciclo de vida via `bollard`: pull de imagens, limites de CPU/memória, volumes gerenciados, redes customizadas, health checks com auto-restart, streaming de logs e cleanup automático no drop.

### Tipos principais

- **`DockerSandbox`** — sandbox assíncrona com `start`, `exec`, `stop`, `destroy`
- **`DockerConfig`** — imagem, limites, rede, volumes, health check, pull policy
- **`ExecResult`** — exit code, stdout, stderr
- **`ContainerStatus`** — Created, Running, Paused, Restarting, Exited, Dead, NotFound
- **`ImagePullPolicy`** — Always, IfNotPresent, Never
- **`NetworkMode`** — None, Bridge, Host, Custom(nome)
- **`HealthCheckConfig`** — comando, intervalo, período inicial, retries, auto-restart
- **`DockerError`** — DaemonUnavailable, CreationFailed, OomKilled, Timeout, etc.

### Exemplo de uso

```rust
use zylos_sandbox::docker_sandbox::*;
use futures_util::StreamExt;

let config = DockerConfig {
    image: "python:3.12-slim".into(),
    cpu_limit: 1.0,
    memory_limit: 512 * 1024 * 1024,
    network_mode: NetworkMode::None,
    managed_volumes: vec![ManagedVolume {
        name: "agent-data".into(),
        container_path: "/data".into(),
        read_only: false,
    }],
    health_check: Some(HealthCheckConfig {
        command: vec!["python".into(), "-c".into(), "print('ok')".into()],
        interval_secs: 10,
        start_period_secs: 5,
        retries: 3,
        auto_restart: true,
    }),
    ..Default::default()
};

let sandbox = DockerSandbox::new(config)?;
sandbox.start().await?;

// Executar comando
let result = sandbox.exec(&["python", "-c", "print('hello')"]).await?;
assert_eq!(result.exit_code, 0);

// Stream de logs
let mut stream = sandbox.logs(true).await?;
while let Some(entry) = stream.next().await {
    let entry = entry?;
    println!("[{:?}] {}", entry.stream, entry.data);
}

// Cleanup completo (container + volumes + rede)
sandbox.destroy().await?;
```

### Health check loop

```rust
// Roda em background; reinicia automaticamente se unhealthy
tokio::spawn(async move {
    sandbox.run_health_check_loop().await.ok();
});
```

### Cleanup automático

O `Drop` do `DockerSandbox` agenda remoção assíncrona de container, volumes gerenciados e redes customizadas caso o runtime Tokio esteja disponível.

---

## browser_antidetect

Geração determinística de fingerprints de navegador e patches JavaScript stealth para evadir detecção de bot (CreepJS, FingerprintJS). Não depende de nenhum navegador externo — gera apenas configuração e scripts.

### Tipos principais

- **`BrowserSandbox`** — perfil + config anti-detect combinados
- **`BrowserProfile`** — user-agent, viewport, timezone, locale, canvas/webgl/audio hashes, hardware concurrency, device memory
- **`AntiDetectConfig`** — rotação de fingerprint, pool de proxies, modo headless, stealth scripts
- **`HeadlessMode`** — Full (`--headless`), New (`--headless=new`), Disabled

### Exemplo de uso

```rust
use zylos_sandbox::{BrowserSandbox, HeadlessMode};

// Perfil determinístico: mesma seed = mesmo perfil
let mut sandbox = BrowserSandbox::new(42);

// Patches JS para injetar antes do carregamento da página
let patches = sandbox.apply_stealth_patches();
for patch in &patches {
    // Injetar via CDP Runtime.evaluate ou similar
    println!("{}", patch);
}

// Score de risco de detecção [0.0, 1.0]
let risk = sandbox.detect_bot_risk();
println!("Bot risk: {:.2}", risk);

// Rotacionar identidade
sandbox.rotate_identity(99);

// Proxy round-robin
sandbox.config.proxy_pool = vec![
    "http://proxy1:8080".into(),
    "http://proxy2:8080".into(),
];
let proxy = sandbox.next_proxy(); // Some("http://proxy1:8080")
```

### Patches stealth aplicados

1. Override de `navigator.webdriver` → `false`
2. Properties de navigator (hardwareConcurrency, deviceMemory, platform, languages)
3. Ruído em Canvas `toDataURL`
4. Override de WebGL vendor/renderer
5. Estabilização de AudioContext
6. Micro-variação em `getBoundingClientRect`
7. Normalização de plugins/mimeTypes
8. Stub de `window.chrome.runtime`

---

## browser_profile_store

Armazenamento persistente de perfis Chromium por agente/equipe. Cada perfil mantém cookies, localStorage e estado de login entre sessões. Metadados indexados por owner e tags.

### Tipos principais

- **`BrowserProfileStore`** — store em memória com backing filesystem
- **`PersistedProfile`** — id, nome, owner, data_dir, fingerprint, seed, contadores de sessão, cookie hash
- **`ProfileStoreConfig`** — base_path, max_profiles, auto_cleanup, stale_after_days
- **`ProfileStoreError`** — NotFound, MaxProfilesReached, LockPoisoned, Io

### Exemplo de uso

```rust
use zylos_sandbox::browser_profile_store::*;

let config = ProfileStoreConfig {
    base_path: "./browser-profiles".into(),
    max_profiles: 50,
    stale_after_days: 30,
    ..Default::default()
};
let store = BrowserProfileStore::new(config);

// Criar ou recuperar perfil existente
let profile = store.get_or_create_profile(
    "researcher".into(),
    "agent-1".into(),
    42, // seed determinística
    vec!["research".into()],
)?;

// Registrar uso em sessão
store.record_session(&profile.id)?;

// Listar perfis por owner
let profiles = store.list_profiles_for_owner("agent-1")?;

// Encontrar perfis obsoletos
let stale = store.find_stale_profiles()?;

// Hash do banco de cookies para detecção de mudanças
let hash = BrowserProfileStore::compute_cookie_hash(&profile.data_dir)?;
```

### Integração com DockerSandbox

Monte o diretório do perfil como volume nomeado no container:

```rust
let config = DockerConfig {
    volumes: vec![VolumeMount {
        host: profile.data_dir.to_string_lossy().into(),
        container: "/root/.config/chromium".into(),
        read_only: false,
    }],
    ..Default::default()
};
```

---

## secure_input (Ghost Typist)

Pipeline de injeção de credenciais onde secrets **nunca** aparecem no contexto do LLM, transcripts de ferramentas ou logs de trace. Credenciais fluem de um vault criptografado diretamente para o stdin do sandbox/browser, bypassando todas as camadas de raciocínio do agente.

### Arquitetura

```
UI mascarada (Leptos) → SecureInputPipeline → WASM/Docker stdin
                              ↑
                     CredentialVault (encrypted at rest)
```

O LLM vê apenas placeholders como `[CRED:github_token:***]`.

### Tipos principais

- **`SecureInputPipeline`** — pipeline de alto nível com métodos `inject_as_typing`, `inject_as_env_var`, `inject_via_cdp`
- **`CredentialVault`** — store com rate limiting, ownership, expiry, audit trail
- **`SecureCredential`** — label, kind, encrypted_payload, fingerprint, owner, expiry, use_count
- **`CredentialKind`** — KeyboardTyping, ClipboardPaste, EnvironmentVariable, FileContent, HttpHeader, BrowserCdpEval
- **`InputMask`** — placeholder seguro para contexto do LLM
- **`InjectionResult`** — success, mask, audit_id, timestamp, error (safe)
- **`GhostTypistError`** — NotFound, Expired, AccessDenied, RateLimited, TargetUnavailable

### Exemplo de uso

```rust
use zylos_sandbox::secure_input::*;

let pipeline = SecureInputPipeline::new(SecureInputConfig {
    max_uses_per_window: 100,
    rate_limit_window_secs: 3600,
    enforce_ownership: true,
    max_lifetime_days: 90,
    audit_logging: true,
});

// Armazenar credencial (payload já criptografado)
let (cred_id, fingerprint) = pipeline.vault().store_credential(
    "GitHub Token".into(),
    CredentialKind::EnvironmentVariable,
    encrypted_bytes,
    "agent-1".into(),
    vec!["github".into()],
    None, // sem expiração
)?;

// Injetar como variável de ambiente — o LLM nunca vê o valor
let result = pipeline.inject_as_env_var(&cred_id, "agent-1", "GITHUB_TOKEN")?;
assert!(result.success);
println!("{}", result.mask.placeholder); // [CRED:github_token:***]

// Injeção via CDP isolated world (sem interpolação direta)
let result = pipeline.inject_via_cdp(&cred_id, "agent-1", "setPasswordField")?;

// Audit trail
let log = pipeline.vault().get_audit_log(10)?;
```

### Garantias de segurança

- **Ownership**: apenas o owner pode injetar/deletar uma credencial
- **Rate limiting**: máximo de usos por janela configurável
- **Expiry**: credenciais expiradas são rejeitadas automaticamente
- **Audit**: todas as injeções (sucesso/falha) registradas com caller, target e timestamp
- **CDP safety**: templates com `${secret}` ou `{secret}` são rejeitados — usar argument binding
- **Masking**: o `InjectionResult` retornado ao LLM contém apenas o placeholder, nunca o secret

---

## screen_share

Sessões de compartilhamento de tela por workspace usando TigerVNC + noVNC. Senhas VNC são geradas internamente e nunca expostas. Tokens de acesso one-time. Auto-disconnect por idle timeout.

### Tipos principais

- **`ScreenShareManager`** — gerenciador assíncrono de sessões
- **`ScreenShareConfig`** — idle_timeout, vnc_port_base, resolução, color depth
- **`SessionInfo`** — session_id, workspace_id, sandbox_type, state, portas, timestamps
- **`SessionState`** — Starting, Active, Idle, Stopped
- **`SandboxType`** — Docker, Wasm
- **`ScreenShareError`** — SessionNotFound, AlreadyActive, ServerStartFailed, IdleTimeout, InvalidToken

### Exemplo de uso

```rust
use zylos_sandbox::screen_share::*;

let manager = ScreenShareManager::new(ScreenShareConfig {
    idle_timeout_secs: 1800, // 30 min
    vnc_port_base: 5900,
    display_width: 1920,
    display_height: 1080,
    color_depth: 24,
});

// Iniciar sessão — retorna info + token one-time
let (info, token) = manager.start_session("ws-agent-1", SandboxType::Docker).await?;
println!("VNC port: {}, noVNC: {}", info.vnc_port, info.novnc_port);

// Validar token (one-time: segunda chamada falha)
let session = manager.validate_token("ws-agent-1", &token).await?;

// Registrar atividade (reseta timer de idle)
manager.record_activity("ws-agent-1").await?;

// Listar sessões ativas
let sessions = manager.list_sessions().await;

// Cleanup de sessões ociosas
let timed_out = manager.cleanup_idle_sessions().await;

// Parar sessão
manager.stop_session("ws-agent-1").await?;
```

---

## Configuração

### Dependências

```toml
[dependencies]
zylos-sandbox = { path = "../zylos-sandbox" }
```

Dependências transitivas principais: `wasmtime 25.0`, `bollard 0.17`, `tokio`, `serde`, `sha2`, `chrono`, `uuid`.

### Feature flags

Atualmente não há feature flags opcionais. Todos os módulos são compilados juntos.

### Requisitos de sistema

| Módulo | Requisito |
|--------|-----------|
| `wasm_sandbox` | Nenhum (wasmtime é embutido) |
| `docker_sandbox` | Docker daemon acessível via socket local |
| `browser_antidetect` | Nenhum (gera apenas config/scripts) |
| `browser_profile_store` | Permissões de escrita no base_path |
| `secure_input` | Backend de criptografia em produção (OS keychain, Vault, etc.) |
| `screen_share` | TigerVNC server instalado, portas disponíveis a partir de vnc_port_base |

---

## Segurança

### Modelo de ameaças

| Ameaça | Mitigação |
|--------|-----------|
| Escape de sandbox WASM | Memória linear isolada, fuel limits, WasiCtx com dirs pré-abertos |
| Container breakout | NetworkMode::None padrão, CPU/memory limits, auto-remove |
| Detecção de bot | Stealth patches, fingerprint rotation, proxy pool |
| Leak de credenciais via LLM | Ghost Typist: secrets nunca entram no contexto; apenas masks |
| VNC não autorizado | Tokens one-time, senhas VNC internas, idle timeout |
| Replay attack em snapshots | Validação SHA-256 do módulo WASM no restore |
| Credential stuffing | Rate limiting por credencial, ownership enforcement, expiry |

### Notas de produção

- **`CredentialVault`**: esta implementação usa armazenamento em memória. Em produção, substitua por OS keychain, HashiCorp Vault ou AWS Secrets Manager.
- **`BrowserProfileStore`**: metadados em memória. Persistência via SQLite está planejada usando `zylos-memory-system`.
- **`screen_share`**: os spawners VNC atuais são placeholders. Em produção, integre com Xvfb + x11vnc dentro do container.
- **`wasm_sandbox`**: resume completo de execução requer wasmtime async (Phase 5).

---

## Testes

### Testes unitários

Todos os módulos possuem testes unitários inline (`#[cfg(test)]`). Execute:

```bash
cargo test -p zylos-sandbox
```

Cobertura por módulo:

| Módulo | Testes | Destaques |
|--------|--------|-----------|
| `wasm_sandbox` | 12 | Criação, snapshot roundtrip, resource limits, network policy, fingerprint, cancellation |
| `docker_sandbox` | 7 | Config validation, status noop, stop/destroy noop + 4 integration tests (ignored) |
| `browser_antidetect` | 11 | Determinismo, stealth patches, risk scoring, identity rotation, proxy round-robin |
| `browser_profile_store` | 9 | CRUD, owner isolation, max profiles, tag filtering, sanitize, cookie hash |
| `secure_input` | 10 | Store/inject, ownership, expiry, rate limit, masking, audit, CDP safety |
| `screen_share` | 11 | Start/stop, duplicate rejection, token validation, idle cleanup, serialization |

### Testes de integração (Docker)

Testes marcados com `#[ignore]` requerem Docker daemon ativo:

```bash
cargo test -p zylos-sandbox -- --ignored
```

Incluem: lifecycle completo, log streaming, volume cleanup, graceful shutdown.

### Benchmarks

```bash
cargo bench -p zylos-sandbox
```

Benchmark Criterion definido em `benches/sandbox.rs`.

---

## Licença

Mesma licença do workspace Zylos. Consulte `LICENSE` na raiz do repositório.