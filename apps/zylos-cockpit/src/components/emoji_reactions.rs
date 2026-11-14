//! EmojiReactions — sistema de reações emoji em mensagens do chat
//!
//! Inspirado em: Slack/Discord reactions, OpenBot message reactions.
//! Permite adicionar/remover reações emoji em mensagens com contagem agregada.
//!
//! ## Features
//! - Picker de emojis frequentes
//! - Contagem de reações por emoji
//! - Toggle (add/remove) por usuário
//! - Persistência via API (placeholder)
//!
//! ## Related Documents
//! - Implementation Plan: [IMPLEMENTATION_PLAN_DETAILED.md](../../docs/IMPLEMENTATION_PLAN_DETAILED.md) §4.5

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// Uma reação emoji em uma mensagem
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reaction {
    pub emoji: String,
    pub count: usize,
    pub user_reacted: bool,
}

/// Emojis frequentes disponíveis no picker
const FREQUENT_EMOJIS: &[&str] = &[
    "👍", "👎", "❤️", "😂", "😮", "😢", "🔥", "🎉", "🤔", "👀", "✅", "❌", "⚡", "🚀", "💡", "🐛",
];

#[component]
pub fn EmojiReactions(
    #[prop(into)] _message_id: String,
    #[prop(default = vec![])] initial_reactions: Vec<Reaction>,
) -> impl IntoView {
    let reactions = RwSignal::new(initial_reactions);
    let show_picker = RwSignal::new(false);

    let toggle_reaction = move |emoji: String| {
        reactions.update(|rxns| {
            if let Some(r) = rxns.iter_mut().find(|r| r.emoji == emoji) {
                if r.user_reacted {
                    r.count = r.count.saturating_sub(1);
                    r.user_reacted = false;
                    if r.count == 0 {
                        rxns.retain(|r| r.emoji != emoji);
                    }
                } else {
                    r.count += 1;
                    r.user_reacted = true;
                }
            } else {
                rxns.push(Reaction {
                    emoji: emoji.clone(),
                    count: 1,
                    user_reacted: true,
                });
            }
        });
        show_picker.set(false);
    };

    view! {
    <div class="emoji-reactions">
    // Existing reactions
    <For
    each=move || reactions.get()
    key=|r| r.emoji.clone()
    children=move |reaction| {
    let emoji = reaction.emoji.clone();
    let count = reaction.count;
    let reacted = reaction.user_reacted;
    let css = if reacted {
    "reaction-chip reaction-active".to_string()
    } else {
    "reaction-chip".to_string()
    };

    view! {
    <button
    class=css
    on:click=move |_| {
    let e = emoji.clone();
    toggle_reaction(e);
    }
    >
    <span class="reaction-emoji">{emoji.clone()}</span>
    <span class="reaction-count">{count}</span>
    </button>
    }
    }
    />

    // Add reaction button
    <button
    class="reaction-add-btn"
    on:click=move |_| show_picker.update(|v| *v = !*v)
    title="Adicionar reação"
    >
    "+"
    </button>

    // Emoji picker popup
    {move || {
    if show_picker.get() {
    view! {
    <div class="emoji-picker">
    <For
    each=move || FREQUENT_EMOJIS.to_vec()
    key=|e| e.to_string()
    children=move |emoji| {
    let e = emoji.to_string();
    view! {
    <button
    class="emoji-picker-item"
    on:click=move |_| {
    let em = e.clone();
    toggle_reaction(em);
    }
    >
    {emoji.to_string()}
    </button>
    }
    }
    />
    </div>
    }.into_any()
    } else {
    view! { <div></div> }.into_any()
    }
    }}
    </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequent_emojis_not_empty() {
        assert!(!FREQUENT_EMOJIS.is_empty());
        assert!(FREQUENT_EMOJIS.len() >= 10);
    }

    #[test]
    fn test_reaction_serialization() {
        let r = Reaction {
            emoji: "👍".into(),
            count: 5,
            user_reacted: true,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("👍"));
        assert!(json.contains("5"));
    }
}
