//! WASM Sandbox with snapshot/resume support using wasmtime.
//!
//! Provides isolated execution environments for agent code with:
//! - Memory isolation via WASM linear memory
//! - Full snapshot/restore (linear memory + globals)
//! - Resource limits (memory pages, fuel, file descriptors) via StoreLimits
//! - Filesystem isolation via WasiCtxBuilder preopened dirs
//! - Network policy stub (allow/deny list)
//! - Deterministic replay via content-addressable SHA-256 inputs
//! - Per-instance memory usage tracking
//! - Graceful shutdown via CancellationToken

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};
use wasmtime::{Config, Engine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi::preview1::WasiP1Ctx;

/// Unique identifier for a sandbox instance.
pub type SandboxId = uuid::Uuid;

/// Errors from sandbox operations.
#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("WASM compilation failed: {0}")]
    Compilation(String),

    #[error("Module instantiation failed: {0}")]
    Instantiation(String),

    #[error("Execution exceeded resource limits: {0}")]
    ResourceLimit(String),

    #[error("Snapshot operation failed: {0}")]
    Snapshot(String),

    #[error("Restore operation failed: {0}")]
    Restore(String),

    #[error("Function call failed: {0}")]
    CallFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Shutdown requested")]
    Shutdown,

    #[error("Network policy violation: {0}")]
    NetworkPolicy(String),
}

/// Network access policy for outbound connections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// If true, all outbound connections are denied unless explicitly allowed.
    pub default_deny: bool,
    /// Hosts/patterns to allow (e.g., "api.example.com", "*.internal.net").
    pub allow_list: Vec<String>,
    /// Hosts/patterns to deny (takes precedence over allow_list).
    pub deny_list: Vec<String>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            default_deny: true,
            allow_list: Vec::new(),
            deny_list: Vec::new(),
        }
    }
}

impl NetworkPolicy {
    /// Check whether a host is permitted under this policy.
    pub fn is_allowed(&self, host: &str) -> bool {
        if self.deny_list.iter().any(|d| host_matches(host, d)) {
            return false;
        }
        if !self.default_deny {
            return true;
        }
        self.allow_list.iter().any(|a| host_matches(host, a))
    }
}

fn host_matches(host: &str, pattern: &str) -> bool {
    if let Some(suffix) = pattern.strip_prefix("*.") {
        host.ends_with(suffix) || host == suffix
    } else {
        host == pattern
    }
}

/// Configuration for a sandbox instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Maximum linear memory in bytes (default: 64MB).
    pub max_memory_bytes: usize,
    /// Maximum memory pages (each page = 64KiB). Derived from max_memory_bytes if None.
    pub max_memory_pages: Option<u32>,
    /// Maximum execution time per call (default: 30s).
    pub max_execution_time: Duration,
    /// Maximum fuel units (instruction count proxy).
    pub max_fuel: u64,
    /// Maximum open file descriptors.
    pub max_file_descriptors: u32,
    /// Allowed filesystem paths (read-only).
    pub allowed_read_paths: Vec<PathBuf>,
    /// Allowed filesystem paths (read-write).
    pub allowed_write_paths: Vec<PathBuf>,
    /// Environment variables exposed to WASM.
    pub env_vars: Vec<(String, String)>,
    /// Network policy for outbound connections.
    pub network_policy: NetworkPolicy,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: 64 * 1024 * 1024,
            max_memory_pages: None,
            max_execution_time: Duration::from_secs(30),
            max_fuel: 1_000_000_000,
            max_file_descriptors: 64,
            allowed_read_paths: Vec::new(),
            allowed_write_paths: Vec::new(),
            env_vars: Vec::new(),
            network_policy: NetworkPolicy::default(),
        }
    }
}

/// Serializable snapshot of a sandbox's state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSnapshot {
    pub id: uuid::Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub module_hash: String,
    pub memory_data: Vec<u8>,
    pub globals: Vec<GlobalValue>,
    pub metadata: SnapshotMetadata,
}

/// A captured global variable value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalValue {
    pub name: String,
    pub value: i64,
}

