//! Slack Channel Adapter (T11.8.2)
//!
//! Implements `ChannelAdapter` for Slack via Bot Token + Web API.
//! Supports: send_message, list_users, list_chats, health_check.
//! Calendar and departments are not applicable to Slack (return empty).

use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;

use crate::channel::{
    CalendarEvent, ChatInfo, ChannelAdapter, DepartmentInfo, MessageId, OutboundMessage,
    SendResult, UserInfo,
};
use crate::error::{IntegrationError, Result};

/// Slack Bot adapter using Web API with Bot User OAuth Token.
pub struct SlackAdapter {
    bot_token: String,
    client: Client,
    base_url: String,
}

impl SlackAdapter {
    /// Create a new Slack adapter with the given Bot User OAuth Token.
    ///
    /// Token should start with `xoxb-` and have scopes:
    /// `chat:write`, `users:read`, `channels:read`, `groups:read`.
    pub fn new(bot_token: String) -> Self {
        Self {
            bot_token,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            base_url: "https://slack.com/api".to_string(),
        }
    }

    /// Create adapter with custom base URL (for testing/proxy).
    #[allow(dead_code)]
    pub fn with_base_url(bot_token: String, base_url: String) -> Self {
        Self {
            bot_token,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            base_url,
        }
    }

    async fn api_get(&self, method: &str, params: &[(&str, &str)]) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, method);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .query(params)
            .send()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("HTTP request failed: {}", e),
            })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("JSON parse failed: {}", e),
            }
        })?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown_error");
            return Err(IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("Slack API error: {}", error),
            });
        }

        Ok(body)
    }

    async fn api_post_json(
        &self,
        method: &str,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value> {
        let url = format!("{}/{}", self.base_url, method);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.bot_token))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(payload)
            .send()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("HTTP request failed: {}", e),
            })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("JSON parse failed: {}", e),
            }
        })?;

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let error = body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown_error");
            return Err(IntegrationError::ProviderExecution {
                provider: "slack".to_string(),
                action: method.to_string(),
                detail: format!("Slack API error: {}", error),
            });
        }

        Ok(body)
    }
}

#[async_trait]
impl ChannelAdapter for SlackAdapter {
    fn platform_id(&self) -> &'static str {
        "slack"
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult> {
        let payload = serde_json::json!({
            "channel": msg.recipient,
            "text": msg.content,
        });

        let body = self.api_post_json("chat.postMessage", &payload).await?;

        let ts = body
            .pointer("/ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(SendResult {
            message_id: MessageId(ts),
            success: true,
            error: None,
            raw_response: Some(body),
        })
    }

    async fn list_users(
        &self,
        _page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Vec<UserInfo>> {
        let limit = page_size.unwrap_or(200).to_string();
        let body = self
            .api_get("users.list", &[("limit", &limit)])
            .await?;

        let members = body
            .get("members")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let users: Vec<UserInfo> = members
            .iter()
            .filter(|m| m.get("deleted").and_then(|v| v.as_bool()) != Some(true))
            .map(|m| {
                let profile = m.get("profile").cloned().unwrap_or_default();
                UserInfo {
                    user_id: m
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: m
                        .get("real_name")
                        .or_else(|| m.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    email: profile
                        .get("email")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    avatar_url: profile
                        .get("image_192")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    department_ids: vec![],
                    extra: HashMap::new(),
                }
            })
            .collect();

        Ok(users)
    }

    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
        let body = self
            .api_get("users.info", &[("user", user_id)])
            .await?;

        let user = body.get("user").cloned().unwrap_or_default();
        let profile = user.get("profile").cloned().unwrap_or_default();

        Ok(UserInfo {
            user_id: user
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(user_id)
                .to_string(),
            name: user
                .get("real_name")
                .or_else(|| user.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            email: profile
                .get("email")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            avatar_url: profile
                .get("image_192")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            department_ids: vec![],
            extra: HashMap::new(),
        })
    }

    async fn list_chats(
        &self,
        _page: Option<u32>,
        page_size: Option<u32>,
    ) -> Result<Vec<ChatInfo>> {
        let limit = page_size.unwrap_or(200).to_string();
        let body = self
            .api_get(
                "conversations.list",
                &[
                    ("types", "public_channel,private_channel"),
                    ("limit", &limit),
                ],
            )
            .await?;

        let channels = body
            .get("channels")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let chats: Vec<ChatInfo> = channels
            .iter()
            .map(|c| ChatInfo {
                chat_id: c
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: c
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                member_count: c
                    .get("num_members")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize),
                owner_id: c
                    .get("creator")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                extra: HashMap::new(),
            })
            .collect();

        Ok(chats)
    }

    async fn list_departments(&self, _parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>> {
        // Slack doesn't have departments concept — return empty
        Ok(vec![])
    }

    async fn list_calendar_events(
        &self,
        _start_time: &str,
        _end_time: &str,
    ) -> Result<Vec<CalendarEvent>> {
        // Slack calendar is not accessible via Bot Token Web API
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool> {
        match self.api_get("auth.test", &[]).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slack_adapter_creation() {
        let adapter = SlackAdapter::new("xoxb-test-token".to_string());
        assert_eq!(adapter.platform_id(), "slack");
    }

    #[tokio::test]
    async fn test_health_check_without_valid_token() {
        let adapter = SlackAdapter::new("xoxb-invalid".to_string());
        let result = adapter.health_check().await;
        // Should return Ok(false) rather than Err when token is invalid
        assert_eq!(result.unwrap(), false);
    }
}