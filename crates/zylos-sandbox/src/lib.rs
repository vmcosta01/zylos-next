//! SANDBOX_ISOLATION: WASM snapshot/resume, Docker per-agent, browser anti-detect, OCR sidecar.
//!
//! This crate provides isolated execution environments for agent code with:
//! - **WASM Sandbox**: Memory-isolated execution via wasmtime with snapshot/resume
//! - **Docker Isolation**: Per-agent container lifecycle management (Phase 4.2)
//! - **Browser Anti-Detect**: Fingerprint rotation and bot detection evasion (Phase 4.3)
//! - **OCR Sidecar**: VLM-based document parsing via dots.mocr (Phase 4.4)

pub mod wasm_sandbox;
pub mod docker_sandbox;
pub mod browser_antidetect;
pub mod browser_profile_store;
pub mod secure_input;

pub use wasm_sandbox::{
    ExecutionResult, GlobalValue, InputFingerprint, NetworkPolicy, SandboxConfig, SandboxError,
    SandboxId, SandboxSnapshot, SnapshotMetadata, WasmSandbox,
};

pub use browser_antidetect::{
    AntiDetectConfig, BrowserProfile, BrowserSandbox, HeadlessMode,
};

pub use browser_profile_store::{
    BrowserProfileStore, PersistedProfile, ProfileStoreConfig, ProfileStoreError,
};

pub use secure_input::{
    SecureInputPipeline, SecureCredential, CredentialVault, InputMask, GhostTypistError,
};
