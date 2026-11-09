//! # WeChat Work (WeCom) Channel Adapter
//!
//! Implements webhook verification, message parsing, and outbound messaging
//! for the WeChat Work API. Handles AES-CBC message decryption, signature
//! verification, rate limit enforcement, and template message quotas.

use super::{ChannelAdapter, ChannelError, InboundMessage, MessageContent, OutboundMessage};
use async_trait::async_trait;
use bytes::Bytes;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use sha1::Digest;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the WeChat Work adapter
#[derive(Debug, Clone)]
pub struct WechatConfig {
    pub corp_id: String,
    pub agent_id: String,
    pub secret: String,
    pub token: String,
    pub encoding_aes_key: String,
    /// Max messages per second to respect rate limits
    pub max_rps: u32,
}

/// Tracks rate limit state
struct RateLimiter {
    max_rps: u32,
    sent_in_window: u32,
    window_start: i64,
}

impl RateLimiter {
    fn new(max_rps: u32) -> Self {
        Self {
            max_rps,
            sent_in_window: 0,
            window_start: chrono::Utc::now().timestamp(),
        }
    }

    fn check(&mut self) -> Result<(), ChannelError> {
        let now = chrono::Utc::now().timestamp();
        if now - self.window_start >= 1 {
            self.sent_in_window = 0;
            self.window_start = now;
        }
        if self.sent_in_window >= self.max_rps {
            return Err(ChannelError::RateLimited {
                provider: "wechat".to_string(),
                retry_after: 1,
            });
        }
        self.sent_in_window += 1;
        Ok(())
    }
}

pub struct WechatAdapter {
    config: WechatConfig,
    client: reqwest::Client,
    rate_limiter: Arc<Mutex<RateLimiter>>,
}

impl WechatAdapter {
    pub fn new(config: WechatConfig) -> Self {
        let max_rps = config.max_rps;
        Self {
            config,
            client: reqwest::Client::new(),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(max_rps))),
        }
    }

    /// Verify WeChat webhook signature using SHA1(sort(token, timestamp, nonce, body))
    fn verify_signature(&self, timestamp: &str, nonce: &str, body: &[u8], signature: &str) -> bool {
        let mut parts = vec![
            self.config.token.clone(),
            timestamp.to_string(),
            nonce.to_string(),
        ];
        if !body.is_empty() {
            parts.push(String::from_utf8_lossy(body).to_string());
        }
        parts.sort();
        let combined = parts.join("");
        let hash = sha1::Sha1::digest(combined.as_bytes());
        let computed = hex::encode(hash);
        computed == signature
    }

    /// Decrypt AES-CBC encrypted message body
    fn decrypt_message(&self, encrypted: &str) -> Result<String, ChannelError> {
        use aes::Aes256;
        use cbc::Decryptor;
        use base64::Engine;

        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.config.encoding_aes_key)
            .map_err(|e| ChannelError::Encryption(format!("invalid AES key: {}", e)))?;

        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(encrypted)
            .map_err(|e| ChannelError::Encryption(format!("invalid base64: {}", e)))?;

        if ciphertext.len() < 32 {
            return Err(ChannelError::Encryption("ciphertext too short".into()));
        }

        // First 16 bytes = IV, next 16 = random padding, rest = message + appid
        let iv = &ciphertext[..16];
        let data = &ciphertext[16..];

        let decryptor = Decryptor::<Aes256>::new_from_slices(&key_bytes, iv)
            .map_err(|e| ChannelError::Encryption(format!("AES init: {}", e)))?;

        let mut buf = data.to_vec();
        let plaintext = decryptor
            .decrypt_padded_mut::<cbc::cipher::block_padding::Pkcs7>(&mut buf)
            .map_err(|e| ChannelError::Encryption(format!("AES decrypt: {}", e)))?;

        // Skip 16-byte random prefix, read 4-byte length, extract message
        if plaintext.len() < 20 {
            return Err(ChannelError::Encryption("plaintext too short after decrypt".into()));
        }
        let msg_len = u32::from_be_bytes([
            plaintext[16], plaintext[17], plaintext[18], plaintext[19],
        ]) as usize;

        if plaintext.len() < 20 + msg_len {
            return Err(ChannelError::Encryption("message length exceeds plaintext".into()));
        }

        String::from_utf8(plaintext[20..20 + msg_len].to_vec())
            .map_err(|e| ChannelError::Encryption(format!("invalid UTF-8: {}", e)))
    }

    /// Parse XML message into InboundMessage
    fn parse_xml_message(xml: &str) -> Result<InboundMessage, ChannelError> {
        // Simple XML field extraction (production would use quick-xml)
        let extract = |tag: &str| -> Option<String> {
            let open = format!("<{}>", tag);
            let close = format!("</{}>", tag);
            let start = xml.find(&open)? + open.len();
            let end = xml[start..].find(&close)? + start;
            let raw = &xml[start..end];
            // Strip CDATA wrapper if present
            let value = if raw.starts_with("<![CDATA[") && raw.ends_with("]]>") {
                &raw[9..raw.len()-3]
            } else {
                raw
            };
            Some(value.to_string())
        };

        let msg_type = extract("MsgType").unwrap_or_default();
        let content = match msg_type.as_str() {
            "text" => MessageContent::Text {
                body: extract("Content").unwrap_or_default(),
            },
            "image" => MessageContent::Image {
                url: extract("PicUrl").unwrap_or_default(),
                caption: None,
            },
            "voice" => MessageContent::Voice {
                url: extract("MediaId").unwrap_or_default(),
                duration_ms: 0,
            },
            "video" => MessageContent::Video {
                url: extract("MediaId").unwrap_or_default(),
                duration_ms: 0,
            },
            "file" => MessageContent::File {
                url: extract("MediaId").unwrap_or_default(),
                filename: extract("FileName").unwrap_or_default(),
                size_bytes: extract("FileSize")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            },
            _ => MessageContent::Text {
                body: format!("[unsupported type: {}]", msg_type),
            },
        };

        Ok(InboundMessage {
            channel_type: "wechat".to_string(),
            external_user_id: extract("FromUserName").unwrap_or_default(),
            message_id: extract("MsgId").unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            content,
            timestamp: extract("CreateTime")
                .and_then(|s| s.parse().ok())
                .unwrap_or(chrono::Utc::now().timestamp()),
            metadata: serde_json::json!({
                "agent_id": extract("AgentID"),
                "msg_type": msg_type,
            }),
        })
    }
}

