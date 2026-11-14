//! # Channel Adapters
//!
//! Trait and implementations for messaging platform integrations.
//! Provides a unified interface for sending messages across different
//! platforms (WeChat Work, Feishu/Lark, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{IntegrationError, Result};

/// Unique identifier for a channel message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MessageId(pub String);

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MessageId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// A message to be sent through a channel adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Target recipient (user ID, chat ID, or department ID depending on platform)
    pub recipient: String,
    /// Message content (plain text or rich text depending on platform support)
    pub content: String,
    /// Optional message type hint (text, markdown, card, etc.)
    pub msg_type: Option<String>,
    /// Additional platform-specific parameters
    pub extra: HashMap<String, serde_json::Value>,
}

/// Result of sending a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    /// Platform-assigned message ID
    pub message_id: MessageId,
    /// Whether the send was successful
    pub success: bool,
    /// Optional error detail if partial failure
    pub error: Option<String>,
    /// Raw platform response for debugging
    pub raw_response: Option<serde_json::Value>,
}

/// Information about a user in the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub department_ids: Vec<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Information about a chat/group/conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub chat_id: String,
    pub name: String,
    pub member_count: Option<usize>,
    pub owner_id: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Information about a department/org unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepartmentInfo {
    pub department_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub member_count: Option<usize>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Calendar event from the platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub event_id: String,
    pub title: String,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub attendees: Vec<String>,
    pub extra: HashMap<String, serde_json::Value>,
}

