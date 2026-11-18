//! Cliente CRDT do cockpit (Fase 11.9 — T11.9.2 / T11.9.4 / T11.9.5).
//!
//! Núcleo **sem DOM** do cliente de edição colaborativa: mantém uma réplica
//! `yrs` (Yjs) + `Awareness` e fala o protocolo y-websocket v1 com o servidor
//! `apps/zylos-gateway/src/crdt_sync.rs` (rota `GET /api/v1/crdt/sync/:doc_id`).
//!
//! Por ser independente de `web-sys`, a lógica é testada nativamente
//! (`cargo test -p zylos-cockpit --lib crdt_client`). O painel Leptos
//! (`components/collab_panel.rs`) apenas conecta o `WebSocket` e repassa frames.
//!
//! ## Resolução de conflitos (modo rascunho)
//! Em modo rascunho as edições locais vão para um `Doc` *fork* (mesmo histórico
//! do documento principal no momento da bifurcação). Updates remotos continuam
//! a chegar no documento principal. O usuário vê o diff local × remoto e escolhe:
//! - **Mesclar (CRDT)** — aplica o delta do fork no principal: ambos os lados
//!   são preservados, ordenação determinística do YATA;
//! - **Manter local** — substitui o texto remoto pelo rascunho;
//! - **Aceitar remoto** — descarta o rascunho.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use yrs::sync::{Awareness, Message, MessageReader, SyncMessage};
use yrs::updates::decoder::{Decode, DecoderV1};
use yrs::updates::encoder::Encode;
use yrs::{ClientID, Doc, GetString, ReadTxn, StateVector, Text, Transact, Update};

/// Root de texto compartilhado com o servidor (`zylos_auth::crdt_persistence::TEXT_ROOT`).
pub const TEXT_ROOT: &str = "content";
/// Tag `Message::Custom` de ping/pong (igual a `crdt_sync::PING_TAG`).
pub const PING_TAG: u8 = 100;

/// Paleta de cores de presença via CSS custom properties (ADR-008).
/// Valores definidos em `styles/design-tokens.css` como --peer-1..--peer-8.
pub const PRESENCE_COLORS: [&str; 8] = [
    "var(--peer-1)",
    "var(--peer-2)",
    "var(--peer-3)",
    "var(--peer-4)",
    "var(--peer-5)",
    "var(--peer-6)",
    "var(--peer-7)",
    "var(--peer-8)",
];

/// Cor determinística para um client id.
pub fn color_for(client: u64) -> &'static str {
    PRESENCE_COLORS[(client % PRESENCE_COLORS.len() as u64) as usize]
}

/// URL do WebSocket de sincronização para um documento.
///
/// Com token salvo (`localStorage['zylos_token']`) acrescenta `?access_token=`,
/// pois WebSocket do navegador não aceita header `Authorization`.
pub fn sync_url(base_ws: &str, doc_id: &str) -> String {
    crate::auth::authed_url(&format!(
        "{}/api/v1/crdt/sync/{}",
        base_ws.trim_end_matches('/'),
        doc_id
    ))
}

/// Estado de presença publicado via awareness (JSON compatível com y-protocols).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub name: String,
    pub color: String,
    /// Cursor/seleção em índices de caractere (`anchor..head`).
    pub cursor: Option<(u32, u32)>,
}

/// Par remoto visto pelo awareness.
#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub client: u64,
    pub presence: Presence,
}

/// Evento produzido pelo processamento de um frame recebido.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    DocChanged,
    PresenceChanged,
    Pong(Vec<u8>),
}

/// Operação de linha no diff visual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    Same(String),
    /// Presente apenas no lado local (rascunho).
    Local(String),
    /// Presente apenas no lado remoto.
    Remote(String),
}

/// Cliente CRDT (uma sala/documento).
pub struct CrdtClient {
    awareness: Awareness,
    client: u64,
    draft: Option<Doc>,
}

