# Fase 4: Sandbox Isolation — Runbook Zylos-Next

## Visão Geral

O SANDBOX_ISOLATION system (`zylos-sandbox`) fornece ambientes de execução isolados para código de agentes com quatro pilares: WASM sandbox via wasmtime com snapshot/resume, Docker isolation per-agent, browser anti-detect com fingerprint rotation, e secure input pipeline com credential vaulting.

**Módulos principais** (de `crates/zylos-sandbox/src/lib.rs`):
- `wasm_sandbox` — WasmSandbox com ExecutionResult, SandboxConfig, SandboxSnapshot, SnapshotMetadata, NetworkPolicy, InputFingerprint, GlobalValue, SandboxId, SandboxError
- `docker_sandbox` — Docker per-agent container lifecycle (Phase 4.2)
- `browser_antidetect` — BrowserSandbox com AntiDetectConfig, BrowserProfile, HeadlessMode
- `browser_profile_store` — BrowserProfileStore com PersistedProfile, ProfileStoreConfig, ProfileStoreError
- `secure_input` — SecureInputPipeline com SecureCredential, CredentialVault, InputMask, GhostTypistError

**Crates relacionados:**
- `zylos-browser` — WebMCP integration (`webmcp.rs`, `webmcp_eval.rs`), headless browsing (`headless.rs`), fingerprint engine (`fingerprint.rs`), browser context (`context.rs`)
- `zylos-docker` — Container management (`container.rs`), approval gates (`approval.rs`)
- `zylos-vlm-ocr` — OCR sidecar via dots.mocr (`client.rs`, `svg.rs`, `types.rs`)
- `zylos-search-mcp` — Search engine com fan-out paralelo (`engine.rs`, `provider.rs`)

**Critério de Saída:** Browser anti-detect passa fingerprint test; WASM snapshot/resume funcional; OCR Elo >1100; WasmSandbox 5/5 testes, DockerSandbox 8/8 testes, SearchMCP 8/8 testes.

## Pré-requisitos

- Fases 0-3 completas
- Dependências: `wasmtime 25.0`, `bollard` (Docker async), `serde`, `tokio`, `sha2`, `hex`
- Docker Engine instalado e acessível (para docker_sandbox)
- Chromium/Chrome instalado (para browser_antidetect)
- Feature flags: `docker` (para Docker sandbox), `browser` (para browser anti-detect)

### Setup

```bash
# Verificar Docker daemon
docker info > /dev/null 2>&1 && echo "✅ Docker OK" || echo "❌ Docker não está rodando"

# Build com todas as features
cargo build -p zylos-sandbox --features docker,browser
cargo build -p zylos-browser
cargo build -p zylos-docker
cargo build -p zylos-search-mcp

# Testes
cargo test -p zylos-sandbox --lib   # Esperado: 13 passed (Wasm 5 + Docker 8)
cargo test -p zylos-browser
cargo test -p zylos-docker
cargo test -p zylos-search-mcp --lib  # Esperado: 8 passed

# Benchmarks
cargo bench -p zylos-sandbox -- wasm_cold_start
cargo bench -p zylos-sandbox -- docker_spawn
cargo bench -p zylos-search-mcp -- fanout_5_providers
```

## Para Dev Senior

### Decisões Arquiteturais

1. **WASM Snapshot/Resume via wasmtime 25.0**: `WasmSandbox` encapsula execução isolada com memória sandboxed. `SandboxConfig` define `memory_limit`, `fuel_limit`, `allow_network`, `allow_fs`. Fuel exhaustion previne loops infinitos. Memory limit enforceado em runtime. Snapshot como blob serializado é portável entre hosts e versionável em git (ADR-042). Cold start target <10ms.

2. **Docker Per-Agent Isolation via bollard**: Async nativo Tokio sem dependência de CLI externo. `DockerSandbox` com `DockerConfig` (image, cpu_limit, memory_limit, network_mode, auto_remove, volumes). Auto-remove funciona mesmo com SIGKILL no processo pai — containers órfãos são impossíveis. cgroups enforceam CPU/RAM limits. Spawn target <500ms.

3. **Browser Anti-Detect Engine**: `BrowserSandbox` cria perfis de browser com fingerprint único por agente. `AntiDetectConfig` controla canvas noise, WebGL renderer, timezone, language, user-agent. `BrowserProfileStore` persiste perfis em disco para reuso entre sessões. `HeadlessMode` suporta headless e headed execution.

4. **Secure Input Pipeline**: `SecureInputPipeline` previne credential leakage via `SecureCredential` (never logged, masked in output). `CredentialVault` armazena secrets encrypted at rest. `InputMask` define padrões de masking. `GhostTypistError` para falhas na simulação de digitação humana.

