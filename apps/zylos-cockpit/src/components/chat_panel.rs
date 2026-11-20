//! Chat Panel — Interface de conversa com bots via `/api/v1/bots/:nome/perguntar` (Fase 8).
//!
//! Padrão visual inspirado no OpenBot (nightly-labs/openbot):
//! - Sidebar esquerda com lista de bots disponíveis
//! - Área central de mensagens com bolhas diferenciadas (usuário vs bot)
//! - Input na parte inferior com envio via Enter ou botão
//! - Estados: Loading, Empty, Success, Error, Disabled
//!
//! Consome: `POST /api/v1/bots/:nome/perguntar` (base do gateway via `gateway_config`)
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
    /// O bot está respondendo uma pergunta agora (`estado = "trabalhando"` no gateway).
    #[serde(default)]
    pub working: bool,
}

/// Resposta de `GET /api/v1/bots`.
#[derive(Debug, Deserialize)]
struct BotsResp {
    bots: Vec<BotInfo>,
}

#[derive(Debug, Deserialize)]
struct BotInfo {
    nome: String,
    provedor: Option<String>,
    /// Nome para mostrar (do `bot.toml`); ausente = o nome da pasta.
    #[serde(default)]
    nome_exibicao: Option<String>,
    problema: Option<String>,
    /// `trabalhando` ou `ocioso`; ausente em gateway antigo (vira ocioso).
    #[serde(default)]
    estado: Option<String>,
}

/// Bots que podem responder: os com `problema` (sem ALMA.md, sem provedor...) ficam de fora.
fn bots_prontos(r: BotsResp) -> Vec<BotEntry> {
    r.bots
        .into_iter()
        .filter(|b| b.problema.is_none())
        .filter_map(|b| {
            Some(BotEntry {
                display_name: b.nome_exibicao.unwrap_or_else(|| b.nome.clone()),
                name: b.nome,
                provider: b.provedor?,
                working: b.estado.as_deref() == Some("trabalhando"),
            })
        })
        .collect()
}

/// Lê os bots do gateway.
async fn fetch_bots() -> Result<Vec<BotEntry>, String> {
    let r: BotsResp = crate::gateway_http::get_json("/api/v1/bots").await?;
    Ok(bots_prontos(r))
}

/// Agente escolhido e lista de agentes, compartilhados entre o chat e a barra lateral
/// (DESIGN_SYSTEM v2.1 §5.1: os agentes moram na lateral, o chat é uma coluna só).
#[derive(Clone, Copy)]
pub struct ChatCtx {
    pub selected_bot: RwSignal<Option<String>>,
    pub bots: RwSignal<Vec<BotEntry>>,
    /// Enquanto a lista não chegou (a tela mostra "carregando", não "nenhum bot").
    pub bots_loading: RwSignal<bool>,
    /// Falha ao ler a lista do gateway.
    pub bots_error: RwSignal<Option<String>>,
}

/// Cria o contexto na raiz do app.
pub fn provide_chat() -> ChatCtx {
    let live = crate::agent_state::use_agent_states();
    let ctx = ChatCtx {
        selected_bot: RwSignal::new(None),
        bots: RwSignal::new(Vec::new()),
        bots_loading: RwSignal::new(true),
        bots_error: RwSignal::new(None),
    };
    provide_context(ctx);
    spawn_local(async move {
        match fetch_bots().await {
            Ok(list) => ctx.bots.set(list),
            Err(e) => ctx
                .bots_error
                .set(Some(crate::ui_error::report("bots", &e))),
        }
        ctx.bots_loading.set(false);
        // O estado trabalhando/ocioso chega ao vivo pelo SSE (`agent_state`). Esta releitura é só
        // reconciliação: corrige a lista e zera o mapa ao vivo com o que o gateway diz agora,
        // para um evento perdido (queda do SSE) não deixar o bot preso em "trabalhando".
        // Falha na releitura mantém a lista anterior, sem acender o erro da tela.
        loop {
            gloo_timers::future::sleep(std::time::Duration::from_secs(BOTS_POLL_SECS)).await;
            if let Ok(list) = fetch_bots().await {
                if let Some(live) = live {
                    live.set(list.iter().map(|b| (b.name.clone(), b.working)).collect());
                }
                if ctx.bots.get_untracked() != list {
                    ctx.bots.set(list);
                }
            }
        }
    });
    ctx
}

