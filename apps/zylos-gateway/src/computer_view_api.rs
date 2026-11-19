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
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::Engine as _;
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

/// `GET /computer-view/sessions/{id}/screenshot` — captura tela atual como PNG binário.
async fn get_screenshot(
    State(handle): State<ComputerViewHandle>,
    Path(id): Path<String>,
) -> Response {
    match handle.manager.capture_screenshot(&id) {
        Ok(data) => {
            let bytes: Vec<u8> =
                match base64::engine::general_purpose::STANDARD.decode(&data.image_base64) {
                    Ok(b) => b,
                    Err(e) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "screenshot_decode_failed",
                            Some(e.to_string()),
                        )
                        .into_response();
                    }
                };
            (StatusCode::OK, [(header::CONTENT_TYPE, "image/png")], bytes).into_response()
        }
        Err(e) if e.contains("not found") => {
            error_response(StatusCode::NOT_FOUND, "session_not_found", Some(e)).into_response()
        }
        Err(e) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "screenshot_unavailable",
            Some(e),
        )
        .into_response(),
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
            "/computer-view/sessions/:id",
            get(get_session).delete(terminate_session),
        )
        .route(
            "/computer-view/sessions/:id/control",
            post(take_control).delete(release_control),
        )
        .route(
            "/computer-view/sessions/:id/screenshot",
            get(get_screenshot),
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

    // Regressão: no axum 0.7 `{id}` é texto literal; as rotas por sessão respondiam sempre 404
    // (o teste "not_found" acima passava por esse motivo errado).
    async fn create(app: &Router) -> String {
        let resp = app
            .clone()
            .oneshot(
                Request::post("/computer-view/sessions")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"bot_id":"backend"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let bytes = axum::body::to_bytes(resp.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        v["session"]["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn rotas_por_sessao_encontram_a_sessao_criada() {
        let app = test_app();
        let id = create(&app).await;

        let get = app
            .clone()
            .oneshot(
                Request::get(format!("/computer-view/sessions/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(get.status(), StatusCode::OK);

        let take = app
            .clone()
            .oneshot(
                Request::post(format!("/computer-view/sessions/{id}/control"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"user_id":"victor"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        // Sessão recém-criada ainda está "connecting": o domínio responde 409, não 404.
        assert_ne!(take.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn screenshot_e_listagem_apontam_para_a_mesma_rota() {
        let app = test_app();
        let id = create(&app).await;
        let list = app
            .clone()
            .oneshot(Request::get("/computer-view/sessions").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(list.into_body(), 1 << 20).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let url = v["sessions"][0]["screenshot_url"].as_str().unwrap();
        assert_eq!(url, format!("/api/v1/computer-view/sessions/{id}/screenshot"));
    }
}
