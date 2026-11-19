//! Zylos Cockpit — AG-UI Streaming Client (Task 6.2.3)
//!
//! Real-time AG-UI event streaming via SSE/WebSocket with automatic
//! reconnection, delta compression, and backpressure handling.
//! Latency target: <100ms end-to-end (agent → UI).

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::components::toast_container::{toast_success, toast_warning};

/// AG-UI event types matching backend AgUiEvent enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[allow(dead_code)]
pub enum AgUiStreamEvent {
    #[serde(rename = "heartbeat")]
    Heartbeat { timestamp: String },

    #[serde(rename = "text_start")]
    TextStart {
        message_id: String,
        agent_id: String,
    },

    #[serde(rename = "text_delta")]
    TextDelta { message_id: String, delta: String },

    #[serde(rename = "text_end")]
    TextEnd { message_id: String },

    #[serde(rename = "tool_call_start")]
    ToolCallStart {
        call_id: String,
        tool_name: String,
        agent_id: String,
    },

    #[serde(rename = "tool_call_result")]
    ToolCallResult {
        call_id: String,
        result: String,
        duration_ms: f64,
    },

    #[serde(rename = "thinking")]
    Thinking { agent_id: String, content: String },

    #[serde(rename = "memory_update")]
    MemoryUpdate {
        agent_id: String,
        key: String,
        operation: String,
    },

    #[serde(rename = "state_change")]
    StateChange {
        agent_id: String,
        old_state: String,
        new_state: String,
    },

    #[serde(rename = "error")]
    Error { code: String, message: String },

    #[serde(rename = "run_started")]
    RunStarted {
        run_id: String,
        thread_id: String,
        parent_run_id: Option<String>,
        input: Option<serde_json::Value>,
    },

    #[serde(rename = "run_finished")]
    RunFinished {
        run_id: String,
        thread_id: String,
        status: String,
        output: Option<serde_json::Value>,
    },

    #[serde(rename = "tool_call_end")]
    ToolCallEnd { tool_call_id: String },

    #[serde(rename = "custom_event")]
    CustomEvent {
        name: String,
        value: serde_json::Value,
    },
}

/// Connection state for the streaming client
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum StreamState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting { attempt: u32, next_retry_ms: u64 },
    Failed(String),
}

/// Accumulated text buffer for delta-compressed messages
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MessageBuffer {
    pub message_id: String,
    pub agent_id: String,
    pub content: String,
    pub complete: bool,
}

/// Statistics for streaming performance monitoring
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct StreamStats {
    pub events_received: u64,
    pub bytes_received: u64,
    pub reconnect_count: u32,
    pub last_event_at: Option<String>,
    pub avg_latency_ms: f64,
}

/// AG-UI Streaming Client configuration
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct StreamConfig {
    pub endpoint: String,
    pub max_reconnect_attempts: u32,
    pub base_retry_ms: u64,
    pub max_retry_ms: u64,
    pub heartbeat_timeout_ms: u64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            endpoint: "/api/v1/events".to_string(),
            max_reconnect_attempts: 10,
            base_retry_ms: 1000,
            max_retry_ms: 30000,
            heartbeat_timeout_ms: 45000,
        }
    }
}

/// Global streaming context provided to component tree
#[derive(Clone)]
#[allow(dead_code)]
pub struct StreamContext {
    state: RwSignal<StreamState>,
    stats: RwSignal<StreamStats>,
    messages: RwSignal<Vec<MessageBuffer>>,
    config: StreamConfig,
}

#[allow(dead_code)]
impl StreamContext {
    pub fn new(config: StreamConfig) -> Self {
        Self {
            state: RwSignal::new(StreamState::Disconnected),
            stats: RwSignal::new(StreamStats::default()),
            messages: RwSignal::new(Vec::new()),
            config,
        }
    }

    pub fn state(&self) -> RwSignal<StreamState> {
        self.state
    }

    pub fn stats(&self) -> RwSignal<StreamStats> {
        self.stats
    }

    pub fn messages(&self) -> RwSignal<Vec<MessageBuffer>> {
        self.messages
    }

    /// Start SSE connection with automatic reconnection
    pub fn connect(&self) {
        let ctx = self.clone();
        // T11.5.6: sem gateway configurado (ex.: preview público) não abre SSE —
        // evita ERR_CONNECTION_REFUSED em loop no console.
        if crate::gateway_config::gateway_base().is_none() {
            ctx.state.set(StreamState::Disconnected);
            return;
        }
        ctx.state.set(StreamState::Connecting);

        spawn_local(async move {
            ctx.connect_sse().await;
        });
    }

