//! # Feishu (Lark) Channel Adapter
//!
//! Implements webhook verification, event subscription v2.0, interactive card
//! callbacks, and outbound messaging for the Feishu/Lark platform. Supports
//! HITL approval escalation via native Feishu approval instances.

use super::{ChannelAdapter, ChannelError, InboundMessage, MessageContent, OutboundMessage};
use async_trait::async_trait;
use bytes::Bytes;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Configuration for the Feishu adapter
#[derive(Debug, Clone)]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
    pub verification_token: String,
    pub encrypt_key: Option<String>,
    /// Max messages per second
    pub max_rps: u32,
}

/// Rate limiter shared across requests
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
                provider: "feishu".to_string(),
                retry_after: 1,
            });
        }
        self.sent_in_window += 1;
        Ok(())
    }
}

/// Feishu event subscription v2.0 schema
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FeishuEvent {
    schema: Option<String>,
    header: Option<FeishuEventHeader>,
    event: Option<serde_json::Value>,
    /// Challenge string for URL verification
    challenge: Option<String>,
    /// Verification token for legacy verification
    token: Option<String>,
    #[serde(rename = "type")]
    event_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FeishuEventHeader {
    event_id: String,
    event_type: String,
    create_time: String,
    token: String,
    app_id: String,
    tenant_key: Option<String>,
}

/// Interactive card callback payload
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CardCallback {
    open_id: Option<String>,
    user_id: Option<String>,
    action: Option<CardAction>,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CardAction {
    value: serde_json::Value,
    tag: String,
}

pub struct FeishuAdapter {
    config: FeishuConfig,
    client: reqwest::Client,
    rate_limiter: Arc<Mutex<RateLimiter>>,
    /// Cached tenant access token
    access_token: Arc<Mutex<Option<(String, i64)>>>,
}

impl FeishuAdapter {
    pub fn new(config: FeishuConfig) -> Self {
        let max_rps = config.max_rps;
        Self {
            config,
            client: reqwest::Client::new(),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(max_rps))),
            access_token: Arc::new(Mutex::new(None)),
        }
    }

    /// Verify Feishu webhook signature using HMAC-SHA256
    fn verify_signature(&self, timestamp: &str, nonce: &str, body: &[u8], signature: &str) -> bool {
        if let Some(ref key) = self.config.encrypt_key {
            let data = format!("{}{}{}{}", timestamp, nonce, key, String::from_utf8_lossy(body));
            let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(data.as_bytes());
            let result = mac.finalize();
            let computed = hex::encode(result.into_bytes());
            computed == signature
        } else {
            // No encrypt key configured — skip signature verification
            true
        }
    }

    /// Get or refresh tenant access token
    async fn get_access_token(&self) -> Result<String, ChannelError> {
        let mut cached = self.access_token.lock().await;
        let now = chrono::Utc::now().timestamp();
        if let Some((ref token, expires_at)) = *cached {
            if expires_at > now + 60 {
                return Ok(token.clone());
            }
        }

        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({
                "app_id": self.config.app_id,
                "app_secret": self.config.app_secret,
            }))
            .send()
            .await
            .map_err(|e| ChannelError::ApiError {
                provider: "feishu".into(),
                message: e.to_string(),
            })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| ChannelError::ApiError {
            provider: "feishu".into(),
            message: format!("token response parse: {}", e),
        })?;

        let token = body["tenant_access_token"]
            .as_str()
            .ok_or_else(|| ChannelError::ApiError {
                provider: "feishu".into(),
                message: "missing tenant_access_token in response".into(),
            })?
            .to_string();

        let expire = body["expire"].as_i64().unwrap_or(7200);
        *cached = Some((token.clone(), now + expire));

        Ok(token)
    }

    /// Parse a Feishu IM event into InboundMessage
    fn parse_im_event(event: &serde_json::Value) -> Result<InboundMessage, ChannelError> {
        let sender = &event["sender"]["sender_id"]["open_id"];
        let msg = &event["message"];

        let msg_type = msg["message_type"].as_str().unwrap_or("text");
        let raw_content = msg["content"].as_str().unwrap_or("{}");
        let content_value: serde_json::Value =
            serde_json::from_str(raw_content).unwrap_or(serde_json::Value::Null);

        let content = match msg_type {
            "text" => MessageContent::Text {
                body: content_value["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
            },
            "image" => MessageContent::Image {
                url: content_value["image_key"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                caption: None,
            },
            "file" => MessageContent::File {
                url: content_value["file_key"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                filename: content_value["file_name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                size_bytes: 0,
            },
            "interactive" => MessageContent::InteractiveCard {
                card_json: raw_content.to_string(),
            },
            _ => MessageContent::Text {
                body: format!("[unsupported feishu type: {}]", msg_type),
            },
        };

        let create_time = msg["create_time"]
            .as_str()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(chrono::Utc::now().timestamp());

        Ok(InboundMessage {
            channel_type: "feishu".to_string(),
            external_user_id: sender.as_str().unwrap_or("").to_string(),
            message_id: msg["message_id"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            content,
            timestamp: create_time,
            metadata: serde_json::json!({
                "chat_id": msg["chat_id"],
                "chat_type": msg["chat_type"],
                "msg_type": msg_type,
            }),
        })
    }

    /// Build an interactive card JSON for HITL approval buttons
    pub fn build_approval_card(
        title: &str,
        description: &str,
        request_id: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "config": { "wide_screen_mode": true },
            "header": {
                "title": { "tag": "plain_text", "content": title },
                "template": "orange"
            },
            "elements": [
                { "tag": "markdown", "content": description },
                {
                    "tag": "action",
                    "actions": [
                        {
                            "tag": "button",
                            "text": { "tag": "plain_text", "content": "✅ Approve" },
                            "type": "primary",
                            "value": { "action": "approve", "request_id": request_id }
                        },
                        {
                            "tag": "button",
                            "text": { "tag": "plain_text", "content": "❌ Reject" },
                            "type": "danger",
                            "value": { "action": "reject", "request_id": request_id }
                        }
                    ]
                }
            ]
        })
    }
}

#[async_trait]
impl ChannelAdapter for FeishuAdapter {
    fn name(&self) -> &str {
        "feishu"
    }

    async fn verify_webhook(
        &self,
        _challenge: &str,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<String, ChannelError> {
        // Parse the body to check for challenge response
        if let Ok(event) = serde_json::from_slice::<FeishuEvent>(body) {
            // URL verification challenge
            if let Some(ch) = event.challenge {
                // Verify token matches
                if let Some(ref token) = event.token {
                    if token != &self.config.verification_token {
                        return Err(ChannelError::WebhookVerification(
                            "verification token mismatch".into(),
                        ));
                    }
                }
                return Ok(serde_json::json!({"challenge": ch}).to_string());
            }

            // Event signature verification
            if let Some(sig) = signature {
                if let Some(ref header) = event.header {
                    if !self.verify_signature(&header.create_time, &header.event_id, body, sig) {
                        return Err(ChannelError::WebhookVerification(
                            "signature mismatch".into(),
                        ));
                    }
                }
            }
        }

        Ok("{}".to_string())
    }

    async fn receive(&self, webhook_payload: Bytes) -> Result<InboundMessage, ChannelError> {
        let event: FeishuEvent = serde_json::from_slice(&webhook_payload)
            .map_err(|e| ChannelError::InvalidMessage(format!("JSON parse: {}", e)))?;

        // Handle challenge (shouldn't reach here normally)
        if let Some(ch) = event.challenge {
            return Err(ChannelError::InvalidMessage(format!(
                "unexpected challenge in receive: {}",
                ch
            )));
        }

        // Handle IM message events
        if let Some(ref evt) = event.event {
            if let Some(ref header) = event.header {
                if header.event_type == "im.message.receive_v1" {
                    return Self::parse_im_event(evt);
                }
            }
        }

        Err(ChannelError::InvalidMessage(
            "unrecognized Feishu event type".into(),
        ))
    }

    async fn send(&self, target: &str, message: OutboundMessage) -> Result<(), ChannelError> {
        // Enforce rate limit
        {
            let mut limiter = self.rate_limiter.lock().await;
            limiter.check()?;
        }

        let token = self.get_access_token().await?;

        let (msg_type, content) = match &message.content {
            MessageContent::Text { body } => (
                "text",
                serde_json::json!({"text": body}).to_string(),
            ),
            MessageContent::Image { url, .. } => (
                "image",
                serde_json::json!({"image_key": url}).to_string(),
            ),
            MessageContent::InteractiveCard { card_json } => (
                "interactive",
                card_json.clone(),
            ),
            _ => {
                return Err(ChannelError::InvalidMessage(
                    "unsupported message type for Feishu".into(),
                ))
            }
        };

        let payload = serde_json::json!({
            "receive_id": target,
            "msg_type": msg_type,
            "content": content,
        });

        let resp = self
            .client
            .post("https://open.feishu.cn/open-apis/im/v1/messages")
            .query(&[("receive_id_type", "open_id")])
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::ApiError {
                provider: "feishu".into(),
                message: e.to_string(),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(ChannelError::ApiError {
                provider: "feishu".into(),
                message: format!("HTTP {}: {}", status, text),
            });
        }

        Ok(())
    }

    fn is_configured(&self) -> bool {
        !self.config.app_id.is_empty() && !self.config.app_secret.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_approval_card() {
        let card = FeishuAdapter::build_approval_card(
            "Deploy Approval",
            "Agent wants to deploy v2.3.1 to production",
            "req-abc-123",
        );
        assert_eq!(card["header"]["title"]["content"], "Deploy Approval");
        let actions = card["elements"][1]["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0]["value"]["action"], "approve");
        assert_eq!(actions[1]["value"]["action"], "reject");
    }

    #[test]
    fn test_parse_im_text_event() {
        let event = serde_json::json!({
            "sender": { "sender_id": { "open_id": "ou_abc123" } },
            "message": {
                "message_id": "om_xyz789",
                "message_type": "text",
                "content": "{\"text\":\"Hello from Feishu\"}",
                "create_time": "1700000000",
                "chat_id": "oc_chat1",
                "chat_type": "p2p"
            }
        });

        let msg = FeishuAdapter::parse_im_event(&event).unwrap();
        assert_eq!(msg.channel_type, "feishu");
        assert_eq!(msg.external_user_id, "ou_abc123");
        assert_eq!(msg.message_id, "om_xyz789");
        if let MessageContent::Text { body } = &msg.content {
            assert_eq!(body, "Hello from Feishu");
        } else {
            panic!("expected text message");
        }
    }
}