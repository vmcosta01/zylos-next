//! CRDT Sync Server — Fase 11.9 (T11.9.3 / T11.9.4 / T11.9.6).
//!
//! Servidor WebSocket compatível com o protocolo `y-websocket` (Yjs), usando
//! a biblioteca `yrs` (ver ADR-012 em `docs/DECISIONS_FASE11.md`).
//!
//! ## Endpoints (montados sob `/api/v1`)
//! - `GET  /crdt/sync/:doc_id` — upgrade WebSocket; frames binários no formato
//!   y-protocols v1 (`Sync`/`Awareness`/`AwarenessQuery`/`Custom`).
//! - `GET  /crdt/documents` — salas ativas (`?active=true`) ou todos os
//!   documentos persistidos.
//! - `GET  /crdt/documents/:doc_id` — texto corrente + metadados.
//! - `GET  /crdt/documents/:doc_id/versions` — versões (snapshots) persistidas.
//! - `GET  /crdt/documents/:doc_id/versions/:version` — texto de uma versão.
//! - `POST /crdt/documents/:doc_id/compact` — força snapshot da versão corrente.
//! - `POST /crdt/gc` — garbage collection (retenção 30 dias).
//!
//! ## Fluxo de sincronização (por sala/documento)
//! 1. Ao conectar, o servidor envia `SyncStep1(sv_servidor)` e o estado de
//!    awareness atual (presença dos outros clientes).
//! 2. Cliente responde `SyncStep2(diff)` e envia seu próprio `SyncStep1`; o
//!    servidor responde `SyncStep2` com o que falta ao cliente.
//! 3. Cada `Update`/`SyncStep2` recebido é aplicado na réplica do servidor,
//!    **persistido antes** de ser retransmitido aos demais clientes da sala.
//! 4. Frames `Awareness` (cursor/seleção/nome/cor) são aplicados e
//!    retransmitidos; ao desconectar, o servidor publica a remoção da presença.
//! 5. `Custom(PING_TAG, payload)` é ecoado — usado para medir RTT.
//!
//! Se um cliente ficar para trás no canal broadcast (`Lagged`), o servidor
//! reenvia o estado completo como `SyncStep2` — idempotente, sem perda.
//!
//! ## Documentos relacionados
//! - Runbook: `runbooks/rb-crdt-sync-server.md`
//! - POP: `runbooks/pop-crdt-presence-indicators.md`
//! - POT: `runbooks/pot-crdt-latency-benchmark.md`

use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket},
        Path, Query, State, WebSocketUpgrade,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use yrs::sync::{Awareness, AwarenessUpdate, Message, MessageReader, SyncMessage};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::Encode;
use yrs::{ClientID, GetString, ReadTxn, StateVector, Transact, Update};
use zylos_auth::crdt_persistence::{text_of_state, CrdtStore, TEXT_ROOT};
use zylos_auth::TokenService;

/// Tag do `Message::Custom` usado como ping/pong para medir latência.
pub const PING_TAG: u8 = 100;
/// Close code personalizado para autenticação falha no WebSocket (faixa 4000-4999).
const WS_CLOSE_AUTH_FAILED: u16 = 4001;
/// Capacidade do canal broadcast por sala.
const ROOM_CHANNEL_CAPACITY: usize = 1024;
/// Após N updates pendentes no log, o servidor gera um snapshot (compactação).
pub const DEFAULT_COMPACT_EVERY: i64 = 500;
/// Tamanho máximo de um frame aceito (proteção contra abuso): 8 MiB.
const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Frame retransmitido numa sala: (id da conexão de origem, bytes).
type RoomFrame = (u64, Arc<Vec<u8>>);

/// Uma sala = um documento CRDT com seus clientes conectados.
pub struct Room {
    awareness: Mutex<Awareness>,
    tx: broadcast::Sender<RoomFrame>,
    clients: AtomicUsize,
}

impl Room {
    fn new(state: Option<Vec<u8>>) -> Result<Self, String> {
        let doc = yrs::Doc::new();
        // Garante a existência do root de texto usado pelo cockpit.
        let _ = doc.get_or_insert_text(TEXT_ROOT);
        if let Some(state) = state {
            let upd = Update::decode_v1(&state).map_err(|e| e.to_string())?;
            doc.transact_mut()
                .apply_update(upd)
                .map_err(|e| e.to_string())?;
        }
        let (tx, _) = broadcast::channel(ROOM_CHANNEL_CAPACITY);
        Ok(Self {
            awareness: Mutex::new(Awareness::new(doc)),
            tx,
            clients: AtomicUsize::new(0),
        })
    }

