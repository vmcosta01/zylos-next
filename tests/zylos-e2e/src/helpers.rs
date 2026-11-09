//! Shared test infrastructure for E2E scenarios.
//!
//! Provides gateway process management, HTTP client helpers,
//! temporary database setup, and assertion utilities.

use anyhow::{Context, Result};
use reqwest::Client;
use std::process::{Child, Command};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Test gateway handle — owns the child process and temp data dir.
#[allow(dead_code)]
pub struct TestGateway {
    pub port: u16,
    pub client: Client,
    _child: Child,
    _tmp: TempDir,
}

impl TestGateway {
    /// Spawn a fresh zylos-gateway on a random port with an ephemeral SQLite DB.
    #[allow(dead_code)]
    pub async fn spawn() -> Result<Self> {
        let tmp = TempDir::new().context("create temp dir")?;
        let db_path = tmp.path().join("test.db");
        let port = 18080 + (std::process::id() % 1000) as u16;

        let child = Command::new("cargo")
            .args([
                "run",
                "-p",
                "zylos-gateway",
                "--",
                "--port",
                &port.to_string(),
                "--db",
                db_path.to_str().unwrap(),
            ])
            .current_dir(env!("CARGO_MANIFEST_DIR").replace("/tests/zylos-e2e", ""))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .context("spawn gateway")?;

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        // Wait for health endpoint
        let base = format!("http://127.0.0.1:{}/api/v1/health", port);
        for _ in 0..60 {
            if client.get(&base).send().await.is_ok() {
                return Ok(Self {
                    port,
                    client,
                    _child: child,
                    _tmp: tmp,
                });
            }
            sleep(Duration::from_millis(500)).await;
        }
        anyhow::bail!("gateway did not become healthy within 30s")
    }

    #[allow(dead_code)]
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestGateway {
    fn drop(&mut self) {
        let _ = self._child.kill();
    }
}

/// Assert that a JSON response contains an expected key.
#[allow(dead_code)]
pub fn assert_json_key(json: &serde_json::Value, key: &str) {
    assert!(
        json.get(key).is_some(),
        "expected key '{}' in response: {}",
        key,
        json
    );
}