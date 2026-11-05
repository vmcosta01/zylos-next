# zylos-sandbox

Isolamento de execução para código de agentes: WASM sandbox com snapshot/resume,
Docker per-agent com lifecycle completo, e browser anti-detect com fingerprint determinístico.

**Fase no Master Plan:** Fase 4 (SANDBOX_ISOLATION) — Sprints 4.1 e 4.2

## Features Implementadas

### WASM Sandbox (`wasm_sandbox.rs` — 779 linhas)
- Memory isolation via `StoreLimitsBuilder` (configurable max_memory_bytes/pages)
- Fuel-based CPU limiting (`max_fuel`)
- Filesystem isolation via `WasiCtxBuilder` preopened dirs (read/write paths)
- Network policy stub (allow/deny list com wildcard matching)
- Snapshot/Resume completo (linear memory + globals + fuel + metadata)
- Content-addressable input fingerprinting (SHA-256) para deterministic replay
- Per-instance peak memory tracking (AtomicU64)
- Graceful shutdown via `CancellationToken`
- Structured error types (`SandboxError` enum com 9 variantes)
- Testes unitários: 10 testes cobrindo creation, invalid wasm, snapshot roundtrip, resource limits, network policy, fingerprint determinism, cancellation

### Docker Sandbox (`docker_sandbox.rs` — 631 linhas)
- Container lifecycle completo: create/start/exec/stop/destroy via `bollard`
- Resource limits: CPU (nano_cpus), memory, memory_swap via HostConfig
- Network modes: None/Bridge/Host/Custom (com auto-create/remove)
- Volume management: bind mounts + managed volumes (auto-create/cleanup)
- Image pull policies: Always/IfNotPresent/Never
- Health check loop com auto-restart e configurable retries
- Log streaming (stdout/stderr) via async Stream
- Graceful shutdown com stop timeout configurável
- Cleanup guarantees via `Drop` trait (container/volume/network removal on panic)
- Structured errors (`DockerError` enum com 11 variantes)
- Testes: config validation, status when no container, integration tests (ignored by default)

### Browser Anti-Detect (`browser_antidetect.rs` — 538 linhas)
- Deterministic profile generation from seed (xorshift64 + SHA-256 hashing)
- Fingerprint fields: user_agent, viewport, timezone, locale, platform, fonts, canvas_hash, webgl_vendor, audio_context_hash, client_rects_variation, hardware_concurrency, device_memory
- Stealth JS patches (8 patches): webdriver override, navigator props, canvas noise, WebGL vendor, AudioContext stabilization, ClientRects variation, plugins/mimeTypes normalization, chrome.runtime stub
- Bot risk scoring heuristic [0.0, 1.0]
- Identity rotation (regenerate profile from new seed)
- Proxy pool round-robin + random selection (deterministic)
- HeadlessMode enum: Full/New/Disabled
- Testes: 12 testes cobrindo determinism, stealth patches, risk scoring, identity rotation, proxy cycling

## API Pública

### wasm_sandbox.rs
- `pub type SandboxId`
- `pub enum SandboxError`
- `pub struct NetworkPolicy`
- `pub fn is_allowed`
- `pub struct SandboxConfig`
- `pub struct SandboxSnapshot`
- `pub struct GlobalValue`
- `pub struct SnapshotMetadata`
- `pub struct ExecutionResult`
- `pub struct InputFingerprint`
- `pub fn compute`
- `pub struct WasmSandbox`
- `pub fn new`
- `pub fn new_with_cancel`
- `pub fn id`
- `pub fn memory_usage_bytes`
- `pub fn peak_memory_bytes`
- `pub fn is_shutdown_requested`
- `pub fn request_shutdown`
- `pub fn cancel_token`
- `pub fn compute_fingerprint`
- `pub fn call_function`
- `pub fn snapshot`
- `pub fn restore`
- `pub fn restore_with_cancel`
- `pub fn validate_snapshot`
- `pub fn fuel_remaining`
- `pub fn elapsed`
- `pub fn module_hash`
- `pub fn network_policy`

