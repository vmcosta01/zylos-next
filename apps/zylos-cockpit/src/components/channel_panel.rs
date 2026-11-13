//! Zylos Cockpit — Channel System (Task 4.1)
//!
//! Painel de canais com sidebar, lista de mensagens e input.
//! Usa RwSignal, spawn_local e mock data. Strings PT-BR direto.
//! Sem chrono — timestamps como strings ISO 8601.

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

/// Fetch JSON genérico via gloo-net (fallback para mock se falhar).
async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    use gloo_net::http::Request;
    let resp = Request::get(url)
        .send()
        .await
        .map_err(|e| format!("{:?}", e))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<T>().await.map_err(|e| format!("{:?}", e))
}

/// Envia POST com body JSON via gloo-net.
async fn post_json<B: Serialize, R: for<'de> Deserialize<'de>>(url: &str, body: &B) -> Result<R, String> {
    use gloo_net::http::Request;
    let resp = Request::post(url)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(body).map_err(|e| format!("{:?}", e))?)
        .map_err(|e| format!("{:?}", e))?
        .send()
        .await
        .map_err(|e| format!("{:?}", e))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json::<R>().await.map_err(|e| format!("{:?}", e))
}

/// Canal do sistema de comunicação
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub description: String,
    pub member_count: u32,
    pub is_private: bool,
    pub unread_count: u32,
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

/// Dados mock para demonstração
fn mock_channels() -> Vec<Channel> {
    vec![
        Channel {
            id: "ch-geral".to_string(),
            name: "Geral".to_string(),
            description: "Conversas gerais da equipe".to_string(),
            member_count: 24,
            is_private: false,
            unread_count: 3,
            last_message_at: "2026-11-11T14:30:00Z".to_string(),
        },
        Channel {
            id: "ch-dev".to_string(),
            name: "Desenvolvimento".to_string(),
            description: "Discussões técnicas e code review".to_string(),
            member_count: 12,
            is_private: false,
            unread_count: 0,
            last_message_at: "2026-11-11T13:15:00Z".to_string(),
        },
        Channel {
            id: "ch-seguranca".to_string(),
            name: "Segurança".to_string(),
            description: "Alertas e auditorias de segurança".to_string(),
            member_count: 5,
            is_private: true,
            unread_count: 7,
            last_message_at: "2026-11-11T15:00:00Z".to_string(),
        },
        Channel {
            id: "ch-anuncios".to_string(),
            name: "Anúncios".to_string(),
            description: "Comunicados oficiais".to_string(),
            member_count: 50,
            is_private: false,
            unread_count: 1,
            last_message_at: "2026-11-10T09:00:00Z".to_string(),
        },
    ]
}

