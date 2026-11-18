//! Zylos Cockpit — Channel System (Task 4.1)
//!
//! Painel de canais com sidebar, lista de mensagens e input. Consome só o gateway:
//! - `GET  /api/v1/channels` / `POST /api/v1/channels` — listar / criar canal
//! - `GET  /api/v1/channels/{id}/messages` / `POST ...` — listar / enviar mensagem
//! - `POST /api/v1/channels/{id}/join` / `.../leave` — entrar / sair
//!
//! Sem gateway → mensagem de erro curta; sem canais → estado vazio.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::gateway_http::{get_json, post_json};
use crate::ui_error::user_message;

/// Canal do sistema de comunicação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub member_count: u32,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub unread_count: u32,
    #[serde(default)]
    pub last_message_at: String,
}

/// Mensagem individual dentro de um canal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelMessage {
    pub id: String,
    pub channel_id: String,
    pub author: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Deserialize)]
struct ChannelsResponse {
    channels: Vec<Channel>,
}

#[derive(Deserialize)]
struct MessagesResponse {
    messages: Vec<ChannelMessage>,
}

#[derive(Deserialize)]
struct MessageCreated {
    message: ChannelMessage,
}

#[derive(Deserialize)]
struct ChannelCreated {
    channel: Channel,
}

fn messages_path(channel_id: &str) -> String {
    format!(
        "/api/v1/channels/{}/messages",
        crate::auth::url_encode(channel_id)
    )
}

/// Formata timestamp ISO para exibição legível
fn format_timestamp(iso: &str) -> String {
    // Extrai HH:MM de string ISO 8601 simples
    if iso.len() >= 16 {
        if let Some(t_pos) = iso.find('T') {
            let time_part = &iso[t_pos + 1..];
            if time_part.len() >= 5 {
                return time_part[..5].to_string();
            }
        }
    }
    iso.to_string()
}