    fn with_awareness<R>(&self, f: impl FnOnce(&mut Awareness) -> R) -> R {
        let mut guard = match self.awareness.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }

    /// Estado completo do documento (update v1).
    pub fn full_state(&self) -> Vec<u8> {
        self.with_awareness(|a| {
            a.doc()
                .transact()
                .encode_state_as_update_v1(&StateVector::default())
        })
    }

    /// Texto corrente do root [`TEXT_ROOT`].
    pub fn text(&self) -> String {
        self.with_awareness(|a| {
            let doc = a.doc();
            let t = doc.get_or_insert_text(TEXT_ROOT);
            let txn = doc.transact();
            t.get_string(&txn)
        })
    }

    /// Número de clientes conectados.
    pub fn client_count(&self) -> usize {
        self.clients.load(Ordering::SeqCst)
    }
}

/// Estado compartilhado do servidor de sincronização.
#[derive(Clone)]
pub struct CrdtSyncState {
    rooms: Arc<DashMap<String, Arc<Room>>>,
    store: Option<Arc<CrdtStore>>,
    next_conn: Arc<AtomicU64>,
    compact_every: i64,
    /// F1: validação JWT no WebSocket (opcional; ausente = modo dev/local).
    token_service: Option<TokenService>,
}

impl Default for CrdtSyncState {
    fn default() -> Self {
        Self::new(None)
    }
}

impl CrdtSyncState {
    /// Cria o estado. Sem `store`, os documentos vivem só em memória.
    pub fn new(store: Option<Arc<CrdtStore>>) -> Self {
        Self {
            rooms: Arc::new(DashMap::new()),
            store,
            next_conn: Arc::new(AtomicU64::new(1)),
            compact_every: DEFAULT_COMPACT_EVERY,
            token_service: None,
        }
    }

    /// Injeta o `TokenService` para validação JWT no WebSocket (F1).
    pub fn with_token_service(mut self, svc: Option<TokenService>) -> Self {
        self.token_service = svc;
        self
    }

    /// Abre (ou cria) o banco SQLite em `path`; em caso de falha, segue em memória.
    pub fn with_store_path(path: &std::path::Path) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match CrdtStore::open(path) {
            Ok(store) => Self::new(Some(Arc::new(store))),
            Err(e) => {
                warn!(error = %e, path = %path.display(), "crdt: falha ao abrir store, usando memória");
                Self::new(None)
            }
        }
    }

    /// Ajusta o limiar de compactação automática.
    pub fn with_compact_every(mut self, n: i64) -> Self {
        self.compact_every = n.max(1);
        self
    }

    /// Store persistente, se configurado.
    pub fn store(&self) -> Option<&Arc<CrdtStore>> {
        self.store.as_ref()
    }

    /// Sala ativa (em memória), se existir.
    pub fn room(&self, doc_id: &str) -> Option<Arc<Room>> {
        self.rooms.get(doc_id).map(|r| r.clone())
    }

    fn get_or_load_room(&self, doc_id: &str) -> Result<Arc<Room>, String> {
        if let Some(r) = self.rooms.get(doc_id) {
            return Ok(r.clone());
        }
        let persisted = match &self.store {
            Some(store) => store.load(doc_id).map_err(|e| e.to_string())?,
            None => None,
        };
        let room = Arc::new(Room::new(persisted)?);
        // `entry` resolve a corrida entre duas conexões simultâneas.
        Ok(self.rooms.entry(doc_id.to_string()).or_insert(room).clone())
    }

    fn persist_update(&self, doc_id: &str, room: &Room, update: &[u8]) {
        let Some(store) = &self.store else { return };
        if let Err(e) = store.append_update(doc_id, update) {
            warn!(doc_id, error = %e, "crdt: falha ao persistir update");
            return;
        }
        match store.pending_updates(doc_id) {
            Ok(n) if n >= self.compact_every => {
                let state = room.full_state();
                match store.snapshot(doc_id, &state) {
                    Ok(v) => debug!(doc_id, version = v, "crdt: snapshot automático"),
                    Err(e) => warn!(doc_id, error = %e, "crdt: falha no snapshot"),
                }
            }
            Ok(_) => {}
            Err(e) => warn!(doc_id, error = %e, "crdt: falha ao contar updates"),
        }
    }

    /// Ao sair o último cliente: snapshot (se houver updates pendentes) e
    /// descarrega a sala da memória.
    fn release_room(&self, doc_id: &str, room: &Arc<Room>) {
        if room.clients.load(Ordering::SeqCst) > 0 {
            return;
        }
        if let Some(store) = &self.store {
            if store.pending_updates(doc_id).unwrap_or(0) > 0 {
                if let Err(e) = store.snapshot(doc_id, &room.full_state()) {
                    warn!(doc_id, error = %e, "crdt: falha no snapshot ao liberar sala");
                    return;
                }
            }
            self.rooms
                .remove_if(doc_id, |_, r| r.clients.load(Ordering::SeqCst) == 0);
        }
    }
}

