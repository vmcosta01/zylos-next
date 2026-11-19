//! Chat Panel — Interface de conversa com bots via `/api/v1/bots/:nome/perguntar` (Fase 8).
//!
//! Padrão visual inspirado no OpenBot (nightly-labs/openbot):
//! - Sidebar esquerda com lista de bots disponíveis
//! - Área central de mensagens com bolhas diferenciadas (usuário vs bot)
//! - Input na parte inferior com envio via Enter ou botão
//! - Estados: Loading, Empty, Success, Error, Disabled
//!
//! Consome: `POST http://localhost:3000/api/v1/bots/:nome/perguntar`
//! Request:  `{ "pergunta": "..." }`
//! Response: `{ "resposta": "...", "modelo": "...", "tokens": N }` ou erro RFC 9457

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Uma mensagem individual no chat.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp_ms: u64,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Bot,
    System,
}

impl MessageRole {
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "Você",
            Self::Bot => "Bot",
            Self::System => "Sistema",
        }
    }

    #[allow(dead_code)]
    pub fn css_class(&self) -> &'static str {
        match self {
            Self::User => "msg-user",
            Self::Bot => "msg-bot",
            Self::System => "msg-system",
        }
    }
}

/// Estado do painel de chat.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatState {
    /// Nenhum bot selecionado ainda.
    Empty,
    /// Aguardando resposta do bot.
    Loading,
    /// Chat ativo com mensagens.
    Active,
    /// Erro de conexão ou API.
    Error(String),
}

/// Entrada de bot disponível na sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BotEntry {
    pub name: String,
    pub display_name: String,
    pub provider: String,
}

fn default_bots() -> Vec<BotEntry> {
    vec![
        BotEntry {
            name: "default".into(),
            display_name: "Assistente Zylos".into(),
            provider: "openai".into(),
        },
        BotEntry {
            name: "dev-agent".into(),
            display_name: "Dev Agent".into(),
            provider: "anthropic".into(),
        },
        BotEntry {
            name: "researcher".into(),
            display_name: "Pesquisador".into(),
            provider: "google".into(),
        },
    ]
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn send_question(bot_name: &str, question: &str) -> Result<String, String> {
    let url = format!("http://localhost:3000/api/v1/bots/{}/perguntar", bot_name);
    let body = serde_json::json!({ "pergunta": question });
    let resp = crate::auth::authed(gloo_net::http::Request::post(&url))
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).map_err(|e| e.to_string())?)
        .map_err(|e| format!("request error: {e}"))?
        .send()
        .await
        .map_err(|e| format!("fetch error: {e}"))?;

    if !resp.ok() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {}: {}", status, text));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("json parse: {e}"))?;
    json.get("resposta")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Resposta sem campo 'resposta'".into())
}

