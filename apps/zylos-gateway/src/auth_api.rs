//! Auth API — endpoints de login, register e token management.
//!
//! F11.11.1/F11.11.8 (remediação CRÍTICA): antes, `POST /auth/login` e
//! `/auth/register` emitiam JWT para **qualquer** `user_id`/`roles` vindos do
//! corpo, sem credencial — qualquer cliente local podia cunhar um token `admin`.
//!
//! Agora:
//! - Não há usuários/senhas no gateway ainda; a emissão de tokens exige a
//!   credencial de bootstrap `ZYLOS_AUTH_BOOTSTRAP_TOKEN` (>= 32 bytes) em
//!   `Authorization: Bearer ...`, comparada em tempo constante. Sem ela
//!   configurada, as rotas respondem 503 (fail-closed).
//! - `roles` são restritos ao conjunto conhecido (admin/editor/viewer/
//!   service_account); papéis desconhecidos => 400.
//! - `ZYLOS_AUTH_SECRET` ausente ou < 32 bytes => 503 (sem segredo fallback).
//! - `/auth/register` está desabilitado (501) até existir um user store.
//! - `/auth/refresh` (rotação + detecção de reuso) e `/auth/logout` (revogação).

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use zylos_auth::tokens::{CreateTokenRequest, TokenError};
use zylos_auth::{TokenConfig, TokenService};

/// Tamanho mínimo da credencial de bootstrap.
pub const MIN_BOOTSTRAP_LEN: usize = 32;
const KNOWN_ROLES: &[&str] = &["admin", "editor", "viewer", "service_account"];

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct LogoutRequest {
    #[serde(default)]
    pub refresh_token: Option<String>,
}

/// Estado do router de auth.
#[derive(Clone)]
pub struct AuthState {
    svc: Option<TokenService>,
    bootstrap: Arc<Option<String>>,
}

impl AuthState {
    /// Constrói a partir do ambiente (`ZYLOS_AUTH_SECRET`, `ZYLOS_AUTH_BOOTSTRAP_TOKEN`).
    pub fn from_env() -> Self {
        let bootstrap = std::env::var("ZYLOS_AUTH_BOOTSTRAP_TOKEN").ok();
        Self::new(TokenConfig::default(), bootstrap)
    }

    /// Construtor explícito (testes / embedding).
    pub fn new(config: TokenConfig, bootstrap: Option<String>) -> Self {
        // F2: abre (ou cria) o store persistente de JWTs revogados.
        let revocation_store = std::env::var("ZYLOS_REVOKED_DB")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| Some(std::path::PathBuf::from("data/revoked.db")))
            .and_then(|path| {
                match zylos_auth::revocation_store::RevocationStore::open(&path) {
                    Ok(s) => Some(std::sync::Arc::new(s)),
                    Err(e) => {
                        tracing::warn!(error = %e, path = %path.display(), "F2: falha ao abrir denylist SQLite");
                        None
                    }
                }
            });
        let svc = match TokenService::new(config) {
            Ok(s) => Some(s.with_revocation_store(revocation_store)),
            Err(e) => {
                tracing::warn!(error = %e, "auth: TokenService desabilitado (ZYLOS_AUTH_SECRET ausente/fraco)");
                None
            }
        };
        let bootstrap = bootstrap.filter(|s| s.len() >= MIN_BOOTSTRAP_LEN);
        Self {
            svc,
            bootstrap: Arc::new(bootstrap),
        }
    }
}

impl AuthState {
    /// Serviço de tokens compartilhado (mesma denylist de revogação do middleware).
    pub fn token_service(&self) -> Option<TokenService> {
        self.svc.clone()
    }
}

/// Cria router com endpoints de autenticação.
pub fn create_auth_router() -> Router {
    create_auth_router_with(AuthState::from_env())
}

/// Variante com estado explícito.
pub fn create_auth_router_with(state: AuthState) -> Router {
    Router::new()
        .route("/auth/login", post(login_handler))
        .route("/auth/register", post(register_handler))
        .route("/auth/refresh", post(refresh_handler))
        .route("/auth/logout", post(logout_handler))
        .with_state(state)
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, tok) = v.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then_some(tok.trim())
        .filter(|t| !t.is_empty())
}

fn to_resp(r: zylos_auth::tokens::TokenResponse) -> Json<LoginResponse> {
    Json(LoginResponse {
        access_token: r.access_token,
        token_type: r.token_type,
        expires_in: r.expires_in,
        refresh_token: r.refresh_token,
    })
}

async fn login_handler(
    State(st): State<AuthState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Response {
    let Some(svc) = st.svc.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "auth not configured");
    };
    let Some(expected) = st.bootstrap.as_ref() else {
        return err(
            StatusCode::SERVICE_UNAVAILABLE,
            "token issuance not configured",
        );
    };
    let provided = bearer(&headers).unwrap_or("");
    if !ct_eq(provided.as_bytes(), expected.as_bytes()) {
        return err(StatusCode::UNAUTHORIZED, "invalid credentials");
    }
    if req.user_id.trim().is_empty()
        || req.tenant_id.trim().is_empty()
        || req.user_id.len() > 128
        || req.tenant_id.len() > 128
    {
        return err(StatusCode::BAD_REQUEST, "invalid user_id/tenant_id");
    }
    if req.roles.is_empty() || req.roles.len() > 4 {
        return err(StatusCode::BAD_REQUEST, "invalid roles");
    }
    if let Some(bad) = req
        .roles
        .iter()
        .find(|r| !KNOWN_ROLES.contains(&r.as_str()))
    {
        return err(StatusCode::BAD_REQUEST, &format!("unknown role '{bad}'"));
    }
    let token_req = CreateTokenRequest {
        user_id: req.user_id,
        tenant_id: req.tenant_id,
        roles: req.roles,
        name: req.name,
    };
    match svc.create_token_pair(&token_req) {
        Ok(resp) => to_resp(resp).into_response(),
        Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "token issuance failed"),
    }
}

