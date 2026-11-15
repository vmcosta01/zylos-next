//! Notion Channel Adapter (T11.8.3)
//!
//! Implements `ChannelAdapter` for Notion via Integration Token + API v1.
//! Supports: send_message (as page comment), list_users, list_chats (databases), health_check.
//! Departments and calendar are not applicable to Notion (return empty).

use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;

use crate::channel::{
    CalendarEvent, ChannelAdapter, ChatInfo, DepartmentInfo, MessageId, OutboundMessage,
    SendResult, UserInfo,
};
use crate::error::{IntegrationError, Result};

const NOTION_API_VERSION: &str = "2022-06-28";

/// Notion adapter using Internal Integration Token.
pub struct NotionAdapter {
    token: String,
    client: Client,
    base_url: String,
}

impl NotionAdapter {
    /// Create a new Notion adapter with an Internal Integration Token.
    ///
    /// Token should start with `ntn_` or `secret_`.
    /// Integration must be connected to relevant pages/databases.
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            base_url: "https://api.notion.com/v1".to_string(),
        }
    }

    async fn api_get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_API_VERSION)
            .send()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("HTTP request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("HTTP {}: {}", status, body),
            });
        }

        resp.json()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("JSON parse failed: {}", e),
            })
    }

    async fn api_post(&self, path: &str, payload: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("{}{}", self.base_url, path);
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_API_VERSION)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("HTTP request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("HTTP {}: {}", status, body),
            });
        }

        resp.json()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "notion".to_string(),
                action: path.to_string(),
                detail: format!("JSON parse failed: {}", e),
            })
    }
}

#[async_trait]
impl ChannelAdapter for NotionAdapter {
    fn platform_id(&self) -> &'static str {
        "notion"
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult> {
        // In Notion, "sending a message" = appending a paragraph block to a page
        // recipient = page ID
        let payload = serde_json::json!({
            "parent": { "page_id": msg.recipient },
            "children": [{
                "object": "block",
                "type": "paragraph",
                "paragraph": {
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": msg.content }
                    }]
                }
            }]
        });

        let body = self.api_post("/blocks", &payload).await?;

        let block_id = body
            .pointer("/results/0/id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(SendResult {
            message_id: MessageId(block_id),
            success: true,
            error: None,
            raw_response: Some(body),
        })
    }

    async fn list_users(
        &self,
        _page: Option<u32>,
        _page_size: Option<u32>,
    ) -> Result<Vec<UserInfo>> {
        let body = self.api_get("/users").await?;

        let results = body
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let users: Vec<UserInfo> = results
            .iter()
            .filter(|u| u.get("type").and_then(|v| v.as_str()) == Some("person"))
            .map(|u| {
                let person = u.get("person").cloned().unwrap_or_default();
                UserInfo {
                    user_id: u
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: u
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    email: person
                        .get("email")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    avatar_url: u
                        .get("avatar_url")
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
        let body = self.api_get(&format!("/users/{}", user_id)).await?;

        let person = body.get("person").cloned().unwrap_or_default();

        Ok(UserInfo {
            user_id: body
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(user_id)
                .to_string(),
            name: body
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            email: person
                .get("email")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            avatar_url: body
                .get("avatar_url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            department_ids: vec![],
            extra: HashMap::new(),
        })
    }

    async fn list_chats(
        &self,
        _page: Option<u32>,
        _page_size: Option<u32>,
    ) -> Result<Vec<ChatInfo>> {
        // Search for databases the integration has access to
        let payload = serde_json::json!({
            "filter": { "value": "database", "property": "object" },
            "page_size": 100
        });

        let body = self.api_post("/search", &payload).await?;

        let results = body
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let chats: Vec<ChatInfo> = results
            .iter()
            .map(|db| {
                // Extract title from database properties
                let title = db
                    .get("title")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|t| t.get("plain_text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Untitled")
                    .to_string();

                ChatInfo {
                    chat_id: db
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: title,
                    member_count: None,
                    owner_id: db
                        .get("created_by")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    extra: HashMap::new(),
                }
            })
            .collect();

        Ok(chats)
    }

    async fn list_departments(&self, _parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>> {
        // Notion doesn't have departments — return empty
        Ok(vec![])
    }

    async fn list_calendar_events(
        &self,
        _start_time: &str,
        _end_time: &str,
    ) -> Result<Vec<CalendarEvent>> {
        // Notion calendar requires specific database query — not implemented in base adapter
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool> {
        match self.api_get("/users/me").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notion_adapter_creation() {
        let adapter = NotionAdapter::new("ntn_test_token".to_string());
        assert_eq!(adapter.platform_id(), "notion");
    }

    #[tokio::test]
    async fn test_health_check_without_valid_token() {
        let adapter = NotionAdapter::new("ntn_invalid".to_string());
        let result = adapter.health_check().await;
        assert!(!result.unwrap());
    }
}
