//! Threads API — persistência real de threads de chat em SQLite.
//!
//! Arquivo: env `ZYLOS_THREADS_DB` (padrão `data/threads.db`; o diretório é
//! criado se faltar). `:memory:` funciona para testes.
//!
//! - `GET    /api/v1/threads` — lista por `position`
//! - `POST   /api/v1/threads` — cria (`{"name", "bot_id"?}`)
//! - `PATCH  /api/v1/threads/:id` — renomeia e/ou reposiciona (`{"name"?, "position"?}`)
//! - `DELETE /api/v1/threads/:id` — apaga a thread e suas mensagens
//! - `GET    /api/v1/threads/:id/messages` — mensagens em ordem cronológica
//! - `POST   /api/v1/threads/:id/messages` — adiciona (`{"role", "content"}`)

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::{
    extract::{Path as UrlPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch},
    Json, Router,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Space que enxerga todas as conversas (espelha `spaces_api::GENERAL_ID`).
const GENERAL_SPACE: &str = "geral";

/// Thread como devolvida pela API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadView {
    /// ID (UUID v4).
    pub id: String,
    /// Nome exibido.
    pub name: String,
    /// Bot associado (vazio se nenhum).
    pub bot_id: String,
    /// Posição na lista (0 = topo).
    pub position: i64,
    /// Número de mensagens.
    pub message_count: usize,
    /// Última mensagem (RFC 3339), se houver.
    pub last_message_at: Option<String>,
    /// Criação (RFC 3339).
    pub created_at: String,
    /// Space dono da conversa (`geral` por padrão).
    pub space_id: String,
}

/// Mensagem de uma thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageView {
    /// ID (UUID v4).
    pub id: String,
    /// Thread dona.
    pub thread_id: String,
    /// `user`, `assistant` ou `system`.
    pub role: String,
    /// Texto.
    pub content: String,
    /// Criação (RFC 3339).
    pub created_at: String,
}

/// Erros do store de threads.
#[derive(Debug)]
pub enum ThreadsError {
    /// Thread inexistente.
    NotFound,
    /// Entrada inválida.
    Invalid(String),
    /// Falha do SQLite.
    Db(rusqlite::Error),
}

impl From<rusqlite::Error> for ThreadsError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Db(e)
    }
}

/// Store SQLite de threads (conexão única protegida por mutex).
#[derive(Clone)]
pub struct ThreadsStore {
    conn: Arc<Mutex<Connection>>,
}

const MAX_NAME: usize = 200;
const MAX_CONTENT: usize = 64 * 1024;