5. **SearchMCP Fan-Out Paralelo**: `SearchEngine` com `ProviderConfig` envia queries a múltiplos providers via `JoinSet`. Timeout individual por provider. DLQ para requisições falhas (retry assíncrono, não bloqueia resultado final). Credenciais nunca aparecem em logs/resultados serializados. Merge target <50ms para 100 resultados.

6. **WebMCP Integration**: `zylos-browser/webmcp.rs` implementa MCP server para browser automation. `webmcp_eval.rs` contém evaluation suite. Tools: navigate, click, fill, screenshot, extract_text.

7. **OCR Sidecar**: `zylos-vlm-ocr` integra com dots.mocr 3B via vLLM para parsing documentos/gráficos → SVG estruturado. Elo target >1100.

### Extension Points

- **Novos sandbox backends**: Implemente trait comum seguindo padrão de `WasmSandbox` (ex: gVisor, Firecracker)
- **Custom network policies**: Estenda `NetworkPolicy` para regras port-level/protocol-level
- **Novos browser fingerprints**: Adicione detectores em `zylos-browser/fingerprint.rs`
- **Custom credential types**: Estenda `SecureCredential` para SSH keys, certificates
- **Novos search providers**: Implemente provider trait em `zylos-search-mcp/provider.rs`
- **OCR model swapping**: Troque backend em `zylos-vlm-ocr/client.rs`

### Performance Tuning

- WASM cold start <10ms, call <5ms para módulo simples
- Docker spawn <500ms, exec <100ms após spawn
- Browser profile load <500ms
- Snapshot size <10MB para agentes típicos
- Search fan-out com 5 providers <2s (limitado pelo mais lento + timeout)
- Use `wasmtime` pooling allocator para reduzir overhead de instanciação

## Para Dev Pleno

### WASM Sandbox Execution

```rust
use zylos_sandbox::{WasmSandbox, SandboxConfig};

let config = SandboxConfig {
    memory_limit: 64 * 1024 * 1024, // 64MB
    fuel_limit: 1_000_000,
    allow_network: false,
    allow_fs: false,
};
let sandbox = WasmSandbox::new(config)?;

// Execute WASM module
let result = sandbox.call_function(wasm_bytes, "process_results", &[input.into()])?;
assert!(result.len() < 1024 * 1024); // <1MB output

// Tentativa de escape deve falhar deterministicamente
let malicious = include_bytes!("../tests/fixtures/malicious_escape.wasm");
let escape_result = sandbox.call_function(malicious, "try_escape", &[]);
assert!(matches!(escape_result, Err(SandboxError::SecurityViolation(_))));
```

### Docker Container Lifecycle

```rust
use zylos_sandbox::{DockerSandbox, DockerConfig, NetworkMode};

let config = DockerConfig {
    image: "python:3.12-slim".to_string(),
    cpu_limit: 0.25,
    memory_limit: 128 * 1024 * 1024, // 128MB
    network_mode: NetworkMode::Bridge,
    auto_remove: true,
    ..Default::default()
};
let sandbox = DockerSandbox::new(config).await?;
let container_id = sandbox.spawn().await?;

// Script que tenta alocar 256MB deve ser OOM-killed
let result = sandbox.exec("python -c 'x = bytearray(256*1024*1024)'").await;
assert!(matches!(result, Err(DockerError::OomKilled)));

// Drop garante cleanup mesmo com crash
drop(sandbox);
tokio::time::sleep(Duration::from_secs(2)).await;
assert!(!container_exists(&container_id).await);
```

### Browser Anti-Detect

```rust
use zylos_sandbox::{BrowserSandbox, AntiDetectConfig, HeadlessMode};

let config = AntiDetectConfig {
    canvas_noise: true,
    webgl_renderer: "Intel Iris Xe".into(),
    timezone: "America/Sao_Paulo".into(),
    language: "pt-BR".into(),
    ..Default::default()
};

let browser = BrowserSandbox::launch(config, HeadlessMode::Headless).await?;
browser.navigate("https://example.com").await?;
let text = browser.extract_text("body").await?;
browser.save_profile("agent_001_profile").await?;
```

### SearchMCP Fan-Out

```rust
use zylos_search_mcp::{SearchEngine, ProviderConfig, SearchOptions};

let engine = SearchEngine::new(vec![
    ProviderConfig { name: "brave", api_key: env::var("BRAVE_KEY")? },
    ProviderConfig { name: "tavily", api_key: env::var("TAVILY_KEY")? },
]);

let results = engine.search("Rust async patterns", SearchOptions::default()).await?;

// Verify credentials never leak
for result in &results {
    assert!(!result.metadata.contains_key("api_key"));
    assert!(!format!("{:?}", result).contains("sk-"));
}
```