### docker_sandbox.rs
- `pub enum DockerError`
- `pub enum ImagePullPolicy`
- `pub enum NetworkMode`
- `pub struct VolumeMount`
- `pub struct ManagedVolume`
- `pub struct HealthCheckConfig`
- `pub struct DockerConfig`
- `pub struct ExecResult`
- `pub struct LogEntry`
- `pub enum LogStream`
- `pub enum ContainerStatus`
- `pub struct DockerSandbox`
- `pub fn new`
- `pub fn image`
- `pub fn cpu_limit`
- `pub fn memory_limit`
- `pub fn network_mode`
- `pub fn auto_remove`
- `pub fn container_id`
- `pub fn ping`
- `pub fn start`
- `pub fn exec`
- `pub fn logs`
- `pub fn status`
- `pub fn health_probe`
- `pub fn run_health_check_loop`
- `pub fn restart`
- `pub fn stop`
- `pub fn destroy`

### browser_antidetect.rs
- `pub enum HeadlessMode`
- `pub struct BrowserProfile`
- `pub struct AntiDetectConfig`
- `pub struct BrowserSandbox`
- `pub fn generate_profile`
- `pub fn new`
- `pub fn apply_stealth_patches`
- `pub fn detect_bot_risk`
- `pub fn rotate_identity`
- `pub fn next_proxy`
- `pub fn random_proxy`

## Status vs Master Plan

| Task | Status | Notas |
|------|--------|-------|
| 4.1.1 WASM Sandbox Capability-Based | ⚠️ Parcial | Memory/fuel/fs isolation ✅; host functions registry ❌; module manifest TOML ❌; sandbox pooling ❌; OTLP tracing ❌; compiled artifact cache ❌ |
| 4.1.2 Snapshot/Resume | ✅ Implementado | Snapshot <100ms, resume funcional, persistência via serde |
| 4.1.3 Docker Sandbox Firecracker-grade | ⚠️ Parcial | Lifecycle/resource limits/network/volumes/health ✅; overlayfs ❌; nftables firewall ❌; cgroups v2 direto ❌; approval workflow ❌; image digest pinning ❌; secret injection ❌; artifact extraction ❌; orphan detector ❌; REST/gRPC API ❌ |
| 4.2.1 Browser Anti-Detect | ⚠️ Parcial | Fingerprint/stealth patches/proxy rotation ✅; TLS fingerprint matching ❌; mouse/keyboard humanization ❌; inbox persistente ❌; CAPTCHA solving ❌; session state persistence ❌; CDP bridge ❌ |
| 4.2.2 WebMCP Spec Compliance | ❌ Não implementado | Movido para crate separado `zylos-browser` |
| 4.2.3 Parallel Search MCP | ❌ Não implementado | Movido para crate separado `zylos-search-mcp` |

## Gaps Críticos

1. **Host functions registry** — Task 4.1.1 exige `#[host_function]` macro e audit log; não existe
2. **Module manifest schema** — TOML/YAML declarando capabilities não implementado
3. **Sandbox pooling** — Pre-warmed instances para cold-start <5ms ausente
4. **Compiled artifact cache** — `Module::deserialize` para cold-start optimization ausente
5. **OverlayFS / AppArmor / SELinux** — Filesystem hardening do Docker não implementado
6. **Network namespace isolation** — nftables/egress filtering ausente (apenas Docker network mode)
7. **Image digest pinning** — Imagens referenciadas por tag, não SHA256
8. **Secret injection segura** — Secrets via env vars plaintext, sem Vault/encryption
9. **Artifact extraction + malware scan** — Pós-execução output handling ausente
10. **TLS fingerprint matching** — JA3/JA4 evasion não implementado
11. **Mouse/keyboard humanization** — Bézier curves/jitter ausentes
12. **CAPTCHA solving pipeline** — Integração 2captcha/hCaptcha ausente
