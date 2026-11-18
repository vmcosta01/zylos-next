//! Reações emoji em mensagens, persistidas em SQLite.
//!
//! - `GET  /api/v1/messages/:id/reactions?user=<u>` — reações agregadas por emoji
//! - `POST /api/v1/messages/:id/reactions` `{emoji, user}` — alterna (adiciona/remove)
//!
//! Banco: `ZYLOS_REACTIONS_DB` (padrão `data/reactions.db`; `:memory:` em testes).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Tamanho máximo (bytes) de emoji, usuário e id de mensagem.
const MAX_FIELD: usize = 64;

/// Reação agregada de um emoji numa mensagem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReactionSummary {
    /// Emoji.
    pub emoji: String,
    /// Quantos usuários reagiram com ele.
    pub count: usize,
    /// Se o usuário consultado reagiu.
    pub user_reacted: bool,
    /// Usuários que reagiram.
    pub users: Vec<String>,
}

/// Store SQLite de reações.
pub struct ReactionStore {
    conn: Mutex<Connection>,
}

impl ReactionStore {
    /// Abre (ou cria) o banco. `":memory:"` abre em memória.
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = if path == ":memory:" {
            Connection::open_in_memory()?
        } else {
            if let Some(parent) = std::path::Path::new(path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            Connection::open(path)?
        };
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS reactions (
                message_id TEXT NOT NULL,
                emoji TEXT NOT NULL,
                user TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                PRIMARY KEY (message_id, emoji, user)
            );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Alterna a reação; devolve `true` se ficou adicionada.
    pub fn toggle(&self, message_id: &str, emoji: &str, user: &str) -> rusqlite::Result<bool> {
        let c = self.conn();
        let removed = c.execute(
            "DELETE FROM reactions WHERE message_id = ?1 AND emoji = ?2 AND user = ?3",
            params![message_id, emoji, user],
        )?;
        if removed > 0 {
            return Ok(false);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        c.execute(
            "INSERT INTO reactions (message_id, emoji, user, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![message_id, emoji, user, now],
        )?;
        Ok(true)
    }

    /// Reações agregadas, na ordem da primeira reação de cada emoji.
    pub fn list(
        &self,
        message_id: &str,
        user: Option<&str>,
    ) -> rusqlite::Result<Vec<ReactionSummary>> {
        let c = self.conn();
        let mut st = c.prepare(
            "SELECT emoji, user FROM reactions WHERE message_id = ?1 ORDER BY created_at, rowid",
        )?;
        let rows = st.query_map(params![message_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut order: Vec<String> = Vec::new();
        let mut by_emoji: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (emoji, u) = row?;
            if !by_emoji.contains_key(&emoji) {
                order.push(emoji.clone());
            }
            by_emoji.entry(emoji).or_default().push(u);
        }
        Ok(order
            .into_iter()
            .map(|emoji| {
                let users = by_emoji.remove(&emoji).unwrap_or_default();
                ReactionSummary {
                    user_reacted: user.is_some_and(|u| users.iter().any(|x| x == u)),
                    count: users.len(),
                    users,
                    emoji,
                }
            })
            .collect())
    }
}

/// Estado do router de reações.
#[derive(Clone)]
pub struct ReactionsState {
    /// Store compartilhado.
    pub store: Arc<ReactionStore>,
}

impl ReactionsState {
    /// Abre `ZYLOS_REACTIONS_DB` (padrão `data/reactions.db`); se falhar, cai para memória.
    pub fn from_env() -> Self {
        let path =
            std::env::var("ZYLOS_REACTIONS_DB").unwrap_or_else(|_| "data/reactions.db".into());
        let store = ReactionStore::open(&path).unwrap_or_else(|e| {
            tracing::warn!(error = ?e, path = %path, "reactions: falha ao abrir store, usando memória");
            ReactionStore::open(":memory:").expect("sqlite em memória")
        });
        Self {
            store: Arc::new(store),
        }
    }

    /// Store em memória (testes).
    pub fn in_memory() -> Self {
        Self {
            store: Arc::new(ReactionStore::open(":memory:").expect("sqlite em memória")),
        }
    }
}

/// Corpo do toggle.
#[derive(Debug, Deserialize)]
pub struct ToggleRequest {
    /// Emoji.
    pub emoji: String,
    /// Usuário que reage.
    pub user: String,
}

/// Query do GET.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Usuário para calcular `user_reacted`.
    pub user: Option<String>,
}

fn bad(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response()
}

fn db_err(e: rusqlite::Error) -> Response {
    tracing::error!(error = ?e, "reactions: erro de banco");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "falha no banco de reações" })),
    )
        .into_response()
}

