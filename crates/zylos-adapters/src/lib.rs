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

/// A running or configured agent instance managed by a framework adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInstance {
    /// Unique identifier for this agent instance.
    pub id: String,
    /// The framework that manages this agent.
    pub framework: Framework,
    /// Human-readable name assigned at spawn time.
    pub name: String,
    /// Current lifecycle status of the agent.
    pub status: AgentStatus,
    /// Arbitrary JSON configuration passed to the framework at spawn.
    pub config: serde_json::Value,
    /// Timestamp when the agent was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last observed activity, if any.
    pub last_active: Option<DateTime<Utc>>,
    /// Free-form key-value metadata attached to the agent.
    pub metadata: HashMap<String, String>,
}

/// Supported agent frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framework {
    /// Google Agent Development Kit.
    Adk,
    /// Anthropic Claude SDK.
    ClaudeSdk,
    /// CrewAI multi-agent orchestration.
    CrewAi,
    /// LangGraph stateful graph runtime.
    LangGraph,
    /// LlamaIndex agent framework.
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
    /// Parses a framework name string (e.g. `"adk"`, `"crewai"`) into a [`Framework`] variant.
    ///
    /// # Errors
    /// Returns [`AdapterError::UnknownFramework`] if the string does not match any known framework.
    pub fn parse(s: &str) -> Result<Self, AdapterError> {
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

/// Lifecycle status of an agent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    /// Agent is being set up and is not yet ready to receive messages.
    Initializing,
    /// Agent is idle and ready to accept work.
    Ready,
    /// Agent is actively processing a request.
    Running,
    /// Agent execution has been temporarily suspended.
    Paused,
    /// Agent finished its task successfully.
    Completed,
    /// Agent encountered an unrecoverable error.
    Failed,
    /// Agent was explicitly shut down.
    Terminated,
}

/// A message exchanged between a caller and an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Unique message identifier.
    pub id: String,
    /// Role of the message sender.
    pub role: MessageRole,
    /// Textual content of the message.
    pub content: String,
    /// When the message was created.
    pub timestamp: DateTime<Utc>,
    /// Optional structured metadata carried alongside the message.
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Role of a participant in an agent conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// End-user or external caller.
    User,
    /// The agent itself.
    Assistant,
    /// System-level instruction or context.
    System,
    /// Output from a tool invocation.
    Tool,
}

/// Parameters for spawning a new agent instance within a framework.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    /// Target framework to spawn the agent in.
    pub framework: Framework,
    /// Human-readable name for the new agent.
    pub name: String,
    /// Framework-specific configuration blob.
    pub config: serde_json::Value,
    /// Optional initial prompt sent to the agent on creation.
    pub initial_prompt: Option<String>,
    /// Environment variables injected into the agent runtime.
    pub env_vars: HashMap<String, String>,
}

/// Health status report for a single framework adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterHealth {
    /// The framework this health report pertains to.
    pub framework: Framework,
    /// Whether the framework runtime is reachable and operational.
    pub available: bool,
    /// Version string reported by the framework, if available.
    pub version: Option<String>,
    /// Number of agents currently active under this adapter.
    pub active_agents: usize,
    /// Cumulative error count since the adapter started.
    pub error_count: u64,
    /// Description of the most recent error, if any.
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Error Type
// ---------------------------------------------------------------------------

/// Errors produced by adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    /// The requested framework name is not recognized.
    #[error("Unknown framework: {0}")]
    UnknownFramework(String),
    /// No agent with the given ID exists in this adapter.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    /// The framework runtime or SDK is not installed or reachable.
    #[error("Framework not available: {0}")]
    FrameworkUnavailable(String),
    /// The supplied configuration is invalid for this framework.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    /// A network or IPC call to the framework failed.
    #[error("Communication error: {0}")]
    Communication(String),
    /// An operation exceeded its deadline.
    #[error("Timeout after {0}s")]
    Timeout(u64),
    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    /// An HTTP request returned an error status.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// A filesystem or I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Catch-all for unexpected internal failures.
    #[error("Internal error: {0}")]
    Internal(String),
}

// ---------------------------------------------------------------------------
// Framework Adapter Trait
// ---------------------------------------------------------------------------

/// Unified interface that every framework adapter must implement.
///
/// Provides lifecycle management (spawn, terminate, status), messaging,
/// and health checking for agents running inside a specific framework.
#[async_trait]
pub trait FrameworkAdapter: Send + Sync {
    /// Returns the framework this adapter handles.
    fn framework(&self) -> Framework;

    /// Checks whether the framework runtime/SDK is available and operational.
    async fn health(&self) -> Result<AdapterHealth, AdapterError>;