/// Unified trait for messaging platform channel adapters.
///
/// Each platform (WeChat Work, Feishu, etc.) implements this trait
/// to provide a consistent interface for the orchestrator.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Returns the platform identifier (e.g., "wechat_work", "feishu").
    fn platform_id(&self) -> &'static str;

    /// Send a message to a recipient.
    async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult>;

    /// List users visible to the authenticated account.
    async fn list_users(&self, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<UserInfo>>;

    /// Get detailed info about a specific user.
    async fn get_user_info(&self, user_id: &str) -> Result<UserInfo>;

    /// List chats/groups the bot has access to.
    async fn list_chats(&self, page: Option<u32>, page_size: Option<u32>) -> Result<Vec<ChatInfo>>;

    /// List departments/org units.
    async fn list_departments(&self, parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>>;

    /// List calendar events within a time range.
    async fn list_calendar_events(
        &self,
        start_time: &str,
        end_time: &str,
    ) -> Result<Vec<CalendarEvent>>;

    /// Check if the adapter is properly configured and authenticated.
    async fn health_check(&self) -> Result<bool>;
}

/// Registry of available channel adapters.
pub struct ChannelRegistry {
    adapters: HashMap<String, Box<dyn ChannelAdapter>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Register a channel adapter.
    pub fn register(&mut self, adapter: Box<dyn ChannelAdapter>) {
        let id = adapter.platform_id().to_string();
        self.adapters.insert(id, adapter);
    }

    /// Get an adapter by platform ID.
    pub fn get(&self, platform_id: &str) -> Option<&dyn ChannelAdapter> {
        self.adapters.get(platform_id).map(|a| a.as_ref())
    }

    /// List all registered platform IDs.
    pub fn list_platforms(&self) -> Vec<&str> {
        self.adapters.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// WeChat Work Adapter
// ---------------------------------------------------------------------------

pub mod wechat_work {
    use super::*;
    use reqwest::Client;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// WeChat Work (企业微信) channel adapter.
    ///
    /// Uses the WeChat Work API with OAuth2 PKCE authentication.
    /// Supports sending messages, listing users/departments, and basic user info.
    #[allow(dead_code)]
    pub struct WeChatWorkAdapter {
        client: Client,
        corp_id: String,
        agent_id: String,
        access_token: Arc<RwLock<Option<String>>>,
        base_url: String,
    }

    impl WeChatWorkAdapter {
        pub fn new(corp_id: String, agent_id: String, access_token: Option<String>) -> Self {
            Self {
                client: Client::new(),
                corp_id,
                agent_id,
                access_token: Arc::new(RwLock::new(access_token)),
                base_url: "https://qyapi.weixin.qq.com/cgi-bin".to_string(),
            }
        }

        async fn ensure_token(&self) -> Result<String> {
            let token = self.access_token.read().await;
            token
                .clone()
                .ok_or_else(|| IntegrationError::NoTokensStored {
                    user_id: "system".to_string(),
                    provider: "wechat_work".to_string(),
                })
        }

        async fn api_get(&self, path: &str, params: &[(&str, &str)]) -> Result<serde_json::Value> {
            let token = self.ensure_token().await?;
            let url = format!("{}{}", self.base_url, path);

            let mut req = self.client.get(&url).query(&[("access_token", &token)]);
            for (k, v) in params {
                req = req.query(&[(k, v)]);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| IntegrationError::ProviderExecution {
                    provider: "wechat_work".to_string(),
                    action: path.to_string(),
                    detail: e.to_string(),
                })?;

            resp.json::<serde_json::Value>().await.map_err(|e| {
                IntegrationError::ProviderExecution {
                    provider: "wechat_work".to_string(),
                    action: path.to_string(),
                    detail: format!("JSON parse error: {}", e),
                }
            })
        }

        async fn api_post(
            &self,
            path: &str,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value> {
            let token = self.ensure_token().await?;
            let url = format!("{}{}?access_token={}", self.base_url, path, token);

            let resp = self
                .client
                .post(&url)
                .json(body)
                .send()
                .await
                .map_err(|e| IntegrationError::ProviderExecution {
                    provider: "wechat_work".to_string(),
                    action: path.to_string(),
                    detail: e.to_string(),
                })?;

            resp.json::<serde_json::Value>().await.map_err(|e| {
                IntegrationError::ProviderExecution {
                    provider: "wechat_work".to_string(),
                    action: path.to_string(),
                    detail: format!("JSON parse error: {}", e),
                }
            })
        }
    }

    #[async_trait]
    impl ChannelAdapter for WeChatWorkAdapter {
        fn platform_id(&self) -> &'static str {
            "wechat_work"
        }

        async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult> {
            let body = serde_json::json!({
                "touser": msg.recipient,
                "msgtype": msg.msg_type.as_deref().unwrap_or("text"),
                "agentid": self.agent_id.parse::<i64>().unwrap_or(0),
                "text": {
                    "content": msg.content
                }
            });

            let resp = self.api_post("/message/send", &body).await?;
            let errcode = resp.get("errcode").and_then(|v| v.as_i64()).unwrap_or(-1);

            Ok(SendResult {
                message_id: MessageId(
                    resp.get("msgid")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                ),
                success: errcode == 0,
                error: if errcode != 0 {
                    resp.get("errmsg")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                },
                raw_response: Some(resp),
            })
        }

        async fn list_users(
            &self,
            page: Option<u32>,
            _page_size: Option<u32>,
        ) -> Result<Vec<UserInfo>> {
            let department_id = page
                .map(|p| p.to_string())
                .unwrap_or_else(|| "1".to_string());
            let resp = self
                .api_get("/user/list", &[("department_id", &department_id)])
                .await?;

            let users = resp
                .get("userlist")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(users
                .into_iter()
                .map(|u| UserInfo {
                    user_id: u
                        .get("userid")
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
                        .get("avatar")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    department_ids: u
                        .get("department")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|n| n.as_i64().map(|i| i.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
            let resp = self.api_get("/user/get", &[("userid", user_id)]).await?;

            Ok(UserInfo {
                user_id: resp
                    .get("userid")
                    .and_then(|v| v.as_str())
                    .unwrap_or(user_id)
                    .to_string(),
                name: resp
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                email: resp
                    .get("email")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                avatar_url: resp
                    .get("avatar")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                department_ids: resp
                    .get("department")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|n| n.as_i64().map(|i| i.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                extra: HashMap::new(),
            })
        }

        async fn list_chats(
            &self,
            _page: Option<u32>,
            _page_size: Option<u32>,
        ) -> Result<Vec<ChatInfo>> {
            // WeChat Work uses appchat for group chats
            // This requires iterating known chat IDs; returning empty for now
            // Full implementation requires chat ID registry
            Ok(vec![])
        }

        async fn list_departments(&self, parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>> {
            let pid = parent_id.unwrap_or("0");
            let resp = self.api_get("/department/list", &[("id", pid)]).await?;

            let depts = resp
                .get("department")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(depts
                .into_iter()
                .map(|d| DepartmentInfo {
                    department_id: d
                        .get("id")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0)
                        .to_string(),
                    name: d
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    parent_id: d
                        .get("parentid")
                        .and_then(|v| v.as_i64())
                        .map(|i| i.to_string()),
                    member_count: None,
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn list_calendar_events(
            &self,
            _start: &str,
            _end: &str,
        ) -> Result<Vec<CalendarEvent>> {
            // WeChat Work calendar API requires separate OAuth scope
            // Returning empty until calendar scope is configured
            Ok(vec![])
        }

        async fn health_check(&self) -> Result<bool> {
            match self.ensure_token().await {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Feishu / Lark Adapter
// ---------------------------------------------------------------------------

pub mod feishu {
    use super::*;
    use reqwest::Client;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Feishu (飞书) / Lark channel adapter.
    ///
    /// Uses the Feishu Open Platform API with tenant_access_token authentication.
    /// Supports sending messages, listing chats/users, departments, and calendar events.
    #[allow(dead_code)]
    pub struct FeishuAdapter {
        client: Client,
        app_id: String,
        app_secret: String,
        tenant_token: Arc<RwLock<Option<String>>>,
        base_url: String,
    }

    impl FeishuAdapter {
        pub fn new(app_id: String, app_secret: String, tenant_token: Option<String>) -> Self {
            Self {
                client: Client::new(),
                app_id,
                app_secret,
                tenant_token: Arc::new(RwLock::new(tenant_token)),
                base_url: "https://open.feishu.cn/open-apis".to_string(),
            }
        }

        async fn ensure_token(&self) -> Result<String> {
            let token = self.tenant_token.read().await;
            token
                .clone()
                .ok_or_else(|| IntegrationError::NoTokensStored {
                    user_id: "system".to_string(),
                    provider: "feishu".to_string(),
                })
        }

        async fn api_get(&self, path: &str, params: &[(&str, &str)]) -> Result<serde_json::Value> {
            let token = self.ensure_token().await?;
            let url = format!("{}{}", self.base_url, path);

            let mut req = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", token));
            for (k, v) in params {
                req = req.query(&[(k, v)]);
            }

            let resp = req
                .send()
                .await
                .map_err(|e| IntegrationError::ProviderExecution {
                    provider: "feishu".to_string(),
                    action: path.to_string(),
                    detail: e.to_string(),
                })?;

            resp.json::<serde_json::Value>().await.map_err(|e| {
                IntegrationError::ProviderExecution {
                    provider: "feishu".to_string(),
                    action: path.to_string(),
                    detail: format!("JSON parse error: {}", e),
                }
            })
        }

        async fn api_post(
            &self,
            path: &str,
            body: &serde_json::Value,
        ) -> Result<serde_json::Value> {
            let token = self.ensure_token().await?;
            let url = format!("{}{}", self.base_url, path);

            let resp = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .json(body)
                .send()
                .await
                .map_err(|e| IntegrationError::ProviderExecution {
                    provider: "feishu".to_string(),
                    action: path.to_string(),
                    detail: e.to_string(),
                })?;

            resp.json::<serde_json::Value>().await.map_err(|e| {
                IntegrationError::ProviderExecution {
                    provider: "feishu".to_string(),
                    action: path.to_string(),
                    detail: format!("JSON parse error: {}", e),
                }
            })
        }
    }

    #[async_trait]
    impl ChannelAdapter for FeishuAdapter {
        fn platform_id(&self) -> &'static str {
            "feishu"
        }

        async fn send_message(&self, msg: &OutboundMessage) -> Result<SendResult> {
            let receive_id_type = msg
                .extra
                .get("receive_id_type")
                .and_then(|v| v.as_str())
                .unwrap_or("open_id");

            let body = serde_json::json!({
                "receive_id": msg.recipient,
                "msg_type": msg.msg_type.as_deref().unwrap_or("text"),
                "content": serde_json::json!({"text": msg.content}).to_string()
            });

            let resp = self
                .api_post(
                    &format!("/im/v1/messages?receive_id_type={}", receive_id_type),
                    &body,
                )
                .await?;

            let code = resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
            let data = resp.get("data");

            Ok(SendResult {
                message_id: MessageId(
                    data.and_then(|d| d.get("message_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                ),
                success: code == 0,
                error: if code != 0 {
                    resp.get("msg")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                },
                raw_response: Some(resp),
            })
        }

        async fn list_users(
            &self,
            page: Option<u32>,
            page_size: Option<u32>,
        ) -> Result<Vec<UserInfo>> {
            let ps = page_size.unwrap_or(50).to_string();
            let pt = page.map(|p| p.to_string()).unwrap_or_default();

            let mut params = vec![("page_size", ps.as_str())];
            if !pt.is_empty() {
                params.push(("page_token", pt.as_str()));
            }

            let resp = self
                .api_get("/contact/v3/users/find_by_department", &params)
                .await?;
            let items = resp
                .get("data")
                .and_then(|d| d.get("items"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(items
                .into_iter()
                .map(|u| UserInfo {
                    user_id: u
                        .get("open_id")
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
                        .get("avatar")
                        .and_then(|a| a.get("avatar_origin"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    department_ids: u
                        .get("department_ids")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|n| n.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn get_user_info(&self, user_id: &str) -> Result<UserInfo> {
            let resp = self
                .api_get(
                    &format!("/contact/v3/users/{}", user_id),
                    &[("user_id_type", "open_id")],
                )
                .await?;

            let data = resp.get("data").and_then(|d| d.get("user"));

            Ok(UserInfo {
                user_id: data
                    .and_then(|u| u.get("open_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(user_id)
                    .to_string(),
                name: data
                    .and_then(|u| u.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                email: data
                    .and_then(|u| u.get("email"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                avatar_url: data
                    .and_then(|u| u.get("avatar"))
                    .and_then(|a| a.get("avatar_origin"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                department_ids: data
                    .and_then(|u| u.get("department_ids"))
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|n| n.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                extra: HashMap::new(),
            })
        }

        async fn list_chats(
            &self,
            page: Option<u32>,
            page_size: Option<u32>,
        ) -> Result<Vec<ChatInfo>> {
            let ps = page_size.unwrap_or(50).to_string();
            let pt = page.map(|p| p.to_string()).unwrap_or_default();

            let mut params = vec![("page_size", ps.as_str())];
            if !pt.is_empty() {
                params.push(("page_token", pt.as_str()));
            }

            let resp = self.api_get("/im/v1/chats", &params).await?;
            let items = resp
                .get("data")
                .and_then(|d| d.get("items"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(items
                .into_iter()
                .map(|c| ChatInfo {
                    chat_id: c
                        .get("chat_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: c
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    member_count: c
                        .get("user_count")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as usize),
                    owner_id: c
                        .get("owner_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn list_departments(&self, parent_id: Option<&str>) -> Result<Vec<DepartmentInfo>> {
            let pid = parent_id.unwrap_or("0");
            let resp = self
                .api_get("/contact/v3/departments", &[("parent_department_id", pid)])
                .await?;

            let items = resp
                .get("data")
                .and_then(|d| d.get("items"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(items
                .into_iter()
                .map(|d| DepartmentInfo {
                    department_id: d
                        .get("open_department_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    name: d
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    parent_id: d
                        .get("parent_department_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    member_count: d
                        .get("member_count")
                        .and_then(|v| v.as_u64())
                        .map(|n| n as usize),
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn list_calendar_events(
            &self,
            start_time: &str,
            end_time: &str,
        ) -> Result<Vec<CalendarEvent>> {
            let resp = self
                .api_get(
                    "/calendar/v4/calendars/primary/events",
                    &[
                        ("start_time", start_time),
                        ("end_time", end_time),
                        ("page_size", "100"),
                    ],
                )
                .await?;

            let items = resp
                .get("data")
                .and_then(|d| d.get("items"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            Ok(items
                .into_iter()
                .map(|e| CalendarEvent {
                    event_id: e
                        .get("event_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    title: e
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    start_time: e
                        .get("start_time")
                        .and_then(|s| s.get("timestamp"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    end_time: e
                        .get("end_time")
                        .and_then(|s| s.get("timestamp"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    location: e
                        .get("location")
                        .and_then(|l| l.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    attendees: e
                        .get("attendees")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|a| {
                                    a.get("attendee_id")
                                        .and_then(|id| id.get("open_id"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string())
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    extra: HashMap::new(),
                })
                .collect())
        }

        async fn health_check(&self) -> Result<bool> {
            match self.ensure_token().await {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_display() {
        let id = MessageId("abc123".to_string());
        assert_eq!(format!("{}", id), "abc123");
    }

    #[test]
    fn test_message_id_from_string() {
        let id: MessageId = "test-id".to_string().into();
        assert_eq!(id.0, "test-id");
    }

    #[test]
    fn test_channel_registry() {
        let registry = ChannelRegistry::new();
        assert!(registry.list_platforms().is_empty());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_outbound_message_serialize() {
        let msg = OutboundMessage {
            recipient: "user123".to_string(),
            content: "Hello".to_string(),
            msg_type: Some("text".to_string()),
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("user123"));
        assert!(json.contains("Hello"));
    }

    #[tokio::test]
    async fn test_wechat_work_health_no_token() {
        let adapter = wechat_work::WeChatWorkAdapter::new(
            "corp123".to_string(),
            "agent456".to_string(),
            None,
        );
        let result = adapter.health_check().await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_feishu_health_no_token() {
        let adapter =
            feishu::FeishuAdapter::new("app123".to_string(), "secret456".to_string(), None);
        let result = adapter.health_check().await.unwrap();
        assert!(!result);
    }
}
