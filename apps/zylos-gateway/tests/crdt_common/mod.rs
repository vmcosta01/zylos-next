//! Harness compartilhado para testes multi-cliente do servidor CRDT (Fase 11.9).
//!
//! Sobe o router `crdt_sync` numa porta aleatória e oferece um cliente Yjs
//! nativo (yrs + tokio-tungstenite) que fala o protocolo y-websocket.
#![allow(dead_code)]

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use yrs::sync::{Awareness, Message, MessageReader, SyncMessage};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::Encode;
use yrs::{ClientID, Doc, GetString, ReadTxn, Text, Transact, Update};
use zylos_auth::crdt_persistence::{CrdtStore, TEXT_ROOT};
use zylos_gateway::crdt_sync::{create_crdt_sync_router, CrdtSyncState, PING_TAG};

/// Servidor de teste em execução.
pub struct TestServer {
    pub addr: SocketAddr,
    pub state: CrdtSyncState,
    handle: tokio::task::JoinHandle<()>,
}

impl TestServer {
    pub async fn start(store: Option<Arc<CrdtStore>>) -> Self {
        let state = CrdtSyncState::new(store);
        let app = axum::Router::new().nest("/api/v1", create_crdt_sync_router(state.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            addr,
            state,
            handle,
        }
    }

    pub fn url(&self, doc: &str) -> String {
        format!("ws://{}/api/v1/crdt/sync/{}", self.addr, doc)
    }

    pub fn http(&self, path: &str) -> String {
        format!("http://{}/api/v1{}", self.addr, path)
    }

    pub fn stop(self) {
        self.handle.abort();
    }
}

/// Cliente Yjs nativo conectado a uma sala.
pub struct Client {
    pub id: u64,
    pub awareness: Arc<Mutex<Awareness>>,
    out: Option<mpsc::UnboundedSender<WsMessage>>,
    /// Notificado a cada update remoto aplicado.
    pub changed: Arc<Notify>,
    /// Instantes de chegada de updates remotos (para benchmark).
    pub arrivals: Arc<Mutex<Vec<Instant>>>,
    pongs: Arc<Mutex<mpsc::UnboundedReceiver<Vec<u8>>>>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

fn decode_all(bytes: &[u8]) -> Vec<Message> {
    let mut d = DecoderV1::from(bytes);
    MessageReader::new(&mut d).filter_map(Result::ok).collect()
}

impl Client {
    /// Cria um cliente desconectado (edição offline).
    pub fn offline(id: u64) -> Self {
        let doc = Doc::with_client_id(id);
        let _ = doc.get_or_insert_text(TEXT_ROOT);
        let (_tx, rx) = mpsc::unbounded_channel();
        Self {
            id,
            awareness: Arc::new(Mutex::new(Awareness::new(doc))),
            out: None,
            changed: Arc::new(Notify::new()),
            arrivals: Arc::new(Mutex::new(Vec::new())),
            pongs: Arc::new(Mutex::new(rx)),
            tasks: Vec::new(),
        }
    }

    pub async fn connect(url: &str, id: u64) -> Self {
        let mut c = Self::offline(id);
        c.attach(url).await;
        c
    }

