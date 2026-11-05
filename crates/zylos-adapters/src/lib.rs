//! # zylos-adapters
//!
//! Multi-framework agent adapters for zylos-gateway:
//! - Google ADK (Agent Development Kit) adapter
//! - Anthropic Claude SDK adapter
//! - CrewAI adapter
//! - LangGraph adapter
//! - LlamaIndex adapter
//!
//! Each adapter implements the `FrameworkAdapter` trait, providing a unified
//! interface for spawning, communicating with, and managing agents across
//! different frameworks.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Core Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstance {
    pub id: String,
    pub framework: Framework,
    pub name: String,
    pub status: AgentStatus,
    pub config: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub last_active: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framework {
    Adk,
    ClaudeSdk,
    CrewAi,
    LangGraph,
    LlamaIndex,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Adk => write!(f, "adk"),
            Self::ClaudeSdk => write!(f, "claude_sdk"),
            Self::CrewAi => write!(f, "crewai"),
            Self::LangGraph => write!(f, "langgraph"),
            Self::LlamaIndex => write!(f, "llamaindex"),
        }
    }
}

impl Framework {
    pub fn from_str(s: &str) -> Result<Self, AdapterError> {
        match s {
            "adk" => Ok(Self::Adk),
            "claude_sdk" => Ok(Self::ClaudeSdk),
            "crewai" => Ok(Self::CrewAi),
            "langgraph" => Ok(Self::LangGraph),
            "llamaindex" => Ok(Self::LlamaIndex),
            _ => Err(AdapterError::UnknownFramework(s.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Initializing,
    Ready,
    Running,
    Paused,
    Completed,
    Failed,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    pub framework: Framework,
    pub name: String,
    pub config: serde_json::Value,
    pub initial_prompt: Option<String>,
    pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    pub framework: Framework,
    pub available: bool,
    pub version: Option<String>,
    pub active_agents: usize,
    pub error_count: u64,
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Unknown framework: {0}")]
    UnknownFramework(String),
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    #[error("Framework not available: {0}")]
    FrameworkUnavailable(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Communication error: {0}")]
    Communication(String),
    #[error("Timeout after {0}s")]
    Timeout(u64),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Framework Adapter Trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait FrameworkAdapter: Send + Sync {
    /// Returns the framework this adapter handles.
    fn framework(&self) -> Framework;

    /// Checks whether the framework runtime/SDK is available.
    async fn health(&self) -> Result<AdapterHealth, AdapterError>;

    /// Spawns a new agent instance within this framework.
    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError>;

    /// Sends a message to an existing agent and returns its response.
    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError>;

    /// Retrieves the current status of an agent.
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError>;

    /// Terminates an agent gracefully.
    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError>;

    /// Lists all active agents managed by this adapter.
    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError>;
}

// ---------------------------------------------------------------------------
// Adapter Registry
// ---------------------------------------------------------------------------

pub struct AdapterRegistry {
    adapters: HashMap<Framework, Box<dyn FrameworkAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    pub fn register(&mut self, adapter: Box<dyn FrameworkAdapter>) {
        let fw = adapter.framework();
        self.adapters.insert(fw, adapter);
    }

    pub fn get(&self, framework: Framework) -> Option<&dyn FrameworkAdapter> {
        self.adapters.get(&framework).map(|a| a.as_ref())
    }

    pub async fn health_all(&self) -> Vec<Result<AdapterHealth, AdapterError>> {
        let mut results = Vec::new();
        for adapter in self.adapters.values() {
            results.push(adapter.health().await);
        }
        results
    }

    pub fn registered_frameworks(&self) -> Vec<Framework> {
        self.adapters.keys().copied().collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ADK Adapter (Google Agent Development Kit)
// ---------------------------------------------------------------------------

pub struct AdkAdapter {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl AdkAdapter {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            api_key,
        }
    }

    fn auth_header(&self) -> Option<(&str, String)> {
        self.api_key
            .as_ref()
            .map(|k| ("Authorization", format!("Bearer {}", k)))
    }
}

#[async_trait]
impl FrameworkAdapter for AdkAdapter {
    fn framework(&self) -> Framework {
        Framework::Adk
    }

    async fn health(&self) -> Result<AdapterHealth, AdapterError> {
        let url = format!("{}/health", self.base_url);
        let mut req = self.client.get(&url);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        match req.send().await {
            Ok(resp) if resp.status().is_success() => Ok(AdapterHealth {
                framework: Framework::Adk,
                available: true,
                version: resp.headers().get("x-adk-version").and_then(|v| v.to_str().ok()).map(|s| s.to_string()),
                active_agents: 0,
                error_count: 0,
                last_error: None,
            }),
            Ok(resp) => Ok(AdapterHealth {
                framework: Framework::Adk,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(AdapterHealth {
                framework: Framework::Adk,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(e.to_string()),
            }),
        }
    }

    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError> {
        let url = format!("{}/agents", self.base_url);
        let body = serde_json::json!({
            "name": request.name,
            "config": request.config,
            "initial_prompt": request.initial_prompt,
        });
        let mut req = self.client.post(&url).json(&body);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "ADK spawn failed: HTTP {}",
                resp.status()
            )));
        }
        let agent: AgentInstance = resp.json().await?;
        Ok(agent)
    }

    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError> {
        let url = format!("{}/agents/{}/messages", self.base_url, agent_id);
        let mut req = self.client.post(&url).json(message);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "ADK send_message failed: HTTP {}",
                resp.status()
            )));
        }
        let reply: AgentMessage = resp.json().await?;
        Ok(reply)
    }

    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError> {
        let url = format!("{}/agents/{}/status", self.base_url, agent_id);
        let mut req = self.client.get(&url);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::AgentNotFound(agent_id.to_string()));
        }
        let body: serde_json::Value = resp.json().await?;
        let status_str = body["status"].as_str().unwrap_or("ready");
        serde_json::from_value(serde_json::Value::String(status_str.to_string()))
            .map_err(|e| AdapterError::Internal(e.to_string()))
    }

    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError> {
        let url = format!("{}/agents/{}", self.base_url, agent_id);
        let mut req = self.client.delete(&url);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "ADK terminate failed: HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> {
        let url = format!("{}/agents", self.base_url);
        let mut req = self.client.get(&url);
        if let Some((k, v)) = self.auth_header() {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "ADK list_agents failed: HTTP {}",
                resp.status()
            )));
        }
        let agents: Vec<AgentInstance> = resp.json().await?;
        Ok(agents)
    }
}