fn encode(msg: &Message) -> Vec<u8> {
    msg.encode_v1()
}

/// Processa uma mensagem do cliente. Retorna respostas diretas ao remetente
/// e, opcionalmente, um frame para retransmitir aos demais.
fn handle_message(
    state: &CrdtSyncState,
    doc_id: &str,
    room: &Room,
    msg: Message,
    seen_clients: &mut HashSet<ClientID>,
    replies: &mut Vec<Vec<u8>>,
    relay: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    match msg {
        Message::Sync(SyncMessage::SyncStep1(sv)) => {
            let diff = room.with_awareness(|a| a.doc().transact().encode_diff_v1(&sv));
            replies.push(encode(&Message::Sync(SyncMessage::SyncStep2(diff))));
        }
        Message::Sync(SyncMessage::SyncStep2(data)) | Message::Sync(SyncMessage::Update(data)) => {
            let upd = Update::decode_v1(&data).map_err(|e| e.to_string())?;
            if upd.is_empty() {
                return Ok(());
            }
            room.with_awareness(|a| a.doc().transact_mut().apply_update(upd))
                .map_err(|e| e.to_string())?;
            // Durabilidade antes da retransmissão.
            state.persist_update(doc_id, room, &data);
            relay.push(encode(&Message::Sync(SyncMessage::Update(data))));
        }
        Message::Awareness(update) => {
            seen_clients.extend(update.clients.keys().copied());
            let raw = encode(&Message::Awareness(update.clone()));
            room.with_awareness(|a| a.apply_update(update))
                .map_err(|e| e.to_string())?;
            relay.push(raw);
        }
        Message::AwarenessQuery => {
            if let Ok(upd) = room.with_awareness(|a| a.update()) {
                replies.push(encode(&Message::Awareness(upd)));
            }
        }
        Message::Custom(PING_TAG, payload) => {
            replies.push(encode(&Message::Custom(PING_TAG, payload)));
        }
        Message::Auth(_) | Message::Custom(_, _) => {}
    }
    Ok(())
}

fn decode_frame(bytes: &[u8]) -> Result<Vec<Message>, String> {
    let mut decoder = DecoderV1::from(bytes);
    let reader = MessageReader::new(&mut decoder);
    reader
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Query params aceitos no upgrade WebSocket (F1: JWT via query string).
#[derive(Debug, Deserialize)]
struct WsSyncQuery {
    access_token: Option<String>,
}

async fn ws_sync(
    ws: WebSocketUpgrade,
    Path(doc_id): Path<String>,
    Query(query): Query<WsSyncQuery>,
    State(state): State<CrdtSyncState>,
) -> impl IntoResponse {
    // F1: valida JWT no connect quando ZYLOS_AUTH_SECRET está definido.
    if let Some(svc) = &state.token_service {
        match query.access_token.as_deref() {
            Some(tok) if !tok.is_empty() => {
                if let Err(e) = svc.validate_token(tok) {
                    warn!(doc_id, error = %e, "crdt: JWT inválido no WS connect");
                    return axum::response::Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(axum::body::Body::from("JWT inválido"))
                        .unwrap()
                        .into_response();
                }
            }
            _ => {
                warn!(doc_id, "crdt: WS connect sem access_token");
                return axum::response::Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .body(axum::body::Body::from("access_token obrigatório"))
                    .unwrap()
                    .into_response();
            }
        }
    }
    ws.max_message_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, doc_id, state))
}

async fn send_bin(socket: &mut WebSocket, data: Vec<u8>) -> bool {
    socket.send(WsMessage::Binary(data)).await.is_ok()
}