fn mock_messages(channel_id: &str) -> Vec<ChannelMessage> {
    match channel_id {
        "ch-geral" => vec![
            ChannelMessage {
                id: "msg-001".to_string(),
                channel_id: "ch-geral".to_string(),
                author: "Ana Silva".to_string(),
                content: "Bom dia, equipe! Alguém viu o relatório de ontem?".to_string(),
                timestamp: "2026-11-11T09:00:00Z".to_string(),
            },
            ChannelMessage {
                id: "msg-002".to_string(),
                channel_id: "ch-geral".to_string(),
                author: "Carlos Mendes".to_string(),
                content: "Sim, já enviei para o repositório compartilhado.".to_string(),
                timestamp: "2026-11-11T09:05:00Z".to_string(),
            },
            ChannelMessage {
                id: "msg-003".to_string(),
                channel_id: "ch-geral".to_string(),
                author: "Beatriz Lima".to_string(),
                content: "Perfeito, vou revisar agora.".to_string(),
                timestamp: "2026-11-11T09:12:00Z".to_string(),
            },
        ],
        "ch-seguranca" => vec![
            ChannelMessage {
                id: "msg-010".to_string(),
                channel_id: "ch-seguranca".to_string(),
                author: "Sentinela".to_string(),
                content: "Alerta: tentativa de acesso não autorizado detectada.".to_string(),
                timestamp: "2026-11-11T14:55:00Z".to_string(),
            },
            ChannelMessage {
                id: "msg-011".to_string(),
                channel_id: "ch-seguranca".to_string(),
                author: "Admin".to_string(),
                content: "Investigando. IP bloqueado temporariamente.".to_string(),
                timestamp: "2026-11-11T15:00:00Z".to_string(),
            },
        ],
        _ => vec![
            ChannelMessage {
                id: "msg-empty".to_string(),
                channel_id: channel_id.to_string(),
                author: "Sistema".to_string(),
                content: "Nenhuma mensagem neste canal ainda.".to_string(),
                timestamp: "2026-11-11T00:00:00Z".to_string(),
            },
        ],
    }
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
    let loading = RwSignal::new(true);

    // Carregar canais do gateway
    spawn_local(async move {
        match fetch_json::<serde_json::Value>("http://localhost:3000/api/v1/channels").await {
            Ok(val) => {
                if let Some(arr) = val.get("channels").and_then(|v| v.as_array()) {
                    let data: Vec<Channel> = arr.iter().filter_map(|c| serde_json::from_value(c.clone()).ok()).collect();
                    channels.set(data.clone());
                    if let Some(first) = data.first() {
                        active_channel_id.set(first.id.clone());
                    }
                }
            }
            Err(_) => {
                // Fallback para mock se gateway indisponível
                let data = mock_channels();
                channels.set(data.clone());
                if let Some(first) = data.first() {
                    active_channel_id.set(first.id.clone());
                }
            }
        }
        loading.set(false);
    });

    // Carregar mensagens quando canal ativo muda
    Effect::new(move |_| {
        let cid = active_channel_id.get();
        if !cid.is_empty() {
            let msgs_signal = messages;
            let cid_clone = cid.clone();
            spawn_local(async move {
                let url = format!("http://localhost:3000/api/v1/channels/{}/messages", cid_clone);
                match fetch_json::<serde_json::Value>(&url).await {
                    Ok(val) => {
                        if let Some(arr) = val.get("messages").and_then(|v| v.as_array()) {
                            let data: Vec<ChannelMessage> = arr.iter().filter_map(|m| serde_json::from_value(m.clone()).ok()).collect();
                            msgs_signal.set(data);
                        }
                    }
                    Err(_) => {
                        msgs_signal.set(mock_messages(&cid_clone));
                    }
                }
            });
        }
    });

    let send_message = move || {
        let text = new_message_text.get_untracked();
        let cid = active_channel_id.get_untracked();
        if text.trim().is_empty() || cid.is_empty() {
            return;
        }
        let msg = ChannelMessage {
            id: format!("msg-new-{}", js_sys::Date::now() as u64),
            channel_id: cid,
            author: "Eu".to_string(),
            content: text.clone(),
            timestamp: js_sys::Date::new_0().to_iso_string().into(),
        };
        messages.update(|msgs| msgs.push(msg));
        new_message_text.set(String::new());
    };

    view! {
        <div class="channel-panel">
            <aside class="channel-sidebar">
                <h3>{"Canais"}</h3>
                <div class="channel-list">
                    <For
                        each=move || channels.get()
                        key=|ch| ch.id.clone()
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
                                    {move || {
                                        if ch_unread > 0 {
                                            view! {
                                                <span class="channel-unread-badge">{ch_unread}</span>
                                            }.into_any()
                                        } else {
                                            ().into_any()
                                        }
                                    }}
                                    <span class="channel-meta">
                                        {"\u{2022}"}{format!(" {} membros", ch_members)}
                                    </span>
                                </div>
                            }
                        }
                    />
                </div>
            </aside>

            <section class="channel-main">
                {move || {
                    if loading.get() {
                        view! {
                            <div class="channel-loading">
                                <p>{"Carregando canais..."}</p>
                            </div>
                        }.into_any()
                    } else {
                        let active_id = active_channel_id.get();
                        let active_channel = channels.get()
                            .iter()
                            .find(|c| c.id == active_id)
                            .cloned();
                        if let Some(ch) = active_channel {
                            let desc = ch.description.clone();
                            let ch_name = ch.name.clone();
                            view! {
                                <div class="channel-header">
                                    <h2>{"# "}{ch_name}</h2>
                                    <p class="channel-description">{desc}</p>
                                    <div class="channel-actions">
                                        <button
                                            class="channel-action-btn"
                                            on:click=move |_| {
                                                let cid = active_channel_id.get_untracked();
                                                if !cid.is_empty() {
                                                    spawn_local(async move {
                                                        let url = format!("http://localhost:3000/api/v1/channels/{}/join", cid);
                                                        let _ = post_json::<_, serde_json::Value>(&url, &serde_json::json!({"user_id": "current-user"})).await;
                                                    });
                                                }
                                            }
                                        >
                                            {"Entrar"}
                                        </button>
                                        <button
                                            class="channel-action-btn channel-action-leave"
                                            on:click=move |_| {
                                                let cid = active_channel_id.get_untracked();
                                                if !cid.is_empty() {
                                                    spawn_local(async move {
                                                        let url = format!("http://localhost:3000/api/v1/channels/{}/leave", cid);
                                                        let _ = post_json::<_, serde_json::Value>(&url, &serde_json::json!({"user_id": "current-user"})).await;
                                                    });
                                                }
                                            }
                                        >
                                            {"Sair"}
                                        </button>
                                    </div>
                                </div>
                                <div class="channel-messages">
                                    <For
                                        each=move || messages.get()
                                        key=|m| m.id.clone()
                                        children=move |msg| {
                                            let author = msg.author.clone();
                                            let content = msg.content.clone();
                                            let time = format_timestamp(&msg.timestamp);
                                            view! {
                                                <div class="channel-message">
                                                    <span class="message-author">{author}</span>
                                                    <span class="message-time">{time}</span>
                                                    <p class="message-content">{content}</p>
                                                </div>
                                            }
                                        }
                                    />
                                </div>
                                <div class="channel-input-area">
                                    <input
                                        class="channel-input"
                                        type="text"
                                        placeholder="Digite sua mensagem..."
                                        prop:value=move || new_message_text.get()
                                        on:input=move |ev| {
                                            new_message_text.set(event_target_value(&ev));
                                        }
                                        on:keydown=move |ev| {
                                            if ev.key() == "Enter" {
                                                send_message();
                                            }
                                        }
                                    />
                                    <button
                                        class="channel-send-btn"
                                        on:click=move |_| { send_message(); }
                                    >
                                        {"Enviar"}
                                    </button>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="channel-empty">
                                    <p>{"Selecione um canal para começar."}</p>
                                </div>
                            }.into_any()
                        }
                    }
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
    fn test_mock_channels_not_empty() {
        let channels = mock_channels();
        assert!(!channels.is_empty());
        assert!(channels.iter().all(|c| !c.id.is_empty()));
    }

    #[test]
    fn test_mock_messages_returns_data() {
        let msgs = mock_messages("ch-geral");
        assert!(!msgs.is_empty());
        assert!(msgs.iter().all(|m| m.channel_id == "ch-geral"));
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