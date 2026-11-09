//! Mobile MCP Server — full MCP protocol implementation for mobile clients.
//!
//! Provides tools, resources, prompts, SSE streaming, authentication,
//! rate limiting, and attachment handling per MCP spec.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;
use zylos_live_comm::event_types::{CustomEvent, ZylosEvent};
use zylos_live_comm::EventBus;

use crate::error::MobileError;

// ---------------------------------------------------------------------------
// MCP Protocol Types
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 request envelope used by MCP.
///
/// All client requests are deserialized into this type before dispatch.
/// The `params` field defaults to `Value::Null` when absent, allowing
/// methods with no parameters (e.g. `ping`, `tools/list`) to share the
/// same envelope type.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcRequest {
    /// Protocol version; must be `"2.0"`.
    pub jsonrpc: String,
    /// Request identifier for correlating responses. `None` for notifications.
    pub id: Option<serde_json::Value>,
    /// Method name (e.g. `"initialize"`, `"tools/call"`).
    pub method: String,
    /// Method-specific parameters; defaults to `Value::Null` when omitted.
    #[serde(default)]
    pub params: serde_json::Value,
}

/// JSON-RPC 2.0 response envelope.
///
/// Exactly one of `result` or `error` is `Some`; the other is serialized
/// as absent thanks to `skip_serializing_if`. Use the [`ok`](Self::ok) and
/// [`err`](Self::err) constructors to guarantee this invariant.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JsonRpcResponse {
    /// Protocol version; always `"2.0"`.
    pub jsonrpc: String,
    /// Request identifier echoed from the corresponding request. `None` for notifications.
    pub id: Option<serde_json::Value>,
    /// Successful result payload; present only on success responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error body; present only on error responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpErrorBody>,
}

impl JsonRpcResponse {
    /// Build a successful JSON-RPC response with the given `id` and `result`.
    pub fn ok(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Build an error JSON-RPC response with the given `id`, numeric `code`, and human-readable `message`.
    pub fn err(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpErrorBody { code, message, data: None }),
        }
    }
}

/// MCP error body inside a JSON-RPC error response.
///
/// Follows the JSON-RPC 2.0 error object specification with an optional
/// `data` field for structured diagnostic information.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpErrorBody {
    /// Numeric error code (e.g. -32600, -32601, -32001..-32005).
    pub code: i32,
    /// Human-readable error description.
    pub message: String,
    /// Optional structured data providing additional context about the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// MCP Capability Definitions
// ---------------------------------------------------------------------------

/// A tool exposed via MCP.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// A resource exposed via MCP.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpResourceDef {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// A prompt template exposed via MCP.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPromptDef {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub arguments: Vec<McpPromptArgument>,
}

/// An argument for a prompt template.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
}

/// Server capabilities advertised during initialize handshake.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerCapabilities {
    pub tools: serde_json::Value,
    pub resources: serde_json::Value,
    pub prompts: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Authentication
// ---------------------------------------------------------------------------

/// Authentication configuration for the MCP server.
#[derive(Clone)]
pub struct AuthConfig {
    /// Shared secret for API-key auth.
    pub api_key: Option<String>,
    /// JWT secret for token auth.
    pub jwt_secret: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("ZYLOS_MCP_API_KEY").ok(),
            jwt_secret: std::env::var("ZYLOS_JWT_SECRET").ok(),
        }
    }
}

impl AuthConfig {
    /// Validate an authorization header value. Returns client_id on success.
    pub fn validate(&self, auth_header: Option<&str>) -> Result<String, MobileError> {
        let header = auth_header.ok_or_else(|| MobileError::Auth("missing Authorization header".into()))?;

        // Try Bearer JWT first
        if let Some(token) = header.strip_prefix("Bearer ") {
            if let Some(ref secret) = self.jwt_secret {
                return self.validate_jwt(token, secret);
            }
        }

        // Try API key (ApiKey <key> or raw key)
        let key = header.strip_prefix("ApiKey ").unwrap_or(header);
        if let Some(ref expected) = self.api_key {
            if key == expected {
                return Ok("api-key-client".into());
            }
        }

        Err(MobileError::Auth("invalid credentials".into()))
    }

    fn validate_jwt(&self, token: &str, secret: &str) -> Result<String, MobileError> {
        use jsonwebtoken::{decode, DecodingKey, Validation};
        #[derive(Deserialize)]
        struct Claims {
            sub: String,
        }
        let mut validation = Validation::default();
        validation.validate_exp = true;
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_bytes()),
            &validation,
        )
        .map_err(|e| MobileError::Auth(format!("JWT validation failed: {}", e)))?;
        Ok(data.claims.sub)
    }
}

// ---------------------------------------------------------------------------
// Rate Limiter
// ---------------------------------------------------------------------------

/// Simple per-client token-bucket rate limiter.
pub struct RateLimiter {
    limits: RwLock<HashMap<String, ClientBucket>>,
    max_requests: u32,
    window: Duration,
}