/// Componente ChatPanel para o cockpit.
#[component]
pub fn ChatPanel() -> impl IntoView {
    let messages = RwSignal::new(Vec::<ChatMessage>::new());
    let state = RwSignal::new(ChatState::Empty);
    let selected_bot = RwSignal::new(None::<String>);
    let input_text = RwSignal::new(String::new());
    let bots = RwSignal::new(default_bots());
    let error_msg = RwSignal::new(None::<String>);

    // Scroll to bottom when messages change
    let scroll_ref = NodeRef::<leptos::html::Div>::new();
    Effect::new(move |_| {
        let _ = messages.get(); // trigger on change
        if let Some(el) = scroll_ref.get() {
            el.set_scroll_top(el.scroll_height());
        }
    });

    let handle_send = move || {
        let text = input_text.get_untracked();
        let bot = selected_bot.get_untracked();
        if text.trim().is_empty() || bot.is_none() {
            return;
        }
        let bot_name = bot.unwrap();
        let user_msg = ChatMessage {
            role: MessageRole::User,
            content: text.clone(),
            timestamp_ms: now_ms(),
            model: None,
        };
        messages.update(|m| m.push(user_msg));
        input_text.set(String::new());
        state.set(ChatState::Loading);
        error_msg.set(None);

        spawn_local(async move {
            match send_question(&bot_name, &text).await {
                Ok(answer) => {
                    let bot_msg = ChatMessage {
                        role: MessageRole::Bot,
                        content: answer,
                        timestamp_ms: now_ms(),
                        model: None,
                    };
                    messages.update(|m| m.push(bot_msg));
                    state.set(ChatState::Active);
                }
                Err(e) => {
                    error_msg.set(Some(e.clone()));
                    state.set(ChatState::Error(e));
                }
            }
        });
    };

    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        if ev.key() == "Enter" && !ev.shift_key() {
            ev.prevent_default();
            handle_send();
        }
    };

    view! {
        <div class="cockpit-layout">
            // Sidebar: agent/bot list (design-system: .cockpit-sidebar)
            <div class="cockpit-sidebar">
                <div class="sidebar-header">
                    <span>"💬"</span>
                    <span>"Bots"</span>
                </div>

                <div class="sidebar-section">
                    <div class="sidebar-section-title">"Agentes"</div>
                    {move || {
                        bots.get().iter().map(|bot| {
                            let name = bot.name.clone();
                            let display = bot.display_name.clone();
                            let provider = bot.provider.clone();
                            let name_for_click = name.clone();
                            let is_selected = move || selected_bot.get().as_deref() == Some(&name);
                            let initials: String = display.chars().take(2).collect::<String>().to_uppercase();
                            view! {
                                <div
                                    class={move || {
                                        if is_selected() { "sidebar-item active" } else { "sidebar-item" }
                                    }}
                                    on:click=move |_| {
                                        selected_bot.set(Some(name_for_click.clone()));
                                        if messages.get_untracked().is_empty() {
                                            state.set(ChatState::Active);
                                        }
                                    }
                                    role="button"
                                    tabindex="0"
                                >
                                    <div class="sidebar-avatar">
                                        {initials.clone()}
                                        <div class="status-dot success"></div>
                                    </div>
                                    <div class="sidebar-item-info">
                                        <div class="sidebar-item-name">{display.clone()}</div>
                                        <div class="sidebar-item-meta">{provider.clone()}</div>
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </div>
            </div>

            // Main chat area (design-system: .cockpit-main)
            <div class="cockpit-main">
                // Chat header
                <div class="chat-header">
                    {move || {
                        if let Some(bot_name) = selected_bot.get() {
                            let bot = bots.get().iter().find(|b| b.name == bot_name).cloned();
                            if let Some(b) = bot {
                                let initials: String = b.display_name.chars().take(2).collect::<String>().to_uppercase();
                                view! {
                                    <div class="sidebar-avatar" style="width:28px;height:28px;font-size:11px;">
                                        {initials}
                                    </div>
                                    <span style="font-weight:600;">{b.display_name}</span>
                                    <span style="font-size:var(--text-xs);color:var(--text-secondary);padding:2px 8px;border-radius:var(--radius-full);background:var(--bg-elevated);">{b.provider}</span>
                                }.into_any()
                            } else {
                                view! { <span style="color:var(--text-secondary);">"Bot desconhecido"</span> }.into_any()
                            }
                        } else {
                            view! { <span style="color:var(--text-secondary);">"Selecione um bot para começar"</span> }.into_any()
                        }
                    }}
                </div>

                // Messages area (design-system: .chat-messages)
                <div node_ref=scroll_ref class="chat-messages">
                    {move || {
                        match state.get() {
                            ChatState::Empty => view! {
                                <div class="empty-state">
                                    <div class="empty-state-icon">"💬"</div>
                                    <div class="empty-state-text">"Selecione um bot na barra lateral para iniciar uma conversa"</div>
                                </div>
                            }.into_any(),
                            ChatState::Error(err) => view! {
                                <div class="empty-state">
                                    <div class="empty-state-icon">"❌"</div>
                                    <div class="empty-state-text" style="color:var(--status-error);">{err}</div>
                                    <button
                                        class="btn btn-primary"
                                        style="margin-top:var(--space-3);"
                                        on:click=move |_| {
                                            state.set(ChatState::Active);
                                            error_msg.set(None);
                                        }
                                    >"Tentar novamente"</button>
                                </div>
                            }.into_any(),
                            _ => {
                                let msgs = messages.get();
                                if msgs.is_empty() && matches!(state.get(), ChatState::Active) {
                                    view! {
                                        <div class="empty-state">
                                            <div class="empty-state-icon">"👋"</div>
                                            <div class="empty-state-text">"Envie uma mensagem para começar"</div>
                                        </div>
                                    }.into_any()
                                } else {
                                    msgs.iter().map(|msg| {
                                        let role_label = msg.role.label();
                                        let content = msg.content.clone();
                                        let is_user = msg.role == MessageRole::User;
                                        let is_system = msg.role == MessageRole::System;
                                        let css_role = if is_user { "user" } else if is_system { "system" } else { "agent" };
                                        let initials: String = role_label.chars().take(2).collect::<String>().to_uppercase();
                                        view! {
                                            <div class={format!("message {}", css_role)}>
                                                {if !is_user && !is_system {
                                                    view! {
                                                        <div class="message-header">
                                                            <div class="message-avatar">{initials.clone()}</div>
                                                            <span>{role_label}</span>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    ().into_any()
                                                }}
                                                <div class="message-bubble">
                                                    {content}
                                                </div>
                                            </div>
                                        }
                                    }).collect_view().into_any()
                                }
                            }
                        }
                    }}

                    // Typing indicator (design-system: .typing-indicator)
                    {move || {
                        if matches!(state.get(), ChatState::Loading) {
                            view! {
                                <div class="typing-indicator">
                                    <div class="typing-dots">
                                        <div class="typing-dot"></div>
                                        <div class="typing-dot"></div>
                                        <div class="typing-dot"></div>
                                    </div>
                                    <span>"Pensando..."</span>
                                </div>
                            }.into_any()
                        } else {
                            ().into_any()
                        }
                    }}
                </div>

                // Aprovações HITL pendentes inline no chat (ADR-001)
                <super::hitl_approval_panel::InlineApprovals />

                // Computador do agente inline (ADR-003)
                <super::inline_computer::InlineComputers />

                // Input area (design-system: .chat-input-area)
                <div class="chat-input-area">
                    {move || {
                        if selected_bot.get().is_some() {
                            let disabled = matches!(state.get(), ChatState::Loading);
                            view! {
                                <div class="chat-input-wrapper">
                                    <textarea
                                        class="chat-textarea"
                                        aria-label="Mensagem para o bot"
                                        placeholder="Digite sua mensagem... (Enter para enviar, Shift+Enter para nova linha)"
                                        rows="2"
                                        prop:value={move || input_text.get()}
                                        on:input=move |ev| {
                                            input_text.set(event_target_value(&ev));
                                        }
                                        on:keydown=handle_keydown
                                        disabled={move || disabled}
                                    />
                                    <button
                                        class="chat-send-btn"
                                        on:click=move |_| handle_send()
                                        disabled={move || disabled || input_text.get().trim().is_empty()}
                                    >"Enviar"</button>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div style="text-align:center;font-size:var(--text-sm);color:var(--text-secondary);padding:var(--space-2);">
                                    "Selecione um bot para enviar mensagens"
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            // Context panel (design-system: .cockpit-context) — shows selected bot details
            <div class="cockpit-context">
                <div class="context-header">"Detalhes"</div>
                <div class="context-content">
                    {move || {
                        if let Some(bot_name) = selected_bot.get() {
                            let bot = bots.get().iter().find(|b| b.name == bot_name).cloned();
                            if let Some(b) = bot {
                                let msg_count = messages.get().len();
                                view! {
                                    <div class="context-section">
                                        <div class="context-section-title">"Agente"</div>
                                        <div class="context-card">
                                            <div class="context-label">"Nome"</div>
                                            <div class="context-value">{b.display_name.clone()}</div>
                                        </div>
                                        <div class="context-card">
                                            <div class="context-label">"Provedor"</div>
                                            <div class="context-value">{b.provider.clone()}</div>
                                        </div>
                                        <div class="context-card">
                                            <div class="context-label">"ID"</div>
                                            <div class="context-value" style="font-family:var(--font-mono);font-size:var(--text-xs);">{b.name.clone()}</div>
                                        </div>
                                    </div>
                                    <div class="context-section">
                                        <div class="context-section-title">"Sessão"</div>
                                        <div class="context-card">
                                            <div class="context-label">"Mensagens"</div>
                                            <div class="context-value">{msg_count.to_string()}</div>
                                        </div>
                                        <div class="context-card">
                                            <div class="context-label">"Status"</div>
                                            <div class="context-value" style="color:var(--status-success);">"● Ativo"</div>
                                        </div>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <div class="empty-state" style="padding:var(--space-4);">
                                        <div class="empty-state-text">"Bot não encontrado"</div>
                                    </div>
                                }.into_any()
                            }
                        } else {
                            view! {
                                <div class="empty-state" style="padding:var(--space-4);">
                                    <div class="empty-state-icon" style="font-size:32px;">"📋"</div>
                                    <div class="empty-state-text">"Selecione um agente para ver detalhes"</div>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>
        </div>
    }
}
