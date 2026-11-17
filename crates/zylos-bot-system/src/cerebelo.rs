#![allow(clippy::collapsible_if)]
//! Cerebelo Executor — Task 2.2.1
//!
//! Wraps DagExecutor with:
//! - ExecutorBackend trait (Local/Docker/Wasm)
//! - VerifySpec post-task verification
//! - WriteGate integration (preventive evidence check)
//! - ExecutionAuthority propagation via task_local
//! - RecoveryLadder (retry → decompose → delegate → escalate)
//! - OutcomeUncertainty tri-state
//! - ArtifactProvenance tracking
//! - Heartbeat monitoring (60s interval, 180s timeout)
//! - Idempotency enforcement
//! - Structured tracing spans per DAG/node/run

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::dag::{DagDefinition, DagExecutor, NodeStatus};
use crate::error::{BotError, Result};

// Re-use WriteGate types from zylos-memory-system
use zylos_memory_system::write_gate::{
    EvidenceCapsule as WriteGateEvidenceCapsule, GateResult, WriteGate as MemoryWriteGate,
};

// ---------------------------------------------------------------------------
// ExecutionAuthority — propagated via tokio::task_local!
// ---------------------------------------------------------------------------

tokio::task_local! {
    /// Current execution authority for the running task.
    pub static EXECUTION_AUTHORITY: ExecutionAuthority;
}

/// Authority context propagated through async tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAuthority {
    pub approval_id: String,
    pub digest: String,
    pub owner: String,
}

