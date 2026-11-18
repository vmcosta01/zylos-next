//! EmojiReactions — reações emoji em mensagens, persistidas no gateway.
//!
//! Consome `apps/zylos-gateway/src/reactions_api.rs`:
//! - `GET  /api/v1/messages/:id/reactions?user=<u>` — carrega reações agregadas
//! - `POST /api/v1/messages/:id/reactions` `{emoji, user}` — alterna a reação do usuário
//!
//! O usuário é [`crate::auth::current_user`]; a lista exibida é sempre a devolvida
//! pelo gateway (sem atualização otimista).

use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};

use crate::gateway_http::{get_json, post_json};
use crate::ui_error::user_message;

/// Uma reação emoji agregada em uma mensagem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reaction {
    /// Emoji.
    pub emoji: String,
    /// Quantos usuários reagiram.
    pub count: usize,
    /// Se o usuário atual reagiu.
    pub user_reacted: bool,
}

#[derive(Debug, Deserialize)]
struct ReactionsResponse {
    reactions: Vec<Reaction>,
}

/// Emojis frequentes disponíveis no picker
const FREQUENT_EMOJIS: &[&str] = &[
    "👍", "👎", "❤️", "😂", "😮", "😢", "🔥", "🎉", "🤔", "👀", "✅", "❌", "⚡", "🚀", "💡", "🐛",
];

/// Caminho da API de reações para uma mensagem.
pub fn reactions_path(message_id: &str) -> String {
    format!(
        "/api/v1/messages/{}/reactions",
        crate::auth::url_encode(message_id)
    )
}

/// Reações de uma mensagem, carregadas e alternadas via gateway.
#[component]
pub fn EmojiReactions(#[prop(into)] message_id: String) -> impl IntoView {
    let reactions = RwSignal::new(Vec::<Reaction>::new());
    let show_picker = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let path = StoredValue::new(reactions_path(&message_id));

    spawn_local(async move {
        let user = crate::auth::current_user();
        let url = format!(
            "{}?user={}",
            path.get_value(),
            crate::auth::url_encode(&user)
        );
        match get_json::<ReactionsResponse>(&url).await {
            Ok(r) => reactions.set(r.reactions),
            Err(e) => error.set(Some(user_message(&e))),
        }
    });

    let toggle_reaction = move |emoji: String| {
        show_picker.set(false);
        spawn_local(async move {
            let body = serde_json::json!({ "emoji": emoji, "user": crate::auth::current_user() });
            match post_json::<_, ReactionsResponse>(&path.get_value(), &body).await {
                Ok(r) => {
                    error.set(None);
                    reactions.set(r.reactions);
                }
                Err(e) => error.set(Some(user_message(&e))),
            }
        });
    };

    view! {
        <div class="emoji-reactions">
            <For
                each=move || reactions.get()
                key=|r| (r.emoji.clone(), r.count, r.user_reacted)
                children=move |reaction| {
                    let emoji = reaction.emoji.clone();
                    let css = if reaction.user_reacted {
                        "reaction-chip reaction-active"
                    } else {
                        "reaction-chip"
                    };
                    view! {
                        <button class=css on:click=move |_| toggle_reaction(emoji.clone())>
                            <span class="reaction-emoji">{reaction.emoji.clone()}</span>
                            <span class="reaction-count">{reaction.count}</span>
                        </button>
                    }
                }
            />
            <button
                class="reaction-add-btn"
                on:click=move |_| show_picker.update(|v| *v = !*v)
                title="Adicionar reação"
            >
                "+"
            </button>
            {move || {
                show_picker.get().then(|| view! {
                    <div class="emoji-picker">
                        {FREQUENT_EMOJIS
                            .iter()
                            .map(|e| {
                                let em = e.to_string();
                                view! {
                                    <button class="emoji-picker-item" on:click=move |_| toggle_reaction(em.clone())>
                                        {*e}
                                    </button>
                                }
                            })
                            .collect_view()}
                    </div>
                })
            }}
            {move || error.get().map(|e| view! { <span class="reaction-error" role="alert">{e}</span> })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequent_emojis_not_empty() {
        assert!(FREQUENT_EMOJIS.len() >= 10);
    }

    #[test]
    fn test_parse_gateway_payload() {
        let raw = r#"{"message_id":"m1","added":true,"reactions":[{"emoji":"👍","count":2,"user_reacted":true,"users":["a","b"]}]}"#;
        let r: ReactionsResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.reactions.len(), 1);
        assert_eq!(r.reactions[0].count, 2);
        assert!(r.reactions[0].user_reacted);
    }

    #[test]
    fn test_reactions_path_encodes_id() {
        assert_eq!(reactions_path("a b"), "/api/v1/messages/a%20b/reactions");
    }
}
