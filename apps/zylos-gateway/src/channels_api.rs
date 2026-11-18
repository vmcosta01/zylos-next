//! Channels API — endpoints HTTP para sistema de canais/mensagens multi-usuário.
//!
//! Backend persistente via SQLite (zylos-channels crate) com sqlx.
//! Substitui o antigo backend em memória (DashMap).
//!
//! ## Endpoints
//! - `GET /channels` — lista todos os canais
//! - `GET /channels/:id` — detalhes de um canal
//! - `POST /channels` — cria novo canal
//! - `GET /channels/:id/messages` — lista mensagens de um canal
//! - `POST /channels/:id/messages` — envia mensagem para um canal
//! - `POST /channels/:id/join` — adiciona membro ao canal
//! - `POST /channels/:id/leave` — remove membro do canal
//!
//! ## Related Documents
//! - Crate: `crates/zylos-channels/`
//! - Componente: `apps/zylos-cockpit/src/components/channel_panel.rs`
//! - Playbook: `playbooks/pb-channels-migration.md`

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;
use zylos_channels::ChannelsStore;

/// Handle Send+Sync-safe para o store de canais.
#[derive(Clone)]
pub struct ChannelsHandle {
    pub store: Arc<ChannelsStore>,
}

unsafe impl Send for ChannelsHandle {}
unsafe impl Sync for ChannelsHandle {}

impl ChannelsHandle {
    /// Cria handle a partir de um store existente.
    pub fn from_store(store: ChannelsStore) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Cria handle com store em memória (para testes).
    pub async fn new_in_memory() -> Self {
        let store = ChannelsStore::in_memory()
            .await
            .expect("failed to create in-memory store");
        store.run_migrations().await.expect("migrations failed");
        store.seed_defaults().await.expect("seed failed");
        Self {
            store: Arc::new(store),
        }
    }
}