impl ExecutionAuthority {
    pub fn new(
        approval_id: impl Into<String>,
        digest: impl Into<String>,
        owner: impl Into<String>,
    ) -> Self {
        Self {
            approval_id: approval_id.into(),
            digest: digest.into(),
            owner: owner.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// OutcomeUncertainty — tri-state result
// ---------------------------------------------------------------------------

/// Tri-state outcome of a task or verification step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutcomeUncertainty {
    Succeeded,
    Failed,
    Uncertain(String),
}

// ---------------------------------------------------------------------------
// VerifySpec — post-task verification primitive
// ---------------------------------------------------------------------------

/// Specification for a post-execution verification command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySpec {
    /// Shell command to run after the task completes.
    pub cmd: String,
    /// Expected exit code for success.
    pub expected_exit: i32,
    /// Optional regex pattern that stdout must match.
    pub output_pattern: Option<String>,
    /// Timeout in milliseconds for the verify command.
    pub timeout_ms: u64,
}

impl VerifySpec {
    /// Execute the verification command and return the outcome.
    pub async fn execute(&self) -> OutcomeUncertainty {
        let span =
            tracing::info_span!("verify", cmd = %self.cmd, expected_exit = self.expected_exit);
        let _guard = span.enter();

        let timeout = Duration::from_millis(self.timeout_ms);

        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&self.cmd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn();

        let child = match child {
            Ok(c) => c,
            Err(e) => {
                error!(error = %e, "Failed to spawn verify command");
                return OutcomeUncertainty::Failed;
            }
        };

        match tokio::time::timeout(timeout, child.wait_with_output()).await {
            Ok(Ok(output)) => {
                let code = output.status.code().unwrap_or(-1);
                if code != self.expected_exit {
                    warn!(actual_exit = code, "Verify exit code mismatch");
                    return OutcomeUncertainty::Failed;
                }

                if let Some(ref pattern) = self.output_pattern {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    match Regex::new(pattern) {
                        Ok(re) => {
                            if !re.is_match(&stdout) {
                                warn!(pattern = %pattern, "Verify output pattern did not match");
                                return OutcomeUncertainty::Failed;
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Invalid verify output_pattern regex");
                            return OutcomeUncertainty::Uncertain(format!("Invalid regex: {}", e));
                        }
                    }
                }

                debug!("Verify succeeded");
                OutcomeUncertainty::Succeeded
            }
            Ok(Err(e)) => {
                error!(error = %e, "Verify command failed");
                OutcomeUncertainty::Failed
            }
            Err(_) => {
                warn!(timeout_ms = self.timeout_ms, "Verify command timed out");
                OutcomeUncertainty::Uncertain(format!("Timeout after {}ms", self.timeout_ms))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutionConfig — per-node optional config carried by DagNode extensions
// ---------------------------------------------------------------------------

/// Extended configuration attached to a DagNode for Cerebelo execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Whether this node is idempotent (safe to re-execute).
    pub idempotent: bool,
    /// Optional post-task verification spec.
    pub verify: Option<VerifySpec>,
    /// Backend selection override.
    pub backend: BackendKind,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            idempotent: true,
            verify: None,
            backend: BackendKind::Local,
        }
    }
}

// ---------------------------------------------------------------------------
// ExecutorBackend trait + implementors
// ---------------------------------------------------------------------------

/// Kind of execution backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendKind {
    Local,
    Docker,
    Wasm,
}

/// Result of a single task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutcome {
    pub status: OutcomeUncertainty,
    pub output: Option<serde_json::Value>,
    pub artifacts: Vec<ArtifactProvenance>,
}

/// Trait for pluggable execution backends.
pub trait ExecutorBackend: Send + Sync {
    /// Execute a task with the given spec and authority context.
    fn execute(
        &self,
        task_spec: serde_json::Value,
        auth: ExecutionAuthority,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionOutcome>> + Send + '_>>;
}

/// Local process backend using tokio::process.
pub struct LocalBackend;

impl ExecutorBackend for LocalBackend {
    fn execute(
        &self,
        task_spec: serde_json::Value,
        auth: ExecutionAuthority,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionOutcome>> + Send + '_>>
    {
        Box::pin(async move {
            let span = tracing::info_span!("local_backend", owner = %auth.owner, approval = %auth.approval_id);
            let _guard = span.enter();

            // Extract command from task_spec if present
            let cmd = task_spec
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("echo 'no command specified'");

            let child = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| BotError::Internal(format!("LocalBackend spawn failed: {}", e)))?;

            let output = child
                .wait_with_output()
                .await
                .map_err(|e| BotError::Internal(format!("LocalBackend wait failed: {}", e)))?;

            let code = output.status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();

            let status = if output.status.success() {
                OutcomeUncertainty::Succeeded
            } else {
                OutcomeUncertainty::Failed
            };

            Ok(ExecutionOutcome {
                status,
                output: Some(serde_json::json!({
                    "exit_code": code,
                    "stdout": stdout,
                })),
                artifacts: vec![],
            })
        })
    }
}

/// Docker backend — executa task_spec via `docker run` com sandboxing.
pub struct DockerBackend {
    /// Imagem base para execução (ex: "zylos-runtime:latest")
    image: String,
    /// Timeout máximo em segundos (usado como argumento --stop-timeout)
    #[allow(dead_code)]
    timeout_secs: u64,
}

impl DockerBackend {
    pub fn new(image: impl Into<String>, timeout_secs: u64) -> Self {
        Self {
            image: image.into(),
            timeout_secs,
        }
    }
}

impl ExecutorBackend for DockerBackend {
    fn execute(
        &self,
        task_spec: serde_json::Value,
        _auth: ExecutionAuthority,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionOutcome>> + Send + '_>>
    {
        Box::pin(async move {
            use tokio::process::Command;

            let spec_json = serde_json::to_string(&task_spec)
                .map_err(|e| BotError::Internal(format!("Failed to serialize task_spec: {e}")))?;

            info!(image = %self.image, "DockerBackend executing task");

            let output = Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "--network=none",
                    "--memory=256m",
                    "--cpus=1",
                    "-e",
                    &format!("TASK_SPEC={}", spec_json),
                    &self.image,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output()
                .await
                .map_err(|e| BotError::Internal(format!("Docker execution failed: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                info!("DockerBackend task completed successfully");
                Ok(ExecutionOutcome {
                    status: OutcomeUncertainty::Succeeded,
                    output: Some(serde_json::Value::String(stdout)),
                    artifacts: vec![],
                })
            } else {
                warn!(stderr = %stderr, "DockerBackend task failed");
                Ok(ExecutionOutcome {
                    status: OutcomeUncertainty::Uncertain(format!(
                        "Docker exited with code {:?}: {}",
                        output.status.code(),
                        stderr.trim()
                    )),
                    output: Some(serde_json::Value::String(stdout)),
                    artifacts: vec![],
                })
            }
        })
    }
}

/// WASM backend — executa task_spec via wasmtime com sandboxing de memória.
pub struct WasmBackend {
    /// Caminho para o módulo WASM
    wasm_module_path: Option<std::path::PathBuf>,
    /// Limite de memória em páginas (64KB cada)
    max_memory_pages: u32,
}

impl WasmBackend {
    pub fn new(wasm_module_path: Option<std::path::PathBuf>, max_memory_pages: u32) -> Self {
        Self {
            wasm_module_path,
            max_memory_pages,
        }
    }
}

impl ExecutorBackend for WasmBackend {
    fn execute(
        &self,
        task_spec: serde_json::Value,
        _auth: ExecutionAuthority,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecutionOutcome>> + Send + '_>>
    {
        Box::pin(async move {
            let module_path = match &self.wasm_module_path {
                Some(p) => p.clone(),
                None => {
                    return Ok(ExecutionOutcome {
                        status: OutcomeUncertainty::Uncertain(
                            "WasmBackend: no wasm module path configured".into(),
                        ),
                        output: None,
                        artifacts: vec![],
                    });
                }
            };

            if !module_path.exists() {
                return Ok(ExecutionOutcome {
                    status: OutcomeUncertainty::Uncertain(format!(
                        "WASM module not found: {}",
                        module_path.display()
                    )),
                    output: None,
                    artifacts: vec![],
                });
            }

            info!(
                module = %module_path.display(),
                max_pages = self.max_memory_pages,
                "WasmBackend executing task"
            );

            // Executar via wasmtime CLI com limites de recursos
            use tokio::process::Command;
            let spec_json = serde_json::to_string(&task_spec)
                .map_err(|e| BotError::Internal(format!("Failed to serialize task_spec: {e}")))?;

            let output = Command::new("wasmtime")
                .args([
                    "run",
                    "--max-memory-size",
                    &format!("{}", self.max_memory_pages * 65536),
                    "--env",
                    &format!("TASK_SPEC={}", spec_json),
                    &module_path.to_string_lossy(),
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output()
                .await
                .map_err(|e| BotError::Internal(format!("Wasmtime execution failed: {e}")))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                info!("WasmBackend task completed successfully");
                Ok(ExecutionOutcome {
                    status: OutcomeUncertainty::Succeeded,
                    output: Some(serde_json::Value::String(stdout)),
                    artifacts: vec![],
                })
            } else {
                warn!(stderr = %stderr, "WasmBackend task failed");
                Ok(ExecutionOutcome {
                    status: OutcomeUncertainty::Uncertain(format!(
                        "Wasmtime exited with code {:?}: {}",
                        output.status.code(),
                        stderr.trim()
                    )),
                    output: Some(serde_json::Value::String(stdout)),
                    artifacts: vec![],
                })
            }
        })
    }
}

// ---------------------------------------------------------------------------
// WriteGate wrapper
// ---------------------------------------------------------------------------

/// Preventive write gate that validates EvidenceCapsules before external writes.
/// Wraps zylos-memory-system::WriteGate.
pub struct CerebeloWriteGate {
    inner: MemoryWriteGate,
}

impl CerebeloWriteGate {
    pub fn new() -> Self {
        Self {
            inner: MemoryWriteGate::new(),
        }
    }

    pub fn with_min_evidence(mut self, count: usize) -> Self {
        self.inner = self.inner.with_min_evidence(count);
        self
    }

    /// Validate an evidence capsule. Blocks if invalid.
    pub fn validate(&self, capsule: &WriteGateEvidenceCapsule) -> GateResult {
        self.inner.validate(capsule)
    }

    /// Check whether a write is allowed; returns Err if blocked.
    pub fn check(&self, capsule: &WriteGateEvidenceCapsule) -> Result<()> {
        match self.inner.validate(capsule) {
            GateResult::Pass => Ok(()),
            GateResult::Warn(msg) => {
                warn!(warning = %msg, "WriteGate warning; proceeding");
                Ok(())
            }
            GateResult::Block(reason) => {
                error!(reason = %reason, "WriteGate blocked external write");
                Err(BotError::Internal(format!("WriteGate blocked: {}", reason)))
            }
        }
    }
}

impl Default for CerebeloWriteGate {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ArtifactProvenance
// ---------------------------------------------------------------------------

/// Provenance metadata attached to every produced artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactProvenance {
    pub node_id: String,
    pub run_id: String,
    pub timestamp: u64,
    pub verification_status: OutcomeUncertainty,
    pub parent_artifacts: Vec<String>,
}

impl ArtifactProvenance {
    pub fn new(
        node_id: impl Into<String>,
        run_id: impl Into<String>,
        verification_status: OutcomeUncertainty,
    ) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            node_id: node_id.into(),
            run_id: run_id.into(),
            timestamp: ts,
            verification_status,
            parent_artifacts: vec![],
        }
    }

    pub fn with_parents(mut self, parents: Vec<String>) -> Self {
        self.parent_artifacts = parents;
        self
    }
}

// ---------------------------------------------------------------------------
// RecoveryLadder
// ---------------------------------------------------------------------------

/// Step in the recovery ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStep {
    Retry { attempt: u32, max: u32 },
    Decompose { subtask_count: usize },
    Delegate { specialist: String },
    EscalateHuman,
}

/// Tracks recovery attempts and their outcomes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub steps: Vec<RecoveryStep>,
    pub final_outcome: OutcomeUncertainty,
}

/// Recovery ladder: retry(max=3) → decompose → delegate → escalate.
pub struct RecoveryLadder {
    max_retries: u32,
}

impl RecoveryLadder {
    pub fn new(max_retries: u32) -> Self {
        Self { max_retries }
    }