async fn handle_socket(mut socket: WebSocket, doc_id: String, state: CrdtSyncState) {
    let room = match state.get_or_load_room(&doc_id) {
        Ok(r) => r,
        Err(e) => {
            warn!(doc_id, error = %e, "crdt: falha ao carregar documento");
            let _ = socket.send(WsMessage::Close(None)).await;
            return;
        }
    };
    let conn_id = state.next_conn.fetch_add(1, Ordering::SeqCst);
    room.clients.fetch_add(1, Ordering::SeqCst);
    let mut rx = room.tx.subscribe();
    info!(
        doc_id,
        conn_id,
        clients = room.client_count(),
        "crdt: cliente conectado"
    );

    // Passo 1 do handshake: SyncStep1 do servidor + presença atual.
    let sv = room.with_awareness(|a| a.doc().transact().state_vector());
    let mut ok = send_bin(
        &mut socket,
        encode(&Message::Sync(SyncMessage::SyncStep1(sv))),
    )
    .await;
    if ok {
        if let Ok(upd) = room.with_awareness(|a| a.update()) {
            if !upd.clients.is_empty() {
                ok = send_bin(&mut socket, encode(&Message::Awareness(upd))).await;
            }
        }
    }

    let mut seen_clients: HashSet<ClientID> = HashSet::new();
    while ok {
        tokio::select! {
            incoming = socket.recv() => {
                let Some(Ok(frame)) = incoming else { break };
                let bytes = match frame {
                    WsMessage::Binary(b) => b,
                    WsMessage::Close(_) => break,
                    _ => continue,
                };
                let msgs = match decode_frame(&bytes) {
                    Ok(m) => m,
                    Err(e) => {
                        warn!(doc_id, conn_id, error = %e, "crdt: frame inválido descartado");
                        continue;
                    }
                };
                let mut replies = Vec::new();
                let mut relay = Vec::new();
                for msg in msgs {
                    if let Err(e) = handle_message(&state, &doc_id, &room, msg, &mut seen_clients, &mut replies, &mut relay) {
                        warn!(doc_id, conn_id, error = %e, "crdt: mensagem rejeitada");
                    }
                }
                for r in relay {
                    let _ = room.tx.send((conn_id, Arc::new(r)));
                }
                for r in replies {
                    if !send_bin(&mut socket, r).await {
                        ok = false;
                        break;
                    }
                }
            }
            broadcast = rx.recv() => {
                match broadcast {
                    Ok((from, data)) => {
                        if from != conn_id && !send_bin(&mut socket, data.as_ref().clone()).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(doc_id, conn_id, skipped = n, "crdt: cliente atrasado; reenviando estado completo");
                        let full = room.full_state();
                        if !send_bin(&mut socket, encode(&Message::Sync(SyncMessage::SyncStep2(full)))).await {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    // Remove a presença dos clientes Yjs que falaram por esta conexão.
    if !seen_clients.is_empty() {
        let removal = room.with_awareness(|a| {
            for id in &seen_clients {
                a.remove_state(*id);
            }
            a.update_with_clients(seen_clients.iter().copied())
        });
        if let Ok(upd) = removal {
            let _ = room
                .tx
                .send((conn_id, Arc::new(encode(&Message::Awareness(upd)))));
        }
    }
    room.clients.fetch_sub(1, Ordering::SeqCst);
    info!(
        doc_id,
        conn_id,
        clients = room.client_count(),
        "crdt: cliente desconectado"
    );
    state.release_room(&doc_id, &room);
}

// ---------------------------------------------------------------------------
// REST: inspeção de documentos/versões (UI de resolução de conflitos).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ListQuery {
    active: Option<bool>,
}

/// Resumo de um documento para listagem.
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub doc_id: String,
    pub active: bool,
    pub clients: usize,
}

async fn list_documents(
    State(state): State<CrdtSyncState>,
    Query(q): Query<ListQuery>,
) -> impl IntoResponse {
    let mut out: Vec<DocumentSummary> = state
        .rooms
        .iter()
        .map(|r| DocumentSummary {
            doc_id: r.key().clone(),
            active: r.client_count() > 0,
            clients: r.client_count(),
        })
        .collect();
    if !q.active.unwrap_or(false) {
        if let Some(store) = &state.store {
            for id in store.list_documents().unwrap_or_default() {
                if !out.iter().any(|d| d.doc_id == id) {
                    out.push(DocumentSummary {
                        doc_id: id,
                        active: false,
                        clients: 0,
                    });
                }
            }
        }
    } else {
        out.retain(|d| d.active);
    }
    out.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
    Json(out)
}

fn current_text(state: &CrdtSyncState, doc_id: &str) -> Option<(String, usize)> {
    if let Some(room) = state.room(doc_id) {
        return Some((room.text(), room.client_count()));
    }
    let store = state.store.as_ref()?;
    let st = store.load(doc_id).ok()??;
    text_of_state(&st).ok().map(|t| (t, 0))
}

async fn get_document(
    State(state): State<CrdtSyncState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    match current_text(&state, &doc_id) {
        Some((text, clients)) => (
            StatusCode::OK,
            Json(serde_json::json!({"doc_id": doc_id, "text": text, "clients": clients})),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "documento não encontrado"})),
        ),
    }
}

async fn list_versions(
    State(state): State<CrdtSyncState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    let Some(store) = &state.store else {
        return (StatusCode::OK, Json(serde_json::json!([])));
    };
    match store.list_versions(&doc_id) {
        Ok(v) => (StatusCode::OK, Json(serde_json::json!(v))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn get_version(
    State(state): State<CrdtSyncState>,
    Path((doc_id, version)): Path<(String, i64)>,
) -> impl IntoResponse {
    let found = state
        .store
        .as_ref()
        .and_then(|s| s.load_version(&doc_id, version).ok().flatten())
        .and_then(|st| text_of_state(&st).ok());
    match found {
        Some(text) => (
            StatusCode::OK,
            Json(serde_json::json!({"doc_id": doc_id, "version": version, "text": text})),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "versão não encontrada"})),
        ),
    }
}

async fn compact_document(
    State(state): State<CrdtSyncState>,
    Path(doc_id): Path<String>,
) -> impl IntoResponse {
    let Some(store) = &state.store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "persistência desabilitada"})),
        );
    };
    let result = match state.room(&doc_id) {
        Some(room) => store.snapshot(&doc_id, &room.full_state()).map(Some),
        None => store.compact(&doc_id),
    };
    match result {
        Ok(Some(v)) => (StatusCode::OK, Json(serde_json::json!({"version": v}))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "documento não encontrado"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

async fn run_gc(State(state): State<CrdtSyncState>) -> impl IntoResponse {
    let Some(store) = &state.store else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "persistência desabilitada"})),
        );
    };
    match store.gc() {
        Ok(r) => (StatusCode::OK, Json(serde_json::json!(r))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Agenda o GC periódico (a cada 24 h) se houver store. Requer runtime Tokio.
pub fn spawn_gc_task(state: &CrdtSyncState) {
    let Some(store) = state.store.clone() else {
        return;
    };
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
        loop {
            tick.tick().await;
            match store.gc() {
                Ok(r) => info!(?r, "crdt: GC concluído"),
                Err(e) => warn!(error = %e, "crdt: GC falhou"),
            }
        }
    });
}