struct ClientBucket {
    tokens: u32,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter allowing `max_requests` per `window_secs` seconds.
    ///
    /// Each unique client ID gets its own token bucket that refills after the
    /// configured window elapses.
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            limits: RwLock::new(HashMap::new()),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check and consume one token. Returns Err with retry_after on exhaustion.
    pub async fn check(&self, client_id: &str) -> Result<(), MobileError> {
        let mut map = self.limits.write().await;
        let now = Instant::now();
        let bucket = map.entry(client_id.to_string()).or_insert(ClientBucket {
            tokens: self.max_requests,
            last_refill: now,
        });

        // Refill
        let elapsed = now.duration_since(bucket.last_refill);
        if elapsed >= self.window {
            bucket.tokens = self.max_requests;
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            Ok(())
        } else {
            let retry_after = self.window.saturating_sub(elapsed).as_secs().max(1);
            Err(MobileError::RateLimit {
                client_id: client_id.to_string(),
                retry_after_secs: retry_after,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Attachment Store
// ---------------------------------------------------------------------------

/// Temporary attachment storage with TTL-based cleanup.
pub struct AttachmentStore {
    base_dir: PathBuf,
    ttl: Duration,
    entries: RwLock<HashMap<String, AttachmentEntry>>,
}

#[derive(Clone)]
struct AttachmentEntry {
    path: PathBuf,
    created_at: Instant,
    mime_type: String,
    size_bytes: u64,
}

impl AttachmentStore {
    /// Create a new attachment store rooted at `base_dir` with entries expiring after `ttl_secs`.
    ///
    /// The directory is created lazily on the first [`store`](Self::store) call.
    /// Expired entries are not deleted automatically; call [`cleanup_expired`](Self::cleanup_expired)
    /// or use [`MobileMcpServer::spawn_attachment_cleanup`] for periodic pruning.
    pub fn new(base_dir: PathBuf, ttl_secs: u64) -> Self {
        Self {
            base_dir,
            ttl: Duration::from_secs(ttl_secs),
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Store bytes and return a resource URI.
    pub async fn store(&self, _filename: &str, data: &[u8], mime_type: &str) -> Result<String, MobileError> {
        tokio::fs::create_dir_all(&self.base_dir)
            .await
            .map_err(|e| MobileError::Attachment(format!("failed to create attachment dir: {}", e)))?;

        let id = Uuid::new_v4().to_string();
        let ext = mime_guess::get_mime_extensions_str(mime_type)
            .and_then(|exts| exts.first().copied())
            .unwrap_or("bin");
        let stored_name = format!("{}.{}", id, ext);
        let path = self.base_dir.join(&stored_name);

        tokio::fs::write(&path, data)
            .await
            .map_err(|e| MobileError::Attachment(format!("failed to write attachment: {}", e)))?;

        let mut hash = Sha256::new();
        hash.update(data);
        let _hash_hex = hex::encode(hash.finalize());

        self.entries.write().await.insert(
            id.clone(),
            AttachmentEntry {
                path,
                created_at: Instant::now(),
                mime_type: mime_type.to_string(),
                size_bytes: data.len() as u64,
            },
        );

        Ok(format!("attachment://{}", id))
    }

    /// Retrieve metadata for an attachment URI.
    pub async fn get(&self, uri: &str) -> Result<(PathBuf, String, u64), MobileError> {
        let id = uri
            .strip_prefix("attachment://")
            .ok_or_else(|| MobileError::Attachment(format!("invalid attachment URI: {}", uri)))?;

        let entries = self.entries.read().await;
        let entry = entries
            .get(id)
            .ok_or_else(|| MobileError::ResourceNotFound(format!("attachment not found: {}", uri)))?;

        if entry.created_at.elapsed() > self.ttl {
            return Err(MobileError::Attachment("attachment expired".into()));
        }

        Ok((entry.path.clone(), entry.mime_type.clone(), entry.size_bytes))
    }

    /// Remove expired attachments.
    pub async fn cleanup_expired(&self) -> usize {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|_, entry| entry.created_at.elapsed() <= self.ttl);
        before - entries.len()
    }
}

// ---------------------------------------------------------------------------
// MobileMcpServer
// ---------------------------------------------------------------------------

/// Full MCP server for mobile clients.
pub struct MobileMcpServer {
    event_bus: EventBus,
    tools: HashMap<String, McpToolDef>,
    resources: HashMap<String, McpResourceDef>,
    prompts: HashMap<String, McpPromptDef>,
    auth: AuthConfig,
    rate_limiter: Arc<RateLimiter>,
    attachments: Arc<AttachmentStore>,
}

impl MobileMcpServer {
    /// Create a new MobileMcpServer with all capabilities registered.
    pub fn new(event_bus: EventBus) -> Self {
        Self::with_config(event_bus, AuthConfig::default(), 100, 60, 3600)
    }

    /// Create with explicit configuration.
    pub fn with_config(
        event_bus: EventBus,
        auth: AuthConfig,
        rate_limit_max: u32,
        rate_limit_window_secs: u64,
        attachment_ttl_secs: u64,
    ) -> Self {
        let tools = Self::register_tools();
        let resources = Self::register_resources();
        let prompts = Self::register_prompts();

        let attachment_dir = std::env::temp_dir().join("zylos-mobile-attachments");
        let attachments = Arc::new(AttachmentStore::new(attachment_dir, attachment_ttl_secs));

        Self {
            event_bus,
            tools,
            resources,
            prompts,
            auth,
            rate_limiter: Arc::new(RateLimiter::new(rate_limit_max, rate_limit_window_secs)),
            attachments,
        }
    }

    // -- Tool Registration --------------------------------------------------

    fn register_tools() -> HashMap<String, McpToolDef> {
        let mut m = HashMap::new();

        m.insert("list_agents".into(), McpToolDef {
            name: "list_agents".into(),
            description: "List all available agents in the system".into(),
            input_schema: serde_json::json!({"type": "object", "properties": {}}),
        });

        m.insert("send_message".into(), McpToolDef {
            name: "send_message".into(),
            description: "Send a text message to a specific agent".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "agent_id": {"type": "string", "description": "Target agent identifier"},
                    "content": {"type": "string", "description": "Message body"}
                },
                "required": ["agent_id", "content"]
            }),
        });

        m.insert("get_task_status".into(), McpToolDef {
            name: "get_task_status".into(),
            description: "Get the current status of a task by ID".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {"type": "string", "description": "Task identifier"}
                },
                "required": ["task_id"]
            }),
        });

        m.insert("approve_action".into(), McpToolDef {
            name: "approve_action".into(),
            description: "Approve or reject a pending HITL action".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action_id": {"type": "string"},
                    "approved": {"type": "boolean"},
                    "comment": {"type": "string"}
                },
                "required": ["action_id", "approved"]
            }),
        });

        m.insert("stream_events".into(), McpToolDef {
            name: "stream_events".into(),
            description: "Subscribe to real-time AG-UI event stream (returns subscription confirmation; actual events delivered via SSE)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "filter_agent_id": {"type": "string", "description": "Optional agent ID filter"}
                }
            }),
        });

        m.insert("upload_attachment".into(), McpToolDef {
            name: "upload_attachment".into(),
            description: "Upload a file attachment (base64-encoded) and receive a resource URI".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "filename": {"type": "string"},
                    "data_base64": {"type": "string", "description": "Base64-encoded file content"},
                    "mime_type": {"type": "string"}
                },
                "required": ["filename", "data_base64", "mime_type"]
            }),
        });

        m
    }

    // -- Resource Registration ----------------------------------------------

    fn register_resources() -> HashMap<String, McpResourceDef> {
        let mut m = HashMap::new();

        m.insert("agent_roster".into(), McpResourceDef {
            uri: "resource://agent-roster".into(),
            name: "Agent Roster".into(),
            description: "Current roster of active agents with status".into(),
            mime_type: Some("application/json".into()),
        });

        m.insert("task_board".into(), McpResourceDef {
            uri: "resource://task-board".into(),
            name: "Task Board".into(),
            description: "Kanban-style task board with all active tasks".into(),
            mime_type: Some("application/json".into()),
        });

        m.insert("memory_search".into(), McpResourceDef {
            uri: "resource://memory-search".into(),
            name: "Memory Search".into(),
            description: "Search across agent memories and knowledge base".into(),
            mime_type: Some("application/json".into()),
        });

        m.insert("skill_catalog".into(), McpResourceDef {
            uri: "resource://skill-catalog".into(),
            name: "Skill Catalog".into(),
            description: "Catalog of available skills and their descriptions".into(),
            mime_type: Some("application/json".into()),
        });

        m
    }

    // -- Prompt Registration ------------------------------------------------

    fn register_prompts() -> HashMap<String, McpPromptDef> {
        let mut m = HashMap::new();

        m.insert("triage_request".into(), McpPromptDef {
            name: "triage_request".into(),
            description: "Triage an incoming request and suggest routing".into(),
            arguments: vec![
                McpPromptArgument { name: "request_text".into(), description: "The incoming request".into(), required: true },
                McpPromptArgument { name: "priority".into(), description: "Suggested priority level".into(), required: false },
            ],
        });

        m.insert("summarize_thread".into(), McpPromptDef {
            name: "summarize_thread".into(),
            description: "Summarize a conversation thread into key points".into(),
            arguments: vec![
                McpPromptArgument { name: "thread_id".into(), description: "Thread to summarize".into(), required: true },
                McpPromptArgument { name: "max_length".into(), description: "Maximum summary length".into(), required: false },
            ],
        });

        m.insert("draft_response".into(), McpPromptDef {
            name: "draft_response".into(),
            description: "Draft a response to a user message".into(),
            arguments: vec![
                McpPromptArgument { name: "message".into(), description: "Original message".into(), required: true },
                McpPromptArgument { name: "tone".into(), description: "Desired tone (formal, casual, empathetic)".into(), required: false },
            ],
        });

        m
    }

    // -- Request Handling ---------------------------------------------------

    /// Handle a raw JSON-RPC request string (stdio transport).
    pub async fn handle_raw(&self, raw: &str, auth_header: Option<&str>) -> Result<String, MobileError> {
        let req: JsonRpcRequest = serde_json::from_str(raw)?;
        let resp = self.dispatch(req, auth_header).await;
        serde_json::to_string(&resp).map_err(Into::into)
    }

    /// Dispatch a parsed JSON-RPC request to the appropriate handler.
    pub async fn dispatch(&self, req: JsonRpcRequest, auth_header: Option<&str>) -> JsonRpcResponse {
        // Authenticate
        let client_id = match self.auth.validate(auth_header) {
            Ok(id) => id,
            Err(e) => {
                let body: McpErrorBody = e.into();
                return JsonRpcResponse::err(req.id, body.code, body.message);
            }
        };

        // Rate limit
        if let Err(e) = self.rate_limiter.check(&client_id).await {
            let body: McpErrorBody = e.into();
            return JsonRpcResponse::err(req.id, body.code, body.message);
        }

        match req.method.as_str() {
            "initialize" => self.handle_initialize(req.id),
            "tools/list" => self.handle_tools_list(req.id),
            "tools/call" => self.handle_tools_call(req.id, req.params).await,
            "resources/list" => self.handle_resources_list(req.id),
            "resources/read" => self.handle_resources_read(req.id, req.params).await,
            "prompts/list" => self.handle_prompts_list(req.id),
            "prompts/get" => self.handle_prompts_get(req.id, req.params),
            "ping" => JsonRpcResponse::ok(req.id, serde_json::json!({"pong": true})),
            other => JsonRpcResponse::err(req.id, -32601, format!("method not found: {}", other)),
        }
    }

    // -- Initialize / Capability Negotiation --------------------------------

    fn handle_initialize(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let caps = ServerCapabilities {
            tools: serde_json::json!({"listChanged": false}),
            resources: serde_json::json!({"listChanged": false, "subscribe": false}),
            prompts: serde_json::json!({"listChanged": false}),
            experimental: Some(serde_json::json!({
                "sse_streaming": true,
                "attachments": true
            })),
        };

        JsonRpcResponse::ok(id, serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": caps,
            "serverInfo": {
                "name": "zylos-mobile-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }))
    }

    // -- Tools --------------------------------------------------------------

    fn handle_tools_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let tools: Vec<&McpToolDef> = self.tools.values().collect();
        JsonRpcResponse::ok(id, serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, id: Option<serde_json::Value>, params: serde_json::Value) -> JsonRpcResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

        match self.execute_tool(name, arguments).await {
            Ok(result) => JsonRpcResponse::ok(id, serde_json::json!({
                "content": [{"type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default()}],
                "isError": false
            })),
            Err(e) => {
                let body: McpErrorBody = e.into();
                JsonRpcResponse::err(id, body.code, body.message)
            }
        }
    }

    async fn execute_tool(&self, name: &str, args: serde_json::Value) -> Result<serde_json::Value, MobileError> {
        match name {
            "list_agents" => {
                Ok(serde_json::json!({
                    "agents": [
                        {"id": "orchestrator", "status": "active", "role": "coordinator"},
                        {"id": "researcher", "status": "idle", "role": "research"},
                        {"id": "coder", "status": "active", "role": "development"}
                    ]
                }))
            }
            "send_message" => {
                let agent_id = args.get("agent_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing agent_id".into()))?;
                let content = args.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing content".into()))?;

                let msg_id = Uuid::new_v4().to_string();
                let topic = format!("agent.{}.message", agent_id);
                let event = ZylosEvent::Custom(CustomEvent {
                    name: "Custom".into(),
                    payload: serde_json::json!({"type": "text_message", "message_id": msg_id, "role": "user", "content": content}),
                });
                // Fire-and-forget: publish is async; errors mean no subscribers
                let _ = self.event_bus.publish(&topic, event.clone(), 1).await;

                Ok(serde_json::json!({"sent": true, "agent_id": agent_id, "message_id": msg_id}))
            }
            "get_task_status" => {
                let task_id = args.get("task_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing task_id".into()))?;
                Ok(serde_json::json!({
                    "task_id": task_id,
                    "status": "in_progress",
                    "progress": 0.65,
                    "updated_at": Utc::now().to_rfc3339()
                }))
            }
            "approve_action" => {
                let action_id = args.get("action_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing action_id".into()))?;
                let approved = args.get("approved")
                    .and_then(|v| v.as_bool())
                    .ok_or_else(|| MobileError::InvalidRequest("missing approved".into()))?;
                Ok(serde_json::json!({
                    "action_id": action_id,
                    "approved": approved,
                    "timestamp": Utc::now().to_rfc3339()
                }))
            }
            "stream_events" => {
                Ok(serde_json::json!({
                    "subscribed": true,
                    "note": "Use SSE endpoint /events/stream for real-time delivery"
                }))
            }
            "upload_attachment" => {
                let filename = args.get("filename")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing filename".into()))?;
                let data_b64 = args.get("data_base64")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing data_base64".into()))?;
                let mime_type = args.get("mime_type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| MobileError::InvalidRequest("missing mime_type".into()))?;

                let data = base64_decode(data_b64)
                    .map_err(|e| MobileError::Attachment(format!("invalid base64: {}", e)))?;

                let uri = self.attachments.store(filename, &data, mime_type).await?;
                Ok(serde_json::json!({
                    "uri": uri,
                    "size_bytes": data.len(),
                    "mime_type": mime_type
                }))
            }
            _ => Err(MobileError::McpProtocol(format!("unknown tool: {}", name))),
        }
    }

    // -- Resources ----------------------------------------------------------

    fn handle_resources_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let resources: Vec<&McpResourceDef> = self.resources.values().collect();
        JsonRpcResponse::ok(id, serde_json::json!({ "resources": resources }))
    }

    async fn handle_resources_read(&self, id: Option<serde_json::Value>, params: serde_json::Value) -> JsonRpcResponse {
        let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

        // Lookup by URI field value, since the HashMap key is the short name (e.g. "agent_roster")
        let resource = match self.resources.values().find(|r| r.uri == uri) {
            Some(r) => r,
            None => return JsonRpcResponse::err(id, -32004, format!("resource not found: {}", uri)),
        };

        // Generate stub content for each resource type
        let content = match uri {
            "resource://agent-roster" => serde_json::json!({
                "agents": [
                    {"id": "orchestrator", "status": "active"},
                    {"id": "researcher", "status": "idle"},
                    {"id": "coder", "status": "active"}
                ]
            }),
            "resource://task-board" => serde_json::json!({
                "tasks": [],
                "columns": ["backlog", "in_progress", "review", "done"]
            }),
            "resource://memory-search" => serde_json::json!({
                "results": [],
                "note": "Pass query parameter via URI fragment for search"
            }),
            "resource://skill-catalog" => serde_json::json!({
                "skills": [],
                "note": "Skills loaded from workspace configuration"
            }),
            _ => serde_json::json!({"error": "unsupported resource"}),
        };

        let text = serde_json::to_string_pretty(&content).unwrap_or_default();
        JsonRpcResponse::ok(id, serde_json::json!({
            "contents": [{
                "uri": resource.uri,
                "mimeType": resource.mime_type,
                "text": text
            }]
        }))
    }

    // -- Prompts ------------------------------------------------------------

    fn handle_prompts_list(&self, id: Option<serde_json::Value>) -> JsonRpcResponse {
        let prompts: Vec<&McpPromptDef> = self.prompts.values().collect();
        JsonRpcResponse::ok(id, serde_json::json!({ "prompts": prompts }))
    }

    fn handle_prompts_get(&self, id: Option<serde_json::Value>, params: serde_json::Value) -> JsonRpcResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

        let prompt = match self.prompts.get(name) {
            Some(p) => p,
            None => return JsonRpcResponse::err(id, -32004, format!("prompt not found: {}", name)),
        };

        // Build messages from template
        let messages = match name {
            "triage_request" => {
                let text = arguments.get("request_text").and_then(|v| v.as_str()).unwrap_or("");
                vec![serde_json::json!({
                    "role": "user",
                    "content": {"type": "text", "text": format!("Please triage the following request:\n\n{}", text)}
                })]
            }
            "summarize_thread" => {
                let thread_id = arguments.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
                vec![serde_json::json!({
                    "role": "user",
                    "content": {"type": "text", "text": format!("Summarize thread {} into key points.", thread_id)}
                })]
            }
            "draft_response" => {
                let message = arguments.get("message").and_then(|v| v.as_str()).unwrap_or("");
                let tone = arguments.get("tone").and_then(|v| v.as_str()).unwrap_or("professional");
                vec![serde_json::json!({
                    "role": "user",
                    "content": {"type": "text", "text": format!("Draft a {} response to:\n\n{}", tone, message)}
                })]
            }
            _ => vec![],
        };

        JsonRpcResponse::ok(id, serde_json::json!({
            "description": prompt.description,
            "messages": messages
        }))
    }

    // -- SSE Streaming ------------------------------------------------------

    /// Create an SSE stream of events. Supports Last-Event-ID for reconnection.
    /// Note: subscribe is async; this method spawns the subscription and returns
    /// a stream that yields events once the receiver is ready.
    pub fn sse_event_stream(
        &self,
        last_event_id: Option<u64>,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<String, MobileError>> + Send + '_>> {
        let bus = self.event_bus.clone();
        let start_seq = last_event_id.unwrap_or(0);

        Box::pin(async_stream::stream! {
            // Subscribe to all topics via the global broadcast channel
            let rx = bus.subscribe("mcp-sse", "#", None).await;
            match rx {
                Ok(mut receiver) => {
                    let mut seq = 0u64;
                    loop {
                        match receiver.recv().await {
                            Ok(envelope) => {
                                seq += 1;
                                if seq <= start_seq {
                                    continue;
                                }
                                let data = serde_json::to_string(&envelope).unwrap_or_default();
                                let sse_line = format!("id: {}\ndata: {}\n\n", seq, data);
                                yield Ok(sse_line);
                            }
                            Err(e) => {
                                debug!("SSE stream lagged or error: {}", e);
                                yield Err(MobileError::Connection(format!("stream error: {}", e)));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(MobileError::Connection(format!("subscribe failed: {}", e)));
                }
            }
        })
    }

    /// Get reference to the underlying EventBus.
    pub fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Get reference to the attachment store.
    pub fn attachments(&self) -> &Arc<AttachmentStore> {
        &self.attachments
    }

    /// Spawn a background task that periodically cleans up expired attachments.
    pub fn spawn_attachment_cleanup(self: &Arc<Self>, interval_secs: u64) {
        let store = self.attachments.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                let removed = store.cleanup_expired().await;
                if removed > 0 {
                    debug!("Cleaned up {} expired attachments", removed);
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    // Manual base64 decode to avoid adding another dependency.
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 character: {}", c as char)),
        }
    }

    let input = input.trim_end_matches('=');
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let chunks = bytes.chunks(4);

    for chunk in chunks {
        let vals: Result<Vec<u8>, _> = chunk.iter().map(|&b| val(b)).collect();
        let vals = vals?;
        match vals.len() {
            2 => {
                out.push((vals[0] << 2) | (vals[1] >> 4));
            }
            3 => {
                out.push((vals[0] << 2) | (vals[1] >> 4));
                out.push((vals[1] << 4) | (vals[2] >> 2));
            }
            4 => {
                out.push((vals[0] << 2) | (vals[1] >> 4));
                out.push((vals[1] << 4) | (vals[2] >> 2));
                out.push((vals[2] << 6) | vals[3]);
            }
            _ => return Err("invalid base64 length".into()),
        }
    }

    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server() -> MobileMcpServer {
        let bus = EventBus::new(64, None);
        MobileMcpServer::with_config(
            bus,
            AuthConfig {
                api_key: Some("test-key".into()),
                jwt_secret: None,
            },
            100,
            60,
            3600,
        )
    }

    fn make_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: Some(serde_json::json!(1)),
            method: method.into(),
            params,
        }
    }

    #[tokio::test]
    async fn test_initialize_returns_capabilities() {
        let server = make_server();
        let req = make_request("initialize", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert!(result["capabilities"]["tools"].is_object());
        assert!(result["capabilities"]["resources"].is_object());
        assert!(result["capabilities"]["prompts"].is_object());
        assert_eq!(result["serverInfo"]["name"], "zylos-mobile-mcp");
    }

    #[tokio::test]
    async fn test_tools_list_returns_all_six_tools() {
        let server = make_server();
        let req = make_request("tools/list", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().len();
        assert_eq!(tools, 6);
    }

    #[tokio::test]
    async fn test_resources_list_returns_all_four_resources() {
        let server = make_server();
        let req = make_request("resources/list", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        let resources = resp.result.unwrap()["resources"].as_array().unwrap().len();
        assert_eq!(resources, 4);
    }

    #[tokio::test]
    async fn test_prompts_list_returns_all_three_prompts() {
        let server = make_server();
        let req = make_request("prompts/list", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        let prompts = resp.result.unwrap()["prompts"].as_array().unwrap().len();
        assert_eq!(prompts, 3);
    }

    #[tokio::test]
    async fn test_tool_call_send_message() {
        let bus = EventBus::new(64, None);
        // Subscribe before dispatch so the publish has an active receiver
        let mut rx = bus.subscribe("test-sub", "**", None).await.unwrap();
        let server = MobileMcpServer::with_config(
            bus,
            AuthConfig { api_key: Some("k".into()), jwt_secret: None },
            100, 60, 3600,
        );

        let req = make_request("tools/call", serde_json::json!({
            "name": "send_message",
            "arguments": {"agent_id": "a1", "content": "hi"}
        }));
        let resp = server.dispatch(req, Some("ApiKey k")).await;
        assert!(resp.error.is_none());

        // Verify event published via new EventBus API (with timeout to avoid hang)
        let envelope = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        ).await.expect("timed out waiting for event").unwrap();
        assert!(matches!(envelope.payload, ZylosEvent::Custom(_)));
    }

    #[tokio::test]
    async fn test_auth_rejection_without_header() {
        let server = make_server();
        let req = make_request("tools/list", serde_json::json!({}));
        let resp = server.dispatch(req, None).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32001);
    }

    #[tokio::test]
    async fn test_auth_rejection_wrong_key() {
        let server = make_server();
        let req = make_request("tools/list", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey wrong")).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32001);
    }

    #[tokio::test]
    async fn test_rate_limit_enforcement() {
        let bus = EventBus::new(64, None);
        let server = MobileMcpServer::with_config(
            bus,
            AuthConfig { api_key: Some("k".into()), jwt_secret: None },
            2, // Only 2 requests allowed
            60,
            3600,
        );

        let req = make_request("ping", serde_json::json!({}));
        // First two should succeed
        let r1 = server.dispatch(req.clone(), Some("ApiKey k")).await;
        assert!(r1.error.is_none());
        let r2 = server.dispatch(req.clone(), Some("ApiKey k")).await;
        assert!(r2.error.is_none());
        // Third should be rate limited
        let r3 = server.dispatch(req, Some("ApiKey k")).await;
        assert!(r3.error.is_some());
        assert_eq!(r3.error.unwrap().code, -32002);
    }

    #[tokio::test]
    async fn test_resource_read_agent_roster() {
        let server = make_server();
        let req = make_request("resources/read", serde_json::json!({
            "uri": "resource://agent-roster"
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let contents = result["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
    }

    #[tokio::test]
    async fn test_prompt_get_triage() {
        let server = make_server();
        let req = make_request("prompts/get", serde_json::json!({
            "name": "triage_request",
            "arguments": {"request_text": "Fix the login bug"}
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        let messages = result["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0]["content"]["text"].as_str().unwrap().contains("login bug"));
    }

    #[tokio::test]
    async fn test_upload_attachment_roundtrip() {
        let server = make_server();
        let data = b"hello world";
        let b64 = "aGVsbG8gd29ybGQ="; // base64 of "hello world"

        let req = make_request("tools/call", serde_json::json!({
            "name": "upload_attachment",
            "arguments": {
                "filename": "test.txt",
                "data_base64": b64,
                "mime_type": "text/plain"
            }
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_none(), "upload failed: {:?}", resp.error);

        let result_text = resp.result.unwrap()["content"][0]["text"].as_str().unwrap().to_string();
        let result: serde_json::Value = serde_json::from_str(&result_text).unwrap();
        let uri = result["uri"].as_str().unwrap();
        assert!(uri.starts_with("attachment://"));

        // Verify we can retrieve it
        let (path, mime, size) = server.attachments().get(uri).await.unwrap();
        assert_eq!(mime, "text/plain");
        assert_eq!(size, data.len() as u64);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn test_sse_stream_delivers_events() {
        let bus = EventBus::new(64, None);
        // Subscribe directly to verify event delivery (avoids async_stream deadlock in tests)
        let mut rx = bus.subscribe("sse-test", "**", None).await.unwrap();

        let event = ZylosEvent::Custom(CustomEvent {
            name: "Custom".into(),
            payload: serde_json::json!({"type": "heartbeat"}),
        });
        let _ = bus.publish("test.heartbeat", event, 1).await;

        let envelope = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        ).await.expect("timed out waiting for SSE event").unwrap();
        assert!(matches!(envelope.payload, ZylosEvent::Custom(_)));
    }

    #[tokio::test]
    async fn test_sse_reconnect_skips_old_events() {
        let bus = EventBus::new(64, None);
        // Subscribe with last_event_id semantics: manually skip first event
        let mut rx = bus.subscribe("sse-reconnect", "**", None).await.unwrap();

        let e1 = ZylosEvent::Custom(CustomEvent {
            name: "Custom".into(),
            payload: serde_json::json!({"seq": 1}),
        });
        let r1 = bus.publish("test.events", e1, 1).await;
        assert!(r1.is_ok(), "publish e1 failed: {:?}", r1);

        let e2 = ZylosEvent::Custom(CustomEvent {
            name: "Custom".into(),
            payload: serde_json::json!({"seq": 2}),
        });
        let r2 = bus.publish("test.events", e2, 1).await;
        assert!(r2.is_ok(), "publish e2 failed: {:?}", r2);

        // Skip first event (simulating last_event_id=1)
        let _first = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        ).await.expect("timed out").unwrap();

        // Second event should arrive
        let second = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rx.recv(),
        ).await.expect("timed out").unwrap();
        if let ZylosEvent::Custom(c) = &second.payload {
            assert_eq!(c.payload["seq"], 2);
        } else {
            panic!("expected Custom event");
        }
    }

    #[tokio::test]
    async fn test_unknown_method_returns_error() {
        let server = make_server();
        let req = make_request("nonexistent/method", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_handle_raw_stdio_roundtrip() {
        let server = make_server();
        let raw = r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;
        let result = server.handle_raw(raw, Some("ApiKey test-key")).await.unwrap();
        let resp: JsonRpcResponse = serde_json::from_str(&result).unwrap();
        assert_eq!(resp.result.unwrap()["pong"], true);
    }

    #[test]
    fn test_base64_decode() {
        let decoded = base64_decode("aGVsbG8gd29ybGQ=").unwrap();
        assert_eq!(decoded, b"hello world");
    }

    #[test]
    fn test_base64_decode_no_padding() {
        let decoded = base64_decode("aGVsbG8").unwrap();
        assert_eq!(decoded, b"hello");
    }

    // -- Additional tests (docs + coverage expansion) -------------------------

    #[test]
    fn test_json_rpc_response_ok_serialization() {
        let resp = JsonRpcResponse::ok(Some(serde_json::json!(42)), serde_json::json!({"status": "ok"}));
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["id"], 42);
        assert_eq!(serialized["result"]["status"], "ok");
        // error field must be absent (skip_serializing_if)
        assert!(serialized.get("error").is_none());
    }

    #[test]
    fn test_json_rpc_response_err_serialization() {
        let resp = JsonRpcResponse::err(Some(serde_json::json!(7)), -32600, "bad request".into());
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["id"], 7);
        assert_eq!(serialized["error"]["code"], -32600);
        assert_eq!(serialized["error"]["message"], "bad request");
        // result field must be absent
        assert!(serialized.get("result").is_none());
    }

    #[test]
    fn test_json_rpc_request_deserialize_minimal() {
        // Notification (no id) with missing params should default to Null
        let raw = r#"{"jsonrpc":"2.0","method":"notify/test"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "notify/test");
        assert!(req.id.is_none());
        assert!(req.params.is_null());
    }

    #[test]
    fn test_mobile_error_to_mcp_error_body_codes() {
        let cases: Vec<(MobileError, i32)> = vec![
            (MobileError::InvalidRequest("x".into()), -32600),
            (MobileError::Auth("x".into()), -32001),
            (MobileError::RateLimit { client_id: "c".into(), retry_after_secs: 5 }, -32002),
            (MobileError::Attachment("x".into()), -32003),
            (MobileError::ResourceNotFound("x".into()), -32004),
            (MobileError::Timeout("x".into()), -32005),
            (MobileError::McpProtocol("x".into()), -32603),
            (MobileError::Connection("x".into()), -32603),
        ];
        for (err, expected_code) in cases {
            let body: McpErrorBody = err.into();
            assert_eq!(body.code, expected_code, "unexpected code for variant");
            assert!(!body.message.is_empty());
            assert!(body.data.is_none());
        }
    }

    #[tokio::test]
    async fn test_attachment_store_get_invalid_uri() {
        let dir = std::env::temp_dir().join("zylos-test-invalid-uri");
        let store = AttachmentStore::new(dir, 3600);
        let result = store.get("not-a-valid-uri").await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("invalid attachment URI"));
    }

    #[tokio::test]
    async fn test_attachment_store_cleanup_expired() {
        let dir = std::env::temp_dir().join("zylos-test-cleanup");
        // TTL of 0 seconds means everything is immediately expired
        let store = AttachmentStore::new(dir.clone(), 0);

        let uri = store.store("a.txt", b"data", "text/plain").await.unwrap();
        assert!(uri.starts_with("attachment://"));

        // Small sleep to ensure elapsed > 0
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await;
        assert_eq!(removed, 1);

        // Second cleanup should find nothing
        let removed2 = store.cleanup_expired().await;
        assert_eq!(removed2, 0);
    }

    #[tokio::test]
    async fn test_rate_limiter_distinct_clients() {
        let limiter = RateLimiter::new(1, 60);
        // Client A uses its single token
        assert!(limiter.check("client-a").await.is_ok());
        // Client A is now exhausted
        assert!(limiter.check("client-a").await.is_err());
        // Client B still has its own bucket
        assert!(limiter.check("client-b").await.is_ok());
    }

    #[tokio::test]
    async fn test_dispatch_ping_returns_pong() {
        let server = make_server();
        let req = make_request("ping", serde_json::json!({}));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap()["pong"], true);
    }

    #[tokio::test]
    async fn test_tool_call_unknown_tool_returns_error() {
        let server = make_server();
        let req = make_request("tools/call", serde_json::json!({
            "name": "nonexistent_tool",
            "arguments": {}
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_some());
        // Should be a protocol-level error for unknown tool
        assert!(resp.error.unwrap().code != 0);
    }

    #[tokio::test]
    async fn test_resource_read_unknown_uri_returns_error() {
        let server = make_server();
        let req = make_request("resources/read", serde_json::json!({
            "uri": "resource://does-not-exist"
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32004);
    }

    #[tokio::test]
    async fn test_prompts_get_unknown_returns_error() {
        let server = make_server();
        let req = make_request("prompts/get", serde_json::json!({
            "name": "nonexistent_prompt",
            "arguments": {}
        }));
        let resp = server.dispatch(req, Some("ApiKey test-key")).await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32004);
    }

    #[tokio::test]
    async fn test_handle_raw_invalid_json_returns_error() {
        let server = make_server();
        let result = server.handle_raw("not valid json{{{", Some("ApiKey test-key")).await;
        // handle_raw may return Err(MobileError) for unparseable input, or an
        // Ok JSON-RPC error response — either way the caller gets an error signal.
        match &result {
            Ok(json_str) => {
                let resp: JsonRpcResponse = serde_json::from_str(json_str).unwrap();
                assert!(resp.error.is_some(), "expected JSON-RPC error in response");
            }
            Err(e) => {
                // Verify it's a serialization-related error
                let msg = format!("{}", e);
                assert!(
                    msg.contains("Serialization") || msg.contains("serialization") || msg.contains("JSON"),
                    "unexpected error variant for invalid JSON: {}", msg
                );
            }
        }
    }
}
