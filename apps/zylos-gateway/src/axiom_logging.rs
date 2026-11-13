//! Módulo de logging estruturado com integração Axiom (T11.4.5).
//!
//! Envia logs formatados em JSON para o Axiom via HTTP API quando
//! a env var `AXIOM_TOKEN` e `AXIOM_DATASET` estão configuradas.
//! Fallback: logs locais via tracing quando Axiom não está configurado.

use tokio::sync::mpsc;
use tracing::{info, warn};

/// Configuração do cliente Axiom
#[derive(Clone)]
#[allow(dead_code)]
pub struct AxiomConfig {
 pub token: String,
 pub dataset: String,
 pub endpoint: String,
}

impl AxiomConfig {
    /// Carrega configuração de env vars. Retorna None se não configurado.
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("AXIOM_TOKEN").ok().filter(|t| !t.is_empty())?;
        let dataset = std::env::var("AXIOM_DATASET").ok().filter(|d| !d.is_empty())?;
        let endpoint = std::env::var("AXIOM_ENDPOINT")
            .unwrap_or_else(|_| "https://api.axiom.co/v1/datasets".to_string());

        Some(Self {
            token,
            dataset,
            endpoint,
        })
    }
}

/// Log entry estruturado para envio ao Axiom
#[derive(serde::Serialize)]
struct AxiomLogEntry {
    #[serde(rename = "_time")]
    timestamp: String,
    level: String,
    message: String,
    service: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
}

/// Cliente Axiom assíncrono com buffer interno
#[derive(Clone)]
#[allow(dead_code)]
pub struct AxiomClient {
 config: AxiomConfig,
 client: reqwest::Client,
 tx: mpsc::Sender<AxiomLogEntry>,
}

impl AxiomClient {
    /// Cria novo cliente Axiom com worker em background.
    /// Retorna None se AxiomConfig não estiver configurado.
    pub fn new() -> Option<Self> {
        let config = AxiomConfig::from_env()?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build reqwest client");

        let (tx, rx) = mpsc::channel::<AxiomLogEntry>(1000);

        let worker_config = config.clone();
        let worker_client = client.clone();
        tokio::spawn(async move {
            Self::worker_loop(rx, worker_config, worker_client).await;
        });

        info!(
            dataset = %config.dataset,
            endpoint = %config.endpoint,
            "Axiom logging initialized"
        );

        Some(Self { config, client, tx })
    }

    /// Envia log estruturado para o buffer. Não bloqueia se buffer cheio.
    pub fn send_log(
        &self,
        level: &str,
        message: &str,
        request_id: Option<&str>,
        method: Option<&str>,
        path: Option<&str>,
        status: Option<u16>,
        latency_ms: Option<u64>,
    ) {
        let entry = AxiomLogEntry {
            timestamp: chrono_like_timestamp(),
            level: level.to_string(),
            message: message.to_string(),
            service: "zylos-gateway".to_string(),
            request_id: request_id.map(|s| s.to_string()),
            method: method.map(|s| s.to_string()),
            path: path.map(|s| s.to_string()),
            status,
            latency_ms,
        };

        if let Err(_) = self.tx.try_send(entry) {
            warn!("Axiom log buffer full, dropping log entry");
        }
    }

    /// Worker loop que envia batches de logs para o Axiom API.
    async fn worker_loop(
        mut rx: mpsc::Receiver<AxiomLogEntry>,
        config: AxiomConfig,
        client: reqwest::Client,
    ) {
        let url = format!("{}/{}/ingest", config.endpoint, config.dataset);
        let mut batch: Vec<AxiomLogEntry> = Vec::with_capacity(100);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));

        loop {
            tokio::select! {
                Some(entry) = rx.recv() => {
                    batch.push(entry);
                    // Flush se batch cheio
                    if batch.len() >= 100 {
                        Self::flush_batch(&client, &url, &config.token, &mut batch).await;
                    }
                }
                _ = interval.tick() => {
                    // Flush periódico
                    if !batch.is_empty() {
                        Self::flush_batch(&client, &url, &config.token, &mut batch).await;
                    }
                }
                else => {
                    // Channel fechado, flush final
                    if !batch.is_empty() {
                        Self::flush_batch(&client, &url, &config.token, &mut batch).await;
                    }
                    break;
                }
            }
        }
    }

    /// Envia batch de logs para o Axiom ingest API.
    async fn flush_batch(
        client: &reqwest::Client,
        url: &str,
        token: &str,
        batch: &mut Vec<AxiomLogEntry>,
    ) {
        if batch.is_empty() {
            return;
        }

        let events: Vec<_> = batch.drain(..).collect();
        let body = serde_json::json!({ "events": events });

        match client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                // Batch enviado com sucesso
            }
            Ok(resp) => {
                warn!(
                    status = %resp.status(),
                    "Axiom ingest failed"
                );
            }
            Err(e) => {
                warn!(error = %e, "Axiom ingest request failed");
            }
        }
    }
}

/// Gera timestamp ISO 8601 sem dependência de chrono.
fn chrono_like_timestamp() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    // Formato simplificado: epoch seconds como string
    // Axiom aceita epoch Unix no campo _time
    secs.to_string()
}