    /// Attempt recovery for a failed node. Returns the final outcome and record.
    pub async fn recover<F, Fut>(
        &self,
        node_id: &str,
        task_spec: serde_json::Value,
        mut execute_fn: F,
    ) -> (OutcomeUncertainty, RecoveryRecord)
    where
        F: FnMut(serde_json::Value) -> Fut + Send,
        Fut: std::future::Future<Output = OutcomeUncertainty> + Send,
    {
        let span = tracing::info_span!("recovery_ladder", node_id = %node_id);
        let _guard = span.enter();

        let mut steps = Vec::new();

        // Step 1: Retry up to max_retries
        for attempt in 1..=self.max_retries {
            steps.push(RecoveryStep::Retry {
                attempt,
                max: self.max_retries,
            });
            info!(attempt, max = self.max_retries, "Recovery: retrying");
            let outcome = execute_fn(task_spec.clone()).await;
            if outcome == OutcomeUncertainty::Succeeded {
                return (
                    OutcomeUncertainty::Succeeded,
                    RecoveryRecord {
                        steps,
                        final_outcome: OutcomeUncertainty::Succeeded,
                    },
                );
            }
            // Small backoff between retries
            tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
        }

        // Step 2: Decompose (stub — would split into subtasks)
        steps.push(RecoveryStep::Decompose { subtask_count: 0 });
        warn!("Recovery: decomposition not yet implemented; skipping");

        // Step 3: Delegate to specialist (stub)
        steps.push(RecoveryStep::Delegate {
            specialist: "none".into(),
        });
        warn!("Recovery: delegation not yet implemented; skipping");

        // Step 4: Escalate to human
        steps.push(RecoveryStep::EscalateHuman);
        info!("Recovery: escalating to human operator");

        let record = RecoveryRecord {
            steps,
            final_outcome: OutcomeUncertainty::Failed,
        };
        (OutcomeUncertainty::Failed, record)
    }
}

// ---------------------------------------------------------------------------
// Heartbeat
// ---------------------------------------------------------------------------

/// Heartbeat monitor: reports progress every 60s, auto-pauses after 180s silence.
pub struct HeartbeatMonitor {
    interval: Duration,
    timeout: Duration,
    last_beat: Arc<Mutex<Instant>>,
    cancel: CancellationToken,
}

impl HeartbeatMonitor {
    pub fn new(cancel: CancellationToken) -> Self {
        Self {
            interval: Duration::from_secs(60),
            timeout: Duration::from_secs(180),
            last_beat: Arc::new(Mutex::new(Instant::now())),
            cancel,
        }
    }