    /// Conecta (ou reconecta) o cliente e executa o handshake SyncStep1.
    pub async fn attach(&mut self, url: &str) {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
        let (mut sink, mut stream) = ws.split();
        let (out_tx, mut out_rx) = mpsc::unbounded_channel::<WsMessage>();
        let (pong_tx, pong_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.pongs = Arc::new(Mutex::new(pong_rx));
        let writer = tokio::spawn(async move {
            while let Some(m) = out_rx.recv().await {
                if sink.send(m).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        let aw = self.awareness.clone();
        let changed = self.changed.clone();
        let arrivals = self.arrivals.clone();
        let reply = out_tx.clone();
        let reader = tokio::spawn(async move {
            while let Some(Ok(frame)) = stream.next().await {
                let WsMessage::Binary(bytes) = frame else {
                    continue;
                };
                let now = Instant::now();
                for msg in decode_all(&bytes) {
                    match msg {
                        Message::Sync(SyncMessage::SyncStep1(sv)) => {
                            let diff = aw.lock().unwrap().doc().transact().encode_diff_v1(&sv);
                            let m = Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1();
                            let _ = reply.send(WsMessage::Binary(m));
                        }
                        Message::Sync(SyncMessage::SyncStep2(u))
                        | Message::Sync(SyncMessage::Update(u)) => {
                            if let Ok(upd) = Update::decode_v1(&u) {
                                let guard = aw.lock().unwrap();
                                guard.doc().transact_mut().apply_update(upd).unwrap();
                            }
                            arrivals.lock().unwrap().push(now);
                            changed.notify_waiters();
                        }
                        Message::Awareness(upd) => {
                            let _ = aw.lock().unwrap().apply_update(upd);
                            changed.notify_waiters();
                        }
                        Message::Custom(PING_TAG, payload) => {
                            let _ = pong_tx.send(payload);
                        }
                        _ => {}
                    }
                }
            }
        });
        // Handshake: envia o nosso SyncStep1 (servidor devolve o que falta).
        let sv = self.awareness.lock().unwrap().doc().transact().state_vector();
        let _ = out_tx.send(WsMessage::Binary(
            Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1(),
        ));
        // Envia updates feitos offline (o servidor também pedirá via SyncStep1).
        self.out = Some(out_tx);
        self.tasks = vec![writer, reader];
    }

    /// Fecha a conexão (as tarefas encerram).
    pub async fn disconnect(&mut self) {
        if let Some(out) = self.out.take() {
            let _ = out.send(WsMessage::Close(None));
        }
        for t in self.tasks.drain(..) {
            let _ = tokio::time::timeout(Duration::from_secs(2), t).await;
        }
    }

    fn send(&self, data: Vec<u8>) {
        if let Some(out) = &self.out {
            let _ = out.send(WsMessage::Binary(data));
        }
    }

    fn edit_with(&self, f: impl FnOnce(&Doc, &yrs::TextRef, &mut yrs::TransactionMut)) {
        let update = {
            let guard = self.awareness.lock().unwrap();
            let doc = guard.doc();
            let text = doc.get_or_insert_text(TEXT_ROOT);
            let sv = doc.transact().state_vector();
            {
                let mut txn = doc.transact_mut();
                f(doc, &text, &mut txn);
            }
            let diff = doc.transact().encode_diff_v1(&sv);
            diff
        };
        self.send(Message::Sync(SyncMessage::Update(update)).encode_v1());
    }

    /// Insere `s` na posição `idx` (bytes UTF-8) e envia o update.
    pub fn insert(&self, idx: u32, s: &str) {
        let s = s.to_string();
        self.edit_with(move |_, t, txn| {
            let len = t.len(txn);
            t.insert(txn, idx.min(len), &s);
        });
    }

    /// Acrescenta `s` ao final.
    pub fn push(&self, s: &str) {
        let s = s.to_string();
        self.edit_with(move |_, t, txn| t.push(txn, &s));
    }

    /// Remove `len` bytes a partir de `idx`.
    pub fn delete(&self, idx: u32, len: u32) {
        self.edit_with(move |_, t, txn| {
            let cur = t.len(txn);
            if idx < cur {
                t.remove_range(txn, idx, len.min(cur - idx));
            }
        });
    }

    pub fn text(&self) -> String {
        let guard = self.awareness.lock().unwrap();
        let doc = guard.doc();
        let t = doc.get_or_insert_text(TEXT_ROOT);
        let txn = doc.transact();
        t.get_string(&txn)
    }

    /// Publica presença (nome, cursor/seleção) via awareness.
    pub fn set_presence(&self, json: &str) {
        let update = {
            let mut guard = self.awareness.lock().unwrap();
            guard.set_local_state_raw(json.to_string());
            guard
                .update_with_clients([ClientID::new(self.id)])
                .unwrap()
        };
        self.send(Message::Awareness(update).encode_v1());
    }

    /// Estado de presença conhecido de outro cliente.
    pub fn presence_of(&self, other: u64) -> Option<serde_json::Value> {
        self.awareness
            .lock()
            .unwrap()
            .state::<serde_json::Value>(ClientID::new(other))
    }

    /// Envia ping e aguarda o eco; devolve o RTT.
    pub async fn ping(&self, seq: u32) -> Duration {
        let t0 = Instant::now();
        self.send(Message::Custom(PING_TAG, seq.to_le_bytes().to_vec()).encode_v1());
        let pongs = self.pongs.clone();
        loop {
            let got = {
                let mut rx = pongs.lock().unwrap();
                rx.try_recv().ok()
            };
            if let Some(p) = got {
                if p == seq.to_le_bytes() {
                    return t0.elapsed();
                }
            } else {
                tokio::task::yield_now().await;
            }
            assert!(t0.elapsed() < Duration::from_secs(5), "ping sem resposta");
        }
    }
}

/// Aguarda até `pred` ser verdadeiro ou estourar o timeout.
pub async fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pred() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    pred()
}

/// Aguarda todos os clientes convergirem para o mesmo texto.
pub async fn wait_converged(clients: &[Client], timeout: Duration) -> bool {
    wait_until(timeout, || {
        let first = clients[0].text();
        clients.iter().all(|c| c.text() == first)
    })
    .await
}