### Secure Input Pipeline

```rust
use zylos_sandbox::{SecureInputPipeline, SecureCredential, InputMask};

let pipeline = SecureInputPipeline::new();
let cred = SecureCredential::from_env("API_KEY")?;
pipeline.register_credential("api_key", cred);

let mask = InputMask::pattern(r"Bearer\s+\S+", "Bearer [REDACTED]");
let safe_output = pipeline.sanitize(raw_output, &[mask]);
```

## Para Dev Junior

### Passo a Passo: Executando Código em Sandbox

1. **Verifique pré-requisitos**: `docker info` deve retornar OK; `rustc --version` >= 1.75
2. **Escolha o tipo de sandbox**: WASM para código leve/determinístico, Docker para código complexo/com deps, Browser para web scraping
3. **Configure limites**: Sempre defina memory_limit e network policy antes de executar
4. **Compile e teste**: `cargo build -p zylos-sandbox && cargo test -p zylos-sandbox --lib`
5. **Valide outputs**: Sempre passe output por SecureInputPipeline antes de logar ou retornar

### Pitfalls Comuns

- **Nunca execute código não confiável sem sandbox**: Mesmo "scripts simples" podem escapar
- **Não ignore NetworkPolicy**: Default deve ser DenyAll; allowlist explicitamente
- **Não reutilize browser profiles entre agentes diferentes**: Cada agente precisa de fingerprint único
- **Não logue credentials**: Use SecureCredential que mascara automaticamente
- **Não assuma que WASM é stateless**: Snapshots preservam estado; reset explicitamente se necessário
- **Não esqueça cleanup**: Containers têm auto_remove, mas browsers devem ser fechados explicitamente

### Debugging Tips

- WASM execution errors incluem stack trace do guest module
- Docker container logs acessíveis via `sandbox.logs()`
- Browser sandbox logs incluem fingerprint hash para correlacionar sessões
- Use `RUST_LOG=zylos_sandbox=debug,zylos_browser=trace` para verbose
- Snapshot metadata inclui checksum para validar integridade

## Troubleshooting

| Erro | Causa | Recuperação |
|------|-------|-------------|
| `SandboxError::SecurityViolation` | WASM tentou acesso proibido (fs/net/memory escape) | Comportamento esperado; sandbox funcionou corretamente |
| `SandboxError::MemoryLimit` | WASM excedeu memory_limit | Aumente limite ou otimize código guest |
| `SandboxError::FuelExhausted` | Loop infinito ou computação excessiva | Aumente fuel_limit ou otimize algoritmo |
| `DockerError::OomKilled` | Container excedeu memory_limit | Comportamento esperado; cgroups funcionou |
| `DockerError::SpawnFailed` | Image não encontrada ou daemon down | `docker pull` image, verifique Docker daemon |
| `BrowserProfileStore::ProfileNotFound` | Perfil não existe em disco | Crie novo perfil ou restaure de backup |
| `GhostTypistError::TimingViolation` | Digitação rápida demais (bot detection) | Ajuste delay parameters no AntiDetectConfig |
| `SecureCredential::EnvMissing` | Variável de ambiente não definida | Defina variável ou use fallback seguro |
| SearchMCP provider timeout | Provider lento ou offline | DLQ captura falha; outros providers ainda retornam resultados |
| Container órfão persiste | auto_remove desabilitado ou bug | Habilite auto_remove; reporte ao Senior se persistir |

## Referências

- **Source**: `crates/zylos-sandbox/src/` (5 módulos + benches)
- **Related crates**: `crates/zylos-browser/src/` (6 módulos), `crates/zylos-docker/src/` (4 módulos), `crates/zylos-vlm-ocr/src/` (5 módulos), `crates/zylos-search-mcp/src/` (4 módulos)
- **Key files**: `zylos-sandbox/src/lib.rs:1-30`, `wasm_sandbox.rs`, `docker_sandbox.rs`, `browser_antidetect.rs`, `secure_input.rs`, `zylos-browser/src/fingerprint.rs`, `zylos-browser/src/webmcp.rs`
- **Benchmarks**: `crates/zylos-sandbox/benches/sandbox.rs`
- **ADRs**: ADR-042 (Wasmtime over Wasmer), ADR-043 (Docker for Full OS Sandboxing)
- **Master Plan**: Linhas 194-197 (Fase 4 SANDBOX_ISOLATION overview)