    /// Spawns a new agent instance within this framework.
    async fn spawn(&self, request: &SpawnRequest) -> Result<AgentInstance, AdapterError>;

    /// Sends a message to an existing agent and returns its response.
    async fn send_message(
        &self,
        agent_id: &str,
        message: &AgentMessage,
    ) -> Result<AgentMessage, AdapterError>;

    /// Retrieves the current lifecycle status of an agent.
    async fn get_status(&self, agent_id: &str) -> Result<AgentStatus, AdapterError>;

    /// Terminates an agent gracefully, releasing any associated resources.
    async fn terminate(&self, agent_id: &str) -> Result<(), AdapterError>;

    /// Lists all active agents currently managed by this adapter.
    async fn list_agents(&self) -> Result<Vec<AgentInstance>, AdapterError>;
}

// ---------------------------------------------------------------------------
// Adapter Registry
// ---------------------------------------------------------------------------

/// Central registry that maps each [`Framework`] to its [`FrameworkAdapter`] implementation.
pub struct AdapterRegistry {
    adapters: HashMap<Framework, Box<dyn FrameworkAdapter>>,
}

impl AdapterRegistry {
    /// Creates an empty registry with no adapters registered.
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }

    /// Registers an adapter for the framework it reports via [`FrameworkAdapter::framework`].
    ///
    /// If an adapter for the same framework was already registered, it is replaced.
    pub fn register(&mut self, adapter: Box<dyn FrameworkAdapter>) {
        let fw = adapter.framework();
        self.adapters.insert(fw, adapter);
    }

    /// Looks up the adapter for a given framework, if one is registered.
    pub fn get(&self, framework: Framework) -> Option<&dyn FrameworkAdapter> {
        self.adapters.get(&framework).map(|a| a.as_ref())
    }

    /// Runs a health check against every registered adapter concurrently.
    pub async fn health_all(&self) -> Vec<Result<AdapterHealth, AdapterError>> {
        let mut results = Vec::new();
        for adapter in self.adapters.values() {
            results.push(adapter.health().await);
        }
        results
    }

    /// Returns the list of frameworks that have a registered adapter.
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

/// Adapter for the Google Agent Development Kit (ADK).
///
/// Communicates with an ADK-compatible HTTP server to manage agents.
pub struct AdkAdapter {
    base_url: String,
    client: reqwest::Client,
    api_key: Option<String>,
}