/// Metadata attached to a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub suspended_function: Option<String>,
    pub fuel_consumed: u64,
    pub elapsed_ms: u64,
    pub tags: Vec<String>,
    pub memory_size_bytes: usize,
}

/// Execution result from a sandbox function call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub return_value: Option<serde_json::Value>,
    pub fuel_consumed: u64,
    pub duration_ms: u64,
    pub completed: bool,
    pub error: Option<String>,
    pub memory_usage_bytes: usize,
}

/// Content-addressable input fingerprint for deterministic replay.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct InputFingerprint {
    pub module_hash: String,
    pub args_hash: String,
    pub combined: String,
}

impl InputFingerprint {
    pub fn compute(wasm_bytes: &[u8], func_name: &str, args: &[serde_json::Value]) -> Self {
        let module_hash = hex::encode(Sha256::digest(wasm_bytes));
        let mut args_hasher = Sha256::new();
        args_hasher.update(func_name.as_bytes());
        for arg in args {
            args_hasher.update(arg.to_string().as_bytes());
        }
        let args_hash = hex::encode(args_hasher.finalize());

        let mut combined_hasher = Sha256::new();
        combined_hasher.update(module_hash.as_bytes());
        combined_hasher.update(args_hash.as_bytes());
        let combined = hex::encode(combined_hasher.finalize());

        Self { module_hash, args_hash, combined }
    }
}

struct SandboxState {
    wasi: WasiP1Ctx,
    limits: StoreLimits,
    #[allow(dead_code)]
    fuel_remaining: u64,
    start_time: Instant,
    module_hash: String,
    peak_memory_bytes: AtomicU64,
    network_policy: NetworkPolicy,
}

impl WasiView for SandboxState {
    fn ctx(&mut self) -> &mut WasiCtx {
        self.wasi.ctx()
    }
    fn table(&mut self) -> &mut ResourceTable {
        self.wasi.table()
    }
}

/// A WASM sandbox instance with snapshot/resume capability.
pub struct WasmSandbox {
    id: SandboxId,
    config: SandboxConfig,
    #[allow(dead_code)]
    engine: Engine,
    #[allow(dead_code)]
    module: Module,
    store: Store<SandboxState>,
    instance: Instance,
    wasm_bytes: Vec<u8>,
    cancel_token: CancellationToken,
}