// ---------------------------------------------------------------------------
// Claude SDK Adapter (Anthropic)
// ---------------------------------------------------------------------------

pub struct ClaudeSdkAdapter {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl ClaudeSdkAdapter {
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.trim_end_matches('/').to_string();
        self
    }
}

#[async_trait]
impl FrameworkAdapter for ClaudeSdkAdapter {
    fn framework(&self) -> Framework {
        Framework::ClaudeSdk
    }

    async fn health(&self) -> Result<AdapterHealth, AdapterError> {
        // Claude API doesn't have a dedicated health endpoint; use messages API with minimal request
        let url = format!("{}/messages", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}]
        });
        match self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(AdapterHealth {
                framework: Framework::ClaudeSdk,
                available: true,
                version: Some(self.model.clone()),
                active_agents: 0,
                error_count: 0,
                last_error: None,
            }),
            Ok(resp) => Ok(AdapterHealth {
                framework: Framework::ClaudeSdk,
                available: false,
                version: Some(self.model.clone()),
                active_agents: 0,
                error_count: 1,
                last_error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(AdapterHealth {
                framework: Framework::ClaudeSdk,
                available: false,
                version: Some(self.model.clone()),
                active_agents: 0,
                error_count: 1,
                last_error: Some(e.to_string()),
            }),
        }
    }

    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError> {
        // Claude SDK is stateless per-request; "spawn" creates a tracked session
        let id = Uuid::new_v4().to_string();
        Ok(AgentInstance {
            id,
            framework: Framework::ClaudeSdk,
            name: request.name.clone(),
            status: AgentStatus::Ready,
            config: request.config.clone(),
            created_at: Utc::now(),
            last_active: None,
            metadata: HashMap::new(),
        })
    }

    async fn send_message(
        &self,
        _agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError> {
        let url = format!("{}/messages", self.base_url);
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [{"role": "user", "content": message.content}]
        });
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "Claude send_message failed: HTTP {}",
                resp.status()
            )));
        }
        let result: serde_json::Value = resp.json().await?;
        let content = result["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c["text"].as_str())
            .unwrap_or("")
            .to_string();
        Ok(AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    async fn get_status(&self, _agent_id: &str) -> Result<AgentStatus, AdapterError> {
        Ok(AgentStatus::Ready)
    }

    async fn terminate(&self, _agent_id: &str) -> Result<(), AdapterError> {
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> {
        Ok(Vec::new())
    }
}

// ---------------------------------------------------------------------------
// CrewAI Adapter
// ---------------------------------------------------------------------------

pub struct CrewAiAdapter {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl CrewAiAdapter {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl FrameworkAdapter for CrewAiAdapter {
    fn framework(&self) -> Framework {
        Framework::CrewAi
    }

    async fn health(&self) -> Result<AdapterHealth, AdapterError> {
        let url = format!("{}/api/v1/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(AdapterHealth {
                framework: Framework::CrewAi,
                available: true,
                version: None,
                active_agents: 0,
                error_count: 0,
                last_error: None,
            }),
            Ok(resp) => Ok(AdapterHealth {
                framework: Framework::CrewAi,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(AdapterHealth {
                framework: Framework::CrewAi,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(e.to_string()),
            }),
        }
    }

    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError> {
        let url = format!("{}/api/v1/crews", self.base_url);
        let body = serde_json::json!({
            "name": request.name,
            "config": request.config,
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "CrewAI spawn failed: HTTP {}",
                resp.status()
            )));
        }
        let agent: AgentInstance = resp.json().await?;
        Ok(agent)
    }

    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError> {
        let url = format!("{}/api/v1/crews/{}/kickoff", self.base_url, agent_id);
        let body = serde_json::json!({
            "inputs": { "message": message.content }
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "CrewAI send_message failed: HTTP {}",
                resp.status()
            )));
        }
        let reply: AgentMessage = resp.json().await?;
        Ok(reply)
    }

    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError> {
        let url = format!("{}/api/v1/crews/{}/status", self.base_url, agent_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::AgentNotFound(agent_id.to_string()));
        }
        let body: serde_json::Value = resp.json().await?;
        let status_str = body["status"].as_str().unwrap_or("ready");
        serde_json::from_value(serde_json::Value::String(status_str.to_string()))
            .map_err(|e| AdapterError::Internal(e.to_string()))
    }

    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError> {
        let url = format!("{}/api/v1/crews/{}", self.base_url, agent_id);
        let resp = self.client.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "CrewAI terminate failed: HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> {
        let url = format!("{}/api/v1/crews", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "CrewAI list_agents failed: HTTP {}",
                resp.status()
            )));
        }
        let agents: Vec<AgentInstance> = resp.json().await?;
        Ok(agents)
    }
}