fn now_ms() -> u64 {
    #[cfg(all(target_family = "wasm", target_os = "unknown"))]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Converte índice de caractere em offset de byte UTF-8 (yrs usa bytes por padrão).
fn byte_at(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Diff mínimo prefixo/sufixo comum (em caracteres) → (início, removidos, inserção).
pub fn text_delta(old: &str, new: &str) -> Option<(usize, usize, String)> {
    if old == new {
        return None;
    }
    let o: Vec<char> = old.chars().collect();
    let n: Vec<char> = new.chars().collect();
    let mut pre = 0;
    while pre < o.len() && pre < n.len() && o[pre] == n[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < o.len() - pre && suf < n.len() - pre && o[o.len() - 1 - suf] == n[n.len() - 1 - suf]
    {
        suf += 1;
    }
    let removed = o.len() - pre - suf;
    let inserted: String = n[pre..n.len() - suf].iter().collect();
    Some((pre, removed, inserted))
}

/// Aplica `old → new` como operações CRDT em `doc` (texto em [`TEXT_ROOT`]).
fn apply_text(doc: &Doc, new: &str) {
    let text = doc.get_or_insert_text(TEXT_ROOT);
    let old = {
        let txn = doc.transact();
        text.get_string(&txn)
    };
    if let Some((start, removed, inserted)) = text_delta(&old, new) {
        let b0 = byte_at(&old, start) as u32;
        let b1 = byte_at(&old, start + removed) as u32;
        let mut txn = doc.transact_mut();
        if b1 > b0 {
            text.remove_range(&mut txn, b0, b1 - b0);
        }
        if !inserted.is_empty() {
            text.insert(&mut txn, b0, &inserted);
        }
    }
}

fn doc_text(doc: &Doc) -> String {
    let t = doc.get_or_insert_text(TEXT_ROOT);
    let txn = doc.transact();
    t.get_string(&txn)
}

/// Diff de linhas por LCS (documentos de edição colaborativa são pequenos).
pub fn line_diff(local: &str, remote: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = local.lines().collect();
    let b: Vec<&str> = remote.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(DiffLine::Same(a[i].to_string()));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            out.push(DiffLine::Local(a[i].to_string()));
            i += 1;
        } else {
            out.push(DiffLine::Remote(b[j].to_string()));
            j += 1;
        }
    }
    out.extend(a[i..].iter().map(|l| DiffLine::Local(l.to_string())));
    out.extend(b[j..].iter().map(|l| DiffLine::Remote(l.to_string())));
    out
}

impl CrdtClient {
    pub fn new(client: u64) -> Self {
        let doc = Doc::with_client_id(client);
        let _ = doc.get_or_insert_text(TEXT_ROOT);
        Self {
            // `Awareness::new` não existe em wasm32-unknown-unknown (sem SystemTime).
            awareness: Awareness::with_clock(doc, now_ms),
            client,
            draft: None,
        }
    }

    pub fn client_id(&self) -> u64 {
        self.client
    }

    /// Frame inicial do handshake (`SyncStep1` com o state vector local).
    pub fn handshake(&self) -> Vec<u8> {
        let sv = self.awareness.doc().transact().state_vector();
        Message::Sync(SyncMessage::SyncStep1(sv)).encode_v1()
    }

    /// Processa um frame binário do servidor; devolve frames de resposta e eventos.
    pub fn handle_frame(&mut self, bytes: &[u8]) -> (Vec<Vec<u8>>, Vec<Event>) {
        let mut replies = Vec::new();
        let mut events = Vec::new();
        let mut dec = DecoderV1::from(bytes);
        let msgs: Vec<Message> = MessageReader::new(&mut dec)
            .filter_map(Result::ok)
            .collect();
        for msg in msgs {
            match msg {
                Message::Sync(SyncMessage::SyncStep1(sv)) => {
                    let diff = self.awareness.doc().transact().encode_diff_v1(&sv);
                    replies.push(Message::Sync(SyncMessage::SyncStep2(diff)).encode_v1());
                }
                Message::Sync(SyncMessage::SyncStep2(u))
                | Message::Sync(SyncMessage::Update(u)) => {
                    if let Ok(upd) = Update::decode_v1(&u) {
                        if self
                            .awareness
                            .doc()
                            .transact_mut()
                            .apply_update(upd)
                            .is_ok()
                        {
                            events.push(Event::DocChanged);
                        }
                    }
                }
                Message::Awareness(upd) => {
                    if self.awareness.apply_update(upd).is_ok() {
                        events.push(Event::PresenceChanged);
                    }
                }
                Message::Custom(PING_TAG, payload) => events.push(Event::Pong(payload)),
                _ => {}
            }
        }
        (replies, events)
    }

    /// Texto do documento principal (estado convergido com o servidor).
    pub fn text(&self) -> String {
        doc_text(self.awareness.doc())
    }

    /// Edição local vinda do textarea. Em modo ao vivo devolve o frame `Update`;
    /// em modo rascunho grava no fork e devolve `None`.
    pub fn local_edit(&mut self, new_text: &str) -> Option<Vec<u8>> {
        if let Some(draft) = &self.draft {
            apply_text(draft, new_text);
            return None;
        }
        let doc = self.awareness.doc();
        let sv = doc.transact().state_vector();
        apply_text(doc, new_text);
        let diff = doc.transact().encode_diff_v1(&sv);
        if diff.len() <= 2 {
            return None; // update vazio
        }
        Some(Message::Sync(SyncMessage::Update(diff)).encode_v1())
    }

    /// Publica presença local; devolve o frame `Awareness`.
    pub fn set_presence(&mut self, p: &Presence) -> Vec<u8> {
        let json = serde_json::to_string(p).unwrap_or_else(|_| "{}".to_string());
        self.awareness.set_local_state_raw(json);
        match self
            .awareness
            .update_with_clients([ClientID::new(self.client)])
        {
            Ok(u) => Message::Awareness(u).encode_v1(),
            Err(_) => Vec::new(),
        }
    }

    /// Pares remotos com presença válida, ordenados por client id.
    pub fn peers(&self) -> Vec<Peer> {
        let mut out: Vec<Peer> = self
            .awareness
            .iter()
            .filter(|(id, _)| id.get() != self.client)
            .filter_map(|(id, st)| {
                let data: Arc<str> = st.data?;
                let presence: Presence = serde_json::from_str(&data).ok()?;
                Some(Peer {
                    client: id.get(),
                    presence,
                })
            })
            .collect();
        out.sort_by_key(|p| p.client);
        out
    }

    /// Frame de ping (RTT) — usado por ferramentas de diagnóstico (POT sync-tuning).
    #[allow(dead_code)]
    pub fn ping(seq: u32) -> Vec<u8> {
        Message::Custom(PING_TAG, seq.to_le_bytes().to_vec()).encode_v1()
    }

    // ----- Modo rascunho / resolução de conflitos (T11.9.5) -----

    pub fn in_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// Bifurca o documento: edições locais passam a ir para o rascunho.
    pub fn begin_draft(&mut self) {
        if self.draft.is_some() {
            return;
        }
        let state = self
            .awareness
            .doc()
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
        let fork = Doc::with_client_id(self.client);
        if let Ok(u) = Update::decode_v1(&state) {
            let _ = fork.transact_mut().apply_update(u);
        }
        self.draft = Some(fork);
    }

    /// Texto do rascunho (ou do principal fora do modo rascunho).
    pub fn draft_text(&self) -> String {
        self.draft
            .as_ref()
            .map(doc_text)
            .unwrap_or_else(|| self.text())
    }

    /// Diff visual rascunho (local) × documento remoto.
    pub fn conflict_diff(&self) -> Vec<DiffLine> {
        line_diff(&self.draft_text(), &self.text())
    }

    /// Há divergência entre rascunho e remoto?
    pub fn has_conflict(&self) -> bool {
        self.draft.is_some() && self.draft_text() != self.text()
    }

    fn update_since(&self, sv: &StateVector) -> Option<Vec<u8>> {
        let diff = self.awareness.doc().transact().encode_diff_v1(sv);
        (diff.len() > 2).then(|| Message::Sync(SyncMessage::Update(diff)).encode_v1())
    }

    /// Mescla CRDT: aplica as operações do rascunho no principal (preserva ambos).
    pub fn merge_draft(&mut self) -> Option<Vec<u8>> {
        let draft = self.draft.take()?;
        let doc = self.awareness.doc();
        let sv = doc.transact().state_vector();
        let delta = draft.transact().encode_diff_v1(&sv);
        if let Ok(u) = Update::decode_v1(&delta) {
            let _ = doc.transact_mut().apply_update(u);
        }
        self.update_since(&sv)
    }

    /// Mantém o rascunho: o texto remoto é substituído pelo local.
    pub fn keep_local(&mut self) -> Option<Vec<u8>> {
        let draft = self.draft.take()?;
        let local = doc_text(&draft);
        let doc = self.awareness.doc();
        let sv = doc.transact().state_vector();
        apply_text(doc, &local);
        self.update_since(&sv)
    }

    /// Aceita o remoto: descarta o rascunho.
    pub fn accept_remote(&mut self) {
        self.draft = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simula o servidor: entrega frames de `from` para `to`.
    fn relay(frames: Vec<Vec<u8>>, to: &mut CrdtClient) -> Vec<Event> {
        frames.iter().flat_map(|f| to.handle_frame(f).1).collect()
    }

    #[test]
    fn crdt_client_delta_handles_unicode() {
        assert_eq!(
            text_delta("olá mundo", "olá, mundo"),
            Some((3, 0, ",".into()))
        );
        assert_eq!(text_delta("abc", "abc"), None);
        assert_eq!(text_delta("ação", "aço"), Some((2, 1, "".into())));
        let doc = Doc::with_client_id(1);
        apply_text(&doc, "ação");
        apply_text(&doc, "a-ção!");
        assert_eq!(doc_text(&doc), "a-ção!");
    }

    #[test]
    fn crdt_client_three_replicas_converge() {
        let mut a = CrdtClient::new(1);
        let mut b = CrdtClient::new(2);
        let mut c = CrdtClient::new(3);
        let fa = a.local_edit("A").unwrap();
        let fb = b.local_edit("B").unwrap();
        let fc = c.local_edit("C").unwrap();
        relay(vec![fb.clone(), fc.clone()], &mut a);
        relay(vec![fc, fa.clone()], &mut b);
        relay(vec![fa, fb], &mut c);
        assert_eq!(a.text(), b.text());
        assert_eq!(b.text(), c.text());
        assert_eq!(a.text().len(), 3);
    }

    #[test]
    fn crdt_client_handshake_answers_step1() {
        let mut a = CrdtClient::new(1);
        a.local_edit("hello");
        let mut b = CrdtClient::new(2);
        let (replies, _) = a.handle_frame(&b.handshake());
        assert_eq!(replies.len(), 1);
        let ev = relay(replies, &mut b);
        assert_eq!(ev, vec![Event::DocChanged]);
        assert_eq!(b.text(), "hello");
    }

    #[test]
    fn crdt_client_presence_roundtrip() {
        let mut a = CrdtClient::new(7);
        let mut b = CrdtClient::new(8);
        let p = Presence {
            name: "Ana".into(),
            color: color_for(7).into(),
            cursor: Some((2, 5)),
        };
        let ev = relay(vec![a.set_presence(&p)], &mut b);
        assert_eq!(ev, vec![Event::PresenceChanged]);
        assert_eq!(
            b.peers(),
            vec![Peer {
                client: 7,
                presence: p
            }]
        );
        assert!(
            a.peers().is_empty(),
            "o próprio cliente não aparece como par"
        );
    }

    #[test]
    fn crdt_client_draft_merge_keeps_both_sides() {
        let mut a = CrdtClient::new(1);
        let mut b = CrdtClient::new(2);
        let base = a.local_edit("linha 1\nlinha 2\n").unwrap();
        relay(vec![base], &mut b);
        a.begin_draft();
        assert!(a.local_edit("linha 1 local\nlinha 2\n").is_none());
        let remote = b.local_edit("linha 1\nlinha 2 remota\n").unwrap();
        relay(vec![remote], &mut a);
        assert!(a.has_conflict());
        let diff = a.conflict_diff();
        assert!(diff.contains(&DiffLine::Local("linha 1 local".into())));
        assert!(diff.contains(&DiffLine::Remote("linha 2 remota".into())));
        let up = a.merge_draft().unwrap();
        relay(vec![up], &mut b);
        assert_eq!(a.text(), "linha 1 local\nlinha 2 remota\n");
        assert_eq!(a.text(), b.text());
    }

    #[test]
    fn crdt_client_keep_local_and_accept_remote() {
        let mut a = CrdtClient::new(1);
        let mut b = CrdtClient::new(2);
        relay(vec![a.local_edit("x").unwrap()], &mut b);
        a.begin_draft();
        a.local_edit("meu");
        relay(vec![b.local_edit("deles").unwrap()], &mut a);
        let up = a.keep_local().unwrap();
        relay(vec![up], &mut b);
        assert_eq!(a.text(), "meu");
        assert_eq!(b.text(), "meu");

        a.begin_draft();
        a.local_edit("descartado");
        a.accept_remote();
        assert!(!a.in_draft());
        assert_eq!(a.text(), "meu");
    }

    #[test]
    fn crdt_client_line_diff_and_url() {
        assert_eq!(
            line_diff("a\nb", "a\nc"),
            vec![
                DiffLine::Same("a".into()),
                DiffLine::Local("b".into()),
                DiffLine::Remote("c".into())
            ]
        );
        assert_eq!(
            sync_url("ws://h:3000/", "d1"),
            "ws://h:3000/api/v1/crdt/sync/d1"
        );
        assert_ne!(color_for(1), color_for(2));
    }
}