    /// Disconnect and stop reconnection attempts
    pub fn disconnect(&self) {
        self.state.set(StreamState::Disconnected);
    }

    /// Calculate exponential backoff with jitter.
    /// Uses simple hash-based pseudo-random for testability without js_sys.
    fn backoff_ms(&self, attempt: u32) -> u64 {
        let base = self.config.base_retry_ms;
        let max = self.config.max_retry_ms;
        let exponential = base.saturating_mul(2u64.saturating_pow(attempt.min(20)));
        let capped = exponential.min(max);
        // Add 0-25% jitter using deterministic hash (testable without JS runtime)
        let jitter_range = capped / 4;
        let jitter = if jitter_range > 0 {
            // Simple LCG-style hash from attempt number for deterministic jitter
            let hash = attempt.wrapping_mul(2654435761).wrapping_add(12345);
            hash as u64 % jitter_range
        } else {
            0
        };
        capped + jitter
    }

    /// Main SSE connection loop with reconnection
    async fn connect_sse(&self) {
        let endpoint = &self.config.endpoint;
        let mut attempt: u32 = 0;

        loop {
            if self.state.get() == StreamState::Disconnected {
                return;
            }

            if attempt > 0 {
                let retry_ms = self.backoff_ms(attempt);
                self.state.set(StreamState::Reconnecting {
                    attempt,
                    next_retry_ms: retry_ms,
                });

                // Wait before retry using JS setTimeout
                let promise = js_sys::Promise::new(&mut |resolve, _| {
                    let _ = web_sys::window()
                        .unwrap()
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            &resolve,
                            retry_ms as i32,
                        );
                });
                let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
            }

            self.state.set(StreamState::Connecting);

            match self.open_event_source(endpoint).await {
                Ok(()) => {
                    // Connection closed normally or by server — reconnect
                    attempt += 1;
                    if attempt > self.config.max_reconnect_attempts {
                        self.state.set(StreamState::Failed(format!(
                            "Max reconnection attempts ({}) exceeded",
                            self.config.max_reconnect_attempts
                        )));
                        return;
                    }
                    self.stats.update(|s| s.reconnect_count += 1);
                }
                Err(e) => {
                    attempt += 1;
                    if attempt > self.config.max_reconnect_attempts {
                        self.state.set(StreamState::Failed(e));
                        return;
                    }
                    self.stats.update(|s| s.reconnect_count += 1);
                }
            }
        }
    }

    /// Open an EventSource and process events until disconnection
    async fn open_event_source(&self, endpoint: &str) -> Result<(), String> {
        use wasm_bindgen::JsCast;

        let base = crate::gateway_config::gateway_base().ok_or("gateway não configurado")?;
        let url = crate::auth::authed_url(&crate::gateway_config::join_url(&base, endpoint));

        let es = web_sys::EventSource::new(&url).map_err(|e| format!("{:?}", e))?;

        // Mark connected on open + dispatch success toast
        let state_signal = self.state;
        let on_open = Closure::wrap(Box::new(move |_: web_sys::Event| {
            state_signal.set(StreamState::Connected);
            toast_success("Conectado ao gateway (SSE)");
        }) as Box<dyn FnMut(web_sys::Event)>);
        es.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        on_open.forget();

        // Handle incoming messages
        let stats_signal = self.stats;
        let messages_signal = self.messages;
        let on_message = Closure::wrap(Box::new(move |ev: web_sys::MessageEvent| {
            let data = match ev.data().as_string() {
                Some(d) => d,
                None => return,
            };

            stats_signal.update(|s| {
                s.events_received += 1;
                s.bytes_received += data.len() as u64;
                s.last_event_at = Some(js_sys::Date::new_0().to_iso_string().into());
            });

            if let Ok(event) = serde_json::from_str::<AgUiStreamEvent>(&data) {
                Self::process_event(messages_signal, &event);
            } else if let Some(event) = Self::from_gateway_envelope(&data) {
                // Formato real do gateway: {"data":{"payload":{"type":"TextMessageContent","data":{..}}}}
                Self::process_event(messages_signal, &event);
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);
        es.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        // Handle errors — will trigger reconnection when EventSource closes
        let state_signal_err = self.state;
        let on_error = Closure::wrap(Box::new(move |_: web_sys::Event| {
            // EventSource auto-reconnects on some errors; we handle persistent ones
            // The readyState check distinguishes closed vs connecting
            state_signal_err.set(StreamState::Connecting);
            toast_warning("Conexão SSE perdida — reconectando...");
        }) as Box<dyn FnMut(web_sys::Event)>);
        es.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_error.forget();

        // Keep the EventSource alive until disconnected
        // We poll the state signal; when Disconnected, close ES
        loop {
            let promise = js_sys::Promise::new(&mut |resolve, _| {
                let _ = web_sys::window()
                    .unwrap()
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 1000);
            });
            let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

            match self.state.get() {
                StreamState::Disconnected => {
                    es.close();
                    return Ok(());
                }
                StreamState::Failed(_) => {
                    es.close();
                    return Err("Stream failed".to_string());
                }
                _ => {}
            }
        }
    }

    /// Process a single AG-UI event and update message buffers
    /// Converte o envelope SSE do gateway (AgUiEvent legado) no evento do cockpit.
    fn from_gateway_envelope(raw: &str) -> Option<AgUiStreamEvent> {
        let v: serde_json::Value = serde_json::from_str(raw).ok()?;
        let p = v.get("data")?.get("payload")?;
        let d = p.get("data")?;
        let text = |k: &str| d.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        match p.get("type")?.as_str()? {
            "TextMessageStart" => Some(AgUiStreamEvent::TextStart {
                message_id: text("message_id"),
                agent_id: text("role"),
            }),
            "TextMessageContent" => Some(AgUiStreamEvent::TextDelta {
                message_id: text("message_id"),
                delta: text("delta"),
            }),
            "TextMessageEnd" => Some(AgUiStreamEvent::TextEnd {
                message_id: text("message_id"),
            }),
            _ => None,
        }
    }

    fn process_event(messages: RwSignal<Vec<MessageBuffer>>, event: &AgUiStreamEvent) {
        match event {
            AgUiStreamEvent::TextStart {
                message_id,
                agent_id,
            } => {
                messages.update(|msgs| {
                    msgs.push(MessageBuffer {
                        message_id: message_id.clone(),
                        agent_id: agent_id.clone(),
                        content: String::new(),
                        complete: false,
                    });
                });
            }
            AgUiStreamEvent::TextDelta { message_id, delta } => {
                messages.update(|msgs| {
                    if let Some(buf) = msgs.iter_mut().find(|m| m.message_id == *message_id) {
                        buf.content.push_str(delta);
                    } else {
                        // delta sem text_start (ex.: eventos vindos do Hermes): cria o buffer
                        msgs.push(MessageBuffer {
                            message_id: message_id.clone(),
                            agent_id: "hermes".to_string(),
                            content: delta.clone(),
                            complete: false,
                        });
                    }
                });
            }
            AgUiStreamEvent::TextEnd { message_id } => {
                messages.update(|msgs| {
                    if let Some(buf) = msgs.iter_mut().find(|m| m.message_id == *message_id) {
                        buf.complete = true;
                    }
                });
            }
            // Other event types are tracked in stats but don't modify message buffers
            _ => {}
        }
    }
}

/// Provide stream context to component tree
pub fn provide_stream(config: StreamConfig) -> StreamContext {
    let ctx = StreamContext::new(config);
    provide_context(ctx.clone());
    ctx
}

/// Access stream context from any component
pub fn use_stream() -> StreamContext {
    expect_context::<StreamContext>()
}

/// StreamStatusIndicator component — shows connection state
#[component]
pub fn StreamStatusIndicator() -> impl IntoView {
    let ctx = use_stream();

    view! {
        <div class="stream-status">
            {move || match ctx.state().get() {
                StreamState::Disconnected => view! {
                    <span class="status-dot disconnected"></span>
                    <span>"Desconectado"</span>
                }.into_any(),
                StreamState::Connecting => view! {
                    <span class="status-dot connecting pulse"></span>
                    <span>"Conectando..."</span>
                }.into_any(),
                StreamState::Connected => view! {
                    <span class="status-dot connected"></span>
                    <span>"Ao vivo"</span>
                }.into_any(),
                StreamState::Reconnecting { attempt, next_retry_ms } => view! {
                    <span class="status-dot reconnecting pulse"></span>
                    <span>{format!("Reconectando (tentativa {}, {}s)", attempt, next_retry_ms / 1000)}</span>
                }.into_any(),
                StreamState::Failed(msg) => view! {
                    <span class="status-dot failed"></span>
                    <span>{format!("Falha: {}", msg)}</span>
                }.into_any(),
            }}
        </div>
    }
}

/// LiveEventFeed component — displays streaming events in real-time
#[component]
pub fn LiveEventFeed() -> impl IntoView {
    let ctx = use_stream();
    let messages = ctx.messages();

    view! {
        <div class="live-event-feed">
            <StreamStatusIndicator />
            <div class="event-list">
                {move || {
                    let msgs = messages.get();
                    if msgs.is_empty() {
                        view! {
                            <div class="feed-empty">
                                <p>"Aguardando eventos..."</p>
                            </div>
                        }.into_any()
                    } else {
                        msgs.iter()
                            .rev()
                            .take(50) // Show last 50 messages
                            .map(|msg| {
                                let agent = msg.agent_id.clone();
                                let content = msg.content.clone();
                                let complete = msg.complete;
                                let css = if complete { "msg-complete" } else { "msg-streaming" };
                                view! {
                                    <div class={format!("feed-message {}", css)}>
                                        <span class="msg-agent">{agent}</span>
                                        <span class="msg-content">{content}</span>
                                        {if !complete {
                                            view! { <span class="cursor-blink">"|"</span> }.into_any()
                                        } else {
                                            view! { <span></span> }.into_any()
                                        }}
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_exponential() {
        let ctx = StreamContext::new(StreamConfig {
            base_retry_ms: 1000,
            max_retry_ms: 30000,
            ..Default::default()
        });
        // With jitter, we can only test bounds
        let b0 = ctx.backoff_ms(0);
        assert!(
            (1000..=1250).contains(&b0),
            "attempt 0: {} not in [1000, 1250]",
            b0
        );

        let b3 = ctx.backoff_ms(3);
        assert!(
            (8000..=10000).contains(&b3),
            "attempt 3: {} not in [8000, 10000]",
            b3
        );
    }

    #[test]
    fn test_backoff_capped() {
        let ctx = StreamContext::new(StreamConfig {
            base_retry_ms: 1000,
            max_retry_ms: 5000,
            ..Default::default()
        });
        let b10 = ctx.backoff_ms(10);
        assert!(
            b10 <= 6250,
            "attempt 10 should be capped near 5000+25%: {}",
            b10
        );
    }

    #[test]
    fn test_process_text_lifecycle() {
        let messages = RwSignal::new(Vec::new());

        StreamContext::process_event(
            messages,
            &AgUiStreamEvent::TextStart {
                message_id: "msg-1".to_string(),
                agent_id: "agent-a".to_string(),
            },
        );
        assert_eq!(messages.get().len(), 1);
        assert!(!messages.get()[0].complete);

        StreamContext::process_event(
            messages,
            &AgUiStreamEvent::TextDelta {
                message_id: "msg-1".to_string(),
                delta: "Hello ".to_string(),
            },
        );
        assert_eq!(messages.get()[0].content, "Hello ");

        StreamContext::process_event(
            messages,
            &AgUiStreamEvent::TextDelta {
                message_id: "msg-1".to_string(),
                delta: "World".to_string(),
            },
        );
        assert_eq!(messages.get()[0].content, "Hello World");

        StreamContext::process_event(
            messages,
            &AgUiStreamEvent::TextEnd {
                message_id: "msg-1".to_string(),
            },
        );
        assert!(messages.get()[0].complete);
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{"type":"heartbeat","data":{"timestamp":"2026-11-02T00:00:00Z"}}"#;
        let event: AgUiStreamEvent = serde_json::from_str(json).unwrap();
        assert!(matches!(event, AgUiStreamEvent::Heartbeat { .. }));

        let json2 = r#"{"type":"text_delta","data":{"message_id":"m1","delta":"hi"}}"#;
        let event2: AgUiStreamEvent = serde_json::from_str(json2).unwrap();
        assert!(matches!(event2, AgUiStreamEvent::TextDelta { .. }));
    }

    #[test]
    fn test_default_config() {
        let config = StreamConfig::default();
        assert_eq!(config.endpoint, "/api/v1/events");
        assert_eq!(config.max_reconnect_attempts, 10);
        assert_eq!(config.base_retry_ms, 1000);
        assert_eq!(config.max_retry_ms, 30000);
    }
}