fn valid(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty() && t.len() <= MAX_FIELD
}

async fn get_reactions(
    State(st): State<ReactionsState>,
    Path(id): Path<String>,
    Query(q): Query<ListQuery>,
) -> Response {
    if !valid(&id) {
        return bad("id de mensagem inválido");
    }
    match st.store.list(&id, q.user.as_deref()) {
        Ok(r) => Json(json!({ "message_id": id, "reactions": r })).into_response(),
        Err(e) => db_err(e),
    }
}

async fn post_reaction(
    State(st): State<ReactionsState>,
    Path(id): Path<String>,
    Json(req): Json<ToggleRequest>,
) -> Response {
    if !valid(&id) {
        return bad("id de mensagem inválido");
    }
    if !valid(&req.emoji) || !valid(&req.user) {
        return bad("emoji e user são obrigatórios (até 64 bytes)");
    }
    let emoji = req.emoji.trim();
    let user = req.user.trim();
    let added = match st.store.toggle(&id, emoji, user) {
        Ok(a) => a,
        Err(e) => return db_err(e),
    };
    match st.store.list(&id, Some(user)) {
        Ok(r) => Json(json!({ "message_id": id, "added": added, "reactions": r })).into_response(),
        Err(e) => db_err(e),
    }
}

/// Router `/messages/:id/reactions` (montar dentro de `nest("/api/v1", ...)`).
pub fn create_reactions_router(state: ReactionsState) -> Router {
    Router::new()
        .route(
            "/messages/:id/reactions",
            get(get_reactions).post(post_reaction),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<&str>,
    ) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(
                body.map(|s| Body::from(s.to_string()))
                    .unwrap_or_else(Body::empty),
            )
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
        )
    }

    #[test]
    fn toggle_adiciona_e_remove() {
        let s = ReactionStore::open(":memory:").unwrap();
        assert!(s.toggle("m1", "👍", "ana").unwrap());
        assert!(s.toggle("m1", "👍", "bia").unwrap());
        let r = s.list("m1", Some("ana")).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].count, 2);
        assert!(r[0].user_reacted);
        assert!(!s.toggle("m1", "👍", "ana").unwrap());
        let r = s.list("m1", Some("ana")).unwrap();
        assert_eq!(r[0].count, 1);
        assert!(!r[0].user_reacted);
        assert!(s.list("m2", None).unwrap().is_empty());
    }

    #[test]
    fn persiste_em_arquivo() {
        let dir = std::env::temp_dir().join(format!("zylos-rx-{}", std::process::id()));
        let path = dir.join("sub").join("reactions.db");
        let p = path.to_str().unwrap();
        {
            let s = ReactionStore::open(p).unwrap();
            s.toggle("m1", "🔥", "ana").unwrap();
        }
        let s = ReactionStore::open(p).unwrap();
        assert_eq!(s.list("m1", None).unwrap()[0].emoji, "🔥");
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn rotas_get_post_toggle() {
        let app = create_reactions_router(ReactionsState::in_memory());
        let (st, v) = call(&app, "GET", "/messages/m1/reactions", None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["reactions"].as_array().unwrap().len(), 0);

        let body = r#"{"emoji":"🚀","user":"ana"}"#;
        let (st, v) = call(&app, "POST", "/messages/m1/reactions", Some(body)).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["added"], true);
        assert_eq!(v["reactions"][0]["count"], 1);
        assert_eq!(v["reactions"][0]["user_reacted"], true);

        let (_, v) = call(&app, "GET", "/messages/m1/reactions?user=bia", None).await;
        assert_eq!(v["reactions"][0]["user_reacted"], false);

        let (_, v) = call(&app, "POST", "/messages/m1/reactions", Some(body)).await;
        assert_eq!(v["added"], false);
        assert_eq!(v["reactions"].as_array().unwrap().len(), 0);

        let (st, _) = call(
            &app,
            "POST",
            "/messages/m1/reactions",
            Some(r#"{"emoji":"","user":"ana"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }
}
