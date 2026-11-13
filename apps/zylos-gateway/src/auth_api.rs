//! Auth API — endpoints de login, register e token management.

use axum::{Json, Router, routing::post};
use serde::{Deserialize, Serialize};
use zylos_auth::tokens::CreateTokenRequest;
use zylos_auth::{TokenConfig, TokenService};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub user_id: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

/// Cria router com endpoints de autenticação.
pub fn create_auth_router() -> Router {
    let token_service = TokenService::new(TokenConfig::default())
        .expect("failed to create token service");

    Router::new()
        .route("/auth/login", post(login_handler))
        .route("/auth/register", post(register_handler))
        .with_state(token_service)
}

async fn login_handler(
    axum::extract::State(svc): axum::extract::State<TokenService>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, axum::http::StatusCode> {
    let token_req = CreateTokenRequest {
        user_id: req.user_id,
        tenant_id: req.tenant_id,
        roles: req.roles,
        name: req.name,
    };

    match svc.create_token(&token_req) {
        Ok(resp) => Ok(Json(LoginResponse {
            access_token: resp.access_token,
            token_type: resp.token_type,
            expires_in: resp.expires_in,
        })),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn register_handler(
    axum::extract::State(svc): axum::extract::State<TokenService>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, axum::http::StatusCode> {
    // Em produção, validar password hash, verificar duplicidade, etc.
    // Por ora, register == login (token emission)
    login_handler(axum::extract::State(svc), Json(req)).await
}