//! Computer View API — endpoints HTTP para o crate `zylos-computer-view`.
//!
//! Expõe sessões de desktop/browser remoto via REST.
//!
//! ## Endpoints
//! - `GET /computer-view/sessions` — lista sessões ativas
//! - `GET /computer-view/sessions/:id` — detalhes de uma sessão
//! - `POST /computer-view/sessions` — cria nova sessão
//! - `DELETE /computer-view/sessions/:id` — encerra sessão
//! - `POST /computer-view/sessions/:id/control` — solicita controle (lease)
//! - `DELETE /computer-view/sessions/:id/control` — libera controle
//!
//! ## Related Documents
//! - Crate: `crates/zylos-computer-view/`
//! - Playbook: `playbooks/pb-computer-view-setup.md`
//! - Runbook: `runbooks/rb-zylos-computer-view.md`

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zylos_computer_view::{
    schema::{CreateSessionRequest, SessionState, TakeControlRequest},
    session::ComputerSessionManager,
};

/// Handle Send+Sync-safe para o session manager.
#[derive(Clone)]
pub struct ComputerViewHandle {
    pub manager: Arc<ComputerSessionManager>,
}

// Safety: ComputerSessionManager wraps a DashMap which is Sync.
unsafe impl Send for ComputerViewHandle {}
unsafe impl Sync for ComputerViewHandle {}

impl ComputerViewHandle {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(ComputerSessionManager::new(30)), // 30 min default lease
        }
    }
}

impl Default for ComputerViewHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Resposta de erro padronizada.
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    detail: Option<String>,
}

fn error_response(status: StatusCode, error: &str, detail: Option<String>) -> impl IntoResponse {
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
            detail,
        }),
    )
}

/// `GET /computer-view/sessions` — lista todas as sessões ativas.
async fn list_sessions(State(handle): State<ComputerViewHandle>) -> impl IntoResponse {
    let sessions = handle.manager.list_active_sessions();
    Json(serde_json::json!({
        "sessions": sessions,
        "total": sessions.len(),
    }))
}

/// `GET /computer-view/sessions/:id` — detalhes de uma sessão específica.
async fn get_session(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.manager.get_session(&id) {
        Some(session) => Json(serde_json::json!({ "session": session })).into_response(),
        None => error_response(
            StatusCode::NOT_FOUND,
            "session_not_found",
            Some(format!("Session '{}' not found", id)),
        )
        .into_response(),
    }
}

/// Body para criar uma nova sessão (mapeia para CreateSessionRequest do crate).
#[derive(Deserialize)]
struct CreateSessionBody {
    bot_id: String,
    vnc_endpoint: Option<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

/// `POST /computer-view/sessions` — cria uma nova sessão de computer view.
async fn create_session(
    State(handle): State<ComputerViewHandle>,
    Json(body): Json<CreateSessionBody>,
) -> impl IntoResponse {
    let req = CreateSessionRequest {
        bot_id: body.bot_id,
        vnc_endpoint: body.vnc_endpoint,
        metadata: Some(body.metadata),
    };
    let session = handle.manager.create_session(req);
    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "session": session })),
    )
        .into_response()
}

/// `DELETE /computer-view/sessions/:id` — encerra uma sessão.
async fn terminate_session(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match handle.manager.update_state(&id, SessionState::Terminated) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            error_response(StatusCode::NOT_FOUND, "session_not_found", Some(e)).into_response()
        }
    }
}

/// `POST /computer-view/sessions/:id/control` — solicita controle da sessão (lease).
async fn take_control(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
    Json(req): Json<TakeControlRequest>,
) -> impl IntoResponse {
    match handle.manager.take_control(&id, req) {
        Ok(session) => (
            StatusCode::OK,
            Json(serde_json::json!({ "session": session })),
        )
            .into_response(),
        Err(e) => {
            error_response(StatusCode::CONFLICT, "control_unavailable", Some(e)).into_response()
        }
    }
}

/// Body para liberar controle.
#[derive(Deserialize)]
struct ReleaseControlBody {
    user_id: String,
}

/// `DELETE /computer-view/sessions/:id/control` — libera o controle da sessão.
async fn release_control(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
    Json(body): Json<ReleaseControlBody>,
) -> impl IntoResponse {
    match handle.manager.release_control(&id, &body.user_id) {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(StatusCode::CONFLICT, "release_failed", Some(e)).into_response(),
    }
}

/// Cria o router de computer-view com seu próprio handle.
pub fn create_computer_view_router(handle: ComputerViewHandle) -> Router {
    Router::new()
        .route(
            "/computer-view/sessions",
            get(list_sessions).post(create_session),
        )
        .route(
            "/computer-view/sessions/{id}",
            get(get_session).delete(terminate_session),
        )
        .route(
            "/computer-view/sessions/{id}/control",
            post(take_control).delete(release_control),
        )
        .with_state(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn test_app() -> Router {
        let handle = ComputerViewHandle::new();
        create_computer_view_router(handle)
    }

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let app = test_app();
        let req = Request::builder()
            .uri("/computer-view/sessions")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_session_not_found() {
        let app = test_app();
        let req = Request::builder()
            .uri("/computer-view/sessions/nonexistent")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
