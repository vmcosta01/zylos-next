//! # Channel Adapters
//!
//! Adapters para canais de mensagens externos (WeChat, Feishu/Lark).
//! Cada adapter implementa `ChannelAdapter` com verificação de webhook,
//! parsing de mensagens inbound e envio de mensagens outbound.

pub mod wechat;
pub mod feishu;

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("webhook verification failed: {0}")]
    WebhookVerification(String),
    #[error("invalid message format: {0}")]
    InvalidMessage(String),
    #[error("API error from {provider}: {message}")]
    ApiError { provider: String, message: String },
    #[error("rate limit exceeded for {provider}, retry after {retry_after}s")]
    RateLimited { provider: String, retry_after: u64 },
    #[error("encryption error: {0}")]
    Encryption(String),
    #[error("session not found: {0}")]
    SessionNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Mensagem recebida de um canal externo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel_type: String,
    pub external_user_id: String,
    pub message_id: String,
    pub content: MessageContent,
    pub timestamp: i64,
    pub metadata: serde_json::Value,
}

/// Conteúdo da mensagem (suporta múltiplos tipos)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text { body: String },
    Image { url: String, caption: Option<String> },
    File { url: String, filename: String, size_bytes: u64 },
    Voice { url: String, duration_ms: u64 },
    Video { url: String, duration_ms: u64 },
    InteractiveCard { card_json: String },
    Template { template_id: String, data: serde_json::Value },
}

/// Mensagem a ser enviada para um canal externo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub content: MessageContent,
    pub reply_to: Option<String>,
}

/// Sessão de usuário vinculada a um canal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSession {
    pub id: String,
    pub channel_type: String,
    pub external_user_id: String,
    pub zylos_user_id: String,
    pub metadata: serde_json::Value,
    pub last_active_at: i64,
    pub created_at: i64,
}

/// Trait que todo adapter de canal deve implementar
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Nome do canal (e.g., "wechat", "feishu")
    fn name(&self) -> &str;

    /// Verifica a assinatura/challenge do webhook
    async fn verify_webhook(&self, challenge: &str, signature: Option<&str>, body: &[u8]) -> Result<String, ChannelError>;

    /// Faz parse de um payload de webhook em uma InboundMessage
    async fn receive(&self, webhook_payload: Bytes) -> Result<InboundMessage, ChannelError>;

    /// Envia uma mensagem para o target especificado
    async fn send(&self, target: &str, message: OutboundMessage) -> Result<(), ChannelError>;

    /// Retorna se o adapter está configurado e pronto
    fn is_configured(&self) -> bool;
}