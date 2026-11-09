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

/// Erros produzidos por adapters de canal durante webhook, parsing ou envio.
#[derive(Debug, Error)]
pub enum ChannelError {
    /// Falha na verificação de assinatura ou challenge do webhook.
    #[error("webhook verification failed: {0}")]
    WebhookVerification(String),
    /// Formato de mensagem inválido ou inesperado.
    #[error("invalid message format: {0}")]
    InvalidMessage(String),
    /// Erro retornado pela API do provedor externo.
    #[error("API error from {provider}: {message}")]
    ApiError { provider: String, message: String },
    /// Limite de taxa excedido; aguardar `retry_after` segundos.
    #[error("rate limit exceeded for {provider}, retry after {retry_after}s")]
    RateLimited { provider: String, retry_after: u64 },
    /// Erro durante criptografia ou descriptografia de payload.
    #[error("encryption error: {0}")]
    Encryption(String),
    /// Sessão não encontrada para o usuário/canal especificado.
    #[error("session not found: {0}")]
    SessionNotFound(String),
    /// Erro de I/O subjacente.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Erro de serialização/deserialização de payload.
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Mensagem recebida de um canal externo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Identificador do canal (e.g., "wechat", "feishu").
    pub channel_type: String,
    /// ID do usuário no sistema externo.
    pub external_user_id: String,
    /// ID único da mensagem no provedor.
    pub message_id: String,
    /// Conteúdo da mensagem (texto, mídia, card, etc.).
    pub content: MessageContent,
    /// Timestamp Unix da mensagem.
    pub timestamp: i64,
    /// Metadados adicionais específicos do canal.
    pub metadata: serde_json::Value,
}

/// Conteúdo da mensagem (suporta múltiplos tipos).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    /// Mensagem de texto simples.
    Text { body: String },
    /// Imagem com URL e legenda opcional.
    Image { url: String, caption: Option<String> },
    /// Arquivo anexado com metadados.
    File { url: String, filename: String, size_bytes: u64 },
    /// Mensagem de voz com duração.
    Voice { url: String, duration_ms: u64 },
    /// Vídeo com duração.
    Video { url: String, duration_ms: u64 },
    /// Card interativo renderizado pelo provedor.
    InteractiveCard { card_json: String },
    /// Template pré-aprovado com dados dinâmicos.
    Template { template_id: String, data: serde_json::Value },
}

/// Mensagem a ser enviada para um canal externo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Conteúdo da mensagem de saída.
    pub content: MessageContent,
    /// ID da mensagem original à qual esta responde, se aplicável.
    pub reply_to: Option<String>,
}

/// Sessão de usuário vinculada a um canal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelSession {
    /// ID único da sessão (UUID v4).
    pub id: String,
    /// Tipo de canal (e.g., "wechat", "feishu").
    pub channel_type: String,
    /// ID do usuário no sistema externo.
    pub external_user_id: String,
    /// ID do usuário correspondente no zylos.
    pub zylos_user_id: String,
    /// Metadados adicionais da sessão.
    pub metadata: serde_json::Value,
    /// Timestamp Unix da última atividade.
    pub last_active_at: i64,
    /// Timestamp Unix de criação da sessão.
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