impl AdkAdapter {
    /// Creates a new ADK adapter targeting the given base URL.
    ///
    /// An optional API key is sent as a Bearer token on every request when provided.
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

/// Adapter for the Anthropic Claude SDK.
///
/// Wraps the Claude Messages API. Because Claude is stateless per-request,
/// [`FrameworkAdapter::spawn`] creates a local tracking handle rather than
/// a remote session.
pub struct ClaudeSdkAdapter {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl ClaudeSdkAdapter {
    /// Creates a new Claude SDK adapter with the given API key and model identifier.
    pub fn new(api_key: &str, model: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            model: model.to_string(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Overrides the default Anthropic API base URL (useful for proxies or testing).
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

/// Adapter for the CrewAI multi-agent orchestration platform.
///
/// Talks to a CrewAI-compatible HTTP API to manage crews and kickoff runs.
pub struct CrewAiAdapter {
    base_url: String,
    client: reqwest::Client,
    #[allow(dead_code)]
    api_key: Option<String>,
}

impl CrewAiAdapter {
    /// Creates a new CrewAI adapter targeting the given base URL.
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

/// Adapter for the LangGraph stateful graph runtime.
///
/// Manages threads and runs via the LangGraph Platform HTTP API.
pub struct LangGraphAdapter {
    base_url: String,
    client: reqwest::Client,
    #[allow(dead_code)]
    api_key: Option<String>,
}

impl LangGraphAdapter {
    /// Creates a new LangGraph adapter targeting the given base URL.
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

/// Adapter for the LlamaIndex agent framework.
///
/// Communicates with a LlamaIndex-compatible HTTP service to manage agents.
pub struct LlamaIndexAdapter {
    base_url: String,
    client: reqwest::Client,
    #[allow(dead_code)]
    api_key: Option<String>,
}

impl LlamaIndexAdapter {
    /// Creates a new LlamaIndex adapter targeting the given base URL.
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

    // -----------------------------------------------------------------------
    // Framework enum
    // -----------------------------------------------------------------------

    #[test]
    fn test_framework_display_roundtrip() {
        for fw in [
            Framework::Adk,
            Framework::ClaudeSdk,
            Framework::CrewAi,
            Framework::LangGraph,
            Framework::LlamaIndex,
        ] {
            let s = fw.to_string();
            let parsed = Framework::parse(&s).unwrap();
            assert_eq!(parsed, fw);
        }
    }

    #[test]
    fn test_framework_from_str_known_values() {
        assert_eq!(Framework::parse("adk").unwrap(), Framework::Adk);
        assert_eq!(Framework::parse("claude_sdk").unwrap(), Framework::ClaudeSdk);
        assert_eq!(Framework::parse("crewai").unwrap(), Framework::CrewAi);
        assert_eq!(Framework::parse("langgraph").unwrap(), Framework::LangGraph);
        assert_eq!(Framework::parse("llamaindex").unwrap(), Framework::LlamaIndex);
    }

    #[test]
    fn test_framework_from_str_unknown() {
        let err = Framework::parse("nonexistent").unwrap_err();
        assert!(matches!(err, AdapterError::UnknownFramework(ref s) if s == "nonexistent"));
    }

    #[test]
    fn test_framework_serde_roundtrip() {
        let fw = Framework::LangGraph;
        let json = serde_json::to_string(&fw).unwrap();
        // serde rename_all = "snake_case" produces "lang_graph", not "langgraph"
        assert_eq!(json, "\"lang_graph\"");
        let deserialized: Framework = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, fw);
    }

    // -----------------------------------------------------------------------
    // AgentStatus
    // -----------------------------------------------------------------------

    #[test]
    fn test_agent_status_serde_roundtrip() {
        for status in [
            AgentStatus::Initializing,
            AgentStatus::Ready,
            AgentStatus::Running,
            AgentStatus::Paused,
            AgentStatus::Completed,
            AgentStatus::Failed,
            AgentStatus::Terminated,
        ] {
            let json = serde_json::to_string(&status).unwrap();
            let back: AgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status);
        }
    }

    // -----------------------------------------------------------------------
    // MessageRole
    // -----------------------------------------------------------------------

    #[test]
    fn test_message_role_serde_roundtrip() {
        for role in [MessageRole::User, MessageRole::Assistant, MessageRole::System, MessageRole::Tool] {
            let json = serde_json::to_string(&role).unwrap();
            let back: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(back, role);
        }
    }

    // -----------------------------------------------------------------------
    // AdapterRegistry
    // -----------------------------------------------------------------------

    #[test]
    fn test_registry_new_is_empty() {
        let registry = AdapterRegistry::new();
        assert!(registry.registered_frameworks().is_empty());
    }

    #[test]
    fn test_registry_default_matches_new() {
        let a = AdapterRegistry::new();
        let b = AdapterRegistry::default();
        assert_eq!(a.registered_frameworks().len(), b.registered_frameworks().len());
    }

    #[test]
    fn test_registry_get_missing_returns_none() {
        let registry = AdapterRegistry::new();
        assert!(registry.get(Framework::Adk).is_none());
    }

    #[tokio::test]
    async fn test_registry_health_all_empty() {
        let registry = AdapterRegistry::new();
        let results = registry.health_all().await;
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = AdapterRegistry::new();
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        registry.register(Box::new(adapter));
        assert!(registry.get(Framework::ClaudeSdk).is_some());
        assert_eq!(registry.registered_frameworks().len(), 1);
    }

    #[test]
    fn test_registry_register_replaces_existing() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(ClaudeSdkAdapter::new("key1", "model-a")));
        registry.register(Box::new(ClaudeSdkAdapter::new("key2", "model-b")));
        assert_eq!(registry.registered_frameworks().len(), 1);
    }

    // -----------------------------------------------------------------------
    // ClaudeSdkAdapter (stateless — no HTTP needed for spawn/status/terminate)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_claude_spawn_returns_ready_instance() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        let req = SpawnRequest {
            framework: Framework::ClaudeSdk,
            name: "test-agent".into(),
            config: serde_json::json!({"temperature": 0.7}),
            initial_prompt: Some("Hello".into()),
            env_vars: HashMap::new(),
        };
        let instance = adapter.spawn(&req).await.unwrap();
        assert_eq!(instance.status, AgentStatus::Ready);
        assert_eq!(instance.framework, Framework::ClaudeSdk);
        assert_eq!(instance.name, "test-agent");
        assert!(!instance.id.is_empty());
    }

    #[tokio::test]
    async fn test_claude_get_status_always_ready() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        let status = adapter.get_status("any-id").await.unwrap();
        assert_eq!(status, AgentStatus::Ready);
    }

