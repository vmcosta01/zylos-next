//! Conversas dos bots guardadas no gateway, uma por (Space, bot).
//!
//! Cada Space tem a sua conversa com cada bot: a thread vive em `/api/v1/threads` com
//! `space_id` e `bot_id`. O Geral também tem conversas próprias; a lista dele mostra tudo,
//! por isso `find_thread` compara o `space_id` da thread, não o filtro da lista.

use serde::{Deserialize, Serialize};

use crate::components::thread_manager::Thread;

/// Mensagem de uma thread como devolvida pelo gateway.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StoredMessage {
    /// `user`, `assistant` ou `system`.
    pub role: String,
    pub content: String,
    /// RFC 3339.
    #[serde(default)]
    pub created_at: String,
}

#[derive(Deserialize)]
struct ThreadsResp {
    threads: Vec<Thread>,
}

#[derive(Deserialize)]
struct MessagesResp {
    messages: Vec<StoredMessage>,
}

#[derive(Serialize)]
struct NewThread<'a> {
    name: &'a str,
    bot_id: &'a str,
    space_id: &'a str,
}

#[derive(Serialize)]
struct NewMessage<'a> {
    role: &'a str,
    content: &'a str,
}

/// A thread (mais antiga) deste bot dentro deste Space, se existir.
pub fn find_thread<'a>(threads: &'a [Thread], space: &str, bot: &str) -> Option<&'a Thread> {
    threads
        .iter()
        .filter(|t| t.space_id == space && t.bot_id == bot)
        .min_by_key(|t| t.position)
}

/// Lê a thread do (Space, bot); `None` se a conversa ainda não começou.
pub async fn load_thread(space: &str, bot: &str) -> Result<Option<Thread>, String> {
    let path = format!("/api/v1/threads?space={}", crate::auth::url_encode(space));
    let r: ThreadsResp = crate::gateway_http::get_json(&path).await?;
    Ok(find_thread(&r.threads, space, bot).cloned())
}

/// Cria a thread do (Space, bot).
pub async fn create_thread(space: &str, bot: &str, name: &str) -> Result<Thread, String> {
    crate::gateway_http::post_json(
        "/api/v1/threads",
        &NewThread {
            name,
            bot_id: bot,
            space_id: space,
        },
    )
    .await
}

/// Mensagens da thread, da mais antiga para a mais nova.
pub async fn load_messages(thread_id: &str) -> Result<Vec<StoredMessage>, String> {
    let path = format!(
        "/api/v1/threads/{}/messages",
        crate::auth::url_encode(thread_id)
    );
    let r: MessagesResp = crate::gateway_http::get_json(&path).await?;
    Ok(r.messages)
}

/// Guarda uma mensagem na thread.
pub async fn save_message(thread_id: &str, role: &str, content: &str) -> Result<(), String> {
    let path = format!(
        "/api/v1/threads/{}/messages",
        crate::auth::url_encode(thread_id)
    );
    let _: serde_json::Value =
        crate::gateway_http::post_json(&path, &NewMessage { role, content }).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, space: &str, bot: &str, pos: i64) -> Thread {
        Thread {
            id: id.into(),
            name: id.into(),
            bot_id: bot.into(),
            position: pos,
            message_count: 0,
            last_message_at: None,
            space_id: space.into(),
        }
    }

    #[test]
    fn acha_so_a_thread_do_space_e_do_bot() {
        let v = vec![
            t("a", "juridico", "default", 1),
            t("b", "geral", "default", 2),
            t("c", "geral", "dev-agent", 3),
        ];
        assert_eq!(find_thread(&v, "geral", "default").unwrap().id, "b");
        assert_eq!(find_thread(&v, "juridico", "default").unwrap().id, "a");
        assert!(find_thread(&v, "juridico", "dev-agent").is_none());
    }

    #[test]
    fn mais_de_uma_usa_a_mais_antiga() {
        let v = vec![t("novo", "ops", "x", 5), t("velho", "ops", "x", 1)];
        assert_eq!(find_thread(&v, "ops", "x").unwrap().id, "velho");
    }

    #[test]
    fn desserializa_mensagens_do_gateway() {
        let r: MessagesResp = serde_json::from_str(
            r#"{"messages":[{"id":"1","thread_id":"t","role":"user","content":"oi","created_at":"2026-01-01T00:00:00Z"}]}"#,
        )
        .unwrap();
        assert_eq!(r.messages[0].role, "user");
        assert_eq!(r.messages[0].content, "oi");
    }
}