impl ThreadsStore {
    /// Abre (criando o diretório pai se preciso) e roda as migrações.
    pub fn open(path: &str) -> Result<Self, ThreadsError> {
        if path != ":memory:" {
            if let Some(dir) = Path::new(path)
                .parent()
                .filter(|d| !d.as_os_str().is_empty())
            {
                std::fs::create_dir_all(dir).map_err(|e| {
                    ThreadsError::Invalid(format!("não criou {}: {e}", dir.display()))
                })?;
            }
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            r#"PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS chat_threads (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                bot_id TEXT NOT NULL DEFAULT '',
                position INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                space_id TEXT NOT NULL DEFAULT 'geral'
            );
            CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_chat_messages_thread
                ON chat_messages(thread_id, created_at);"#,
        )?;
        // Bancos criados antes dos Spaces não têm a coluna.
        let tem_space: bool = conn
            .prepare("SELECT 1 FROM pragma_table_info('chat_threads') WHERE name = 'space_id'")?
            .exists([])?;
        if !tem_space {
            conn.execute(
                "ALTER TABLE chat_threads ADD COLUMN space_id TEXT NOT NULL DEFAULT 'geral'",
                [],
            )?;
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Abre o arquivo de `ZYLOS_THREADS_DB` (padrão `data/threads.db`).
    pub fn from_env() -> Result<Self, ThreadsError> {
        let path =
            std::env::var("ZYLOS_THREADS_DB").unwrap_or_else(|_| "data/threads.db".to_string());
        Self::open(&path)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn get_with(conn: &Connection, id: &str) -> Result<Option<ThreadView>, ThreadsError> {
        Ok(conn
            .query_row(
                r#"SELECT t.id, t.name, t.bot_id, t.position, t.created_at,
                      (SELECT COUNT(*) FROM chat_messages m WHERE m.thread_id = t.id),
                      (SELECT MAX(created_at) FROM chat_messages m WHERE m.thread_id = t.id),
                      t.space_id
               FROM chat_threads t WHERE t.id = ?1"#,
                params![id],
                row_to_thread,
            )
            .optional()?)
    }

    /// Lista as threads por posição (todas).
    pub fn list(&self) -> Result<Vec<ThreadView>, ThreadsError> {
        self.list_in_space(None)
    }

    /// Lista as threads de um Space; `None` ou `geral` devolve todas (o Geral enxerga tudo).
    pub fn list_in_space(&self, space: Option<&str>) -> Result<Vec<ThreadView>, ThreadsError> {
        let filtro = space.filter(|s| !s.is_empty() && *s != GENERAL_SPACE);
        let conn = self.lock();
        let mut st = conn.prepare(
            r#"SELECT t.id, t.name, t.bot_id, t.position, t.created_at,
                  (SELECT COUNT(*) FROM chat_messages m WHERE m.thread_id = t.id),
                  (SELECT MAX(created_at) FROM chat_messages m WHERE m.thread_id = t.id),
                  t.space_id
           FROM chat_threads t
           WHERE (?1 IS NULL OR t.space_id = ?1)
           ORDER BY t.position ASC, t.created_at ASC"#,
        )?;
        let rows = st.query_map(params![filtro], row_to_thread)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Cria uma thread no fim da lista, no Space Geral.
    pub fn create(&self, name: &str, bot_id: &str) -> Result<ThreadView, ThreadsError> {
        self.create_in_space(name, bot_id, GENERAL_SPACE)
    }

    /// Cria uma thread no fim da lista, dentro de um Space.
    pub fn create_in_space(
        &self,
        name: &str,
        bot_id: &str,
        space: &str,
    ) -> Result<ThreadView, ThreadsError> {
        let name = valid_name(name)?;
        let space = valid_space(space)?;
        let conn = self.lock();
        let next: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM chat_threads",
            [],
            |r| r.get(0),
        )?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chat_threads (id, name, bot_id, position, created_at, space_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, name, bot_id, next, chrono::Utc::now().to_rfc3339(), space],
        )?;
        Self::get_with(&conn, &id)?.ok_or(ThreadsError::NotFound)
    }

    /// Renomeia e/ou move a thread. Ao mover, as demais são renumeradas 0..n.
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        position: Option<i64>,
    ) -> Result<ThreadView, ThreadsError> {
        let name = name.map(valid_name).transpose()?;
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let exists: Option<String> = tx
            .query_row(
                "SELECT id FROM chat_threads WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(ThreadsError::NotFound);
        }
        if let Some(n) = name {
            tx.execute(
                "UPDATE chat_threads SET name = ?1 WHERE id = ?2",
                params![n, id],
            )?;
        }
        if let Some(pos) = position {
            let mut ids: Vec<String> = {
                let mut st = tx
                    .prepare("SELECT id FROM chat_threads ORDER BY position ASC, created_at ASC")?;
                let r = st.query_map([], |r| r.get::<_, String>(0))?;
                r.collect::<Result<Vec<_>, _>>()?
            };
            ids.retain(|x| x != id);
            let at = pos.clamp(0, ids.len() as i64) as usize;
            ids.insert(at, id.to_string());
            for (i, tid) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE chat_threads SET position = ?1 WHERE id = ?2",
                    params![i as i64, tid],
                )?;
            }
        }
        tx.commit()?;
        Self::get_with(&conn, id)?.ok_or(ThreadsError::NotFound)
    }

    /// Apaga a thread (e as mensagens, por cascata).
    pub fn delete(&self, id: &str) -> Result<(), ThreadsError> {
        let conn = self.lock();
        conn.execute(
            "DELETE FROM chat_messages WHERE thread_id = ?1",
            params![id],
        )?;
        let n = conn.execute("DELETE FROM chat_threads WHERE id = ?1", params![id])?;
        if n == 0 {
            Err(ThreadsError::NotFound)
        } else {
            Ok(())
        }
    }

    /// Mensagens da thread em ordem cronológica.
    pub fn messages(&self, thread_id: &str) -> Result<Vec<MessageView>, ThreadsError> {
        let conn = self.lock();
        if Self::get_with(&conn, thread_id)?.is_none() {
            return Err(ThreadsError::NotFound);
        }
        let mut st = conn.prepare(
            "SELECT id, thread_id, role, content, created_at FROM chat_messages
             WHERE thread_id = ?1 ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = st.query_map(params![thread_id], |r| {
            Ok(MessageView {
                id: r.get(0)?,
                thread_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// Adiciona uma mensagem à thread.
    pub fn add_message(
        &self,
        thread_id: &str,
        role: &str,
        content: &str,
    ) -> Result<MessageView, ThreadsError> {
        if !matches!(role, "user" | "assistant" | "system") {
            return Err(ThreadsError::Invalid(
                "role deve ser user, assistant ou system".into(),
            ));
        }
        if content.trim().is_empty() || content.len() > MAX_CONTENT {
            return Err(ThreadsError::Invalid(
                "conteúdo vazio ou longo demais".into(),
            ));
        }
        let conn = self.lock();
        if Self::get_with(&conn, thread_id)?.is_none() {
            return Err(ThreadsError::NotFound);
        }
        let msg = MessageView {
            id: uuid::Uuid::new_v4().to_string(),
            thread_id: thread_id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        conn.execute(
            "INSERT INTO chat_messages (id, thread_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![msg.id, msg.thread_id, msg.role, msg.content, msg.created_at],
        )?;
        Ok(msg)
    }
}

fn valid_name(name: &str) -> Result<String, ThreadsError> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_NAME {
        return Err(ThreadsError::Invalid(format!(
            "nome obrigatório (até {MAX_NAME} caracteres)"
        )));
    }
    Ok(n.to_string())
}

fn valid_space(space: &str) -> Result<String, ThreadsError> {
    let s = space.trim();
    let ok = !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(s.to_string())
    } else {
        Err(ThreadsError::Invalid("space inválido".into()))
    }
}

fn row_to_thread(r: &rusqlite::Row<'_>) -> rusqlite::Result<ThreadView> {
    let count: i64 = r.get(5)?;
    Ok(ThreadView {
        id: r.get(0)?,
        name: r.get(1)?,
        bot_id: r.get(2)?,
        position: r.get(3)?,
        created_at: r.get(4)?,
        message_count: count.max(0) as usize,
        last_message_at: r.get(6)?,
        space_id: r.get(7)?,
    })
}

// === HTTP ===

/// Corpo de `POST /threads`.
#[derive(Debug, Deserialize)]
pub struct CreateThreadRequest {
    /// Nome da thread.
    pub name: String,
    /// Bot associado (opcional).
    #[serde(default)]
    pub bot_id: Option<String>,
    /// Space dono (padrão `geral`).
    #[serde(default)]
    pub space_id: Option<String>,
}

/// Query de `GET /threads`.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Filtra pelo Space (`geral` ou ausente = todas).
    #[serde(default)]
    pub space: Option<String>,
}

/// Corpo de `PATCH /threads/:id`.
#[derive(Debug, Deserialize)]
pub struct UpdateThreadRequest {
    /// Novo nome.
    #[serde(default)]
    pub name: Option<String>,
    /// Nova posição (0 = topo).
    #[serde(default)]
    pub position: Option<i64>,
}

/// Corpo de `POST /threads/:id/messages`.
#[derive(Debug, Deserialize)]
pub struct AddMessageRequest {
    /// `user`, `assistant` ou `system`.
    pub role: String,
    /// Texto.
    pub content: String,
}

fn err_response(e: ThreadsError) -> Response {
    let (status, msg) = match e {
        ThreadsError::NotFound => (StatusCode::NOT_FOUND, "thread não encontrada".to_string()),
        ThreadsError::Invalid(m) => (StatusCode::BAD_REQUEST, m),
        ThreadsError::Db(e) => {
            tracing::error!(error = %e, "falha no store de threads");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "falha ao acessar threads".to_string(),
            )
        }
    };
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

async fn run<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ThreadsError> + Send + 'static,
) -> Result<T, ThreadsError> {
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|e| Err(ThreadsError::Invalid(format!("tarefa abortada: {e}"))))
}

async fn list_threads(State(s): State<ThreadsStore>, Query(q): Query<ListQuery>) -> Response {
    match run(move || s.list_in_space(q.space.as_deref())).await {
        Ok(threads) => Json(serde_json::json!({ "threads": threads })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn create_thread(
    State(s): State<ThreadsStore>,
    Json(req): Json<CreateThreadRequest>,
) -> Response {
    let bot = req.bot_id.unwrap_or_default();
    let space = req.space_id.unwrap_or_else(|| GENERAL_SPACE.to_string());
    match run(move || s.create_in_space(&req.name, &bot, &space)).await {
        Ok(t) => (StatusCode::CREATED, Json(t)).into_response(),
        Err(e) => err_response(e),
    }
}

async fn update_thread(
    State(s): State<ThreadsStore>,
    UrlPath(id): UrlPath<String>,
    Json(req): Json<UpdateThreadRequest>,
) -> Response {
    if req.name.is_none() && req.position.is_none() {
        return err_response(ThreadsError::Invalid("informe name e/ou position".into()));
    }
    match run(move || s.update(&id, req.name.as_deref(), req.position)).await {
        Ok(t) => Json(t).into_response(),
        Err(e) => err_response(e),
    }
}

async fn delete_thread(State(s): State<ThreadsStore>, UrlPath(id): UrlPath<String>) -> Response {
    match run(move || s.delete(&id)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

async fn list_messages(State(s): State<ThreadsStore>, UrlPath(id): UrlPath<String>) -> Response {
    match run(move || s.messages(&id)).await {
        Ok(m) => Json(serde_json::json!({ "messages": m })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn add_message(
    State(s): State<ThreadsStore>,
    UrlPath(id): UrlPath<String>,
    Json(req): Json<AddMessageRequest>,
) -> Response {
    match run(move || s.add_message(&id, &req.role, &req.content)).await {
        Ok(m) => (StatusCode::CREATED, Json(m)).into_response(),
        Err(e) => err_response(e),
    }
}

/// Router de `/threads` (montado dentro de `/api/v1`).
pub fn create_threads_router(store: ThreadsStore) -> Router {
    Router::new()
        .route("/threads", get(list_threads).post(create_thread))
        .route("/threads/:id", patch(update_thread).delete(delete_thread))
        .route(
            "/threads/:id/messages",
            get(list_messages).post(add_message),
        )
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn app() -> Router {
        create_threads_router(ThreadsStore::open(":memory:").unwrap())
    }

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
        let v = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, v)
    }

    #[tokio::test]
    async fn threads_por_space() {
        let app = app();
        let (_, a) = call(
            &app,
            "POST",
            "/threads",
            Some(r#"{"name":"A","space_id":"juridico"}"#),
        )
        .await;
        assert_eq!(a["space_id"], "juridico");
        call(
            &app,
            "POST",
            "/threads",
            Some(r#"{"name":"B","space_id":"ops"}"#),
        )
        .await;
        let (_, c) = call(&app, "POST", "/threads", Some(r#"{"name":"C"}"#)).await;
        assert_eq!(c["space_id"], "geral");

        let (_, j) = call(&app, "GET", "/threads?space=juridico", None).await;
        assert_eq!(j["threads"].as_array().unwrap().len(), 1);
        assert_eq!(j["threads"][0]["name"], "A");
        let (_, g) = call(&app, "GET", "/threads?space=geral", None).await;
        assert_eq!(g["threads"].as_array().unwrap().len(), 3);
        let (_, t) = call(&app, "GET", "/threads", None).await;
        assert_eq!(t["threads"].as_array().unwrap().len(), 3);
        let (st, _) = call(
            &app,
            "POST",
            "/threads",
            Some(r#"{"name":"D","space_id":"a b"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn migra_banco_sem_coluna_space() {
        let dir = std::env::temp_dir().join(format!("zylos-thr-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.db");
        {
            let c = Connection::open(&path).unwrap();
            c.execute_batch(
                "CREATE TABLE chat_threads (id TEXT PRIMARY KEY, name TEXT NOT NULL, bot_id TEXT NOT NULL DEFAULT '', position INTEGER NOT NULL, created_at TEXT NOT NULL);
                 INSERT INTO chat_threads VALUES ('x','Antiga','',0,'2026-01-01T00:00:00Z');",
            )
            .unwrap();
        }
        let store = ThreadsStore::open(path.to_str().unwrap()).unwrap();
        let l = store.list().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].space_id, "geral");
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn lista_vazia() {
        let (st, v) = call(&app(), "GET", "/threads", None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["threads"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn crud_completo_de_threads() {
        let app = app();
        let (st, a) = call(&app, "POST", "/threads", Some(r#"{"name":"A"}"#)).await;
        assert_eq!(st, StatusCode::CREATED);
        let (_, b) = call(
            &app,
            "POST",
            "/threads",
            Some(r#"{"name":"B","bot_id":"x"}"#),
        )
        .await;
        assert_eq!(a["position"], 0);
        assert_eq!(b["position"], 1);
        let a_id = a["id"].as_str().unwrap().to_string();
        let b_id = b["id"].as_str().unwrap().to_string();

        // renomear
        let (st, r) = call(
            &app,
            "PATCH",
            &format!("/threads/{a_id}"),
            Some(r#"{"name":"A2"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(r["name"], "A2");

        // reordenar: B para o topo
        let (st, _) = call(
            &app,
            "PATCH",
            &format!("/threads/{b_id}"),
            Some(r#"{"position":0}"#),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let (_, l) = call(&app, "GET", "/threads", None).await;
        assert_eq!(l["threads"][0]["id"], b_id.as_str());
        assert_eq!(l["threads"][1]["id"], a_id.as_str());
        assert_eq!(l["threads"][1]["position"], 1);

        // apagar
        let (st, _) = call(&app, "DELETE", &format!("/threads/{a_id}"), None).await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        let (st, _) = call(&app, "DELETE", &format!("/threads/{a_id}"), None).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (_, l) = call(&app, "GET", "/threads", None).await;
        assert_eq!(l["threads"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mensagens_da_thread() {
        let app = app();
        let (_, t) = call(&app, "POST", "/threads", Some(r#"{"name":"Chat"}"#)).await;
        let id = t["id"].as_str().unwrap().to_string();
        let uri = format!("/threads/{id}/messages");
        let (st, _) = call(
            &app,
            "POST",
            &uri,
            Some(r#"{"role":"user","content":"oi"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        let (st, _) = call(
            &app,
            "POST",
            &uri,
            Some(r#"{"role":"hacker","content":"x"}"#),
        )
        .await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        let (st, m) = call(&app, "GET", &uri, None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(m["messages"][0]["content"], "oi");
        let (_, l) = call(&app, "GET", "/threads", None).await;
        assert_eq!(l["threads"][0]["message_count"], 1);
        assert!(l["threads"][0]["last_message_at"].is_string());
        let (st, _) = call(&app, "GET", "/threads/nao-existe/messages", None).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn valida_entrada() {
        let app = app();
        let (st, _) = call(&app, "POST", "/threads", Some(r#"{"name":"   "}"#)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
        let (st, _) = call(&app, "PATCH", "/threads/x", Some(r#"{"name":"n"}"#)).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
        let (st, _) = call(&app, "PATCH", "/threads/x", Some(r#"{}"#)).await;
        assert_eq!(st, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn cria_diretorio_do_arquivo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("threads.db");
        let s = ThreadsStore::open(p.to_str().unwrap()).unwrap();
        s.create("x", "").unwrap();
        assert!(p.exists());
        let s2 = ThreadsStore::open(p.to_str().unwrap()).unwrap();
        assert_eq!(s2.list().unwrap().len(), 1);
    }
}
