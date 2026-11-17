//! Painel de edição colaborativa (Fase 11.9 — T11.9.2 / T11.9.4 / T11.9.5).
//!
//! - Conecta via WebSocket binário a `GET /api/v1/crdt/sync/:doc_id`
//!   (servidor `apps/zylos-gateway/src/crdt_sync.rs`, protocolo y-websocket v1).
//! - Réplica local `yrs` (Yjs) compilada para wasm32 — ver `crate::crdt_client`.
//! - Presença: lista de pares com cor e cursor (awareness).
//! - Resolução de conflitos: modo rascunho → diff visual local × remoto →
//!   botões Mesclar (CRDT) / Manter local / Aceitar remoto.
//!
//! Estado não-`Send` (doc yrs, `WebSocket`) vive em `thread_local!`; os
//! handlers capturam apenas sinais (`Copy`).

use std::cell::RefCell;

use leptos::prelude::*;

use crate::crdt_client::{color_for, sync_url, CrdtClient, DiffLine, Event, Peer, Presence};

thread_local! {
    static CLIENT: RefCell<Option<CrdtClient>> = const { RefCell::new(None) };
    #[cfg(target_arch = "wasm32")]
    static SOCKET: RefCell<Option<web_sys::WebSocket>> = const { RefCell::new(None) };
}

/// Estado de conexão exibido no cabeçalho.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Conn {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Conn {
    fn label(&self) -> &'static str {
        match self {
            Conn::Disconnected => "Desconectado",
            Conn::Connecting => "Conectando…",
            Conn::Connected => "Conectado",
            Conn::Error => "Erro de conexão",
        }
    }
}

#[derive(Clone, Copy)]
struct Ui {
    text: RwSignal<String>,
    remote_text: RwSignal<String>,
    peers: RwSignal<Vec<Peer>>,
    conn: RwSignal<Conn>,
    draft: RwSignal<bool>,
    diff: RwSignal<Vec<DiffLine>>,
}

impl Ui {
    /// Recalcula sinais a partir da réplica.
    fn refresh(&self) {
        CLIENT.with(|c| {
            if let Some(c) = c.borrow().as_ref() {
                self.remote_text.set(c.text());
                self.text.set(c.draft_text());
                self.peers.set(c.peers());
                self.draft.set(c.in_draft());
                self.diff.set(if c.has_conflict() {
                    c.conflict_diff()
                } else {
                    Vec::new()
                });
            }
        });
    }
}

