//! WebRTC Relay — WebSocket endpoint para streaming de tela e controle remoto.
//!
//! ## Endpoint
//! - `GET /ws/computer-view/:session_id` — upgrade para WebSocket
//!
//! ## Protocolo (JSON frames)
//! ### Server → Client
//! - `{"type":"frame","data":"<base64_png>","ts":1234567890,"seq":42}`
//! - `{"type":"quality","fps":15.2,"latency_ms":85,"bitrate_kbps":1200}`
//! - `{"type":"error","message":"..."}`
//!
//! ### Client → Server
//! - `{"type":"click","x":100,"y":200,"button":"left"}`
//! - `{"type":"touch","x":100,"y":200,"action":"start"}`
//! - `{"type":"key","code":"Enter","pressed":true}`
//! - `{"type":"ping","ts":1234567890}`
//!
//! ## Related Documents
//! - Playbook: `playbooks/pb-webrtc-setup.md`
//! - POP: `playbooks/pop-relay-emergency-stop.md`
//! - Runbook: `runbooks/rb-webrtc-relay.md`
//! - SOAR: `docs/soar/soar-relay-latency-alert.md`

use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Mensagem do servidor para o cliente (relay frame ou métrica).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RelayServerMsg {
    #[serde(rename = "frame")]
    Frame { data: String, ts: u64, seq: u64 },
    #[serde(rename = "quality")]
    Quality {
        fps: f64,
        latency_ms: u64,
        bitrate_kbps: u64,
    },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Mensagem do cliente para o servidor (controle ou ping).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RelayClientMsg {
    #[serde(rename = "click")]
    Click { x: i32, y: i32, button: String },
    #[serde(rename = "touch")]
    Touch { x: i32, y: i32, action: String },
    #[serde(rename = "key")]
    Key { code: String, pressed: bool },
    #[serde(rename = "ping")]
    Ping { ts: u64 },
}

/// Estado compartilhado do relay: broadcast channels por sessão.
#[derive(Clone)]
pub struct WebRtcRelayState {
    /// Canal de broadcast por session_id para frames de tela.
    pub sessions: Arc<DashMap<String, broadcast::Sender<RelayServerMsg>>>,
}

impl WebRtcRelayState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Obtém ou cria um canal de broadcast para a sessão.
    pub fn get_or_create_channel(&self, session_id: &str) -> broadcast::Sender<RelayServerMsg> {
        self.sessions
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let (tx, _) = broadcast::channel(64);
                info!(session_id = %session_id, "WebRTC relay channel created");
                tx
            })
            .value()
            .clone()
    }

    /// Remove canal quando não há mais consumidores.
    pub fn cleanup_session(&self, session_id: &str) {
        if let Some((_, tx)) = self.sessions.remove(session_id) {
            if tx.receiver_count() == 0 {
                info!(session_id = %session_id, "WebRTC relay channel cleaned up");
            } else {
                // Re-inserir se ainda há consumidores
                self.sessions.insert(session_id.to_string(), tx);
            }
        }
    }
}

impl Default for WebRtcRelayState {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl Send for WebRtcRelayState {}
unsafe impl Sync for WebRtcRelayState {}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Handler de upgrade WebSocket para relay de computer view.
async fn ws_computer_view(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<WebRtcRelayState>,
) -> impl IntoResponse {
    info!(session_id = %session_id, "WebSocket upgrade requested for computer-view relay");
    let tx = state.get_or_create_channel(&session_id);
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();

    ws.on_upgrade(move |socket| handle_ws_socket(socket, session_id_clone, tx, state_clone))
}

/// Gerencia conexão WebSocket individual: lê comandos do cliente e envia frames.
async fn handle_ws_socket(
    socket: axum::extract::ws::WebSocket,
    session_id: String,
    tx: broadcast::Sender<RelayServerMsg>,
    state: WebRtcRelayState,
) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    let (mut sink, mut stream) = socket.split();
    let mut rx = tx.subscribe();

    // Clones para cada task + cleanup pós-select
    let session_id_send = session_id.clone();
    let session_id_cleanup = session_id.clone();
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let json = match serde_json::to_string(&msg) {
                        Ok(j) => j,
                        Err(e) => {
                            warn!(error = %e, "Failed to serialize relay message");
                            continue;
                        }
                    };
                    if sink.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(lagged = n, session_id = %session_id_send, "Client lagged, skipping frames");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Task para ler mensagens do cliente (controle/ping)
    let session_id_for_recv = session_id.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<RelayClientMsg>(&text) {
                        match client_msg {
                            RelayClientMsg::Click { x, y, button } => {
                                info!(session_id = %session_id_for_recv, x, y, button = %button, "Remote click received");
                                // TODO: Encaminhar para o agente via computer-view control API
                            }
                            RelayClientMsg::Touch { x, y, action } => {
                                info!(session_id = %session_id_for_recv, x, y, action = %action, "Remote touch received");
                            }
                            RelayClientMsg::Key { code, pressed } => {
                                info!(session_id = %session_id_for_recv, code = %code, pressed, "Remote key received");
                            }
                            RelayClientMsg::Ping { ts } => {
                                let latency = now_millis().saturating_sub(ts);
                                let quality = RelayServerMsg::Quality {
                                    fps: 0.0,
                                    latency_ms: latency,
                                    bitrate_kbps: 0,
                                };
                                let _ = tx.send(quality);
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    state.cleanup_session(&session_id_cleanup);
    info!(session_id = %session_id_cleanup, "WebSocket relay connection closed");
}

/// Cria o router de WebRTC relay.
pub fn create_webrtc_relay_router(state: WebRtcRelayState) -> Router {
    Router::new()
        .route("/ws/computer-view/{session_id}", get(ws_computer_view))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_state_new() {
        let state = WebRtcRelayState::new();
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_get_or_create_channel() {
        let state = WebRtcRelayState::new();
        let _tx = state.get_or_create_channel("test-session");
        assert_eq!(state.sessions.len(), 1);
        let _tx2 = state.get_or_create_channel("test-session");
        assert_eq!(state.sessions.len(), 1);
    }

    #[test]
    fn test_cleanup_session() {
        let state = WebRtcRelayState::new();
        let _tx = state.get_or_create_channel("cleanup-test");
        assert_eq!(state.sessions.len(), 1);
        state.cleanup_session("cleanup-test");
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn test_server_msg_serialize() {
        let msg = RelayServerMsg::Frame {
            data: "abc123".to_string(),
            ts: 1000,
            seq: 1,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"frame\""));
    }

    #[test]
    fn test_client_msg_deserialize() {
        let json = r#"{"type":"click","x":100,"y":200,"button":"left"}"#;
        let msg: RelayClientMsg = serde_json::from_str(json).unwrap();
        match msg {
            RelayClientMsg::Click { x, y, button } => {
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(button, "left");
            }
            _ => panic!("Expected Click variant"),
        }
    }

    #[test]
    fn test_quality_msg_serialize() {
        let msg = RelayServerMsg::Quality {
            fps: 15.5,
            latency_ms: 85,
            bitrate_kbps: 1200,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"quality\""));
        assert!(json.contains("\"fps\":15.5"));
    }
}