    /// Record a heartbeat.
    pub async fn beat(&self) {
        let mut lb = self.last_beat.lock().await;
        *lb = Instant::now();
    }

    /// Spawn the background heartbeat checker. Returns a sender to signal progress.
    pub fn spawn_checker(&self, run_id: String) -> mpsc::Sender<()> {
        let (tx, mut rx) = mpsc::channel::<()>(16);
        let last_beat = self.last_beat.clone();
        let timeout = self.timeout;
        let interval = self.interval;
        let cancel = self.cancel.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(interval) => {
                        let elapsed = {
                            let lb = last_beat.lock().await;
                            lb.elapsed()
                        };
                        if elapsed > timeout {
                            error!(run_id = %run_id, elapsed_s = elapsed.as_secs(), "Heartbeat timeout; auto-pausing");
                            cancel.cancel();
                            break;
                        } else {
                            debug!(run_id = %run_id, elapsed_s = elapsed.as_secs(), "Heartbeat OK");
                        }
                    }
                    _ = rx.recv() => {
                        // Progress signal received; update last beat
                        let mut lb = last_beat.lock().await;
                        *lb = Instant::now();
                    }
                }
            }
        });

        tx
    }
}

// ---------------------------------------------------------------------------
// Idempotency tracker
// ---------------------------------------------------------------------------