fn send(frame: Vec<u8>) {
    if frame.is_empty() {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    SOCKET.with(|s| {
        if let Some(ws) = s.borrow().as_ref() {
            if ws.ready_state() == web_sys::WebSocket::OPEN {
                let _ = ws.send_with_u8_array(&frame);
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = frame;
}

fn with_client<R>(f: impl FnOnce(&mut CrdtClient) -> R) -> Option<R> {
    CLIENT.with(|c| c.borrow_mut().as_mut().map(f))
}

fn random_client_id() -> u64 {
    // 53 bits: compatível com Yjs (Number.MAX_SAFE_INTEGER).
    (uuid::Uuid::new_v4().as_u128() as u64) & ((1u64 << 53) - 1)
}

#[cfg(target_arch = "wasm32")]
fn connect(ui: Ui, url: String, name: String) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    // Fecha conexão anterior.
    SOCKET.with(|s| {
        if let Some(ws) = s.borrow_mut().take() {
            let _ = ws.close();
        }
    });
    let id = random_client_id();
    CLIENT.with(|c| *c.borrow_mut() = Some(CrdtClient::new(id)));
    ui.conn.set(Conn::Connecting);

    let ws = match WebSocket::new(&url) {
        Ok(w) => w,
        Err(_) => {
            ui.conn.set(Conn::Error);
            return;
        }
    };
    ws.set_binary_type(BinaryType::Arraybuffer);

    let on_open = Closure::<dyn FnMut()>::new(move || {
        ui.conn.set(Conn::Connected);
        if let Some((hs, pr)) = with_client(|c| {
            let p = Presence {
                name: name.clone(),
                color: color_for(id).to_string(),
                cursor: None,
            };
            (c.handshake(), c.set_presence(&p))
        }) {
            send(hs);
            send(pr);
        }
    });
    let on_msg = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
        let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() else {
            return;
        };
        let bytes = js_sys::Uint8Array::new(&buf).to_vec();
        if let Some((replies, events)) = with_client(|c| c.handle_frame(&bytes)) {
            for r in replies {
                send(r);
            }
            if events.iter().any(|ev| !matches!(ev, Event::Pong(_))) {
                ui.refresh();
            }
        }
    });
    let on_close = Closure::<dyn FnMut()>::new(move || ui.conn.set(Conn::Disconnected));
    let on_err = Closure::<dyn FnMut()>::new(move || ui.conn.set(Conn::Error));
    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    ws.set_onerror(Some(on_err.as_ref().unchecked_ref()));
    on_open.forget();
    on_msg.forget();
    on_close.forget();
    on_err.forget();
    SOCKET.with(|s| *s.borrow_mut() = Some(ws));
}

#[cfg(not(target_arch = "wasm32"))]
fn connect(ui: Ui, _url: String, _name: String) {
    CLIENT.with(|c| *c.borrow_mut() = Some(CrdtClient::new(random_client_id())));
    ui.conn.set(Conn::Error);
}

fn default_ws_base() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(loc) = web_sys::window().map(|w| w.location()) {
            let proto = if loc.protocol().map(|p| p == "https:").unwrap_or(false) {
                "wss"
            } else {
                "ws"
            };
            if let Ok(host) = loc.host() {
                if !host.is_empty() && !host.starts_with("localhost:8080") {
                    return format!("{proto}://{host}");
                }
            }
        }
    }
    "ws://localhost:3000".to_string()
}

#[cfg(target_arch = "wasm32")]
fn textarea_selection(target: Option<web_sys::EventTarget>) -> Option<(u32, u32)> {
    use wasm_bindgen::JsCast;
    let ta = target?.dyn_into::<web_sys::HtmlTextAreaElement>().ok()?;
    Some((ta.selection_start().ok()??, ta.selection_end().ok()??))
}

#[cfg(not(target_arch = "wasm32"))]
fn textarea_selection(_target: Option<web_sys::EventTarget>) -> Option<(u32, u32)> {
    None
}