/// De quanto em quanto tempo o cockpit reconcilia a lista de bots e o estado ao vivo com o gateway.
const BOTS_POLL_SECS: u64 = 30;

/// Contexto do chat, se o app o criou.
pub fn use_chat() -> Option<ChatCtx> {
    use_context::<ChatCtx>()
}

/// Iniciais para o avatar (até duas letras, maiúsculas).
pub fn initials(name: &str) -> String {
    name.chars().take(2).collect::<String>().to_uppercase()
}

fn now_ms() -> u64 {
    // `SystemTime::now()` dá panic no wasm ("time not implemented on this platform").
    js_sys::Date::now() as u64
}

async fn send_question(bot_name: &str, question: &str) -> Result<String, String> {
    let url = crate::gateway_http::api_url(&format!(
        "/api/v1/bots/{}/perguntar",
        crate::auth::url_encode(bot_name)
    ))?;
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

/// Mensagem guardada no gateway → mensagem do chat (`system` e desconhecidos viram Sistema).
fn to_chat(m: crate::chat_store::StoredMessage) -> ChatMessage {
    let role = match m.role.as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Bot,
        _ => MessageRole::System,
    };
    // RFC 3339 → ms; se não vier, 0 (a ordem já é a da lista).
    let timestamp_ms = js_sys::Date::parse(&m.created_at);
    ChatMessage {
        role,
        content: m.content,
        timestamp_ms: if timestamp_ms.is_nan() {
            0
        } else {
            timestamp_ms as u64
        },
        model: None,
    }
}

/// Conversa guardada do (Space, bot): id da thread e mensagens; `None` se ainda não começou.
async fn load_conversation(
    space: &str,
    bot: &str,
) -> Result<Option<(String, Vec<ChatMessage>)>, String> {
    let Some(thread) = crate::chat_store::load_thread(space, bot).await? else {
        return Ok(None);
    };
    let msgs = crate::chat_store::load_messages(&thread.id).await?;
    Ok(Some((thread.id, msgs.into_iter().map(to_chat).collect())))
}

/// Id da thread do (Space, bot): a já carregada ou uma nova, criada na primeira mensagem.
async fn ensure_thread(
    current: RwSignal<Option<String>>,
    space: &str,
    bot: &str,
    display: &str,
) -> Option<String> {
    if let Some(id) = current.get_untracked() {
        return Some(id);
    }
    match crate::chat_store::create_thread(space, bot, display).await {
        Ok(t) => {
            current.set(Some(t.id.clone()));
            Some(t.id)
        }
        Err(e) => {
            let _ = crate::ui_error::report("conversa", &e);
            None
        }
    }
}

/// Guarda a mensagem no gateway; falha não interrompe o chat, só acende o aviso.
async fn persist(thread: Option<&str>, role: &str, content: &str, warning: RwSignal<bool>) {
    let ok = match thread {
        Some(id) => match crate::chat_store::save_message(id, role, content).await {
            Ok(()) => true,
            Err(e) => {
                let _ = crate::ui_error::report("conversa", &e);
                false
            }
        },
        None => false,
    };
    if !ok {
        warning.set(true);
    }
}