// ---------------------------------------------------------------------------
// LangGraph Adapter
// ---------------------------------------------------------------------------

pub struct LangGraphAdapter {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl LangGraphAdapter {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl FrameworkAdapter for LangGraphAdapter {
    fn framework(&self) -> Framework {
        Framework::LangGraph
    }

    async fn health(&self) -> Result<AdapterHealth, AdapterError> {
        let url = format!("{}/ok", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(AdapterHealth {
                framework: Framework::LangGraph,
                available: true,
                version: None,
                active_agents: 0,
                error_count: 0,
                last_error: None,
            }),
            Ok(resp) => Ok(AdapterHealth {
                framework: Framework::LangGraph,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(AdapterHealth {
                framework: Framework::LangGraph,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(e.to_string()),
            }),
        }
    }

    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError> {
        let url = format!("{}/threads", self.base_url);
        let body = serde_json::json!({
            "metadata": { "name": request.name, "config": request.config }
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LangGraph spawn failed: HTTP {}",
                resp.status()
            )));
        }
        let result: serde_json::Value = resp.json().await?;
        let thread_id = result["thread_id"].as_str().unwrap_or("").to_string();
        Ok(AgentInstance {
            id: thread_id,
            framework: Framework::LangGraph,
            name: request.name.clone(),
            status: AgentStatus::Ready,
            config: request.config.clone(),
            created_at: Utc::now(),
            last_active: None,
            metadata: HashMap::new(),
        })
    }

    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError> {
        let url = format!("{}/threads/{}/runs/wait", self.base_url, agent_id);
        let body = serde_json::json!({
            "assistant_id": "default",
            "input": { "messages": [{"role": "human", "content": message.content}] }
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LangGraph send_message failed: HTTP {}",
                resp.status()
            )));
        }
        let result: serde_json::Value = resp.json().await?;
        let content = result["values"]["messages"]
            .as_array()
            .and_then(|msgs| msgs.last())
            .and_then(|m| m["content"].as_str())
            .unwrap_or("")
            .to_string();
        Ok(AgentMessage {
            id: Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        })
    }

    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError> {
        let url = format!("{}/threads/{}/state", self.base_url, agent_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::AgentNotFound(agent_id.to_string()));
        }
        Ok(AgentStatus::Ready)
    }

    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError> {
        let url = format!("{}/threads/{}/cancel", self.base_url, agent_id);
        let resp = self.client.post(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LangGraph terminate failed: HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> {
        let url = format!("{}/threads/search", self.base_url);
        let resp = self.client.post(&url).json(&serde_json::json!({})).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LangGraph list_agents failed: HTTP {}",
                resp.status()
            )));
        }
        let threads: Vec<serde_json::Value> = resp.json().await?;
        let agents = threads
            .into_iter()
            .map(|t| AgentInstance {
                id: t["thread_id"].as_str().unwrap_or("").to_string(),
                framework: Framework::LangGraph,
                name: t["metadata"]["name"].as_str().unwrap_or("unnamed").to_string(),
                status: AgentStatus::Ready,
                config: t["metadata"]["config"].clone(),
                created_at: Utc::now(),
                last_active: None,
                metadata: HashMap::new(),
            })
            .collect();
        Ok(agents)
    }
}