/// Painel de edição colaborativa em tempo real.
#[component]
pub fn CollabPanel() -> impl IntoView {
    let ui = Ui {
        text: RwSignal::new(String::new()),
        remote_text: RwSignal::new(String::new()),
        peers: RwSignal::new(Vec::new()),
        conn: RwSignal::new(Conn::Disconnected),
        draft: RwSignal::new(false),
        diff: RwSignal::new(Vec::new()),
    };
    let doc_id = RwSignal::new("cockpit-notes".to_string());
    let name = RwSignal::new("operador".to_string());

    let on_connect = move |_| {
        let url = sync_url(&default_ws_base(), &doc_id.get_untracked());
        connect(ui, url, name.get_untracked());
    };

    let publish_cursor = move |target: Option<web_sys::EventTarget>| {
        let cursor = textarea_selection(target);
        let me = name.get_untracked();
        if let Some(frame) = with_client(|c| {
            let p = Presence {
                name: me,
                color: color_for(c.client_id()).to_string(),
                cursor,
            };
            c.set_presence(&p)
        }) {
            send(frame);
        }
    };

    let on_input = move |ev: leptos::ev::Event| {
        let value = event_target_value(&ev);
        if let Some(Some(frame)) = with_client(|c| c.local_edit(&value)) {
            send(frame);
        }
        publish_cursor(ev.target());
        ui.refresh();
    };
    let on_select = move |ev: leptos::ev::Event| publish_cursor(ev.target());
    let on_keyup = move |ev: leptos::ev::KeyboardEvent| publish_cursor(ev.target());
    let on_click = move |ev: leptos::ev::MouseEvent| publish_cursor(ev.target());

    let toggle_draft = move |_| {
        with_client(|c| c.begin_draft());
        ui.refresh();
    };
    let merge = move |_| {
        if let Some(Some(f)) = with_client(|c| c.merge_draft()) {
            send(f);
        }
        ui.refresh();
    };
    let keep_local = move |_| {
        if let Some(Some(f)) = with_client(|c| c.keep_local()) {
            send(f);
        }
        ui.refresh();
    };
    let accept_remote = move |_| {
        with_client(|c| c.accept_remote());
        ui.refresh();
    };

    view! {
        <section class="collab-panel" aria-label="Edição colaborativa">
            <header class="collab-header">
                <h2>"Edição colaborativa (CRDT)"</h2>
                <label>"Documento "
                    <input class="collab-doc-id" prop:value=move || doc_id.get()
                        on:input=move |ev| doc_id.set(event_target_value(&ev)) />
                </label>
                <label>"Nome "
                    <input class="collab-name" prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev)) />
                </label>
                <button class="btn collab-connect" on:click=on_connect>"Conectar"</button>
                <span class=move || format!("collab-status collab-status-{:?}", ui.conn.get()).to_lowercase()
                    role="status">{move || ui.conn.get().label()}</span>
            </header>

            <div class="collab-body">
                <div class="collab-editor">
                    <textarea class="collab-textarea" rows="16" aria-label="Documento compartilhado"
                        prop:value=move || ui.text.get()
                        on:input=on_input
                        on:select=on_select
                        on:keyup=on_keyup
                        on:click=on_click></textarea>
                    <div class="collab-draft-bar">
                        <Show when=move || !ui.draft.get()
                            fallback=|| view! { <span class="collab-draft-on">"Modo rascunho: edições locais isoladas"</span> }>
                            <button class="btn collab-begin-draft" on:click=toggle_draft>
                                "Editar como rascunho"
                            </button>
                        </Show>
                    </div>
                </div>

                <aside class="collab-presence" aria-label="Presença">
                    <h3>{move || format!("Online ({})", ui.peers.get().len())}</h3>
                    <ul>
                        <For each=move || ui.peers.get() key=|p| p.client let:peer>
                            <li class="collab-peer">
                                <span class="collab-peer-dot"
                                    style=format!("background:{};display:inline-block;width:.75rem;height:.75rem;border-radius:50%", peer.presence.color)></span>
                                " " {peer.presence.name.clone()}
                                <small class="collab-peer-cursor" style=format!("color:{}", peer.presence.color)>
                                    {match peer.presence.cursor {
                                        Some((a, b)) if a == b => format!(" cursor {a}"),
                                        Some((a, b)) => format!(" seleção {a}–{b}"),
                                        None => String::new(),
                                    }}
                                </small>
                            </li>
                        </For>
                    </ul>
                </aside>
            </div>

            <Show when=move || ui.draft.get()>
                <section class="collab-conflict" aria-label="Resolução de conflitos">
                    <h3>"Local × remoto"</h3>
                    <Show when=move || !ui.diff.get().is_empty()
                        fallback=|| view! { <p class="collab-no-conflict">"Sem divergência."</p> }>
                        <pre class="collab-diff">
                            {move || ui.diff.get().into_iter().map(|l| match l {
                                DiffLine::Same(s) => view! { <div class="diff-same">{format!("  {s}")}</div> }.into_any(),
                                DiffLine::Local(s) => view! { <div class="diff-local" style="background:rgba(60,180,75,.2)">{format!("+ {s}")}</div> }.into_any(),
                                DiffLine::Remote(s) => view! { <div class="diff-remote" style="background:rgba(67,99,216,.2)">{format!("~ {s}")}</div> }.into_any(),
                            }).collect_view()}
                        </pre>
                    </Show>
                    <div class="collab-conflict-actions">
                        <button class="btn collab-merge" on:click=merge>"Mesclar (CRDT)"</button>
                        <button class="btn collab-keep-local" on:click=keep_local>"Manter local"</button>
                        <button class="btn collab-accept-remote" on:click=accept_remote>"Aceitar remoto"</button>
                    </div>
                    <details>
                        <summary>"Texto remoto atual"</summary>
                        <pre class="collab-remote-text">{move || ui.remote_text.get()}</pre>
                    </details>
                </section>
            </Show>
        </section>
    }
}