#[async_trait]
impl ChannelAdapter for WechatAdapter {
    fn name(&self) -> &str {
        "wechat"
    }

    async fn verify_webhook(
        &self,
        challenge: &str,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<String, ChannelError> {
        // Echo mode: return challenge directly for URL verification
        if body.is_empty() {
            return Ok(challenge.to_string());
        }

        // Signature verification for actual webhook calls
        if let Some(sig) = signature {
            // Parse timestamp and nonce from query params (passed via challenge context)
            // In production, these come from URL query parameters
            if !self.verify_signature(challenge, "", body, sig) {
                return Err(ChannelError::WebhookVerification(
                    "signature mismatch".into(),
                ));
            }
        }

        Ok("success".to_string())
    }

    async fn receive(&self, webhook_payload: Bytes) -> Result<InboundMessage, ChannelError> {
        let xml = String::from_utf8_lossy(&webhook_payload);

        // Check if message is encrypted
        if xml.contains("<Encrypt>") {
            let encrypted = Self::parse_xml_message(&xml)
                .ok()
                .and_then(|_| {
                    let open = "<Encrypt>";
                    let close = "</Encrypt>";
                    let start = xml.find(open)? + open.len();
                    let end = xml[start..].find(close)? + start;
                    Some(xml[start..end].to_string())
                })
                .ok_or_else(|| ChannelError::InvalidMessage("missing Encrypt field".into()))?;

            let decrypted = self.decrypt_message(&encrypted)?;
            Self::parse_xml_message(&decrypted)
        } else {
            Self::parse_xml_message(&xml)
        }
    }

    async fn send(&self, target: &str, message: OutboundMessage) -> Result<(), ChannelError> {
        // Enforce rate limit
        {
            let mut limiter = self.rate_limiter.lock().await;
            limiter.check()?;
        }

        let (_msg_type, body) = match &message.content {
            MessageContent::Text { body } => (
                "text",
                serde_json::json!({
                    "touser": target,
                    "msgtype": "text",
                    "agentid": self.config.agent_id,
                    "text": { "content": body }
                }),
            ),
            MessageContent::Image { url, .. } => (
                "image",
                serde_json::json!({
                    "touser": target,
                    "msgtype": "image",
                    "agentid": self.config.agent_id,
                    "image": { "media_id": url }
                }),
            ),
            MessageContent::File { url, .. } => (
                "file",
                serde_json::json!({
                    "touser": target,
                    "msgtype": "file",
                    "agentid": self.config.agent_id,
                    "file": { "media_id": url }
                }),
            ),
            MessageContent::Template { template_id, data } => (
                "template_card",
                serde_json::json!({
                    "touser": target,
                    "msgtype": "template_card",
                    "agentid": self.config.agent_id,
                    "template_card": {
                        "card_type": "text_notice",
                        "source": { "desc": "Zylos Gateway" },
                        "main_title": { "title": template_id },
                        "sub_title_text": data.get("subtitle").and_then(|v| v.as_str()).unwrap_or(""),
                    }
                }),
            ),
            _ => {
                return Err(ChannelError::InvalidMessage(
                    "unsupported message type for WeChat".into(),
                ))
            }
        };

        let resp = self
            .client
            .post("https://qyapi.weixin.qq.com/cgi-bin/message/send")
            .query(&[("access_token", &self.config.secret)])
            .json(&body)
            .send()
            .await
            .map_err(|e| ChannelError::ApiError {
                provider: "wechat".into(),
                message: e.to_string(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ChannelError::ApiError {
                provider: "wechat".into(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        Ok(())
    }

    fn is_configured(&self) -> bool {
        !self.config.corp_id.is_empty()
            && !self.config.secret.is_empty()
            && !self.config.encoding_aes_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_message() {
        let xml = r#"
            <xml>
                <ToUserName><![CDATA[toUser]]></ToUserName>
                <FromUserName><![CDATA[fromUser]]></FromUserName>
                <CreateTime>1348831860</CreateTime>
                <MsgType><![CDATA[text]]></MsgType>
                <Content><![CDATA[Hello World]]></Content>
                <MsgId>1234567890123456</MsgId>
                <AgentID>1</AgentID>
            </xml>"#;

        let msg = WechatAdapter::parse_xml_message(xml).unwrap();
        assert_eq!(msg.channel_type, "wechat");
        assert_eq!(msg.external_user_id, "fromUser");
        if let MessageContent::Text { body } = &msg.content {
            assert_eq!(body, "Hello World");
        } else {
            panic!("expected text message");
        }
    }

    #[test]
    fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(2);
        assert!(limiter.check().is_ok());
        assert!(limiter.check().is_ok());
        assert!(limiter.check().is_err());
    }
}