// ---------------------------------------------------------------------------
// LlamaIndex Adapter
// ---------------------------------------------------------------------------

pub struct LlamaIndexAdapter {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl LlamaIndexAdapter {
    pub fn new(base_url: &str, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

#[async_trait]
impl FrameworkAdapter for LlamaIndexAdapter {
    fn framework(&self) -> Framework {
        Framework::LlamaIndex
    }

    async fn health(&self) -> Result<AdapterHealth, AdapterError> {
        let url = format!("{}/api/health", self.base_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(AdapterHealth {
                framework: Framework::LlamaIndex,
                available: true,
                version: None,
                active_agents: 0,
                error_count: 0,
                last_error: None,
            }),
            Ok(resp) => Ok(AdapterHealth {
                framework: Framework::LlamaIndex,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(format!("HTTP {}", resp.status())),
            }),
            Err(e) => Ok(AdapterHealth {
                framework: Framework::LlamaIndex,
                available: false,
                version: None,
                active_agents: 0,
                error_count: 1,
                last_error: Some(e.to_string()),
            }),
        }
    }

    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError> {
        let url = format!("{}/api/agents", self.base_url);
        let body = serde_json::json!({
            "name": request.name,
            "config": request.config,
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LlamaIndex spawn failed: HTTP {}",
                resp.status()
            )));
        }
        let agent: AgentInstance = resp.json().await?;
        Ok(agent)
    }

    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError> {
        let url = format!("{}/api/agents/{}/chat", self.base_url, agent_id);
        let body = serde_json::json!({
            "message": message.content
        });
        let resp = self.client.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LlamaIndex send_message failed: HTTP {}",
                resp.status()
            )));
        }
        let reply: AgentMessage = resp.json().await?;
        Ok(reply)
    }

    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError> {
        let url = format!("{}/api/agents/{}/status", self.base_url, agent_id);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::AgentNotFound(agent_id.to_string()));
        }
        let body: serde_json::Value = resp.json().await?;
        let status_str = body["status"].as_str().unwrap_or("ready");
        serde_json::from_value(serde_json::Value::String(status_str.to_string()))
            .map_err(|e| AdapterError::Internal(e.to_string()))
    }

    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError> {
        let url = format!("{}/api/agents/{}", self.base_url, agent_id);
        let resp = self.client.delete(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LlamaIndex terminate failed: HTTP {}",
                resp.status()
            )));
        }
        Ok(())
    }

    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError> {
        let url = format!("{}/api/agents", self.base_url);
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(AdapterError::Communication(format!(
                "LlamaIndex list_agents failed: HTTP {}",
                resp.status()
            )));
        }
        let agents: Vec<AgentInstance> = resp.json().await?;
        Ok(agents)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_framework_roundtrip() {
        for fw in [
            Framework::Adk,
            Framework::ClaudeSdk,
            Framework::CrewAi,
            Framework::LangGraph,
            Framework::LlamaIndex,
        ] {
            let s = fw.to_string();
            let parsed = Framework::from_str(&s).unwrap();
            assert_eq!(parsed, fw);
        }
    }

    #[test]
    fn test_unknown_framework() {
        assert!(Framework::from_str("nonexistent").is_err());
    }

    #[test]
    fn test_registry_default() {
        let registry = AdapterRegistry::new();
        assert!(registry.registered_frameworks().is_empty());
    }

    #[tokio::test]
    async fn test_claude_spawn_returns_ready() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        let req = SpawnRequest {
            framework: Framework::ClaudeSdk,
            name: "test-agent".into(),
            config: serde_json::json!({}),
            initial_prompt: None,
            env_vars: HashMap::new(),
        };
        let instance = adapter.spawn(&req).await.unwrap();
        assert_eq!(instance.status, AgentStatus::Ready);
        assert_eq!(instance.framework, Framework::ClaudeSdk);
    }
}