/// `GET /channels` — lista todos os canais.
async fn list_channels(State(handle): State<ChannelsHandle>) -> impl IntoResponse {
    match handle.store.list_channels().await {
        Ok(channels) => Json(serde_json::json!({
            "channels": channels,
            "total": channels.len(),
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// `GET /channels/:id` — detalhes de um canal.
async fn get_channel(
    State(handle): State<ChannelsHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.store.get_channel(&id).await {
        Ok(channel) => Json(serde_json::json!({ "channel": channel })).into_response(),
        Err(zylos_channels::store::ChannelsError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "channel_not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Body para criar canal.
#[derive(Deserialize)]
struct CreateChannelRequest {
    name: String,
    description: Option<String>,
    is_private: Option<bool>,
}

/// `POST /channels` — cria novo canal.
async fn create_channel(
    State(handle): State<ChannelsHandle>,
    Json(req): Json<CreateChannelRequest>,
) -> impl IntoResponse {
    match handle
        .store
        .create_channel(
            &req.name,
            req.description.as_deref().unwrap_or(""),
            req.is_private.unwrap_or(false),
        )
        .await
    {
        Ok(channel) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "channel": channel })),
        )
            .into_response(),
        Err(zylos_channels::store::ChannelsError::AlreadyExists(_)) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "channel_already_exists" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// `GET /channels/:id/messages` — lista mensagens de um canal.
async fn list_messages(
    State(handle): State<ChannelsHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.store.list_messages(&id).await {
        Ok(messages) => Json(serde_json::json!({
            "messages": messages,
            "total": messages.len(),
        }))
        .into_response(),
        Err(zylos_channels::store::ChannelsError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "channel_not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Body para enviar mensagem.
#[derive(Deserialize)]
struct SendMessageRequest {
    author: String,
    content: String,
}

/// `POST /channels/:id/messages` — envia mensagem para um canal.
async fn send_message(
    State(handle): State<ChannelsHandle>,
    Path(id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> impl IntoResponse {
    match handle
        .store
        .send_message(&id, &req.author, &req.content)
        .await
    {
        Ok(msg) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "message": msg })),
        )
            .into_response(),
        Err(zylos_channels::store::ChannelsError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "channel_not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Body para join/leave.
#[derive(Deserialize)]
struct MemberRequest {
    user_id: String,
}

/// `POST /channels/:id/join` — adiciona membro ao canal.
async fn join_channel(
    State(handle): State<ChannelsHandle>,
    Path(id): Path<String>,
    Json(req): Json<MemberRequest>,
) -> impl IntoResponse {
    match handle.store.join_channel(&id, &req.user_id).await {
        Ok(member) => (
            StatusCode::OK,
            Json(serde_json::json!({ "member": member })),
        )
            .into_response(),
        Err(zylos_channels::store::ChannelsError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "channel_not_found" })),
        )
            .into_response(),
        Err(zylos_channels::store::ChannelsError::MemberAlreadyExists) => (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "already_member" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// `POST /channels/:id/leave` — remove membro do canal.
async fn leave_channel(
    State(handle): State<ChannelsHandle>,
    Path(id): Path<String>,
    Json(req): Json<MemberRequest>,
) -> impl IntoResponse {
    match handle.store.leave_channel(&id, &req.user_id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "left" })),
        )
            .into_response(),
        Err(zylos_channels::store::ChannelsError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "member_not_found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// Cria o router de channels com seu próprio handle.
pub fn create_channels_router(handle: ChannelsHandle) -> Router {
    Router::new()
        .route("/channels", get(list_channels).post(create_channel))
        .route("/channels/:id", get(get_channel))
        .route(
            "/channels/:id/messages",
            get(list_messages).post(send_message),
        )
        .route("/channels/:id/join", post(join_channel))
        .route("/channels/:id/leave", post(leave_channel))
        .with_state(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        let handle = ChannelsHandle::new_in_memory().await;
        create_channels_router(handle)
    }

    #[tokio::test]
    async fn test_list_channels_seeded() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/channels")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_channel_not_found() {
        let app = test_app().await;
        let req = Request::builder()
            .uri("/channels/nonexistent")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_messages_nonexistent_channel() {
        let app = test_app().await;
        // Listing messages for a channel that doesn't exist should return 404
        let req = Request::builder()
            .uri("/channels/nonexistent-channel/messages")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    async fn call(app: &Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, serde_json::Value) {
        let mut b = Request::builder().method(method).uri(uri);
        if body.is_some() {
            b = b.header("content-type", "application/json");
        }
        let req = b
            .body(body.map(|s| Body::from(s.to_string())).unwrap_or_else(Body::empty))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    /// Fluxo completo usado pelo cockpit: criar canal → enviar → listar → join/leave.
    /// Garante que as rotas com parâmetro (`:id`, sintaxe do axum 0.7) casam.
    #[tokio::test]
    async fn test_create_send_list_join_leave() {
        let app = test_app().await;
        let (st, v) = call(&app, "POST", "/channels", Some(r#"{"name":"rota-teste"}"#)).await;
        assert_eq!(st, StatusCode::CREATED);
        let id = v["channel"]["id"].as_str().unwrap().to_string();

        let (st, _) = call(&app, "POST", "/channels", Some(r#"{"name":"rota-teste"}"#)).await;
        assert_eq!(st, StatusCode::CONFLICT);

        let (st, v) = call(&app, "GET", &format!("/channels/{id}"), None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["channel"]["name"], "rota-teste");

        let uri = format!("/channels/{id}/messages");
        let (st, v) = call(&app, "POST", &uri, Some(r#"{"author":"ana","content":"oi"}"#)).await;
        assert_eq!(st, StatusCode::CREATED);
        assert_eq!(v["message"]["content"], "oi");

        let (st, v) = call(&app, "GET", &uri, None).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["total"], 1);
        assert_eq!(v["messages"][0]["author"], "ana");

        let join = format!("/channels/{id}/join");
        let (st, _) = call(&app, "POST", &join, Some(r#"{"user_id":"ana"}"#)).await;
        assert_eq!(st, StatusCode::OK);
        let (st, _) = call(&app, "POST", &join, Some(r#"{"user_id":"ana"}"#)).await;
        assert_eq!(st, StatusCode::CONFLICT);
        let leave = format!("/channels/{id}/leave");
        let (st, _) = call(&app, "POST", &leave, Some(r#"{"user_id":"ana"}"#)).await;
        assert_eq!(st, StatusCode::OK);
    }
}