/// Componente principal do painel de canais
#[component]
pub fn ChannelPanel() -> impl IntoView {
    let channels = RwSignal::new(Vec::<Channel>::new());
    let messages = RwSignal::new(Vec::<ChannelMessage>::new());
    let active_channel_id = RwSignal::new(String::new());
    let new_message_text = RwSignal::new(String::new());
    let new_channel_name = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(None::<String>);
    let notice = RwSignal::new(None::<String>);

    let show_err = move |e: String| {
        web_sys::console::warn_1(&format!("channels: {e}").into());
        error.set(Some(user_message(&e)));
    };

    // Carregar canais do gateway
    spawn_local(async move {
        match get_json::<ChannelsResponse>("/api/v1/channels").await {
            Ok(resp) => {
                if let Some(first) = resp.channels.first() {
                    active_channel_id.set(first.id.clone());
                }
                channels.set(resp.channels);
            }
            Err(e) => show_err(e),
        }
        loading.set(false);
    });

    // Carregar mensagens quando canal ativo muda
    Effect::new(move |_| {
        let cid = active_channel_id.get();
        if cid.is_empty() {
            return;
        }
        messages.set(Vec::new());
        spawn_local(async move {
            match get_json::<MessagesResponse>(&messages_path(&cid)).await {
                Ok(resp) => {
                    if active_channel_id.get_untracked() == cid {
                        messages.set(resp.messages);
                    }
                }
                Err(e) => show_err(e),
            }
        });
    });

    let send_message = move || {
        let text = new_message_text.get_untracked();
        let cid = active_channel_id.get_untracked();
        if text.trim().is_empty() || cid.is_empty() {
            return;
        }
        spawn_local(async move {
            let body = serde_json::json!({
                "author": crate::auth::current_user(),
                "content": text.trim(),
            });
            match post_json::<_, MessageCreated>(&messages_path(&cid), &body).await {
                Ok(created) => {
                    error.set(None);
                    new_message_text.set(String::new());
                    if active_channel_id.get_untracked() == cid {
                        messages.update(|m| m.push(created.message));
                    }
                }
                Err(e) => show_err(e),
            }
        });
    };

    let create_channel = move || {
        let name = new_channel_name.get_untracked().trim().to_string();
        if name.is_empty() {
            return;
        }
        spawn_local(async move {
            let body = serde_json::json!({ "name": name, "description": "" });
            match post_json::<_, ChannelCreated>("/api/v1/channels", &body).await {
                Ok(created) => {
                    error.set(None);
                    new_channel_name.set(String::new());
                    let id = created.channel.id.clone();
                    channels.update(|c| c.push(created.channel));
                    active_channel_id.set(id);
                }
                Err(e) if e == "HTTP 409" => {
                    error.set(Some("Já existe um canal com esse nome.".into()))
                }
                Err(e) => show_err(e),
            }
        });
    };

    let member_action = move |action: &'static str| {
        let cid = active_channel_id.get_untracked();
        if cid.is_empty() {
            return;
        }
        spawn_local(async move {
            let path = format!(
                "/api/v1/channels/{}/{action}",
                crate::auth::url_encode(&cid)
            );
            let body = serde_json::json!({ "user_id": crate::auth::current_user() });
            match post_json::<_, serde_json::Value>(&path, &body).await {
                Ok(_) => {
                    error.set(None);
                    notice.set(Some(if action == "join" {
                        "Você entrou no canal.".into()
                    } else {
                        "Você saiu do canal.".into()
                    }));
                    if let Ok(resp) = get_json::<ChannelsResponse>("/api/v1/channels").await {
                        channels.set(resp.channels);
                    }
                }
                Err(e) if e == "HTTP 409" => {
                    notice.set(Some("Você já é membro deste canal.".into()))
                }
                Err(e) if e == "HTTP 404" && action == "leave" => {
                    notice.set(Some("Você não é membro deste canal.".into()))
                }
                Err(e) => show_err(e),
            }
        });
    };

    view! {
        <div class="channel-panel">
            <aside class="channel-sidebar">
                <h3>{"Canais"}</h3>
                <div class="channel-list">
                    <For
                        each=move || channels.get()
                        key=|ch| (ch.id.clone(), ch.member_count)
                        children=move |channel| {
                            let ch_id = channel.id.clone();
                            let ch_id_for_active = ch_id.clone();
                            let ch_name = channel.name.clone();
                            let ch_unread = channel.unread_count;
                            let ch_private = channel.is_private;
                            let ch_members = channel.member_count;
                            let is_active = move || active_channel_id.get() == ch_id_for_active;
                            let item_class = move || {
                                if is_active() {
                                    "channel-item channel-active".to_string()
                                } else {
                                    "channel-item".to_string()
                                }
                            };
                            view! {
                                <div
                                    class=item_class
                                    on:click=move |_| active_channel_id.set(ch_id.clone())
                                >
                                    <span class="channel-name">
                                        {if ch_private { "\u{1F512} " } else { "# " }}
                                        {ch_name.clone()}
                                    </span>
                                    {(ch_unread > 0).then(|| view! {
                                        <span class="channel-unread-badge">{ch_unread}</span>
                                    })}
                                    <span class="channel-meta">
                                        {"\u{2022}"}{format!(" {} membros", ch_members)}
                                    </span>
                                </div>
                            }
                        }
                    />
                </div>
                <div class="channel-create">
                    <input
                        class="channel-input"
                        type="text"
                        aria-label="Nome do novo canal"
                        placeholder="Novo canal..."
                        prop:value=move || new_channel_name.get()
                        on:input=move |ev| new_channel_name.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                create_channel();
                            }
                        }
                    />
                    <button class="channel-action-btn" on:click=move |_| create_channel()>
                        {"Criar"}
                    </button>
                </div>
            </aside>

            <section class="channel-main">
                {move || error.get().map(|e| view! { <div class="channel-error" role="alert">{e}</div> })}
                {move || notice.get().map(|n| view! { <div class="channel-notice" role="status">{n}</div> })}
                {move || {
                    if loading.get() {
                        return view! {
                            <div class="channel-loading">
                                <p>{"Carregando canais..."}</p>
                            </div>
                        }.into_any();
                    }
                    if channels.with(|c| c.is_empty()) {
                        return view! {
                            <div class="channel-empty">
                                <p>{"Nenhum canal ainda. Crie o primeiro ao lado."}</p>
                            </div>
                        }.into_any();
                    }
                    let active_id = active_channel_id.get();
                    let active_channel = channels.with(|c| c.iter().find(|c| c.id == active_id).cloned());
                    let Some(ch) = active_channel else {
                        return view! {
                            <div class="channel-empty">
                                <p>{"Selecione um canal para começar."}</p>
                            </div>
                        }.into_any();
                    };
                    view! {
                        <div class="channel-header">
                            <h2>{"# "}{ch.name.clone()}</h2>
                            <p class="channel-description">{ch.description.clone()}</p>
                            <div class="channel-actions">
                                <button class="channel-action-btn" on:click=move |_| member_action("join")>
                                    {"Entrar"}
                                </button>
                                <button
                                    class="channel-action-btn channel-action-leave"
                                    on:click=move |_| member_action("leave")
                                >
                                    {"Sair"}
                                </button>
                            </div>
                        </div>
                        <div class="channel-messages">
                            {move || messages.with(|m| m.is_empty()).then(|| view! {
                                <p class="channel-empty">{"Nenhuma mensagem neste canal ainda."}</p>
                            })}
                            <For
                                each=move || messages.get()
                                key=|m| m.id.clone()
                                children=move |msg| {
                                    let time = format_timestamp(&msg.timestamp);
                                    view! {
                                        <div class="channel-message">
                                            <span class="message-author">{msg.author}</span>
                                            <span class="message-time">{time}</span>
                                            <p class="message-content">{msg.content}</p>
                                            <crate::components::emoji_reactions::EmojiReactions message_id=msg.id.clone() />
                                        </div>
                                    }
                                }
                            />
                        </div>
                        <div class="channel-input-area">
                            <input
                                class="channel-input"
                                type="text"
                                aria-label="Mensagem do canal"
                                placeholder="Digite sua mensagem..."
                                prop:value=move || new_message_text.get()
                                on:input=move |ev| new_message_text.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" {
                                        send_message();
                                    }
                                }
                            />
                            <button class="channel-send-btn" on:click=move |_| send_message()>
                                {"Enviar"}
                            </button>
                        </div>
                    }.into_any()
                }}
            </section>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_serialization_roundtrip() {
        let channel = Channel {
            id: "ch-test".to_string(),
            name: "Teste".to_string(),
            description: "Canal de teste".to_string(),
            member_count: 10,
            is_private: true,
            unread_count: 5,
            last_message_at: "2026-11-11T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&channel).unwrap();
        let deserialized: Channel = serde_json::from_str(&json).unwrap();
        assert_eq!(channel, deserialized);
    }

    #[test]
    fn test_message_serialization_roundtrip() {
        let msg = ChannelMessage {
            id: "msg-test".to_string(),
            channel_id: "ch-test".to_string(),
            author: "Tester".to_string(),
            content: "Olá mundo".to_string(),
            timestamp: "2026-11-11T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ChannelMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_parse_gateway_channels_payload() {
        let raw = r#"{"channels":[{"id":"c1","name":"geral","description":"d","member_count":2,"is_private":false,"unread_count":0,"last_message_at":"2026-11-11T12:00:00Z"}],"total":1}"#;
        let resp: ChannelsResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.channels.len(), 1);
        assert_eq!(resp.channels[0].name, "geral");
    }

    #[test]
    fn test_messages_path_encodes_id() {
        assert_eq!(messages_path("a b"), "/api/v1/channels/a%20b/messages");
    }

    #[test]
    fn test_format_timestamp_valid() {
        assert_eq!(format_timestamp("2026-11-11T14:30:00Z"), "14:30");
    }

    #[test]
    fn test_format_timestamp_invalid() {
        assert_eq!(format_timestamp("short"), "short");
    }

    #[test]
    fn test_channel_partial_eq() {
        let a = Channel {
            id: "ch-1".to_string(),
            name: "A".to_string(),
            description: "".to_string(),
            member_count: 1,
            is_private: false,
            unread_count: 0,
            last_message_at: "".to_string(),
        };
        let b = a.clone();
        assert_eq!(a, b);
    }
}