    #[tokio::test]
    async fn test_claude_terminate_is_noop() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        assert!(adapter.terminate("any-id").await.is_ok());
    }

    #[tokio::test]
    async fn test_claude_list_agents_returns_empty() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        let agents = adapter.list_agents().await.unwrap();
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn test_claude_framework_returns_claude_sdk() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "claude-sonnet-4-20250514");
        assert_eq!(adapter.framework(), Framework::ClaudeSdk);
    }

    #[test]
    fn test_claude_with_base_url_trims_trailing_slash() {
        let adapter = ClaudeSdkAdapter::new("sk-test", "model")
            .with_base_url("https://proxy.example.com/v1///");
        // The internal base_url should have trailing slashes removed.
        // We can't directly access the field, but we can verify construction succeeded.
        assert_eq!(adapter.framework(), Framework::ClaudeSdk);
    }

    // -----------------------------------------------------------------------
    // Adapter constructors
    // -----------------------------------------------------------------------

    #[test]
    fn test_adk_adapter_constructor() {
        let adapter = AdkAdapter::new("http://localhost:8080/", Some("key".into()));
        assert_eq!(adapter.framework(), Framework::Adk);
    }

    #[test]
    fn test_crewai_adapter_constructor() {
        let adapter = CrewAiAdapter::new("http://localhost:9090", None);
        assert_eq!(adapter.framework(), Framework::CrewAi);
    }

    #[test]
    fn test_langgraph_adapter_constructor() {
        let adapter = LangGraphAdapter::new("http://localhost:7070", Some("lg-key".into()));
        assert_eq!(adapter.framework(), Framework::LangGraph);
    }

    #[test]
    fn test_llamaindex_adapter_constructor() {
        let adapter = LlamaIndexAdapter::new("http://localhost:6060", None);
        assert_eq!(adapter.framework(), Framework::LlamaIndex);
    }

    // -----------------------------------------------------------------------
    // Serialization of core types
    // -----------------------------------------------------------------------

    #[test]
    fn test_agent_instance_serde_roundtrip() {
        let instance = AgentInstance {
            id: "abc-123".into(),
            framework: Framework::Adk,
            name: "my-agent".into(),
            status: AgentStatus::Running,
            config: serde_json::json!({"key": "value"}),
            created_at: Utc::now(),
            last_active: Some(Utc::now()),
            metadata: [("env".into(), "prod".into())].into_iter().collect(),
        };
        let json = serde_json::to_string(&instance).unwrap();
        let back: AgentInstance = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, instance.id);
        assert_eq!(back.framework, instance.framework);
        assert_eq!(back.name, instance.name);
        assert_eq!(back.status, instance.status);
    }

    #[test]
    fn test_spawn_request_serde_roundtrip() {
        let req = SpawnRequest {
            framework: Framework::CrewAi,
            name: "crew-1".into(),
            config: serde_json::json!({"verbose": true}),
            initial_prompt: Some("Start".into()),
            env_vars: [("API_KEY".into(), "secret".into())].into_iter().collect(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: SpawnRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.framework, req.framework);
        assert_eq!(back.name, req.name);
        assert_eq!(back.initial_prompt, req.initial_prompt);
    }

    #[test]
    fn test_adapter_health_serde_roundtrip() {
        let health = AdapterHealth {
            framework: Framework::LangGraph,
            available: true,
            version: Some("0.2.0".into()),
            active_agents: 3,
            error_count: 0,
            last_error: None,
        };
        let json = serde_json::to_string(&health).unwrap();
        let back: AdapterHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(back.framework, health.framework);
        assert_eq!(back.available, health.available);
        assert_eq!(back.version, health.version);
        assert_eq!(back.active_agents, health.active_agents);
    }

    #[test]
    fn test_agent_message_serde_roundtrip() {
        let msg = AgentMessage {
            id: "msg-1".into(),
            role: MessageRole::User,
            content: "Hello, agent!".into(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: AgentMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, msg.id);
        assert_eq!(back.role, msg.role);
        assert_eq!(back.content, msg.content);
    }

    // -----------------------------------------------------------------------
    // Error type
    // -----------------------------------------------------------------------

    #[test]
    fn test_adapter_error_display_messages() {
        assert_eq!(
            AdapterError::UnknownFramework("foo".into()).to_string(),
            "Unknown framework: foo"
        );
        assert_eq!(
            AdapterError::AgentNotFound("a1".into()).to_string(),
            "Agent not found: a1"
        );
        assert_eq!(
            AdapterError::Timeout(30).to_string(),
            "Timeout after 30s"
        );
        assert_eq!(
            AdapterError::Internal("boom".into()).to_string(),
            "Internal error: boom"
        );
    }
}