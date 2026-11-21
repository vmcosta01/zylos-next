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

/// Limite padrão de sessões WS relay simultâneas (F15).
const DEFAULT_MAX_RELAY_SESSIONS: usize = 64;

/// Estado compartilhado do relay: broadcast channels por sessão.
#[derive(Clone)]
pub struct WebRtcRelayState {
    /// Canal de broadcast por session_id para frames de tela.
    pub sessions: Arc<DashMap<String, broadcast::Sender<RelayServerMsg>>>,
    /// F15: limite máximo de sessões simultâneas.
    pub max_sessions: usize,
    /// Entrada remota (cliques e teclas). `None` = desligada: o relay só repassa imagem.
    pub entrada: Option<Arc<crate::remote_input::Entrada>>,
}

impl WebRtcRelayState {
    pub fn new() -> Self {
        let max_sessions = std::env::var("ZYLOS_MAX_RELAY_SESSIONS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(DEFAULT_MAX_RELAY_SESSIONS);
        Self {
            sessions: Arc::new(DashMap::new()),
            max_sessions,
            entrada: None,
        }
    }

    /// Liga a entrada remota (já com as travas de `remote_input`).
    pub fn with_entrada(mut self, entrada: Arc<crate::remote_input::Entrada>) -> Self {
        self.entrada = Some(entrada);
        self
    }

    /// Obtém ou cria um canal de broadcast para a sessão.
    /// F15: retorna `None` se o limite de sessões simultâneas for atingido.
    pub fn get_or_create_channel(
        &self,
        session_id: &str,
    ) -> Option<broadcast::Sender<RelayServerMsg>> {
        // Se já existe, retorna sem checar limite (reconexão na mesma sessão).
        if let Some(tx) = self.sessions.get(session_id) {
            return Some(tx.value().clone());
        }
        // Limite de sessões simultâneas.
        if self.sessions.len() >= self.max_sessions {
            warn!(
                session_id = %session_id,
                max = self.max_sessions,
                "F15: relay session limit reached, rejecting new connection"
            );
            return None;
        }
        Some(
            self.sessions
                .entry(session_id.to_string())
                .or_insert_with(|| {
                    let (tx, _) = broadcast::channel(64);
                    info!(session_id = %session_id, "WebRTC relay channel created");
                    tx
                })
                .value()
                .clone(),
        )
    }

    /// Remove canal quando não há mais consumidores.
    /// Publica um frame de imagem (PNG/JPEG já validado) para quem assiste a sessão.
    /// Devolve quantos espectadores estavam ligados; `None` se o limite de sessões do relay
    /// foi atingido. Sem espectador o frame é descartado (o relay não guarda histórico).
    pub fn publish_frame(&self, session_id: &str, image: &[u8]) -> Option<usize> {
        use base64::Engine as _;
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tx = self.get_or_create_channel(session_id)?;
        let msg = RelayServerMsg::Frame {
            data: base64::engine::general_purpose::STANDARD.encode(image),
            ts: now_millis(),
            seq: SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        Some(tx.send(msg).unwrap_or(0))
    }

    /// Quantos espectadores (WebSockets) estão ligados à sessão agora.
    pub fn viewers(&self, session_id: &str) -> usize {
        self.sessions
            .get(session_id)
            .map(|tx| tx.value().receiver_count())
            .unwrap_or(0)
    }

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
    auth: Option<axum::Extension<zylos_auth::middleware::AuthContext>>,
) -> impl IntoResponse {
    // Quem é, segundo o JWT que o gateway JÁ verificou (nunca um campo mandado pelo cliente).
    // Sem login (gateway sem segredo, ou sem token) não há usuário: a entrada fica recusada.
    let usuario: Option<String> = auth.map(|a| a.0.claims.sub.clone());
    info!(session_id = %session_id, "WebSocket upgrade requested for computer-view relay");
    let Some(tx) = state.get_or_create_channel(&session_id) else {
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "relay session limit reached",
        )
            .into_response();
    };
    let session_id_clone = session_id.clone();
    let state_clone = state.clone();

    ws.on_upgrade(move |socket| {
        handle_ws_socket(socket, session_id_clone, tx, state_clone, usuario)
    })
    .into_response()
}

/// Evita repetir o mesmo aviso a cada evento (uma sequência de teclas recusadas gera um aviso só).
#[derive(Default)]
struct Avisos {
    ultimo: Option<String>,
}

impl Avisos {
    /// O aviso vai só para a conexão que mandou o evento (não para os outros espectadores).
    fn avisar(&mut self, tx: &tokio::sync::mpsc::Sender<RelayServerMsg>, texto: String) {
        if self.ultimo.as_deref() != Some(texto.as_str()) {
            let _ = tx.try_send(RelayServerMsg::Error {
                message: texto.clone(),
            });
            self.ultimo = Some(texto);
        }
    }