/// Componente ChatPanel para o cockpit.
#[component]
pub fn ChatPanel() -> impl IntoView {
    let messages = RwSignal::new(Vec::<ChatMessage>::new());
    let shared = use_chat();
    let state = RwSignal::new(
        if shared.is_some_and(|c| c.selected_bot.get_untracked().is_some()) {
            ChatState::Active
        } else {
            ChatState::Empty
        },
    );
    let selected_bot = shared
        .map(|c| c.selected_bot)
        .unwrap_or_else(|| RwSignal::new(None::<String>));
    let input_text = RwSignal::new(String::new());
    let bots = shared
        .map(|c| c.bots)
        .unwrap_or_else(|| RwSignal::new(Vec::new()));
    let error_msg = RwSignal::new(None::<String>);
    // Conversa guardada no gateway: uma por (Space, bot).
    let thread_id = RwSignal::new(None::<String>);
    let save_warning = RwSignal::new(false);
    let spaces = crate::spaces::use_spaces();
    let active_space = move || {
        spaces
            .map(|s| s.active_id.get_untracked())
            .unwrap_or_else(|| crate::spaces::GENERAL_ID.to_string())
    };

    // Trocar de bot ou de Space carrega a conversa daquele par.
    Effect::new(move |_| {
        let bot = selected_bot.get();
        let space = spaces
            .map(|s| s.active_id.get())
            .unwrap_or_else(|| crate::spaces::GENERAL_ID.to_string());
        messages.set(Vec::new());
        thread_id.set(None);
        save_warning.set(false);
        let Some(bot) = bot else { return };
        spawn_local(async move {
            let loaded = load_conversation(&space, &bot).await;
            // Descarta o resultado se a seleção mudou enquanto carregava.
            let ainda_atual = selected_bot.get_untracked().as_deref() == Some(bot.as_str())
                && active_space() == space;
            if !ainda_atual {
                return;
            }
            match loaded {
                Ok(Some((id, msgs))) => {
                    thread_id.set(Some(id));
                    messages.set(msgs);
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = crate::ui_error::report("conversa", &e);
                }
            }
            state.set(ChatState::Active);
        });
    });

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

        let space = active_space();
        let display = bots
            .get_untracked()
            .iter()
            .find(|b| b.name == bot_name)
            .map(|b| b.display_name.clone())
            .unwrap_or_else(|| bot_name.clone());
        spawn_local(async move {
            // Mesma seleção de quando enviou? Senão a resposta só é guardada, não mostrada.
            let ainda_atual = || {
                selected_bot.get_untracked().as_deref() == Some(bot_name.as_str())
                    && active_space() == space
            };
            let tid = ensure_thread(thread_id, &space, &bot_name, &display).await;
            persist(tid.as_deref(), "user", &text, save_warning).await;
            match send_question(&bot_name, &text).await {
                Ok(answer) => {
                    persist(tid.as_deref(), "assistant", &answer, save_warning).await;
                    if ainda_atual() {
                        let bot_msg = ChatMessage {
                            role: MessageRole::Bot,
                            content: answer,
                            timestamp_ms: now_ms(),
                            model: None,
                        };
                        messages.update(|m| m.push(bot_msg));
                        state.set(ChatState::Active);
                    }
                }
                Err(e) => {
                    if ainda_atual() {
                        error_msg.set(Some(e.clone()));
                        state.set(ChatState::Error(e));
                    }
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

    // Título e subtítulo da página: nome do Space e quantos agentes têm acesso.
    let space_title = move || {
        spaces
            .and_then(|s| s.active())
            .map(|sp| sp.name)
            .unwrap_or_else(|| "Geral".to_string())
    };
    let space_subtitle = move || {
        let n = bots
            .get()
            .iter()
            .filter(|b| spaces.map(|s| s.allows(&b.name)).unwrap_or(true))
            .count();
        match (selected_bot.get(), n) {
            (Some(name), _) => bots
                .get()
                .iter()
                .find(|b| b.name == name)
                .map(|b| format!("Conversando com {}", b.display_name))
                .unwrap_or_default(),
            (None, 1) => "1 agente com acesso a este Space".to_string(),
            (None, n) => format!("{n} agentes com acesso a este Space"),
        }
    };

    view! {
        <div class="cockpit-layout">
            <div class="cockpit-main">
                // Título do Space e quem tem acesso (DESIGN_SYSTEM v2.1 §5.2)
                <div class="chat-header">
                    <h1 class="page-title">{move || space_title()}</h1>
                    <p class="sub">{move || space_subtitle()}</p>
                </div>

                // Escolha de agente em telas estreitas, onde a lateral some
                <div class="chat-agents" role="group" aria-label="Agentes">
                    {move || {
                        let spaces = crate::spaces::use_spaces();
                        bots.get().iter().filter(|b| spaces.map(|s| s.allows(&b.name)).unwrap_or(true)).map(|bot| {
                            let name = bot.name.clone();
                            let name_click = bot.name.clone();
                            let label = bot.display_name.clone();
                            view! {
                                <button
                                    class="btn"
                                    aria-pressed=move || (selected_bot.get().as_deref() == Some(name.as_str())).to_string()
                                    on:click=move |_| selected_bot.set(Some(name_click.clone()))
                                >{label}</button>
                            }
                        }).collect_view()
                    }}
                </div>

                // Messages area (design-system: .chat-messages)
                <div node_ref=scroll_ref class="chat-messages">
                    {move || {
                        match state.get() {
                            ChatState::Empty => view! {
                                <div class="empty-state">
                                    <div class="empty-state-icon"><i class="ph ph-chats-circle" aria-hidden="true"></i></div>
                                    <div class="empty-state-text">{move || {
                                        if bots.get().is_empty() {
                                            match shared.and_then(|c| c.bots_error.get()) {
                                                Some(e) => format!("Não foi possível ler os bots do gateway: {e}"),
                                                None if shared.is_some_and(|c| c.bots_loading.get()) => "Carregando os bots…".to_string(),
                                                None => "Nenhum bot configurado. Crie uma pasta com bot.toml e ALMA.md na pasta de bots do gateway.".to_string(),
                                            }
                                        } else {
                                            "Selecione um bot na barra lateral para iniciar uma conversa".to_string()
                                        }
                                    }}</div>
                                </div>
                            }.into_any(),
                            ChatState::Error(err) => view! {
                                <div class="empty-state">
                                    <div class="empty-state-icon"><i class="ph ph-x-circle" aria-hidden="true"></i></div>
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
                                            <div class="empty-state-icon"><i class="ph ph-hand-waving" aria-hidden="true"></i></div>
                                            <div class="empty-state-text">"Envie uma mensagem para começar"</div>
                                        </div>
                                    }.into_any()
                                } else {
                                    // O agente responde com o nome dele, não com "Bot".
                                    let bot_display = selected_bot
                                        .get()
                                        .and_then(|n| bots.get().iter().find(|b| b.name == n).map(|b| b.display_name.clone()))
                                        .unwrap_or_else(|| MessageRole::Bot.label().to_string());
                                    msgs.iter().map(|msg| {
                                        let role_label = if msg.role == MessageRole::Bot {
                                            bot_display.clone()
                                        } else {
                                            msg.role.label().to_string()
                                        };
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

                {move || save_warning.get().then(|| view! {
                    <div class="error-banner" role="status">
                        <p>"Não foi possível guardar esta conversa no gateway. Ela some ao recarregar."</p>
                    </div>
                })}

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
        </div>
    }
}

#[cfg(test)]
mod testes_bots {
    use super::*;

    #[test]
    fn so_bots_sem_problema_viram_agentes() {
        let r: BotsResp = serde_json::from_str(
            r#"{"bots":[
                {"nome":"vigia","provedor":"nvidia","modelo":"m","problema":null},
                {"nome":"sem-alma","provedor":null,"modelo":null,"problema":"falta o arquivo ALMA.md"}
            ]}"#,
        )
        .unwrap();
        let l = bots_prontos(r);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "vigia");
        assert_eq!(l[0].provider, "nvidia");
        assert!(!l[0].working, "gateway antigo (sem estado) = ocioso");
        assert_eq!(l[0].display_name, "vigia", "sem nome_exibicao: o da pasta");
    }

    #[test]
    fn nome_de_exibicao_vem_do_gateway() {
        let r: BotsResp = serde_json::from_str(
            r#"{"bots":[
                {"nome":"vigia","provedor":"nvidia","modelo":"m","problema":null,"nome_exibicao":"Vigia da Noite"}
            ]}"#,
        )
        .unwrap();
        let l = bots_prontos(r);
        assert_eq!(l[0].name, "vigia", "o identificador não muda");
        assert_eq!(l[0].display_name, "Vigia da Noite");
    }

    #[test]
    fn estado_trabalhando_vira_working() {
        let r: BotsResp = serde_json::from_str(
            r#"{"bots":[
                {"nome":"vigia","provedor":"nvidia","modelo":"m","problema":null,"estado":"trabalhando"},
                {"nome":"arq","provedor":"openai","modelo":"m","problema":null,"estado":"ocioso"}
            ]}"#,
        )
        .unwrap();
        let l = bots_prontos(r);
        assert!(l[0].working);
        assert!(!l[1].working);
    }
}