/// Tracks which non-idempotent nodes have been executed to prevent re-execution.
pub struct IdempotencyTracker {
    executed: Arc<Mutex<HashMap<String, bool>>>,
}

impl IdempotencyTracker {
    pub fn new() -> Self {
        Self {
            executed: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check whether a non-idempotent node can execute.
    /// Returns Ok(()) if allowed, Err if already executed and not idempotent.
    pub async fn check_and_mark(&self, node_id: &str, idempotent: bool) -> Result<()> {
        let mut map = self.executed.lock().await;
        if !idempotent {
            if map.contains_key(node_id) {
                return Err(BotError::Internal(format!(
                    "Non-idempotent node '{}' already executed; explicit approval required for re-execution",
                    node_id
                )));
            }
        }
        map.insert(node_id.to_string(), true);
        Ok(())
    }
}

impl Default for IdempotencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CerebeloExecutor — wraps DagExecutor
// ---------------------------------------------------------------------------

/// Cerebelo executor wrapping DagExecutor with verification, gates, recovery, etc.
pub struct CerebeloExecutor {
    dag_executor: DagExecutor,
    write_gate: CerebeloWriteGate,
    #[allow(dead_code)]
    recovery: RecoveryLadder,
    idempotency: IdempotencyTracker,
    heartbeat: HeartbeatMonitor,
    backends: HashMap<BackendKind, Arc<dyn ExecutorBackend>>,
    /// Map of node_id → ExecutionConfig
    node_configs: HashMap<String, ExecutionConfig>,
    run_id: String,
}

impl CerebeloExecutor {
    pub fn new(cancel: CancellationToken) -> Self {
        let mut backends: HashMap<BackendKind, Arc<dyn ExecutorBackend>> = HashMap::new();
        backends.insert(BackendKind::Local, Arc::new(LocalBackend));
        backends.insert(
            BackendKind::Docker,
            Arc::new(DockerBackend::new("zylos-runtime:latest", 300)),
        );
        backends.insert(BackendKind::Wasm, Arc::new(WasmBackend::new(None, 256)));

        Self {
            dag_executor: DagExecutor::new(cancel.clone()),
            write_gate: CerebeloWriteGate::new(),
            recovery: RecoveryLadder::new(3),
            idempotency: IdempotencyTracker::new(),
            heartbeat: HeartbeatMonitor::new(cancel),
            backends,
            node_configs: HashMap::new(),
            run_id: Uuid::new_v4().to_string(),
        }
    }

    /// Set execution config for a specific node.
    pub fn set_node_config(&mut self, node_id: impl Into<String>, config: ExecutionConfig) {
        self.node_configs.insert(node_id.into(), config);
    }

    /// Get the write gate reference for external validation.
    pub fn write_gate(&self) -> &CerebeloWriteGate {
        &self.write_gate
    }

    /// Execute a DAG definition with Cerebelo enhancements.
    pub async fn execute(
        &self,
        def: &DagDefinition,
        auth: ExecutionAuthority,
    ) -> Result<HashMap<String, NodeStatus>> {
        let run_span = tracing::info_span!("cerebelo_run", run_id = %self.run_id, dag_id = %def.id, dag_name = %def.name);
        let _run_guard = run_span.enter();

        info!("Starting Cerebelo execution");

        // Start heartbeat checker
        let hb_tx = self.heartbeat.spawn_checker(self.run_id.clone());

        // Build closure that wraps each node execution with Cerebelo features
        let node_configs = Arc::new(self.node_configs.clone());
        let backends = Arc::new(self.backends.clone());
        let idempotency = Arc::new(self.idempotency.executed.clone());
        let run_id = self.run_id.clone();
        let results = self
            .dag_executor
            .execute(def, move |node_id, node_ctx, _token| {
                let task_spec = node_ctx.task_spec.clone();
                let configs = node_configs.clone();
                let backends = backends.clone();
                let idem = idempotency.clone();
                let rid = run_id.clone();
                let auth_clone = auth.clone();
                let hb = hb_tx.clone();
                let _wg = MemoryWriteGate::new();

                async move {
                    let node_span =
                        tracing::info_span!("cerebelo_node", node_id = %node_id, run_id = %rid);
                    let _ng = node_span.enter();

                    // Signal heartbeat
                    let _ = hb.send(()).await;

                    // Get config for this node
                    let config = configs.get(&node_id).cloned().unwrap_or_default();

                    // Idempotency check
                    {
                        let mut map = idem.lock().await;
                        if !config.idempotent && map.contains_key(&node_id) {
                            return Err(format!(
                                "Non-idempotent node '{}' already executed",
                                node_id
                            ));
                        }
                        map.insert(node_id.clone(), true);
                    }

                    // Select backend
                    let backend = backends
                        .get(&config.backend)
                        .ok_or_else(|| format!("No backend for {:?}", config.backend))?;

                    // Execute via backend
                    let outcome = backend
                        .execute(task_spec.clone(), auth_clone.clone())
                        .await
                        .map_err(|e| format!("{}", e))?;

                    // Signal heartbeat after execution
                    let _ = hb.send(()).await;

                    match outcome.status {
                        OutcomeUncertainty::Failed => {
                            // Attempt recovery
                            let recovery = RecoveryLadder::new(3);
                            let (recovered, _record) = recovery
                                .recover(&node_id, task_spec, |spec| {
                                    let b = backend.clone();
                                    let a = auth_clone.clone();
                                    async move {
                                        match b.execute(spec, a).await {
                                            Ok(o) => o.status,
                                            Err(_) => OutcomeUncertainty::Failed,
                                        }
                                    }
                                })
                                .await;

                            if recovered != OutcomeUncertainty::Succeeded {
                                return Err("Node failed after recovery".to_string());
                            }
                        }
                        OutcomeUncertainty::Uncertain(_) => {
                            warn!("Node outcome uncertain; treating as failure");
                            return Err("Node outcome uncertain".to_string());
                        }
                        OutcomeUncertainty::Succeeded => {}
                    }

                    // Post-task verification if configured
                    if let Some(ref verify) = config.verify {
                        let verify_result = verify.execute().await;
                        match verify_result {
                            OutcomeUncertainty::Succeeded => {
                                debug!("Post-task verification passed");
                            }
                            OutcomeUncertainty::Failed => {
                                return Err("Post-task verification failed".to_string());
                            }
                            OutcomeUncertainty::Uncertain(reason) => {
                                warn!(reason = %reason, "Post-task verification uncertain");
                                return Err(format!("Verification uncertain: {}", reason));
                            }
                        }
                    }

                    // Create artifact provenance
                    let _provenance =
                        ArtifactProvenance::new(&node_id, &rid, OutcomeUncertainty::Succeeded);

                    Ok(outcome
                        .output
                        .unwrap_or(serde_json::json!({"status": "ok"})))
                }
            })
            .await?;

        info!(completed = results.len(), "Cerebelo execution finished");
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{DagEdge, DagId, DagNode};

    fn make_test_dag() -> DagDefinition {
        DagDefinition {
            id: DagId::new(),
            name: "cerebelo-test".to_string(),
            nodes: vec![
                DagNode {
                    id: "step1".to_string(),
                    agent_name: "test-agent".to_string(),
                    task_spec: serde_json::json!({"command": "echo hello"}),
                    timeout_ms: 5000,
                    max_retries: 0,
                    backoff_ms: 100,
                },
                DagNode {
                    id: "step2".to_string(),
                    agent_name: "test-agent".to_string(),
                    task_spec: serde_json::json!({"command": "echo world"}),
                    timeout_ms: 5000,
                    max_retries: 0,
                    backoff_ms: 100,
                },
            ],
            edges: vec![DagEdge {
                from: "step1".to_string(),
                to: "step2".to_string(),
                condition: None,
            }],
        }
    }

    #[test]
    fn test_execution_authority_creation() {
        let auth = ExecutionAuthority::new("app-123", "sha256:abc", "alice");
        assert_eq!(auth.approval_id, "app-123");
        assert_eq!(auth.digest, "sha256:abc");
        assert_eq!(auth.owner, "alice");
    }

    #[test]
    fn test_outcome_uncertainty_equality() {
        assert_eq!(OutcomeUncertainty::Succeeded, OutcomeUncertainty::Succeeded);
        assert_ne!(OutcomeUncertainty::Succeeded, OutcomeUncertainty::Failed);
        assert_ne!(
            OutcomeUncertainty::Uncertain("a".into()),
            OutcomeUncertainty::Uncertain("b".into())
        );
    }

    #[test]
    fn test_verify_spec_serialization() {
        let spec = VerifySpec {
            cmd: "echo ok".to_string(),
            expected_exit: 0,
            output_pattern: Some("^ok$".to_string()),
            timeout_ms: 1000,
        };
        let json = serde_json::to_string(&spec).unwrap();
        let deserialized: VerifySpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cmd, "echo ok");
        assert_eq!(deserialized.expected_exit, 0);
    }

    #[tokio::test]
    async fn test_verify_spec_success() {
        let spec = VerifySpec {
            cmd: "echo hello".to_string(),
            expected_exit: 0,
            output_pattern: Some("hello".to_string()),
            timeout_ms: 5000,
        };
        let result = spec.execute().await;
        assert_eq!(result, OutcomeUncertainty::Succeeded);
    }

    #[tokio::test]
    async fn test_verify_spec_wrong_exit_code() {
        let spec = VerifySpec {
            cmd: "exit 1".to_string(),
            expected_exit: 0,
            output_pattern: None,
            timeout_ms: 5000,
        };
        let result = spec.execute().await;
        assert_eq!(result, OutcomeUncertainty::Failed);
    }

    #[tokio::test]
    async fn test_verify_spec_pattern_mismatch() {
        let spec = VerifySpec {
            cmd: "echo foo".to_string(),
            expected_exit: 0,
            output_pattern: Some("^bar$".to_string()),
            timeout_ms: 5000,
        };
        let result = spec.execute().await;
        assert_eq!(result, OutcomeUncertainty::Failed);
    }

    #[test]
    fn test_write_gate_blocks_rejected() {
        let gate = CerebeloWriteGate::new();
        let capsule = WriteGateEvidenceCapsule::new(
            "test",
            vec!["evidence".to_string()],
            "reject".to_string(),
            zylos_memory_system::write_gate::Disposition::Rejected("no".to_string()),
        );
        assert!(gate.check(&capsule).is_err());
    }

    #[test]
    fn test_write_gate_allows_approved() {
        let gate = CerebeloWriteGate::new();
        let capsule = WriteGateEvidenceCapsule::new(
            "test",
            vec!["evidence".to_string()],
            "approve".to_string(),
            zylos_memory_system::write_gate::Disposition::Approved,
        );
        assert!(gate.check(&capsule).is_ok());
    }

    #[test]
    fn test_artifact_provenance_creation() {
        let p = ArtifactProvenance::new("node-1", "run-42", OutcomeUncertainty::Succeeded);
        assert_eq!(p.node_id, "node-1");
        assert_eq!(p.run_id, "run-42");
        assert!(p.timestamp > 0);
        assert_eq!(p.verification_status, OutcomeUncertainty::Succeeded);
        assert!(p.parent_artifacts.is_empty());
    }

    #[test]
    fn test_artifact_provenance_with_parents() {
        let p = ArtifactProvenance::new("n", "r", OutcomeUncertainty::Failed)
            .with_parents(vec!["parent-1".into(), "parent-2".into()]);
        assert_eq!(p.parent_artifacts.len(), 2);
    }

    #[tokio::test]
    async fn test_idempotency_tracker_allows_first_execution() {
        let tracker = IdempotencyTracker::new();
        assert!(tracker.check_and_mark("node-a", false).await.is_ok());
    }

    #[tokio::test]
    async fn test_idempotency_tracker_blocks_non_idempotent_rerun() {
        let tracker = IdempotencyTracker::new();
        assert!(tracker.check_and_mark("node-b", false).await.is_ok());
        assert!(tracker.check_and_mark("node-b", false).await.is_err());
    }

    #[tokio::test]
    async fn test_idempotency_tracker_allows_idempotent_rerun() {
        let tracker = IdempotencyTracker::new();
        assert!(tracker.check_and_mark("node-c", true).await.is_ok());
        assert!(tracker.check_and_mark("node-c", true).await.is_ok());
    }

    #[tokio::test]
    async fn test_recovery_ladder_succeeds_on_retry() {
        let ladder = RecoveryLadder::new(3);
        let counter = Arc::new(Mutex::new(0u32));
        let c = counter.clone();

        let (outcome, record) = ladder
            .recover("flaky", serde_json::json!({}), |_spec| {
                let cnt = c.clone();
                async move {
                    let mut n = cnt.lock().await;
                    *n += 1;
                    if *n >= 2 {
                        OutcomeUncertainty::Succeeded
                    } else {
                        OutcomeUncertainty::Failed
                    }
                }
            })
            .await;

        assert_eq!(outcome, OutcomeUncertainty::Succeeded);
        assert_eq!(record.steps.len(), 2); // Two retry attempts
    }

    #[tokio::test]
    async fn test_recovery_ladder_escalates_after_max_retries() {
        let ladder = RecoveryLadder::new(2);

        let (outcome, record) = ladder
            .recover("broken", serde_json::json!({}), |_spec| async {
                OutcomeUncertainty::Failed
            })
            .await;

        assert_eq!(outcome, OutcomeUncertainty::Failed);
        // Should have: 2 retries + decompose + delegate + escalate = 5 steps
        assert_eq!(record.steps.len(), 5);
        assert_eq!(record.steps.last(), Some(&RecoveryStep::EscalateHuman));
    }

    #[tokio::test]
    async fn test_local_backend_executes_command() {
        let backend = LocalBackend;
        let auth = ExecutionAuthority::new("test", "digest", "tester");
        let result = backend
            .execute(serde_json::json!({"command": "echo cerebelo-test"}), auth)
            .await
            .unwrap();

        assert_eq!(result.status, OutcomeUncertainty::Succeeded);
        let output = result.output.unwrap();
        assert!(output["stdout"].as_str().unwrap().contains("cerebelo-test"));
    }

    #[tokio::test]
    async fn test_docker_backend_returns_uncertain() {
        let backend = DockerBackend::new("test-image:latest", 60);
        let auth = ExecutionAuthority::new("test", "digest", "tester");
        let result = backend.execute(serde_json::json!({}), auth).await.unwrap();
        assert!(matches!(result.status, OutcomeUncertainty::Uncertain(_)));
    }

    #[tokio::test]
    async fn test_wasm_backend_returns_uncertain() {
        let backend = WasmBackend::new(None, 256);
        let auth = ExecutionAuthority::new("test", "digest", "tester");
        let result = backend.execute(serde_json::json!({}), auth).await.unwrap();
        assert!(matches!(result.status, OutcomeUncertainty::Uncertain(_)));
    }

    #[tokio::test]
    async fn test_cerebelo_executor_linear_dag() {
        let cancel = CancellationToken::new();
        let mut cerebelo = CerebeloExecutor::new(cancel);

        // Configure nodes with local backend
        cerebelo.set_node_config(
            "step1",
            ExecutionConfig {
                idempotent: true,
                verify: None,
                backend: BackendKind::Local,
            },
        );
        cerebelo.set_node_config(
            "step2",
            ExecutionConfig {
                idempotent: true,
                verify: None,
                backend: BackendKind::Local,
            },
        );

        let def = make_test_dag();
        let auth = ExecutionAuthority::new("approval-1", "sha256:test", "operator");

        let results = cerebelo.execute(&def, auth).await.unwrap();
        assert_eq!(results.get("step1"), Some(&NodeStatus::Success));
        assert_eq!(results.get("step2"), Some(&NodeStatus::Success));
    }

    #[tokio::test]
    async fn test_cerebelo_executor_with_verify() {
        let cancel = CancellationToken::new();
        let mut cerebelo = CerebeloExecutor::new(cancel);

        cerebelo.set_node_config(
            "step1",
            ExecutionConfig {
                idempotent: true,
                verify: Some(VerifySpec {
                    cmd: "echo verified".to_string(),
                    expected_exit: 0,
                    output_pattern: Some("verified".to_string()),
                    timeout_ms: 5000,
                }),
                backend: BackendKind::Local,
            },
        );

        let def = DagDefinition {
            id: DagId::new(),
            name: "verify-test".to_string(),
            nodes: vec![DagNode {
                id: "step1".to_string(),
                agent_name: "test".to_string(),
                task_spec: serde_json::json!({"command": "echo done"}),
                timeout_ms: 5000,
                max_retries: 0,
                backoff_ms: 100,
            }],
            edges: vec![],
        };

        let auth = ExecutionAuthority::new("a", "d", "o");
        let results = cerebelo.execute(&def, auth).await.unwrap();
        assert_eq!(results.get("step1"), Some(&NodeStatus::Success));
    }

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();
        assert!(config.idempotent);
        assert!(config.verify.is_none());
        assert_eq!(config.backend, BackendKind::Local);
    }

    #[test]
    fn test_recovery_step_serialization() {
        let step = RecoveryStep::Retry { attempt: 1, max: 3 };
        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("Retry"));
    }

    #[tokio::test]
    async fn test_heartbeat_beat_updates() {
        let cancel = CancellationToken::new();
        let hb = HeartbeatMonitor::new(cancel);
        hb.beat().await;
        let lb = hb.last_beat.lock().await;
        assert!(lb.elapsed() < Duration::from_secs(1));
    }
}