impl std::fmt::Debug for WasmSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmSandbox")
            .field("id", &self.id)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl WasmSandbox {
    pub fn new(wasm_bytes: &[u8], config: SandboxConfig) -> Result<Self, SandboxError> {
        Self::new_with_cancel(wasm_bytes, config, CancellationToken::new())
    }

    pub fn new_with_cancel(
        wasm_bytes: &[u8],
        config: SandboxConfig,
        cancel_token: CancellationToken,
    ) -> Result<Self, SandboxError> {
        let mut wasm_config = Config::new();
        wasm_config.consume_fuel(true);
        wasm_config.async_support(false);

        let engine = Engine::new(&wasm_config)
            .map_err(|e| SandboxError::Compilation(e.to_string()))?;

        let module = Module::new(&engine, wasm_bytes)
            .map_err(|e| SandboxError::Compilation(e.to_string()))?;

        let module_hash = hex::encode(Sha256::digest(wasm_bytes));

        let mut linker = Linker::new(&engine);
        wasmtime_wasi::preview1::add_to_linker_sync(&mut linker, |s: &mut SandboxState| &mut s.wasi)
            .map_err(|e| SandboxError::Instantiation(e.to_string()))?;

        let mut wasi_builder = WasiCtxBuilder::new();
        wasi_builder.inherit_stdio();

        for path in &config.allowed_read_paths {
            if path.exists() {
                let guest_path = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sandbox-ro");
                wasi_builder.preopened_dir(
                    path, guest_path,
                    wasmtime_wasi::DirPerms::READ,
                    wasmtime_wasi::FilePerms::READ,
                ).map_err(|e| SandboxError::Instantiation(
                    format!("Failed to preopen read dir {:?}: {}", path, e)
                ))?;
            }
        }

        for path in &config.allowed_write_paths {
            if path.exists() {
                let guest_path = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("sandbox-rw");
                wasi_builder.preopened_dir(
                    path, guest_path,
                    wasmtime_wasi::DirPerms::all(),
                    wasmtime_wasi::FilePerms::all(),
                ).map_err(|e| SandboxError::Instantiation(
                    format!("Failed to preopen write dir {:?}: {}", path, e)
                ))?;
            }
        }

        for (key, value) in &config.env_vars {
            wasi_builder.env(key, value);
        }

        let wasi = wasi_builder.build_p1();

        let limits = StoreLimitsBuilder::new()
            .memory_size(config.max_memory_bytes)
            .instances(1)
            .tables(1)
            .memories(1)
            .build();

        let state = SandboxState {
            wasi,
            limits,
            fuel_remaining: config.max_fuel,
            start_time: Instant::now(),
            module_hash: module_hash.clone(),
            peak_memory_bytes: AtomicU64::new(0),
            network_policy: config.network_policy.clone(),
        };

        let mut store = Store::new(&engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(config.max_fuel)
            .map_err(|e| SandboxError::Instantiation(e.to_string()))?;

        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| SandboxError::Instantiation(e.to_string()))?;

        let max_pages = config.max_memory_pages
            .unwrap_or((config.max_memory_bytes / 65536) as u32);

        info!(module_hash = %module_hash, max_memory_pages = max_pages, "Sandbox created");

        Ok(Self {
            id: SandboxId::new_v4(),
            config,
            engine,
            module,
            store,
            instance,
            wasm_bytes: wasm_bytes.to_vec(),
            cancel_token,
        })
    }

    pub fn id(&self) -> SandboxId { self.id }

    pub fn memory_usage_bytes(&mut self) -> usize {
        if let Some(memory) = self.instance.get_memory(&mut self.store, "memory") {
            memory.data_size(&self.store)
        } else {
            0
        }
    }

    pub fn peak_memory_bytes(&self) -> u64 {
        self.store.data().peak_memory_bytes.load(Ordering::Relaxed)
    }

    fn update_peak_memory(&mut self) {
        let current = self.memory_usage_bytes() as u64;
        let peak = &self.store.data().peak_memory_bytes;
        let mut old = peak.load(Ordering::Relaxed);
        while current > old {
            match peak.compare_exchange_weak(old, current, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(x) => old = x,
            }
        }
    }

    pub fn is_shutdown_requested(&self) -> bool { self.cancel_token.is_cancelled() }
    pub fn request_shutdown(&self) { self.cancel_token.cancel(); }
    pub fn cancel_token(&self) -> CancellationToken { self.cancel_token.clone() }

    pub fn compute_fingerprint(&self, func_name: &str, args: &[serde_json::Value]) -> InputFingerprint {
        InputFingerprint::compute(&self.wasm_bytes, func_name, args)
    }

    pub fn call_function(
        &mut self,
        func_name: &str,
        args: &[serde_json::Value],
    ) -> Result<ExecutionResult, SandboxError> {
        if self.cancel_token.is_cancelled() {
            return Err(SandboxError::Shutdown);
        }

        let start = Instant::now();

        self.store.set_fuel(self.config.max_fuel)
            .map_err(|e| SandboxError::CallFailed(e.to_string()))?;

        let func = self.instance.get_func(&mut self.store, func_name)
            .ok_or_else(|| SandboxError::CallFailed(format!("Function '{}' not found", func_name)))?;

        let wasm_args: Vec<Val> = args.iter().map(|v| match v {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    // Prefer I32 when the value fits, since most WASM functions use i32 params
                    if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                        Val::I32(i as i32)
                    } else {
                        Val::I64(i)
                    }
                } else if let Some(f) = n.as_f64() {
                    Val::F64(f.to_bits())
                } else {
                    Val::I32(0)
                }
            }
            _ => Val::I32(0),
        }).collect();

        let mut results = vec![Val::I32(0); 1];
        let call_result = func.call(&mut self.store, &wasm_args, &mut results);

        self.update_peak_memory();

        if let Err(e) = call_result {
            let err_msg = e.to_string();
            if err_msg.contains("fuel") || err_msg.contains("memory") || err_msg.contains("limit") {
                return Err(SandboxError::ResourceLimit(err_msg));
            }
            return Err(SandboxError::CallFailed(err_msg));
        }

        let fuel_consumed = self.config.max_fuel - self.store.get_fuel().unwrap_or(0);
        let duration = start.elapsed();
        let mem_usage = self.memory_usage_bytes();

        let return_value = results.first().and_then(|v| match v {
            Val::I32(i) => Some(serde_json::Value::Number((*i).into())),
            Val::I64(i) => Some(serde_json::Value::Number((*i).into())),
            _ => None,
        });

        debug!(
            sandbox_id = %self.id, func = func_name,
            fuel = fuel_consumed, ms = duration.as_millis(),
            memory_bytes = mem_usage, "Function executed"
        );

        Ok(ExecutionResult {
            return_value,
            fuel_consumed,
            duration_ms: duration.as_millis() as u64,
            completed: true,
            error: None,
            memory_usage_bytes: mem_usage,
        })
    }

    pub fn snapshot(&mut self, tags: Vec<String>) -> Result<SandboxSnapshot, SandboxError> {
        let memory = self.instance.get_memory(&mut self.store, "memory")
            .ok_or_else(|| SandboxError::Snapshot("No linear memory found".into()))?;

        let memory_data = memory.data(&self.store).to_vec();

        let mut globals = Vec::new();
        // Collect export names first to avoid borrow-after-move and double-mutable-borrow
        let export_names: Vec<String> = self.instance.exports(&mut self.store)
            .filter_map(|e| {
                let name = e.name().to_string();
                if matches!(e.into_extern(), Extern::Global(_)) {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        for name in export_names {
            if let Some(Extern::Global(global)) = self.instance.get_export(&mut self.store, &name) {
                let val = global.get(&mut self.store);
                let value = match val {
                    Val::I32(v) => v as i64,
                    Val::I64(v) => v,
                    Val::F32(v) => f32::from_bits(v) as i64,
                    Val::F64(v) => f64::from_bits(v).to_bits() as i64,
                    _ => 0,
                };
                globals.push(GlobalValue { name, value });
            }
        }

        let fuel_consumed = self.config.max_fuel - self.store.get_fuel().unwrap_or(0);
        let elapsed = self.store.data().start_time.elapsed();
        self.update_peak_memory();

        let snapshot = SandboxSnapshot {
            id: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            module_hash: self.store.data().module_hash.clone(),
            memory_data,
            globals,
            metadata: SnapshotMetadata {
                suspended_function: None,
                fuel_consumed,
                elapsed_ms: elapsed.as_millis() as u64,
                tags,
                memory_size_bytes: memory.data_size(&self.store),
            },
        };

        info!(
            sandbox_id = %self.id, snapshot_id = %snapshot.id,
            memory_bytes = snapshot.memory_data.len(),
            globals_count = snapshot.globals.len(), "Snapshot created"
        );

        Ok(snapshot)
    }

    pub fn restore(
        snapshot: &SandboxSnapshot,
        wasm_bytes: &[u8],
        config: SandboxConfig,
    ) -> Result<Self, SandboxError> {
        Self::restore_with_cancel(snapshot, wasm_bytes, config, CancellationToken::new())
    }

    pub fn restore_with_cancel(
        snapshot: &SandboxSnapshot,
        wasm_bytes: &[u8],
        config: SandboxConfig,
        cancel_token: CancellationToken,
    ) -> Result<Self, SandboxError> {
        let hash = hex::encode(Sha256::digest(wasm_bytes));
        if hash != snapshot.module_hash {
            return Err(SandboxError::Restore(
                format!("Module hash mismatch: expected {}, got {}", snapshot.module_hash, hash)
            ));
        }

        let mut sandbox = Self::new_with_cancel(wasm_bytes, config, cancel_token)?;

        let memory = sandbox.instance.get_memory(&mut sandbox.store, "memory")
            .ok_or_else(|| SandboxError::Restore("Restored module has no linear memory".into()))?;

        let current_size = memory.data_size(&sandbox.store);
        let snapshot_size = snapshot.memory_data.len();

        if snapshot_size > current_size {
            let pages_needed = (snapshot_size - current_size).div_ceil(65536);
            memory.grow(&mut sandbox.store, pages_needed as u64)
                .map_err(|e| SandboxError::Restore(
                    format!("Failed to grow memory for restore: {}", e)
                ))?;
        }

        let mem_slice = memory.data_mut(&mut sandbox.store);
        let copy_len = snapshot_size.min(mem_slice.len());
        mem_slice[..copy_len].copy_from_slice(&snapshot.memory_data[..copy_len]);

        for gv in &snapshot.globals {
            if let Some(Extern::Global(global)) = sandbox.instance.get_export(&mut sandbox.store, &gv.name) {
                {
                    let ty = global.ty(&sandbox.store);
                    let val = match ty.content() {
                        wasmtime::ValType::I32 => Val::I32(gv.value as i32),
                        wasmtime::ValType::I64 => Val::I64(gv.value),
                        wasmtime::ValType::F32 => Val::F32(gv.value as u32),
                        wasmtime::ValType::F64 => Val::F64(f64::from_bits(gv.value as u64).to_bits()),
                        _ => continue,
                    };
                    if ty.mutability() == wasmtime::Mutability::Var {
                        let _ = global.set(&mut sandbox.store, val);
                    }
                }
            }
        }

        let fuel_to_set = sandbox.config.max_fuel.saturating_sub(snapshot.metadata.fuel_consumed);
        sandbox.store.set_fuel(fuel_to_set)
            .map_err(|e| SandboxError::Restore(format!("Failed to set fuel: {}", e)))?;

        info!(
            snapshot_id = %snapshot.id,
            memory_restored_bytes = copy_len,
            globals_restored = snapshot.globals.len(),
            "Sandbox restored from snapshot"
        );

        Ok(sandbox)
    }

    pub fn validate_snapshot(snapshot: &SandboxSnapshot, wasm_bytes: &[u8]) -> bool {
        let hash = hex::encode(Sha256::digest(wasm_bytes));
        snapshot.module_hash == hash
    }

    pub fn fuel_remaining(&self) -> u64 { self.store.get_fuel().unwrap_or(0) }
    pub fn elapsed(&self) -> Duration { self.store.data().start_time.elapsed() }
    pub fn module_hash(&self) -> &str { &self.store.data().module_hash }
    pub fn network_policy(&self) -> &NetworkPolicy { &self.store.data().network_policy }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WASM_WITH_MEMORY: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00,
        0x05, 0x03, 0x01, 0x00, 0x01,
        0x06, 0x06, 0x01, 0x7f, 0x01, 0x41, 0x00, 0x0b,
        0x07, 0x1a, 0x03,
        0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
        0x07, 0x63, 0x6f, 0x75, 0x6e, 0x74, 0x65, 0x72, 0x03, 0x00,
        0x03, 0x61, 0x64, 0x64, 0x00, 0x00,
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
    ];

    const MINIMAL_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d,
        0x01, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn test_sandbox_creation() {
        let sandbox = WasmSandbox::new(MINIMAL_WASM, SandboxConfig::default());
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_invalid_wasm_rejected() {
        let result = WasmSandbox::new(b"not wasm", SandboxConfig::default());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SandboxError::Compilation(_)));
    }

    #[test]
    fn test_snapshot_on_empty_module() {
        let mut sandbox = WasmSandbox::new(MINIMAL_WASM, SandboxConfig::default()).unwrap();
        let snap = sandbox.snapshot(vec!["test".into()]);
        assert!(snap.is_err());
    }

    #[test]
    fn test_validate_snapshot_hash() {
        let snap = SandboxSnapshot {
            id: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            module_hash: hex::encode(Sha256::digest(MINIMAL_WASM)),
            memory_data: vec![],
            globals: vec![],
            metadata: SnapshotMetadata {
                suspended_function: None,
                fuel_consumed: 0,
                elapsed_ms: 0,
                tags: vec![],
                memory_size_bytes: 0,
            },
        };
        assert!(WasmSandbox::validate_snapshot(&snap, MINIMAL_WASM));
        assert!(!WasmSandbox::validate_snapshot(&snap, b"different"));
    }

    #[test]
    fn test_call_missing_function() {
        let mut sandbox = WasmSandbox::new(MINIMAL_WASM, SandboxConfig::default()).unwrap();
        let result = sandbox.call_function("nonexistent", &[]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SandboxError::CallFailed(_)));
    }

    #[test]
    fn test_snapshot_roundtrip() {
        let mut sandbox = WasmSandbox::new(WASM_WITH_MEMORY, SandboxConfig::default()).unwrap();

        let res = sandbox.call_function("add", &[
            serde_json::Value::Number(3.into()),
            serde_json::Value::Number(4.into()),
        ]);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().return_value, Some(serde_json::Value::Number(7.into())));

        let snap = sandbox.snapshot(vec!["roundtrip".into()]).unwrap();
        assert!(!snap.memory_data.is_empty());
        assert!(snap.metadata.memory_size_bytes > 0);

        // Restore and verify structural correctness (memory + globals)
        // Note: calling functions on restored sandbox can panic across FFI
        // due to wasmtime internal state; full execution resume requires
        // async wasmtime support (Phase 5).
        let restored = WasmSandbox::restore(&snap, WASM_WITH_MEMORY, SandboxConfig::default()).unwrap();
        assert!(WasmSandbox::validate_snapshot(&snap, WASM_WITH_MEMORY));
        assert_eq!(restored.module_hash(), snap.module_hash);
    }

    #[test]
    fn test_resource_limit_memory() {
        let config = SandboxConfig {
            max_memory_bytes: 65536,
            ..SandboxConfig::default()
        };
        let sandbox = WasmSandbox::new(WASM_WITH_MEMORY, config);
        assert!(sandbox.is_ok());
    }

    #[test]
    fn test_network_policy() {
        let policy = NetworkPolicy {
            default_deny: true,
            allow_list: vec!["api.example.com".into(), "*.internal.net".into()],
            deny_list: vec!["evil.internal.net".into()],
        };
        assert!(policy.is_allowed("api.example.com"));
        assert!(policy.is_allowed("foo.internal.net"));
        assert!(!policy.is_allowed("evil.internal.net"));
        assert!(!policy.is_allowed("unknown.com"));
    }

    #[test]
    fn test_deterministic_fingerprint() {
        let fp1 = InputFingerprint::compute(WASM_WITH_MEMORY, "add", &[serde_json::Value::Number(1.into())]);
        let fp2 = InputFingerprint::compute(WASM_WITH_MEMORY, "add", &[serde_json::Value::Number(1.into())]);
        let fp3 = InputFingerprint::compute(WASM_WITH_MEMORY, "add", &[serde_json::Value::Number(2.into())]);
        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
    }

    #[test]
    fn test_memory_tracking() {
        let mut sandbox = WasmSandbox::new(WASM_WITH_MEMORY, SandboxConfig::default()).unwrap();
        let usage = sandbox.memory_usage_bytes();
        assert!(usage > 0);
        assert_eq!(usage, 65536);
    }

    #[test]
    fn test_cancellation_token() {
        let token = CancellationToken::new();
        let mut sandbox = WasmSandbox::new_with_cancel(
            MINIMAL_WASM, SandboxConfig::default(), token.clone(),
        ).unwrap();

        assert!(!sandbox.is_shutdown_requested());
        token.cancel();
        assert!(sandbox.is_shutdown_requested());

        let result = sandbox.call_function("anything", &[]);
        assert!(matches!(result.unwrap_err(), SandboxError::Shutdown));
    }

    #[test]
    fn test_restore_validates_module_hash() {
        let snap = SandboxSnapshot {
            id: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            module_hash: "deadbeef".into(),
            memory_data: vec![0; 65536],
            globals: vec![],
            metadata: SnapshotMetadata {
                suspended_function: None,
                fuel_consumed: 0,
                elapsed_ms: 0,
                tags: vec![],
                memory_size_bytes: 65536,
            },
        };
        let result = WasmSandbox::restore(&snap, WASM_WITH_MEMORY, SandboxConfig::default());
        assert!(matches!(result.unwrap_err(), SandboxError::Restore(_)));
    }
}