async fn register_handler() -> Response {
    // Sem user store: registro aberto equivaleria a auto-emissão de privilégios.
    err(
        StatusCode::NOT_IMPLEMENTED,
        "self-registration disabled (F11.11.8)",
    )
}

async fn refresh_handler(State(st): State<AuthState>, Json(req): Json<RefreshRequest>) -> Response {
    let Some(svc) = st.svc.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "auth not configured");
    };
    match svc.refresh(&req.refresh_token) {
        Ok(r) => to_resp(r).into_response(),
        Err(TokenError::Revoked) => err(StatusCode::UNAUTHORIZED, "refresh token revoked"),
        Err(_) => err(StatusCode::UNAUTHORIZED, "invalid refresh token"),
    }
}

async fn logout_handler(
    State(st): State<AuthState>,
    headers: HeaderMap,
    body: Option<Json<LogoutRequest>>,
) -> Response {
    let Some(svc) = st.svc.as_ref() else {
        return err(StatusCode::SERVICE_UNAVAILABLE, "auth not configured");
    };
    let Some(tok) = bearer(&headers) else {
        return err(StatusCode::UNAUTHORIZED, "missing bearer token");
    };
    if svc.revoke_token(tok).is_err() {
        return err(StatusCode::UNAUTHORIZED, "invalid token");
    }
    if let Some(Json(LogoutRequest {
        refresh_token: Some(rt),
    })) = body
    {
        svc.revoke_refresh(&rt);
    }
    StatusCode::NO_CONTENT.into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    const SECRET: &str = "gateway-auth-test-secret-0123456789";
    const BOOT: &str = "bootstrap-credential-0123456789-abcdef";

    fn router(boot: Option<&str>, secret: &str) -> Router {
        create_auth_router_with(AuthState::new(
            TokenConfig {
                secret: secret.into(),
                ..TokenConfig::default()
            },
            boot.map(String::from),
        ))
    }

    fn login_req(auth: Option<&str>, roles: &str) -> Request<Body> {
        let mut b = Request::post("/auth/login").header("content-type", "application/json");
        if let Some(a) = auth {
            b = b.header("authorization", a);
        }
        b.body(Body::from(format!(
            r#"{{"user_id":"u1","tenant_id":"t1","roles":{roles}}}"#
        )))
        .unwrap()
    }

    async fn json(resp: Response) -> serde_json::Value {
        let b = axum::body::to_bytes(resp.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&b).unwrap_or(serde_json::Value::Null)
    }

    #[tokio::test]
    async fn login_without_credential_cannot_mint_admin() {
        let r = router(Some(BOOT), SECRET)
            .oneshot(login_req(None, r#"["admin"]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let r = router(Some(BOOT), SECRET)
            .oneshot(login_req(
                Some("Bearer wrong-wrong-wrong-wrong-wrong-wrong"),
                r#"["admin"]"#,
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn login_fail_closed_when_unconfigured() {
        let r = router(None, SECRET)
            .oneshot(login_req(Some(&format!("Bearer {BOOT}")), r#"["viewer"]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        // bootstrap curto é ignorado
        let r = router(Some("short"), SECRET)
            .oneshot(login_req(Some("Bearer short"), r#"["viewer"]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
        // segredo JWT fraco => serviço desabilitado
        let r = router(Some(BOOT), "weak")
            .oneshot(login_req(Some(&format!("Bearer {BOOT}")), r#"["viewer"]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn login_rejects_unknown_roles() {
        let r = router(Some(BOOT), SECRET)
            .oneshot(login_req(
                Some(&format!("Bearer {BOOT}")),
                r#"["superuser"]"#,
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
        let r = router(Some(BOOT), SECRET)
            .oneshot(login_req(Some(&format!("Bearer {BOOT}")), r#"[]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn register_is_disabled() {
        let r = router(Some(BOOT), SECRET)
            .oneshot(
                Request::post("/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"user_id":"u","tenant_id":"t","roles":["admin"]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[tokio::test]
    async fn login_refresh_logout_flow() {
        let app = router(Some(BOOT), SECRET);
        let r = app
            .clone()
            .oneshot(login_req(Some(&format!("bearer {BOOT}")), r#"["editor"]"#))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let v = json(r).await;
        let access = v["access_token"].as_str().unwrap().to_string();
        let rt = v["refresh_token"].as_str().unwrap().to_string();

        let refresh = |tok: String| {
            Request::post("/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(format!(r#"{{"refresh_token":"{tok}"}}"#)))
                .unwrap()
        };
        let r = app.clone().oneshot(refresh(rt.clone())).await.unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        let v2 = json(r).await;
        let rt2 = v2["refresh_token"].as_str().unwrap().to_string();
        // reuso do rt antigo => 401 e família revogada
        let r = app.clone().oneshot(refresh(rt)).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
        let r = app.clone().oneshot(refresh(rt2)).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

        // logout revoga o access token
        let logout = || {
            Request::post("/auth/logout")
                .header("authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap()
        };
        let r = app.clone().oneshot(logout()).await.unwrap();
        assert_eq!(r.status(), StatusCode::NO_CONTENT);
        let r = app.oneshot(logout()).await.unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }
}