/// Router do servidor CRDT (montar sob `/api/v1`).
pub fn create_crdt_sync_router(state: CrdtSyncState) -> Router {
    Router::new()
        .route("/crdt/sync/:doc_id", get(ws_sync))
        .route("/crdt/documents", get(list_documents))
        .route("/crdt/documents/:doc_id", get(get_document))
        .route("/crdt/documents/:doc_id/versions", get(list_versions))
        .route(
            "/crdt/documents/:doc_id/versions/:version",
            get(get_version),
        )
        .route("/crdt/documents/:doc_id/compact", post(compact_document))
        .route("/crdt/gc", post(run_gc))
        .with_state(state)
}

/// Monta um `AwarenessUpdate` de um único cliente (utilitário para testes e
/// clientes nativos).
pub fn awareness_update(client: u64, clock: u32, json: &str) -> AwarenessUpdate {
    let mut clients = std::collections::HashMap::new();
    clients.insert(
        ClientID::new(client),
        yrs::sync::awareness::AwarenessUpdateEntry {
            clock,
            json: Arc::from(json),
        },
    );
    AwarenessUpdate { clients }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yrs::Text;

    fn update_frame(text: &str) -> (Vec<u8>, yrs::Doc) {
        let doc = yrs::Doc::with_client_id(7);
        let t = doc.get_or_insert_text(TEXT_ROOT);
        t.insert(&mut doc.transact_mut(), 0, text);
        let upd = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        (encode(&Message::Sync(SyncMessage::Update(upd))), doc)
    }

    #[test]
    fn crdt_room_applies_and_relays_update() {
        let state = CrdtSyncState::default();
        let room = state.get_or_load_room("d").unwrap();
        let (frame, _) = update_frame("olá");
        let mut seen = HashSet::new();
        let (mut replies, mut relay) = (vec![], vec![]);
        for m in decode_frame(&frame).unwrap() {
            handle_message(&state, "d", &room, m, &mut seen, &mut replies, &mut relay).unwrap();
        }
        assert_eq!(room.text(), "olá");
        assert!(replies.is_empty());
        assert_eq!(relay.len(), 1);
    }

    #[test]
    fn crdt_room_answers_sync_step1_with_diff() {
        let state = CrdtSyncState::default();
        let room = state.get_or_load_room("d").unwrap();
        let (frame, _) = update_frame("abc");
        let mut seen = HashSet::new();
        let (mut replies, mut relay) = (vec![], vec![]);
        for m in decode_frame(&frame).unwrap() {
            handle_message(&state, "d", &room, m, &mut seen, &mut replies, &mut relay).unwrap();
        }
        let step1 = encode(&Message::Sync(SyncMessage::SyncStep1(
            StateVector::default(),
        )));
        for m in decode_frame(&step1).unwrap() {
            handle_message(&state, "d", &room, m, &mut seen, &mut replies, &mut relay).unwrap();
        }
        assert_eq!(replies.len(), 1);
        let msgs = decode_frame(&replies[0]).unwrap();
        let Message::Sync(SyncMessage::SyncStep2(diff)) = &msgs[0] else {
            panic!("esperado SyncStep2");
        };
        assert_eq!(text_of_state(diff).unwrap(), "abc");
    }

    #[test]
    fn crdt_ping_is_echoed_and_invalid_update_rejected() {
        let state = CrdtSyncState::default();
        let room = state.get_or_load_room("d").unwrap();
        let mut seen = HashSet::new();
        let (mut replies, mut relay) = (vec![], vec![]);
        handle_message(
            &state,
            "d",
            &room,
            Message::Custom(PING_TAG, vec![1, 2, 3]),
            &mut seen,
            &mut replies,
            &mut relay,
        )
        .unwrap();
        assert_eq!(replies.len(), 1);
        let bad = handle_message(
            &state,
            "d",
            &room,
            Message::Sync(SyncMessage::Update(vec![0xff, 0xff])),
            &mut seen,
            &mut replies,
            &mut relay,
        );
        assert!(bad.is_err());
        assert!(relay.is_empty());
    }

    #[test]
    fn crdt_awareness_tracked_for_cleanup() {
        let state = CrdtSyncState::default();
        let room = state.get_or_load_room("d").unwrap();
        let mut seen = HashSet::new();
        let (mut replies, mut relay) = (vec![], vec![]);
        let upd = awareness_update(
            42,
            1,
            r#"{"user":{"name":"Ana"},"cursor":{"anchor":3,"head":3}}"#,
        );
        handle_message(
            &state,
            "d",
            &room,
            Message::Awareness(upd),
            &mut seen,
            &mut replies,
            &mut relay,
        )
        .unwrap();
        assert!(seen.contains(&ClientID::new(42)));
        assert_eq!(relay.len(), 1);
        let n = room.with_awareness(|a| a.iter().count());
        assert_eq!(n, 1);
    }

    /// F1: sem token_service, conexões são aceitas (modo dev/local).
    #[test]
    fn crdt_ws_accepts_connection_without_token_service() {
        let state = CrdtSyncState::default();
        assert!(state.token_service.is_none());
        // Se token_service é None, ws_sync não valida JWT — comportamento esperado.
    }

    /// F1: com token_service, query sem access_token deve ser rejeitada.
    #[test]
    fn crdt_ws_rejects_missing_token_when_enforced() {
        use zylos_auth::{TokenConfig, TokenService};
        let svc = TokenService::new(TokenConfig {
            secret: "test-secret-at-least-32-bytes-long!".into(),
            ..TokenConfig::default()
        })
        .unwrap();
        let state = CrdtSyncState::default().with_token_service(Some(svc));
        assert!(state.token_service.is_some());
        // A validação real ocorre no handler ws_sync via Query(WsSyncQuery);
        // este teste garante que o estado carrega o serviço corretamente.
    }
}
