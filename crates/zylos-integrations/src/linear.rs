//! Linear Channel Adapter (T11.8.4)
//!
//! Implements `ChannelAdapter` for Linear via Personal Access Token + GraphQL API.
//! Supports: send_message (as issue comment), list_users, list_chats (issues/teams), health_check.
//! Departments and calendar are not applicable to Linear (return empty).

use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;

use crate::channel::{
    CalendarEvent, ChannelAdapter, ChatInfo, DepartmentInfo, MessageId, OutboundMessage,
    SendResult, UserInfo,
};
use crate::error::{IntegrationError, Result};

/// Linear adapter using Personal Access Token with GraphQL API.
pub struct LinearAdapter {
    token: String,
    client: Client,
    api_url: String,
}

impl LinearAdapter {
    /// Create a new Linear adapter with a Personal Access Token.
    ///
    /// Generate at: <https://linear.app/settings/account/security/personal-access-tokens>
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("failed to build reqwest client"),
            api_url: "https://api.linear.app/graphql".to_string(),
        }
    }

    async fn graphql(
        &self,
        query: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let mut payload = serde_json::json!({ "query": query });
        if let Some(vars) = variables {
            payload["variables"] = vars.clone();
        }

        let resp = self
            .client
            .post(&self.api_url)
            .header("Authorization", &self.token)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| IntegrationError::ProviderExecution {
                provider: "linear".to_string(),
                action: "graphql".to_string(),
                detail: format!("HTTP request failed: {}", e),
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(IntegrationError::ProviderExecution {
                provider: "linear".to_string(),
                action: "graphql".to_string(),
                detail: format!("HTTP {}: {}", status, body),
            });
        }

        let body: serde_json::Value =
            resp.json()
                .await
                .map_err(|e| IntegrationError::ProviderExecution {
                    provider: "linear".to_string(),
                    action: "graphql".to_string(),
                    detail: format!("JSON parse failed: {}", e),
                })?;

        // Check for GraphQL errors
        if let Some(errors) = body.get("errors").and_then(|v| v.as_array()) {
            if !errors.is_empty() {
                let msg = errors[0]
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown GraphQL error");
                return Err(IntegrationError::ProviderExecution {
                    provider: "linear".to_string(),
                    action: "graphql".to_string(),
                    detail: format!("GraphQL error: {}", msg),
                });
            }
        }

        Ok(body)
    }
}

#[async_trait]
impl ChannelAdapter for LinearAdapter {
    fn platform_id(&self) -> &'static str {
        "linear"
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult> {
        // In Linear, "sending a message" = adding a comment to an issue
        // recipient = issue ID
        let query = r#"
            mutation CommentCreate($issueId: String!, $body: String!) {
                commentCreate(input: { issueId: $issueId, body: $body }) {
                    success
                    comment {
                        id
                    }
                }
            }
        "#;

        let variables = serde_json::json!({
            "issueId": msg.recipient,
            "body": msg.content,
        });

        let body = self.graphql(query, Some(&variables)).await?;

        let comment_id = body
            .pointer("/data/commentCreate/comment/id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let success = body
            .pointer("/data/commentCreate/success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(SendResult {
            message_id: MessageId(comment_id),
            success,
            error: if success {
                None
            } else {
                Some("commentCreate returned success=false".to_string())
            },
            raw_response: Some(body),
        })
    }

    async fn list_users(
        &self,
        _page: Option<u32>,
        _page_size: Option<u32>,
    ) -> Result<Vec<UserInfo>> {
        let query = r#"
            query {
                users(first: 100) {
                    nodes {
                        id
                        name
                        email
                        avatarUrl
                    }
                }
            }
        "#;

        let body = self.graphql(query, None).await?;

        let nodes = body
            .pointer("/data/users/nodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let users: Vec<UserInfo> = nodes
            .iter()
            .map(|u| UserInfo {
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
                email: u
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                avatar_url: u
                    .get("avatarUrl")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                department_ids: vec![],
                extra: HashMap::new(),
            })
            .collect();

        Ok(users)
    }

    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
        let query = r#"
            query User($id: String!) {
                user(id: $id) {
                    id
                    name
                    email
                    avatarUrl
                }
            }
        "#;

        let variables = serde_json::json!({ "id": user_id });
        let body = self.graphql(query, Some(&variables)).await?;

        let user = body.pointer("/data/user").cloned().unwrap_or_default();

        Ok(UserInfo {
            user_id: user
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(user_id)
                .to_string(),
            name: user
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            email: user
                .get("email")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            avatar_url: user
                .get("avatarUrl")
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
        // List teams as "chats" in Linear context
        let query = r#"
            query {
                teams(first: 50) {
                    nodes {
                        id
                        name
                        key
                    }
                }
            }
        "#;

        let body = self.graphql(query, None).await?;

        let nodes = body
            .pointer("/data/teams/nodes")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let chats: Vec<ChatInfo> = nodes
            .iter()
            .map(|t| {
                let key = t.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
                ChatInfo {
                    chat_id: t
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: format!("[{}] {}", key, name),
                    member_count: None,
                    owner_id: None,
                    extra: HashMap::new(),
                }
            })
            .collect();

        Ok(chats)
    }

    async fn list_departments(&self, _parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>> {
        // Linear doesn't have departments — return empty
        Ok(vec![])
    }

    async fn list_calendar_events(
        &self,
        _start_time: &str,
        _end_time: &str,
    ) -> Result<Vec<CalendarEvent>> {
        // Linear doesn't have calendar events — return empty
        Ok(vec![])
    }

    async fn health_check(&self) -> Result<bool> {
        let query = r#"{ viewer { id name } }"#;
        match self.graphql(query, None).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_adapter_creation() {
        let adapter = LinearAdapter::new("lin_api_test_token".to_string());
        assert_eq!(adapter.platform_id(), "linear");
    }

    #[tokio::test]
    async fn test_health_check_without_valid_token() {
        let adapter = LinearAdapter::new("lin_api_invalid".to_string());
        let result = adapter.health_check().await;
        assert!(!result.unwrap());
    }
}