    /// Um evento aceito: o próximo problema volta a ser avisado.
    fn limpar(&mut self) {
        self.ultimo = None;
    }
}

/// Gerencia conexão WebSocket individual: lê comandos do cliente e envia frames.
async fn handle_ws_socket(
    socket: axum::extract::ws::WebSocket,
    session_id: String,
    tx: broadcast::Sender<RelayServerMsg>,
    state: WebRtcRelayState,
    usuario: Option<String>,
) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    let (mut sink, mut stream) = socket.split();
    let mut rx = tx.subscribe();
    // Canal só desta conexão: avisos de recusa não vão para os outros espectadores.
    let (aviso_tx, mut aviso_rx) = tokio::sync::mpsc::channel::<RelayServerMsg>(16);

    // Clones para cada task + cleanup pós-select
    let session_id_send = session_id.clone();
    let session_id_cleanup = session_id.clone();
    let send_task = tokio::spawn(async move {
        loop {
            let recebida = tokio::select! {
                r = rx.recv() => r,
                Some(m) = aviso_rx.recv() => Ok(m),
            };
            match recebida {
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
    let entrada = state.entrada.clone();
    let usuario_recv = usuario;
    let recv_task = tokio::spawn(async move {
        let mut limite = crate::remote_input::LimiteDeTaxa::new();
        let mut avisos = Avisos::default();
        // Teclas que ESTA conexão apertou e ainda não soltou: soltas à força quando ela acaba.
        let mut apertadas = std::collections::HashSet::<String>::new();
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<RelayClientMsg>(&text) {
                        match client_msg {
                            RelayClientMsg::Click { x, y, button } => {
                                info!(session_id = %session_id_for_recv, x, y, button = %button, "Remote click received");
                                if !limite.permite() {
                                    avisos.avisar(&aviso_tx, "Eventos demais por segundo".into());
                                } else if let Some(e) = &entrada {
                                    match e.clique(
                                        &session_id_for_recv,
                                        usuario_recv.as_deref(),
                                        x,
                                        y,
                                        &button,
                                    ) {
                                        Ok(()) => avisos.limpar(),
                                        Err(r) => avisos.avisar(&aviso_tx, r.texto()),
                                    }
                                } else {
                                    avisos.avisar(
                                        &aviso_tx,
                                        "Entrada remota desligada neste gateway".into(),
                                    );
                                }
                            }
                            RelayClientMsg::Touch { x, y, action } => {
                                info!(session_id = %session_id_for_recv, x, y, action = %action, "Remote touch received");
                            }
                            RelayClientMsg::Key { code, pressed } => {
                                // O código da tecla NÃO vai para o log: seria senha digitada em texto puro.
                                info!(session_id = %session_id_for_recv, pressed, "Remote key received");
                                if let (false, Some(e)) = (pressed, &entrada) {
                                    // Soltar uma tecla que esta conexão apertou nunca é recusado
                                    // (nem pelo limite de taxa): senão ela fica presa no sistema.
                                    if apertadas.remove(&code) {
                                        let _ = e.soltar(&code);
                                        continue;
                                    }
                                }
                                if !limite.permite() {
                                    avisos.avisar(&aviso_tx, "Eventos demais por segundo".into());
                                } else if let Some(e) = &entrada {
                                    match e.tecla(
                                        &session_id_for_recv,
                                        usuario_recv.as_deref(),
                                        &code,
                                        pressed,
                                    ) {
                                        Ok(()) => {
                                            avisos.limpar();
                                            if pressed {
                                                apertadas.insert(code);
                                            }
                                        }
                                        Err(r) => avisos.avisar(&aviso_tx, r.texto()),
                                    }
                                } else {
                                    avisos.avisar(
                                        &aviso_tx,
                                        "Entrada remota desligada neste gateway".into(),
                                    );
                                }
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
        // Conexão acabou: solta qualquer tecla que ficou apertada.
        if let Some(e) = &entrada {
            for tecla in &apertadas {
                let _ = e.soltar(tecla);
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
        .route("/ws/computer-view/:session_id", get(ws_computer_view))
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
        assert!(state.get_or_create_channel("test-session").is_some());
        assert_eq!(state.sessions.len(), 1);
        // Same session returns existing channel without hitting limit.
        assert!(state.get_or_create_channel("test-session").is_some());
        assert_eq!(state.sessions.len(), 1);
    }

    #[test]
    fn test_cleanup_session() {
        let state = WebRtcRelayState::new();
        assert!(state.get_or_create_channel("cleanup-test").is_some());
        assert_eq!(state.sessions.len(), 1);
        state.cleanup_session("cleanup-test");
        assert!(state.sessions.is_empty());
    }

    #[test]
    fn f15_relay_session_cap_enforced() {
        let mut state = WebRtcRelayState::new();
        state.max_sessions = 2;
        assert!(state.get_or_create_channel("s1").is_some());
        assert!(state.get_or_create_channel("s2").is_some());
        // Third distinct session must be rejected.
        assert!(state.get_or_create_channel("s3").is_none());
        // Reconnecting to an existing session still works.
        assert!(state.get_or_create_channel("s1").is_some());
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
